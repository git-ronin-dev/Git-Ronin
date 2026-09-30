//! In-app credential prompts.
//!
//! git (and ssh) ask for missing credentials through `GIT_ASKPASS` /
//! `SSH_ASKPASS`: they run a program with the prompt as its argument and
//! read the answer from its stdout. Our own executable is that program. Run
//! that way, it connects back to the app over a localhost socket, the app
//! shows the prompt, and the answer goes back the same way.
//!
//! Credential helpers configured in git still come first; this is only the
//! fallback. The socket accepts only requests carrying a random token that
//! is passed to git's children in the environment.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::state::lock;

const ADDR_ENV: &str = "RONIN_ASKPASS_ADDR";
const TOKEN_ENV: &str = "RONIN_ASKPASS_TOKEN";
/// Set for background commands (auto-fetch): prompts fail at once instead
/// of interrupting the user.
pub const BACKGROUND_ENV: &str = "RONIN_ASKPASS_BACKGROUND";

/// Event asking the UI for a credential; answered by `credential_respond`.
pub const CREDENTIAL_REQUEST: &str = "credential-request";

/// How long a prompt waits for the user before git is told "no".
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_PROMPT: u64 = 16 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialRequest {
    id: u64,
    prompt: String,
    /// Passwords, passphrases and tokens are typed hidden.
    secret: bool,
}

/// Shows a request to the user; false if it couldn't be shown.
type Notify = dyn Fn(CredentialRequest) -> bool + Send + Sync;

/// Answers a prompt without asking the user (an account's token), if it can.
pub type Resolver = dyn Fn(&str) -> Option<String> + Send + Sync;

pub struct Askpass {
    addr: SocketAddr,
    token: String,
    exe: PathBuf,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<Option<String>>>>,
    resolver: Mutex<Option<Arc<Resolver>>>,
}

impl Askpass {
    /// Listens for prompts on a free localhost port and forwards them to
    /// the UI as `CREDENTIAL_REQUEST` events.
    pub fn start(app: AppHandle) -> io::Result<Arc<Self>> {
        Self::start_with(Box::new(move |request| {
            app.emit(CREDENTIAL_REQUEST, request).is_ok()
        }))
    }

    fn start_with(notify: Box<Notify>) -> io::Result<Arc<Self>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|e| io::Error::other(e.to_string()))?;
        let askpass = Arc::new(Self {
            addr: listener.local_addr()?,
            token: token.iter().map(|b| format!("{b:02x}")).collect(),
            exe: std::env::current_exe()?,
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            resolver: Mutex::new(None),
        });
        let server = askpass.clone();
        std::thread::Builder::new()
            .name("askpass".into())
            .spawn(move || {
                let notify: Arc<Notify> = notify.into();
                for stream in listener.incoming().flatten() {
                    let server = server.clone();
                    let notify = notify.clone();
                    std::thread::spawn(move || {
                        let _ = server.serve(&*notify, stream);
                    });
                }
            })?;
        Ok(askpass)
    }

    /// Environment that makes git and ssh prompt through the app.
    pub fn env(&self) -> Vec<(&'static str, String)> {
        let exe = self.exe.to_string_lossy().into_owned();
        vec![
            ("GIT_ASKPASS", exe.clone()),
            ("SSH_ASKPASS", exe),
            // OpenSSH 8.4+: use the askpass program even with a terminal.
            ("SSH_ASKPASS_REQUIRE", "force".into()),
            (ADDR_ENV, self.addr.to_string()),
            (TOKEN_ENV, self.token.clone()),
        ]
    }

    pub fn set_resolver(&self, resolver: Box<Resolver>) {
        *lock(&self.resolver) = Some(resolver.into());
    }

    /// Passes the user's answer (`None` if they cancelled) to the waiting prompt.
    pub fn respond(&self, id: u64, answer: Option<String>) {
        if let Some(tx) = lock(&self.pending).remove(&id) {
            let _ = tx.send(answer);
        }
    }

    fn serve(&self, notify: &Notify, mut stream: TcpStream) -> io::Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut request = String::new();
        (&mut stream)
            .take(MAX_PROMPT)
            .read_to_string(&mut request)?;
        let mut parts = request.splitn(3, '\n');
        let (Some(token), Some(background), Some(prompt)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return Ok(());
        };
        if token != self.token {
            return Ok(());
        }
        let resolver = lock(&self.resolver).clone();
        if let Some(answer) = resolver.and_then(|r| r(prompt.trim())) {
            return stream.write_all(format!("1\n{answer}").as_bytes());
        }
        // Background commands never interrupt the user.
        if background == "1" {
            return stream.write_all(b"0\n");
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = channel();
        lock(&self.pending).insert(id, tx);
        let lower = prompt.to_lowercase();
        let secret = ["password", "passphrase", "token", "pin for", "pin:"]
            .iter()
            .any(|w| lower.contains(w));
        let request = CredentialRequest {
            id,
            prompt: prompt.trim().to_owned(),
            secret,
        };
        let answer = if notify(request) {
            rx.recv_timeout(ANSWER_TIMEOUT).ok().flatten()
        } else {
            None
        };
        lock(&self.pending).remove(&id);
        match answer {
            Some(answer) => stream.write_all(format!("1\n{answer}").as_bytes()),
            None => stream.write_all(b"0\n"),
        }
    }
}

