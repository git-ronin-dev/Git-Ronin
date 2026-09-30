//! Fixes to the process environment for the way the app was started.
//! [`prepare`] runs first thing in `main`, before any thread exists, since
//! changing the environment is only safe then.

use std::ffi::OsString;
use std::sync::OnceLock;

/// A change to a child's environment; `None` removes the variable.
pub type EnvChange = (OsString, Option<OsString>);

/// Applies every fix for this platform.
pub fn prepare() {
    #[cfg(target_os = "linux")]
    linux::prepare();
    #[cfg(target_os = "macos")]
    macos::use_login_path();
}

/// What programs the app starts (git, the terminal's shell, the browser)
/// must not inherit from it.
pub fn child_env() -> &'static [EnvChange] {
    static CHANGES: OnceLock<Vec<EnvChange>> = OnceLock::new();
    CHANGES.get_or_init(platform_changes)
}

#[cfg(target_os = "linux")]
fn platform_changes() -> Vec<EnvChange> {
    let vars = std::env::vars_os().collect();
    let mut changes = linux::appimage_changes(&vars);
    changes.extend(linux::flatpak_changes(&vars));
    changes
}

#[cfg(not(target_os = "linux"))]
fn platform_changes() -> Vec<EnvChange> {
    Vec::new()
}

/// Applies [`child_env`] to a command.
pub fn apply(cmd: &mut std::process::Command) {
    for (key, value) in child_env() {
        match value {
            Some(value) => cmd.env(key, value),
            None => cmd.env_remove(key),
        };
    }
}

