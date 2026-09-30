//! Runs the settings sync ([`ronin_config::sync`]) in the background:
//! pulls on startup, commits changes a few seconds after they're made,
//! pulls and pushes every `pushMinutes`, and pushes on exit.

use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ronin_config::merge::{self, Change};
use ronin_config::sync::{self, PendingMerge, PullOutcome, SyncStatus};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::{AppState, CmdResult, err, lock};

/// Event emitted when the portable settings changed underneath the UI.
pub const CONFIG_CHANGED: &str = "config-changed";
/// Event emitted when the sync status changed.
pub const SYNC_CHANGED: &str = "sync-changed";

/// Quiet time after a settings change before it is committed.
const COMMIT_DELAY: Duration = Duration::from_secs(3);
/// Longest the app waits for the final push when it exits.
const EXIT_WAIT: Duration = Duration::from_secs(8);

enum Msg {
    /// The portable settings were written.
    Changed,
    /// Sync settings changed, or a sync was asked for: sync now.
    Restart,
}

#[derive(Default)]
pub struct SyncService {
    tx: Mutex<Option<Sender<Msg>>>,
    status: Mutex<Runtime>,
    pending: Mutex<Option<PendingMerge>>,
    /// One git command in the sync repository at a time.
    busy: Mutex<()>,
}

#[derive(Default)]
struct Runtime {
    running: bool,
    last_sync: Option<u64>,
    error: Option<String>,
}

