use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use ts_rs::TS;

use crate::{Error, Result};

pub const MIN_GIT_VERSION: GitVersion = GitVersion {
    major: 2,
    minor: 30,
    patch: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Parses `git --version` output such as `git version 2.45.1.windows.1`
    /// or `git version 2.39.3 (Apple Git-146)`.
    pub fn parse(output: &str) -> Option<Self> {
        let version = output.trim().strip_prefix("git version ")?;
        let mut parts = version.split(|c: char| !c.is_ascii_digit());
        let mut next = || -> Option<u32> { parts.next()?.parse().ok() };
        Some(Self {
            major: next()?,
            minor: next()?,
            patch: next().unwrap_or(0),
        })
    }
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Handle to the system `git` binary.
#[derive(Debug, Clone)]
pub struct GitCli {
    exe: PathBuf,
    version: GitVersion,
    env: Vec<(OsString, OsString)>,
}

impl GitCli {
    /// Locates `git` on `PATH` and checks it meets [`MIN_GIT_VERSION`].
    pub fn discover() -> Result<Self> {
        Self::with_executable("git")
    }

    pub fn with_executable(exe: impl Into<PathBuf>) -> Result<Self> {
        let exe = exe.into();
        let output = Command::new(&exe)
            .arg("--version")
            .output()
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => Error::GitNotFound,
                _ => e.into(),
            })?;
        let version = GitVersion::parse(&String::from_utf8_lossy(&output.stdout))
            .ok_or(Error::GitNotFound)?;
        if version < MIN_GIT_VERSION {
            return Err(Error::GitTooOld { found: version });
        }
        Ok(Self {
            exe,
            version,
            env: Vec::new(),
        })
    }

    /// Adds an environment variable to every git invocation.
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    pub fn version(&self) -> GitVersion {
        self.version
    }

    /// Runs git in `cwd` and returns its stdout.
    pub fn run<I, S>(&self, cwd: &Path, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, None, &[0])
    }

    /// Runs git with `input` on stdin and returns its stdout.
    pub fn run_with_input<I, S>(&self, cwd: &Path, args: I, input: &[u8]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, Some(input), &[0])
    }

    /// Like [`run`](Self::run), but also accepts the given non-zero exit
    /// codes (e.g. `git diff --no-index` exits with 1 when files differ).
    pub fn run_accepting<I, S>(&self, cwd: &Path, args: I, codes: &[i32]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, None, codes)
    }

    fn exec<I, S>(&self, cwd: &Path, args: I, input: Option<&[u8]>, codes: &[i32]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<OsString> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
        let mut cmd = self.command(cwd);
        cmd.args(&args);
        let output = match input {
            None => cmd.output()?,
            Some(input) => {
                let mut child = cmd
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                // Written from another thread: git may fill its stdout pipe
                // before it has read all of its input.
                let mut stdin = child.stdin.take().expect("stdin is piped");
                let input = input.to_owned();
                let writer = std::thread::spawn(move || stdin.write_all(&input));
                let output = child.wait_with_output()?;
                // A broken pipe means git stopped reading early; its exit
                // status and stderr say why.
                match writer.join() {
                    Ok(Err(e)) if e.kind() != io::ErrorKind::BrokenPipe => return Err(e.into()),
                    _ => {}
                }
                output
            }
        };
        if output.status.code().is_some_and(|c| codes.contains(&c)) {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        Err(Error::Command {
            args: args
                .iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            code: output
                .status
                .code()
                .map_or_else(|| "killed by signal".into(), |c| c.to_string()),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }

    fn command(&self, cwd: &Path) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.current_dir(cwd)
            // Stable, English output regardless of the user's locale.
            .env("LC_ALL", "C")
            // Never hang on a terminal prompt; the UI supplies its own.
            .env("GIT_TERMINAL_PROMPT", "0")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_version_strings() {
        assert_eq!(GitVersion::parse("git version 2.55.0\n"), Some(v(2, 55, 0)));
        assert_eq!(
            GitVersion::parse("git version 2.45.1.windows.1"),
            Some(v(2, 45, 1))
        );
        assert_eq!(
            GitVersion::parse("git version 2.39.3 (Apple Git-146)"),
            Some(v(2, 39, 3))
        );
        assert_eq!(GitVersion::parse("git version 2.40"), Some(v(2, 40, 0)));
        assert_eq!(GitVersion::parse("hg version 6.0"), None);
    }

    #[test]
    fn orders_versions() {
        assert!(v(2, 29, 9) < MIN_GIT_VERSION);
        assert!(v(2, 30, 0) >= MIN_GIT_VERSION);
        assert!(v(3, 0, 0) > MIN_GIT_VERSION);
    }
}