/// When git or ssh started this executable as its askpass program, answers
/// the prompt and returns the exit code; otherwise `None`.
pub fn client() -> Option<i32> {
    let addr = std::env::var(ADDR_ENV).ok()?;
    let token = std::env::var(TOKEN_ENV).ok()?;
    let background = std::env::var_os(BACKGROUND_ENV).is_some();
    let prompt = std::env::args().nth(1).unwrap_or_default();
    Some(match ask(&addr, &token, background, &prompt) {
        Ok(Some(answer)) => {
            println!("{answer}");
            0
        }
        _ => 1,
    })
}

fn ask(addr: &str, token: &str, background: bool, prompt: &str) -> io::Result<Option<String>> {
    let mut stream = TcpStream::connect(addr)?;
    let background = u8::from(background);
    stream.write_all(format!("{token}\n{background}\n{prompt}").as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply.strip_prefix("1\n").map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_prompts_and_rejects_strangers() {
        let answers: Arc<Mutex<Option<Arc<Askpass>>>> = Arc::default();
        let seen: Arc<Mutex<Vec<CredentialRequest>>> = Arc::default();
        let (a, s) = (answers.clone(), seen.clone());
        let askpass = Askpass::start_with(Box::new(move |request| {
            let server = lock(&a).clone().unwrap();
            let answer = (!request.prompt.starts_with("Username")).then(|| "hunter2".to_owned());
            let id = request.id;
            lock(&s).push(request);
            std::thread::spawn(move || server.respond(id, answer));
            true
        }))
        .unwrap();
        *lock(&answers) = Some(askpass.clone());
        let addr = askpass.addr.to_string();

        let prompt = "Password for 'https://me@example.com': ";
        let answer = ask(&addr, &askpass.token, false, prompt).unwrap();
        assert_eq!(answer.as_deref(), Some("hunter2"));
        // Cancelled.
        let answer = ask(
            &addr,
            &askpass.token,
            false,
            "Username for 'https://example.com': ",
        );
        assert_eq!(answer.unwrap(), None);
        // Wrong token: never shown.
        assert_eq!(ask(&addr, "nope", false, prompt).unwrap(), None);
        // In the background nobody is asked…
        assert_eq!(ask(&addr, &askpass.token, true, prompt).unwrap(), None);
        // …but a resolver may still answer.
        askpass.set_resolver(Box::new(|p| p.contains("known").then(|| "tok".to_owned())));
        let known = "Password for 'https://known.example': ";
        assert_eq!(
            ask(&addr, &askpass.token, true, known).unwrap().as_deref(),
            Some("tok")
        );

        let seen = lock(&seen);
        assert_eq!(seen.len(), 2);
        assert!(seen[0].secret);
        assert!(!seen[1].secret);
        assert_eq!(seen[0].prompt, prompt.trim());
    }
}