impl SyncService {
    /// Starts the background worker. It idles while sync is off.
    pub fn start(&self, app: AppHandle) {
        let (tx, rx) = mpsc::channel();
        *lock(&self.tx) = Some(tx);
        std::thread::spawn(move || {
            let mut commit_at: Option<Instant> = None;
            let mut sync_at = Some(Instant::now());
            loop {
                let state = app.state::<AppState>();
                let settings = state.config().get().local.sync.clone();
                let Some(settings) = settings else {
                    commit_at = None;
                    match rx.recv() {
                        Ok(Msg::Restart) => sync_at = Some(Instant::now()),
                        Ok(Msg::Changed) => {}
                        Err(_) => return,
                    }
                    continue;
                };
                let next = [commit_at, sync_at].into_iter().flatten().min();
                let wait = next.map(|t| t.saturating_duration_since(Instant::now()));
                let msg = match wait {
                    Some(wait) => rx.recv_timeout(wait),
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match msg {
                    Ok(Msg::Changed) => commit_at = Some(Instant::now() + COMMIT_DELAY),
                    Ok(Msg::Restart) => sync_at = Some(Instant::now()),
                    Err(RecvTimeoutError::Disconnected) => return,
                    Err(RecvTimeoutError::Timeout) => {
                        let now = Instant::now();
                        if sync_at.is_some_and(|t| t <= now) {
                            commit_at = None;
                            let _ = state.sync.run(&app, true);
                            sync_at = (settings.push_minutes > 0).then(|| {
                                Instant::now()
                                    + Duration::from_secs(u64::from(settings.push_minutes) * 60)
                            });
                        } else if commit_at.is_some_and(|t| t <= now) {
                            commit_at = None;
                            state.sync.commit(&app);
                        }
                    }
                }
            }
        });
    }

    /// The portable settings were written; commit them soon.
    pub fn changed(&self) {
        self.send(Msg::Changed);
    }

    /// Sync as soon as possible (after enabling, or when asked to).
    pub fn restart(&self) {
        self.send(Msg::Restart);
    }

    fn send(&self, msg: Msg) {
        if let Some(tx) = lock(&self.tx).as_ref() {
            let _ = tx.send(msg);
        }
    }

    pub fn status(&self, app: &AppHandle) -> SyncStatus {
        let state = app.state::<AppState>();
        let settings = state.config().get().local.sync.clone();
        let runtime = lock(&self.status);
        let conflicts = lock(&self.pending)
            .as_ref()
            .map(|p| {
                p.conflicts
                    .iter()
                    .map(|c| Change {
                        key: merge::display(&c.path),
                        current: c.ours.as_ref().map(merge::show),
                        incoming: c.theirs.as_ref().map(merge::show),
                    })
                    .collect()
            })
            .unwrap_or_default();
        SyncStatus {
            settings,
            running: runtime.running,
            last_sync: runtime.last_sync,
            error: runtime.error.clone(),
            conflicts,
        }
    }

    /// Commits local changes without touching the network.
    fn commit(&self, app: &AppHandle) {
        let state = app.state::<AppState>();
        let _busy = lock(&self.busy);
        let dir = state.config().dir().to_owned();
        let result = state
            .background_git()
            .and_then(|git| sync::commit(&git, &dir).map_err(err));
        if let Err(e) = result {
            lock(&self.status).error = Some(e);
            let _ = app.emit(SYNC_CHANGED, ());
        }
    }

    /// Pulls, merges and pushes. `background` runs never prompt for
    /// credentials.
    pub fn run(&self, app: &AppHandle, background: bool) -> CmdResult<()> {
        let state = app.state::<AppState>();
        let _busy = lock(&self.busy);
        let (settings, dir) = {
            let config = state.config();
            (config.get().local.sync.clone(), config.dir().to_owned())
        };
        let Some(settings) = settings else {
            return Ok(());
        };
        self.update(app, |r| r.running = true);
        let git = if background {
            state.background_git()
        } else {
            state.git()
        };
        let result = git.and_then(|git| sync::sync(&git, &dir, &settings.branch).map_err(err));
        let result = match result {
            Ok(PullOutcome::UpToDate) => Ok(()),
            Ok(PullOutcome::Updated) => {
                *lock(&self.pending) = None;
                reload(app);
                Ok(())
            }
            Ok(PullOutcome::Conflicts(pending)) => {
                *lock(&self.pending) = Some(pending);
                Ok(())
            }
            Err(e) => Err(e),
        };
        self.update(app, |r| {
            r.running = false;
            r.error = result.as_ref().err().cloned();
            if result.is_ok() {
                r.last_sync = Some(now());
            }
        });
        result
    }

    /// Settles the waiting merge: for each conflict, `true` takes the other
    /// machine's value. Then pushes.
    pub fn resolve(&self, app: &AppHandle, take_theirs: &[bool]) -> CmdResult<()> {
        let state = app.state::<AppState>();
        {
            let _busy = lock(&self.busy);
            let mut pending = lock(&self.pending);
            let Some(merge) = pending.as_mut() else {
                return Err("there is nothing to resolve".into());
            };
            if take_theirs.len() != merge.conflicts.len() {
                return Err("the conflicts changed; sync again".into());
            }
            for (conflict, &theirs) in merge.conflicts.iter().zip(take_theirs) {
                if theirs {
                    merge::set(&mut merge.merged, &conflict.path, conflict.theirs.clone());
                }
            }
            let dir = state.config().dir().to_owned();
            sync::finish(&state.git()?, &dir, &merge.theirs, &merge.merged).map_err(err)?;
            *pending = None;
        }
        reload(app);
        self.run(app, false)
    }

    /// Commits and pushes before the app exits, giving up after a few
    /// seconds.
    pub fn shutdown(&self, app: &AppHandle) {
        *lock(&self.tx) = None;
        let (settings, dir) = {
            let state = app.state::<AppState>();
            let config = state.config();
            (config.get().local.sync.clone(), config.dir().to_owned())
        };
        let Some(settings) = settings else {
            return;
        };
        let Ok(git) = app.state::<AppState>().background_git() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sync::commit(&git, &dir);
            let _ = sync::push(&git, &dir, &settings.branch);
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(EXIT_WAIT);
    }

    fn update(&self, app: &AppHandle, f: impl FnOnce(&mut Runtime)) {
        f(&mut lock(&self.status));
        let _ = app.emit(SYNC_CHANGED, ());
    }

    /// Forgets a waiting merge (sync was turned off).
    pub fn clear(&self) {
        *lock(&self.pending) = None;
        *lock(&self.status) = Runtime::default();
    }
}

/// Picks up settings a sync wrote.
fn reload(app: &AppHandle) {
    let state = app.state::<AppState>();
    let warnings = state.config().reload_portable();
    for warning in warnings {
        lock(&state.config_warnings).push(warning);
    }
    if let Some(warning) = state.apply_profile() {
        lock(&state.config_warnings).push(warning);
    }
    let _ = app.emit(CONFIG_CHANGED, ());
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
