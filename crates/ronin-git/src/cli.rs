use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read, Write};
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
        self.exec(cwd, args, None)?.check(&[0])
    }

    /// Runs git with `input` on stdin and returns its stdout.
    pub fn run_with_input<I, S>(&self, cwd: &Path, args: I, input: &[u8]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, Some(input))?.check(&[0])
    }

    /// Like [`run`](Self::run), but also accepts the given non-zero exit
    /// codes (e.g. `git diff --no-index` exits with 1 when files differ).
    pub fn run_accepting<I, S>(&self, cwd: &Path, args: I, codes: &[i32]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, None)?.check(codes)
    }

    /// Runs git and returns how it finished, whatever its exit code.
    pub(crate) fn run_raw<I, S>(&self, cwd: &Path, args: I) -> Result<Finished>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.exec(cwd, args, None)
    }

    /// Runs a long git command (fetch, push, clone, …), passing each
    /// progress update git prints on stderr to `on_progress`. Pass
    /// `--progress` in `args`: git only reports progress to a terminal
    /// otherwise. Returns how it finished, whatever its exit code.
    pub(crate) fn run_streaming<I, S>(
        &self,
        cwd: &Path,
        args: I,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<Finished>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<OsString> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
        let mut child = self
            .command(cwd)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let reader = std::thread::spawn(move || {
            let mut out = Vec::new();
            stdout.read_to_end(&mut out).map(|_| out)
        });

        // Progress lines end in '\r' while they update and '\n' when done;
        // messages always end in '\n'. Only the messages are kept.
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let mut messages: Vec<String> = Vec::new();
        let mut line: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = match stderr.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            for &b in &buf[..n] {
                if b != b'\r' && b != b'\n' {
                    line.push(b);
                    continue;
                }
                let text = String::from_utf8_lossy(&line).trim().to_owned();
                line.clear();
                if text.is_empty() {
                    continue;
                }
                match Progress::parse(&text) {
                    Some(progress) => on_progress(progress),
                    None if b == b'\n' => {
                        on_progress(Progress {
                            message: text.clone(),
                            percent: None,
                        });
                        messages.push(text);
                    }
                    None => {}
                }
            }
        }
        if !line.is_empty() {
            messages.push(String::from_utf8_lossy(&line).trim().to_owned());
        }
        let status = child.wait()?;
        let stdout = reader
            .join()
            .map_err(|_| io::Error::other("stdout reader panicked"))??;
        Ok(Finished {
            args: join_args(&args),
            code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: messages.join("\n"),
        })
    }

    fn exec<I, S>(&self, cwd: &Path, args: I, input: Option<&[u8]>) -> Result<Finished>
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
        Ok(Finished {
            args: join_args(&args),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
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
            // Commands that would open an editor (merge --continue, revert,
            // rebase --continue) keep the message git prepared.
            .env("GIT_EDITOR", "true")
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

/// How a git command finished.
#[derive(Debug)]
pub(crate) struct Finished {
    args: String,
    /// `None` if git was killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Finished {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Stdout if git exited with one of `codes`, else an error with stderr.
    pub fn check(self, codes: &[i32]) -> Result<String> {
        if self.code.is_some_and(|c| codes.contains(&c)) {
            Ok(self.stdout)
        } else {
            Err(self.into_error())
        }
    }

    pub fn into_error(self) -> Error {
        Error::Command {
            args: self.args,
            code: self
                .code
                .map_or_else(|| "killed by signal".into(), |c| c.to_string()),
            stderr: self.stderr,
        }
    }
}

fn join_args(args: &[OsString]) -> String {
    args.iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// One progress update from a long-running command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    /// E.g. "Receiving objects", or a whole message line from git.
    pub message: String,
    pub percent: Option<u8>,
}

impl Progress {
    /// Parses lines such as `Receiving objects:  45% (450/1000), 1.2 MiB`
    /// or `remote: Counting objects: 100% (10/10), done.`
    fn parse(line: &str) -> Option<Self> {
        let line = line.strip_prefix("remote: ").unwrap_or(line);
        let (phase, rest) = line.split_once(": ")?;
        let digits = rest.trim_start().split('%').next()?;
        let percent: u8 = digits.parse().ok()?;
        Some(Self {
            message: phase.trim().to_owned(),
            percent: Some(percent.min(100)),
        })
    }
}

/// Rejects a user-supplied name or revision that git would read as an
/// option.
pub(crate) fn operand(value: &str) -> Result<&str> {
    if value.is_empty() || value.starts_with('-') {
        return Err(Error::Invalid(format!("invalid name: “{value}”")));
    }
    Ok(value)
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
    fn parses_progress_lines() {
        let p = |s| Progress::parse(s);
        assert_eq!(
            p("Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s"),
            Some(Progress {
                message: "Receiving objects".into(),
                percent: Some(45)
            })
        );
        assert_eq!(
            p("remote: Counting objects: 100% (10/10), done.")
                .unwrap()
                .message,
            "Counting objects"
        );
        assert_eq!(p("From github.com:owner/repo"), None);
        assert_eq!(p("error: failed to push some refs"), None);
    }

    #[test]
    fn orders_versions() {
        assert!(v(2, 29, 9) < MIN_GIT_VERSION);
        assert!(v(2, 30, 0) >= MIN_GIT_VERSION);
        assert!(v(3, 0, 0) > MIN_GIT_VERSION);
    }
}
