//! In-app credential prompts.
//!
//! git (and ssh) ask for missing credentials through `GIT_ASKPASS` /
//! `SSH_ASKPASS`: they run a program with the prompt as its argument and
//! read the answer from its stdout. Our own executable is that program. Run
//! that way, it connects back to the app over a localhost socket, the app
//! shows the prompt, and the answer goes back the same way.
//!
//! The executable is also a git credential helper, appended after the
//! user's own (so theirs still come first): it answers with the token of an
//! account on the host, and git tells it whether the token worked. Only
//! when no helper has a credential does git prompt through askpass.
//!
//! The socket accepts only requests carrying a random token that is passed
//! to git's children in the environment.

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

/// A git credential helper: gets the action (`get`, `store`, `erase`) and
/// git's `key=value` lines, and for `get` may answer with its own.
pub type Helper = dyn Fn(&str, &str) -> Option<String> + Send + Sync;

/// First argument when git runs the executable as a credential helper.
const HELPER_ARG: &str = "credential-helper";

pub struct Askpass {
    addr: SocketAddr,
    token: String,
    exe: PathBuf,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<Option<String>>>>,
    helper: Mutex<Option<Arc<Helper>>>,
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
            helper: Mutex::new(None),
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

    /// Environment that makes git and ssh prompt through the app, and
    /// adds the app as git's last credential helper.
    pub fn env(&self) -> Vec<(String, String)> {
        let exe = self.exe.to_string_lossy().into_owned();
        let mut env: Vec<(String, String)> = vec![
            ("GIT_ASKPASS".into(), exe.clone()),
            ("SSH_ASKPASS".into(), exe.clone()),
            // OpenSSH 8.4+: use the askpass program even with a terminal.
            ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
            (ADDR_ENV.into(), self.addr.to_string()),
            (TOKEN_ENV.into(), self.token.clone()),
        ];
        // git 2.31+ reads extra config from the environment, after every
        // config file; keep any the user set.
        let count: usize = std::env::var("GIT_CONFIG_COUNT")
            .ok()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let quoted = format!("'{}'", exe.replace('\'', "'\\''"));
        env.extend([
            ("GIT_CONFIG_COUNT".into(), (count + 1).to_string()),
            (
                format!("GIT_CONFIG_KEY_{count}"),
                "credential.helper".into(),
            ),
            (
                format!("GIT_CONFIG_VALUE_{count}"),
                format!("!{quoted} {HELPER_ARG}"),
            ),
        ]);
        env
    }

    pub fn set_helper(&self, helper: Box<Helper>) {
        *lock(&self.helper) = Some(helper.into());
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
        let (Some(token), Some(mode), Some(prompt)) = (parts.next(), parts.next(), parts.next())
        else {
            return Ok(());
        };
        if token != self.token {
            return Ok(());
        }
        if let Some(action) = mode.strip_prefix("helper:") {
            let helper = lock(&self.helper).clone();
            return match helper.and_then(|h| h(action, prompt)) {
                Some(answer) => stream.write_all(format!("1\n{answer}").as_bytes()),
                None => stream.write_all(b"0\n"),
            };
        }
        // Background commands never interrupt the user.
        if mode == "1" {
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
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_default();
    if first == HELPER_ARG {
        // Never start the app from here, whatever happens.
        let (Ok(addr), Ok(token)) = (std::env::var(ADDR_ENV), std::env::var(TOKEN_ENV)) else {
            return Some(0);
        };
        let action = args.next().unwrap_or_default();
        let mut input = String::new();
        let _ = io::stdin().take(MAX_PROMPT).read_to_string(&mut input);
        if let Ok(Some(answer)) = send(&addr, &token, &format!("helper:{action}"), &input) {
            print!("{answer}");
        }
        return Some(0);
    }
    let addr = std::env::var(ADDR_ENV).ok()?;
    let token = std::env::var(TOKEN_ENV).ok()?;
    let background = std::env::var_os(BACKGROUND_ENV).is_some();
    Some(match ask(&addr, &token, background, &first) {
        Ok(Some(answer)) => {
            println!("{answer}");
            0
        }
        _ => 1,
    })
}

fn ask(addr: &str, token: &str, background: bool, prompt: &str) -> io::Result<Option<String>> {
    send(addr, token, if background { "1" } else { "0" }, prompt)
}

fn send(addr: &str, token: &str, mode: &str, body: &str) -> io::Result<Option<String>> {
    let mut stream = TcpStream::connect(addr)?;
    stream.write_all(format!("{token}\n{mode}\n{body}").as_bytes())?;
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
        // …and the credential helper answers from its own knowledge.
        askpass.set_helper(Box::new(|action, input| {
            (action == "get" && input.contains("host=known.example"))
                .then(|| "username=me\npassword=tok\n".to_owned())
        }));
        let get = |input: &str| send(&addr, &askpass.token, "helper:get", input).unwrap();
        assert_eq!(
            get("protocol=https\nhost=known.example\n").as_deref(),
            Some("username=me\npassword=tok\n")
        );
        assert_eq!(get("protocol=https\nhost=other.example\n"), None);
        assert_eq!(
            send(&addr, "nope", "helper:get", "host=known.example").unwrap(),
            None
        );

        let seen = lock(&seen);
        assert_eq!(seen.len(), 2);
        assert!(seen[0].secret);
        assert!(!seen[1].secret);
        assert_eq!(seen[0].prompt, prompt.trim());
    }
}
