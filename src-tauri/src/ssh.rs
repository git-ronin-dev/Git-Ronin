//! SSH keys in `~/.ssh`: listing them and generating ed25519 keys. Keys are
//! generated in-process, so a passphrase never appears on a command line.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SshKey {
    /// File name of the private key, e.g. `id_ed25519`.
    pub name: String,
    /// Path of the private key; the public key is this plus `.pub`.
    pub path: String,
    pub has_private: bool,
    /// The public key line, as it would be pasted into a hosting service.
    pub public_key: String,
    pub algorithm: String,
    pub comment: String,
    /// `SHA256:…`, as `ssh-keygen -l` shows it.
    pub fingerprint: String,
}

/// Keys in `ssh_dir`, by name. Public key files that don't parse are skipped.
pub fn list(ssh_dir: &Path) -> Vec<SshKey> {
    let Ok(entries) = fs::read_dir(ssh_dir) else {
        return Vec::new();
    };
    let mut keys: Vec<SshKey> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.strip_suffix(".pub")?.to_owned();
            let text = fs::read_to_string(&path).ok()?;
            let public = PublicKey::from_openssh(text.trim()).ok()?;
            Some(describe(&ssh_dir.join(&name), name, &public))
        })
        .collect();
    keys.sort_by(|a, b| a.name.cmp(&b.name));
    keys
}

fn describe(private: &Path, name: String, public: &PublicKey) -> SshKey {
    SshKey {
        name,
        path: private.to_string_lossy().into_owned(),
        has_private: private.is_file(),
        public_key: public.to_openssh().unwrap_or_default(),
        algorithm: public.algorithm().to_string(),
        comment: public.comment().to_owned(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
    }
}

/// Generates an ed25519 key pair as `ssh_dir/name` and `name.pub`, the
/// private key encrypted with `passphrase` unless it is empty. Never
/// overwrites an existing key.
pub fn generate(
    ssh_dir: &Path,
    name: &str,
    comment: &str,
    passphrase: &str,
) -> Result<SshKey, String> {
    let valid = !name.is_empty()
        && !name.starts_with('.')
        && !name.ends_with(".pub")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !valid {
        return Err(format!(
            "“{name}” is not a valid key name: use letters, digits, “.”, “_” and “-”"
        ));
    }
    let private_path = ssh_dir.join(name);
    let public_path = ssh_dir.join(format!("{name}.pub"));
    if private_path.exists() || public_path.exists() {
        return Err(format!("a key named “{name}” already exists"));
    }

    let mut key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).map_err(|e| e.to_string())?;
    key.set_comment(comment.trim());
    let public = key.public_key().clone();
    if !passphrase.is_empty() {
        key = key
            .encrypt(&mut OsRng, passphrase)
            .map_err(|e| e.to_string())?;
    }
    let private_text = key.to_openssh(LineEnding::LF).map_err(|e| e.to_string())?;
    let public_text = public.to_openssh().map_err(|e| e.to_string())?;

    create_dir(ssh_dir).map_err(|e| format!("could not create {}: {e}", ssh_dir.display()))?;
    write_new(&private_path, private_text.as_bytes(), true)
        .map_err(|e| format!("could not write {}: {e}", private_path.display()))?;
    if let Err(e) = write_new(&public_path, format!("{public_text}\n").as_bytes(), false) {
        let _ = fs::remove_file(&private_path);
        return Err(format!("could not write {}: {e}", public_path.display()));
    }
    Ok(describe(&private_path, name.to_owned(), &public))
}

/// `~/.ssh` must not be readable by others, or ssh refuses to use it.
fn create_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_new(path: &Path, contents: &[u8], private: bool) -> std::io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if private { 0o600 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = private;
    options.open(path)?.write_all(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_lists_and_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".ssh");
        let key = generate(&dir, "id_work", "jo@work.example", "correct horse").unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert!(key.public_key.ends_with(" jo@work.example"));
        assert!(key.fingerprint.starts_with("SHA256:"));
        assert!(key.has_private);

        let private = fs::read_to_string(dir.join("id_work")).unwrap();
        let parsed = PrivateKey::from_openssh(&private).unwrap();
        assert!(parsed.is_encrypted());
        assert!(parsed.decrypt("correct horse").is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("id_work"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        generate(&dir, "plain", "", "").unwrap();
        let plain =
            PrivateKey::from_openssh(fs::read_to_string(dir.join("plain")).unwrap()).unwrap();
        assert!(!plain.is_encrypted());

        fs::write(dir.join("junk.pub"), "not a key").unwrap();
        fs::write(dir.join("orphan.pub"), format!("{}\n", key.public_key)).unwrap();
        let listed = list(&dir);
        let names: Vec<(&str, bool)> = listed
            .iter()
            .map(|k| (k.name.as_str(), k.has_private))
            .collect();
        assert_eq!(
            names,
            [("id_work", true), ("orphan", false), ("plain", true)]
        );
        assert_eq!(listed[0], key);

        assert!(
            generate(&dir, "id_work", "", "")
                .unwrap_err()
                .contains("already exists")
        );
        assert!(generate(&dir, "../evil", "", "").is_err());
        assert!(generate(&dir, "x.pub", "", "").is_err());
    }
}
