//! Shells in pseudo-terminals for the terminal panel. Output streams to the
//! UI over a channel; input and resizes come back as commands.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;

use crate::state::lock;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TerminalEvent {
    Data {
        data: String,
    },
    /// The shell exited; `code` is `None` when it was killed.
    Exit {
        code: Option<u32>,
    },
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    next: AtomicU32,
    sessions: Arc<Mutex<HashMap<u32, Session>>>,
}

impl Terminals {
    /// Starts `shell` (the platform's default when empty) in `cwd`.
    pub fn open(
        &self,
        shell: &str,
        cwd: &Path,
        size: (u16, u16),
        env: impl IntoIterator<Item = (&'static str, std::ffi::OsString)>,
        mut emit: impl FnMut(TerminalEvent) + Send + 'static,
    ) -> Result<u32, String> {
        let (cols, rows) = size;
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        let shell = if shell.trim().is_empty() {
            default_shell()
        } else {
            shell.trim().to_owned()
        };
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(cwd);
        for (key, value) in crate::env::child_env() {
            match value {
                Some(value) => cmd.env(key, value),
                None => cmd.env_remove(key),
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in env {
            cmd.env(key, value);
        }
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("could not start {shell}: {e}"))?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.sessions).insert(
            id,
            Session {
                master: pair.master,
                writer,
                child,
            },
        );

        let sessions = Arc::clone(&self.sessions);
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut pending: Vec<u8> = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        pending.extend_from_slice(&buf[..n]);
                        let data = take_utf8(&mut pending);
                        if !data.is_empty() {
                            emit(TerminalEvent::Data { data });
                        }
                    }
                }
            }
            let session = lock(&sessions).remove(&id);
            let code =
                session.and_then(|mut s| s.child.wait().ok().map(|status| status.exit_code()));
            emit(TerminalEvent::Exit { code });
        });
        Ok(id)
    }

    pub fn write(&self, id: u32, data: &str) -> Result<(), String> {
        let mut sessions = lock(&self.sessions);
        let session = sessions.get_mut(&id).ok_or("the terminal has closed")?;
        session
            .writer
            .write_all(data.as_bytes())
            .and_then(|()| session.writer.flush())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = lock(&self.sessions);
        let session = sessions.get(&id).ok_or("the terminal has closed")?;
        session
            .master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn close(&self, id: u32) {
        if let Some(mut session) = lock(&self.sessions).remove(&id) {
            let _ = session.child.kill();
        }
    }
}

fn default_shell() -> String {
    if cfg!(windows) {
        "powershell.exe".into()
    } else {
        // Inside a Flatpak the user's shell may not exist.
        std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty() && std::path::Path::new(s).exists())
            .unwrap_or_else(|| "/bin/sh".into())
    }
}

/// Takes the longest valid UTF-8 prefix of `bytes`, leaving an incomplete
/// character at the end for the next read. Invalid bytes are replaced.
fn take_utf8(bytes: &mut Vec<u8>) -> String {
    let keep = match std::str::from_utf8(bytes) {
        Ok(_) => 0,
        // An incomplete sequence at the end: wait for the rest.
        Err(e) if e.error_len().is_none() => bytes.len() - e.valid_up_to(),
        Err(_) => 0,
    };
    let rest = bytes.split_off(bytes.len() - keep);
    let text = String::from_utf8_lossy(bytes).into_owned();
    *bytes = rest;
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_incomplete_characters_for_the_next_read() {
        let mut bytes = "añ".as_bytes().to_vec();
        let last = bytes.pop().unwrap();
        assert_eq!(take_utf8(&mut bytes), "a");
        bytes.push(last);
        assert_eq!(take_utf8(&mut bytes), "ñ");
        assert!(bytes.is_empty());
        let mut invalid = vec![b'x', 0xff, b'y'];
        assert_eq!(take_utf8(&mut invalid), "x\u{fffd}y");
    }

    #[cfg(unix)]
    #[test]
    fn runs_a_shell_and_reports_its_exit() {
        use std::sync::mpsc;
        use std::time::Duration;

        let terminals = Terminals::default();
        let (tx, rx) = mpsc::channel();
        let tmp = tempfile::tempdir().unwrap();
        let id = terminals
            .open("/bin/sh", tmp.path(), (80, 24), [], move |e| {
                let _ = tx.send(e);
            })
            .unwrap();
        terminals.resize(id, 100, 30).unwrap();
        terminals
            .write(id, "echo ronin-$((6*7)); exit 3\n")
            .unwrap();

        let mut output = String::new();
        loop {
            match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                TerminalEvent::Data { data } => output.push_str(&data),
                TerminalEvent::Exit { code } => {
                    assert_eq!(code, Some(3));
                    break;
                }
            }
        }
        assert!(output.contains("ronin-42"), "{output}");
        assert!(terminals.write(id, "x").is_err());
    }
}