/// Opens a web page in the default browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("Not a web address: {url}"));
    }
    if child_env().is_empty() {
        return tauri_plugin_opener::open_url(&url, None::<&str>).map_err(|e| e.to_string());
    }
    // In an AppImage or a Flatpak: started by hand so the browser gets a
    // clean environment.
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    apply(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start xdg-open: {e}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    use super::EnvChange;

    /// Search paths the AppImage's launcher prepends its own folders to.
    const PATH_LISTS: &[&str] = &[
        "PATH",
        "LD_LIBRARY_PATH",
        "XDG_DATA_DIRS",
        "GSETTINGS_SCHEMA_DIR",
        "PYTHONPATH",
        "PERLLIB",
        "QT_PLUGIN_PATH",
        "GST_PLUGIN_SYSTEM_PATH",
        "GST_PLUGIN_SYSTEM_PATH_1_0",
    ];

    /// Variables the launcher and its GTK hook set outright, plus the
    /// AppImage runtime's own.
    const SET_BY_APPIMAGE: &[&str] = &[
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "OWD",
        "PYTHONHOME",
        "PYTHONDONTWRITEBYTECODE",
        "GTK_DATA_PREFIX",
        "GTK_THEME",
        "GI_TYPELIB_PATH",
        "GIO_MODULE_DIR",
        "GTK_EXE_PREFIX",
        "GTK_PATH",
        "GTK_IM_MODULE_FILE",
        "GDK_PIXBUF_MODULE_FILE",
    ];

    /// Inside an AppImage the launcher points library and module paths at
    /// the bundled copies. The app needs them, but the user's git would
    /// then load the bundle's libcurl and OpenSSL (built on an older
    /// distribution), and GTK programs such as pinentry or the browser the
    /// bundle's modules and theme. Children get their folders taken out.
    pub fn appimage_changes(vars: &HashMap<OsString, OsString>) -> Vec<EnvChange> {
        let Some(appdir) = vars
            .get(OsStr::new("APPDIR"))
            .filter(|_| vars.contains_key(OsStr::new("APPIMAGE")))
        else {
            return Vec::new();
        };
        let appdir = appdir
            .as_bytes()
            .strip_suffix(b"/")
            .unwrap_or(appdir.as_bytes());
        let mut changes = Vec::new();
        for name in PATH_LISTS {
            let Some(value) = vars.get(OsStr::new(name)) else {
                continue;
            };
            let kept: Vec<&[u8]> = value
                .as_bytes()
                .split(|&b| b == b':')
                .filter(|dir| !dir.is_empty() && !inside(dir, appdir))
                .collect();
            let value = (!kept.is_empty()).then(|| OsStr::from_bytes(&kept.join(&b':')).to_owned());
            changes.push((OsString::from(name), value));
        }
        changes.extend(
            SET_BY_APPIMAGE
                .iter()
                .filter(|name| vars.contains_key(OsStr::new(name)))
                .map(|name| (OsString::from(name), None)),
        );
        changes
    }

    /// Flatpak points the XDG base directories into the app's own folder,
    /// so git would miss `~/.config/git/config` and `ignore`. Children get
    /// the host's back (`--filesystem=home` makes them visible).
    pub fn flatpak_changes(vars: &HashMap<OsString, OsString>) -> Vec<EnvChange> {
        let home = vars.get(OsStr::new("HOME"));
        let (Some(home), true) = (home, vars.contains_key(OsStr::new("FLATPAK_ID"))) else {
            return Vec::new();
        };
        [
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_DATA_HOME", ".local/share"),
            ("XDG_CACHE_HOME", ".cache"),
            ("XDG_STATE_HOME", ".local/state"),
        ]
        .into_iter()
        .map(|(name, default)| {
            let host = vars
                .get(OsStr::new(&format!("HOST_{name}")))
                .cloned()
                .unwrap_or_else(|| Path::new(home).join(default).into_os_string());
            (OsString::from(name), Some(host))
        })
        .collect()
    }

    fn inside(dir: &[u8], root: &[u8]) -> bool {
        dir.strip_prefix(root)
            .is_some_and(|rest| rest.is_empty() || rest[0] == b'/')
    }

    pub fn prepare() {
        // WebKitGTK's DMA-BUF renderer shows a blank or flickering window
        // with NVIDIA's proprietary driver (above all on Wayland).
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
            && Path::new("/proc/driver/nvidia/version").exists()
        {
            // SAFETY: no other thread exists yet.
            unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    const MARK: &str = "__RONIN_PATH__";

    /// Apps started from Finder or the Dock get launchd's minimal `PATH`,
    /// which misses the git, git-lfs and gpg that Homebrew or MacPorts
    /// installed. The user's login shell knows where they are.
    pub fn use_login_path() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        // fish keeps PATH as a list, which "$PATH" would join with spaces.
        let value = if shell.ends_with("/fish") {
            "(string join : $PATH)"
        } else {
            "\"$PATH\""
        };
        let Ok(mut child) = Command::new(&shell)
            .args(["-ilc", &format!("printf '{MARK}%s{MARK}' {value}")])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        let mut stdout = child.stdout.take().expect("piped");
        let (tx, rx) = mpsc::channel();
        // Read on a helper thread, so a shell that hangs in its startup
        // files can't hold up the app. Stop at the second mark: something
        // the shell started in the background may keep the pipe open.
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let mut buf = [0; 4096];
            while let Ok(n @ 1..) = stdout.read(&mut buf) {
                out.extend_from_slice(&buf[..n]);
                if let Some(path) = between_marks(&String::from_utf8_lossy(&out)) {
                    let _ = tx.send(path.to_owned());
                    return;
                }
            }
        });
        let path = rx.recv_timeout(Duration::from_secs(3)).ok();
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
        }
        let _ = child.wait();
        if let Some(path) = path.filter(|p| !p.is_empty()) {
            // SAFETY: the only other thread has finished or is blocked
            // reading a pipe; it never reads the environment.
            unsafe { std::env::set_var("PATH", path) };
        }
    }

    fn between_marks(out: &str) -> Option<&str> {
        let start = out.find(MARK)? + MARK.len();
        let len = out[start..].find(MARK)?;
        Some(&out[start..start + len])
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<OsString, OsString> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    fn change(key: &str, value: Option<&str>) -> EnvChange {
        (key.into(), value.map(OsString::from))
    }

    #[test]
    fn outside_an_appimage_nothing_changes() {
        let env = vars(&[("PATH", "/usr/bin"), ("APPDIR", "/tmp/x")]);
        assert!(linux::appimage_changes(&env).is_empty());
        assert!(linux::flatpak_changes(&env).is_empty());
    }

    #[test]
    fn appimage_folders_are_taken_out_of_search_paths() {
        let env = vars(&[
            ("APPIMAGE", "/home/u/Git Ronin.AppImage"),
            ("APPDIR", "/tmp/.mount_GitRoAbc/"),
            (
                "PATH",
                "/tmp/.mount_GitRoAbc/usr/bin/:/usr/local/bin:/usr/bin",
            ),
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_GitRoAbc/usr/lib/:/tmp/.mount_GitRoAbc/lib/:",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_GitRoAbc/usr/share:/usr/share:/usr/local/share",
            ),
            ("GTK_THEME", "Adwaita:dark"),
            ("HOME", "/home/u"),
        ]);
        let mut changes = linux::appimage_changes(&env);
        changes.sort();
        assert_eq!(
            changes,
            vec![
                change("APPDIR", None),
                change("APPIMAGE", None),
                change("GTK_THEME", None),
                change("LD_LIBRARY_PATH", None),
                change("PATH", Some("/usr/local/bin:/usr/bin")),
                change("XDG_DATA_DIRS", Some("/usr/share:/usr/local/share")),
            ]
        );
    }

    #[test]
    fn in_a_flatpak_children_get_the_hosts_xdg_folders() {
        let env = vars(&[
            ("FLATPAK_ID", "com.gitronin.desktop"),
            ("HOME", "/home/u"),
            (
                "XDG_CONFIG_HOME",
                "/home/u/.var/app/com.gitronin.desktop/config",
            ),
            ("HOST_XDG_DATA_HOME", "/data/u"),
        ]);
        assert_eq!(
            linux::flatpak_changes(&env),
            vec![
                change("XDG_CONFIG_HOME", Some("/home/u/.config")),
                change("XDG_DATA_HOME", Some("/data/u")),
                change("XDG_CACHE_HOME", Some("/home/u/.cache")),
                change("XDG_STATE_HOME", Some("/home/u/.local/state")),
            ]
        );
    }
}
