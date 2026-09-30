//! IPC commands for hosting services: accounts (tokens in the keyring),
//! repositories, pull requests, issues, CI status and the launchpad. Also
//! answers git's HTTPS credential prompts with an account's token.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ronin_config::accounts::{account_id, host_of};
use ronin_config::{Account, Config, ProviderKind};
use ronin_git::RangeDiff;
use ronin_hosting::oauth::{self, DeviceCode, Poll};
use ronin_hosting::{
    Capabilities, CiStatus, Credential, Error as HostingError, HostedRepo, Involvement, Issue,
    MergeMethod, NewPullRequest, PrSource, Provider, PullRequest, PullRequestDetail, User,
};
use serde::Serialize;
use tauri::AppHandle;
use ts_rs::TS;

use crate::commands::{blocking, reporter};
use crate::state::{AppState, CmdResult, err, lock};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Device sign-ins in progress, by id.
#[derive(Default)]
pub struct DeviceFlows {
    next: AtomicU32,
    flows: std::sync::Mutex<HashMap<u32, Arc<DeviceFlow>>>,
}

pub struct DeviceFlow {
    kind: ProviderKind,
    url: String,
    client_id: String,
    code: DeviceCode,
    cancelled: AtomicBool,
}

/// Accounts whose token git reported rejected, and when.
#[derive(Default)]
pub struct Rejected(std::sync::Mutex<HashMap<String, Instant>>);

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountView {
    pub account: Account,
    pub capabilities: Capabilities,
    /// A token is available (in the keyring or, failing that, in memory).
    pub signed_in: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SignIn {
    pub account: Account,
    /// Set when the token could not be saved for later sessions.
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceStart {
    pub flow: u32,
    pub code: DeviceCode,
}

/// A remote of a local repository that is on one of the accounts' services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoLink {
    pub account: String,
    pub kind: ProviderKind,
    pub remote: String,
    /// The repository on the service, as `HostedRepo::path`.
    pub path: String,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchpadPr {
    pub account: String,
    pub pr: PullRequest,
    pub involvement: Vec<Involvement>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LaunchpadIssue {
    pub account: String,
    pub issue: Issue,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountError {
    pub account: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Launchpad {
    pub pull_requests: Vec<LaunchpadPr>,
    pub issues: Vec<LaunchpadIssue>,
    pub errors: Vec<AccountError>,
}

/// The id of the profile in use (an unset or stale id means the first).
fn active_profile(config: &Config) -> String {
    config
        .portable
        .profile(&config.local.active_profile)
        .map(|p| p.id.clone())
        .unwrap_or_default()
}

/// Accounts of the profile in use. An account whose profile was deleted
/// belongs to the first profile.
fn profile_accounts(config: &Config) -> Vec<Account> {
    let active = active_profile(config);
    config
        .local
        .accounts
        .iter()
        .filter(|a| config.portable.profile(&a.profile).map(|p| p.id.as_str()) == Some(&active))
        .cloned()
        .collect()
}

fn me(account: &Account) -> User {
    User {
        id: account.user_id.clone(),
        username: account.username.clone(),
        name: account.name.clone(),
        avatar_url: None,
        slug: account.username.clone(),
    }
}

impl AppState {
    fn account(&self, id: &str) -> CmdResult<Account> {
        self.config()
            .get()
            .local
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| "the account was removed".to_owned())
    }

    /// The account's token, renewed first if it's about to expire.
    fn credential(&self, account: &Account) -> CmdResult<Credential> {
        let renewable = |c: &Credential| {
            c.expiring(now()) && !account.oauth_client_id.is_empty() && !c.refresh_token.is_empty()
        };
        let credential = self.secrets.get(&account.id)?;
        if !renewable(&credential) {
            return Ok(credential);
        }
        // Refresh tokens work once: requests that find the token expiring
        // together wait for the first to renew it.
        let _renewing = lock(&self.renewing);
        let credential = self.secrets.get(&account.id)?;
        if !renewable(&credential) {
            return Ok(credential);
        }
        let renewed = oauth::refresh(
            self.transport.clone(),
            account.kind,
            &account.url,
            &account.oauth_client_id,
            &credential,
            now(),
        )
        .map_err(err)?;
        self.secrets.set(&account.id, &renewed);
        Ok(renewed)
    }

    fn provider(&self, id: &str) -> CmdResult<(Account, Box<dyn Provider>)> {
        let account = self.account(id)?;
        let credential = self.credential(&account)?;
        let provider =
            ronin_hosting::connect(&account, &credential, self.transport.clone()).map_err(err)?;
        Ok((account, provider))
    }

    /// Checks `credential` and saves the account it belongs to, replacing
    /// an earlier sign-in of the same user.
    fn sign_in(&self, mut account: Account, credential: &Credential) -> CmdResult<SignIn> {
        let provider =
            ronin_hosting::connect(&account, credential, self.transport.clone()).map_err(err)?;
        let user = provider.user().map_err(err)?;
        account.username = if account.kind == ProviderKind::BitbucketServer {
            user.slug.clone()
        } else {
            user.username.clone()
        };
        account.name = user.name;
        account.avatar_url = user.avatar_url.unwrap_or_default();
        account.user_id = user.id;

        let mut config = self.config();
        let profile = active_profile(config.get());
        let existing = config.get().local.accounts.iter().find(|a| {
            a.kind == account.kind && a.url == account.url && a.username == account.username
        });
        account.profile = existing.map_or(profile, |a| a.profile.clone());
        account.id = match existing {
            Some(a) => a.id.clone(),
            None => account_id(
                account.kind,
                &host_of(&account.url),
                &account.username,
                &config.get().local.accounts,
            ),
        };
        let warning = self.secrets.set(&account.id, credential);
        lock(&self.rejected.0).remove(&account.id);
        let saved = account.clone();
        config
            .update_local(|l| match l.accounts.iter_mut().find(|a| a.id == saved.id) {
                Some(a) => *a = saved,
                None => l.accounts.push(saved),
            })
            .map_err(err)?;
        Ok(SignIn { account, warning })
    }

    /// Git's credential helper protocol for account tokens: `get` answers
    /// with the token of an account (of the profile in use) on the host;
    /// `erase` means git found it rejected, so it isn't offered again for a
    /// while (the user is asked instead), and `store` that it worked.
    pub fn credential_helper(&self, action: &str, input: &str) -> Option<String> {
        let fields: HashMap<&str, &str> = input.lines().filter_map(|l| l.split_once('=')).collect();
        let protocol = *fields.get("protocol")?;
        if !matches!(protocol, "https" | "http") {
            return None;
        }
        let host = fields.get("host")?.to_ascii_lowercase();
        let user = fields.get("username").copied();
        let accounts = profile_accounts(self.config().get());
        let account = accounts.into_iter().find(|a| {
            a.kind != ProviderKind::Jira
                && a.url.starts_with(&format!("{protocol}://"))
                && authority(&a.url) == host
                && user.is_none_or(|u| u == git_username(a))
        })?;
        let mut rejected = lock(&self.rejected.0);
        rejected.retain(|_, at| at.elapsed() < Duration::from_secs(10 * 60));
        let ours = || {
            fields
                .get("password")
                .is_none_or(|p| self.secrets.get(&account.id).is_ok_and(|c| c.token == *p))
        };
        match action {
            "get" if !rejected.contains_key(&account.id) => {
                drop(rejected);
                let token = self.credential(&account).ok()?.token;
                Some(format!(
                    "username={}\npassword={token}\n",
                    git_username(&account)
                ))
            }
            "erase" if ours() => {
                rejected.insert(account.id, Instant::now());
                None
            }
            "store" if ours() => {
                rejected.remove(&account.id);
                None
            }
            _ => None,
        }
    }
}

/// `host[:port]` of a URL, lowercased.
fn authority(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or_default();
    authority
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The user name git sends with an account's token over HTTPS.
fn git_username(account: &Account) -> String {
    match account.kind {
        ProviderKind::Gitlab if !account.oauth_client_id.is_empty() => "oauth2".into(),
        ProviderKind::Bitbucket => "x-bitbucket-api-token-auth".into(),
        _ => account.username.clone(),
    }
}

#[tauri::command]
pub async fn hosting_accounts(app: AppHandle) -> CmdResult<Vec<AccountView>> {
    blocking(&app, |_, s| {
        let accounts = s.config().get().local.accounts.clone();
        Ok(accounts
            .into_iter()
            .map(|account| AccountView {
                capabilities: ronin_hosting::capabilities(account.kind),
                signed_in: s.secrets.has(&account.id),
                account,
            })
            .collect())
    })
    .await
}

/// The OAuth application built in for a service, if any.
#[tauri::command]
pub fn hosting_client_id(kind: ProviderKind, url: String) -> Option<String> {
    let url = ronin_hosting::normalize_url(kind, &url).ok()?;
    oauth::default_client_id(kind, &url).map(str::to_owned)
}

/// Signs in with a personal access token (and, for Atlassian cloud
/// services, the account's email as `login`).
#[tauri::command]
pub async fn hosting_sign_in(
    app: AppHandle,
    kind: ProviderKind,
    url: String,
    login: String,
    token: String,
) -> CmdResult<SignIn> {
    blocking(&app, move |_, s| {
        let account = Account {
            kind,
            url: ronin_hosting::normalize_url(kind, &url).map_err(err)?,
            login: login.trim().to_owned(),
            ..Account::default()
        };
        s.sign_in(account, &Credential::token(token.trim()))
    })
    .await
}

/// Starts signing in through the browser; show the code, then wait with
/// `hosting_device_wait`.
#[tauri::command]
pub async fn hosting_device_start(
    app: AppHandle,
    kind: ProviderKind,
    url: String,
    client_id: String,
) -> CmdResult<DeviceStart> {
    blocking(&app, move |_, s| {
        let url = ronin_hosting::normalize_url(kind, &url).map_err(err)?;
        let client_id = client_id.trim().to_owned();
        if client_id.is_empty() {
            return Err("enter the client id of an OAuth application".into());
        }
        let code = oauth::start(s.transport.clone(), kind, &url, &client_id).map_err(err)?;
        let flow = s.device_flows.next.fetch_add(1, Ordering::Relaxed);
        lock(&s.device_flows.flows).insert(
            flow,
            Arc::new(DeviceFlow {
                kind,
                url,
                client_id,
                code: code.clone(),
                cancelled: AtomicBool::new(false),
            }),
        );
        Ok(DeviceStart { flow, code })
    })
    .await
}

/// Waits until the user authorized the app in the browser.
#[tauri::command]
pub async fn hosting_device_wait(app: AppHandle, flow: u32) -> CmdResult<SignIn> {
    blocking(&app, move |_, s| {
        let device = lock(&s.device_flows.flows)
            .get(&flow)
            .cloned()
            .ok_or("the sign-in was cancelled")?;
        let result = wait_for_device(s, &device);
        lock(&s.device_flows.flows).remove(&flow);
        let credential = result?;
        let account = Account {
            kind: device.kind,
            url: device.url.clone(),
            oauth_client_id: device.client_id.clone(),
            ..Account::default()
        };
        s.sign_in(account, &credential)
    })
    .await
}

fn wait_for_device(s: &AppState, device: &DeviceFlow) -> CmdResult<Credential> {
    let deadline = Instant::now() + Duration::from_secs(device.code.expires_in);
    let mut interval = device.code.interval;
    loop {
        // Sleep in small steps so cancelling is quick.
        let wake = Instant::now() + Duration::from_secs(interval);
        while Instant::now() < wake {
            if device.cancelled.load(Ordering::Relaxed) {
                return Err("the sign-in was cancelled".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if Instant::now() > deadline {
            return Err("the code expired; start again".into());
        }
        match oauth::poll(
            s.transport.clone(),
            device.kind,
            &device.url,
            &device.client_id,
            &device.code,
            now(),
        ) {
            Ok(Poll::Done(credential)) => return Ok(credential),
            Ok(Poll::Pending { interval: next }) => interval = next,
            // A dropped connection shouldn't end the sign-in.
            Err(HostingError::Network(_)) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[tauri::command]
pub fn hosting_device_cancel(state: tauri::State<'_, AppState>, flow: u32) {
    if let Some(device) = lock(&state.device_flows.flows).remove(&flow) {
        device.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Forgets an account and deletes its token.
#[tauri::command]
pub async fn hosting_sign_out(app: AppHandle, id: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.secrets.delete(&id);
        s.config()
            .update_local(|l| l.accounts.retain(|a| a.id != id))
            .map_err(err)
    })
    .await
}

/// Moves an account to another profile.
#[tauri::command]
pub async fn hosting_move_account(app: AppHandle, id: String, profile: String) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.config()
            .update_local(|l| {
                if let Some(a) = l.accounts.iter_mut().find(|a| a.id == id) {
                    a.profile = profile;
                }
            })
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_repos(app: AppHandle, account: String) -> CmdResult<Vec<HostedRepo>> {
    blocking(&app, move |_, s| {
        s.provider(&account)?.1.repos(1000).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_fork(app: AppHandle, account: String, path: String) -> CmdResult<HostedRepo> {
    blocking(&app, move |_, s| {
        s.provider(&account)?.1.fork(&path).map_err(err)
    })
    .await
}

/// The remotes of an open repository that are on the profile's accounts,
/// `origin` and `upstream` first.
#[tauri::command]
pub async fn hosting_links(app: AppHandle, repo: String) -> CmdResult<Vec<RepoLink>> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        let refs = ronin_git::list_refs(&s.git()?, &session.path).map_err(err)?;
        let accounts = profile_accounts(s.config().get());
        let mut links: Vec<RepoLink> = Vec::new();
        for remote in &refs.remotes {
            let Some(url) = &remote.url else { continue };
            for account in &accounts {
                if let Some(path) = ronin_hosting::remote::repo_path(account, url) {
                    links.push(RepoLink {
                        account: account.id.clone(),
                        kind: account.kind,
                        remote: remote.name.clone(),
                        path,
                        capabilities: ronin_hosting::capabilities(account.kind),
                    });
                    break;
                }
            }
        }
        let rank = |name: &str| match name {
            "origin" => 0,
            "upstream" => 1,
            _ => 2,
        };
        links.sort_by_key(|l| rank(&l.remote));
        Ok(links)
    })
    .await
}

#[tauri::command]
pub async fn hosting_pull_requests(
    app: AppHandle,
    account: String,
    path: String,
) -> CmdResult<Vec<PullRequest>> {
    blocking(&app, move |_, s| {
        s.provider(&account)?.1.pull_requests(&path).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_pull_request(
    app: AppHandle,
    account: String,
    path: String,
    number: u64,
) -> CmdResult<PullRequestDetail> {
    blocking(&app, move |_, s| {
        s.provider(&account)?
            .1
            .pull_request(&path, number)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_create_pull_request(
    app: AppHandle,
    account: String,
    path: String,
    pr: NewPullRequest,
) -> CmdResult<PullRequest> {
    blocking(&app, move |_, s| {
        s.provider(&account)?
            .1
            .create_pull_request(&path, &pr)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_comment(
    app: AppHandle,
    account: String,
    path: String,
    number: u64,
    body: String,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.provider(&account)?
            .1
            .comment(&path, number, &body)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_approve(
    app: AppHandle,
    account: String,
    path: String,
    number: u64,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let (account, provider) = s.provider(&account)?;
        provider.approve(&path, number, &me(&account)).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_merge(
    app: AppHandle,
    account: String,
    path: String,
    number: u64,
    method: MergeMethod,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        s.provider(&account)?
            .1
            .merge(&path, number, method)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_issues(
    app: AppHandle,
    account: String,
    path: String,
) -> CmdResult<Vec<Issue>> {
    blocking(&app, move |_, s| {
        s.provider(&account)?.1.issues(&path).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_create_issue(
    app: AppHandle,
    account: String,
    path: String,
    title: String,
    body: String,
) -> CmdResult<Issue> {
    blocking(&app, move |_, s| {
        s.provider(&account)?
            .1
            .create_issue(&path, &title, &body)
            .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn hosting_ci_status(
    app: AppHandle,
    account: String,
    path: String,
    sha: String,
) -> CmdResult<CiStatus> {
    blocking(&app, move |_, s| {
        match s.provider(&account)?.1.ci_status(&path, &sha) {
            // A commit that was never pushed has no checks.
            Err(HostingError::NotFound(_)) | Err(HostingError::Api { status: 422, .. }) => {
                Ok(CiStatus::default())
            }
            result => result.map_err(err),
        }
    })
    .await
}

#[tauri::command]
pub async fn hosting_add_ssh_key(
    app: AppHandle,
    account: String,
    title: String,
    key: String,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let (account, provider) = s.provider(&account)?;
        provider
            .add_ssh_key(&me(&account), &title, &key)
            .map_err(err)
    })
    .await
}

/// Pull requests and issues of the profile's accounts that involve the
/// user. Accounts are asked in parallel; one failing doesn't hide the rest.
#[tauri::command]
pub async fn hosting_launchpad(app: AppHandle) -> CmdResult<Launchpad> {
    blocking(&app, move |_, s| {
        let accounts = profile_accounts(s.config().get());
        let results: Vec<(String, Result<LaunchpadItems, String>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = accounts
                .iter()
                .map(|account| scope.spawn(move || (account.id.clone(), launchpad_of(s, account))))
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        let mut launchpad = Launchpad::default();
        for (account, result) in results {
            match result {
                Ok((prs, issues)) => {
                    launchpad.pull_requests.extend(prs);
                    launchpad.issues.extend(issues);
                }
                Err(message) => launchpad.errors.push(AccountError { account, message }),
            }
        }
        launchpad
            .pull_requests
            .sort_by_key(|p| std::cmp::Reverse(p.pr.updated));
        launchpad
            .issues
            .sort_by_key(|i| std::cmp::Reverse(i.issue.updated));
        Ok(launchpad)
    })
    .await
}

type LaunchpadItems = (Vec<LaunchpadPr>, Vec<LaunchpadIssue>);

/// What a service can't do counts as nothing to show.
fn supported<T>(result: ronin_hosting::Result<Vec<T>>) -> Result<Vec<T>, String> {
    match result {
        Err(HostingError::Unsupported(_)) => Ok(Vec::new()),
        other => other.map_err(err),
    }
}

fn launchpad_of(s: &AppState, account: &Account) -> Result<LaunchpadItems, String> {
    let (account, provider) = s.provider(&account.id)?;
    let me = me(&account);
    let prs = supported(provider.my_pull_requests(&me))?
        .into_iter()
        .map(|m| LaunchpadPr {
            account: account.id.clone(),
            pr: m.pr,
            involvement: m.involvement,
        })
        .collect();
    let issues = supported(provider.my_issues(&me))?
        .into_iter()
        .map(|issue| LaunchpadIssue {
            account: account.id.clone(),
            issue,
        })
        .collect();
    Ok((prs, issues))
}

/// Fetches a pull request's commits into an open repository (from its
/// remote, or the fork's URL), plus the target branch if its tip is missing.
#[tauri::command]
pub async fn pr_fetch(
    app: AppHandle,
    repo: String,
    remote: String,
    source: PrSource,
    head: Option<String>,
    base: Option<String>,
    target_branch: String,
) -> CmdResult<()> {
    blocking(&app, move |app, s| {
        let session = s.session(&repo)?;
        let git = s.git()?;
        let mut report = reporter(app, &repo);
        let from = source.url.as_deref().unwrap_or(&remote);
        let heads: Vec<&str> = head.iter().map(String::as_str).collect();
        ronin_git::fetch_commits(
            &git,
            &session.path,
            from,
            &[&source.refspec],
            &heads,
            &mut report,
        )
        .map_err(err)?;
        let target = format!("refs/heads/{target_branch}");
        let bases: Vec<&str> = base.iter().map(String::as_str).collect();
        ronin_git::fetch_commits(
            &git,
            &session.path,
            &remote,
            &[&target],
            &bases,
            &mut report,
        )
        .map_err(err)
    })
    .await
}

/// What `head` changes since it left `base`.
#[tauri::command]
pub async fn range_files(
    app: AppHandle,
    repo: String,
    base: String,
    head: String,
) -> CmdResult<RangeDiff> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::range_files(&s.git()?, &session.path, &base, &head).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn branch_issue(
    app: AppHandle,
    repo: String,
    branch: String,
) -> CmdResult<Option<String>> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::branch_issue(&s.git()?, &session.path, &branch).map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn set_branch_issue(
    app: AppHandle,
    repo: String,
    branch: String,
    issue: Option<String>,
) -> CmdResult<()> {
    blocking(&app, move |_, s| {
        let session = s.session(&repo)?;
        ronin_git::set_branch_issue(
            &s.git()?,
            Path::new(&session.path),
            &branch,
            issue.as_deref(),
        )
        .map_err(err)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_user_names_suit_each_service() {
        let account = |kind, client: &str| Account {
            kind,
            username: "jo".into(),
            oauth_client_id: client.into(),
            ..Account::default()
        };
        assert_eq!(git_username(&account(ProviderKind::Github, "")), "jo");
        assert_eq!(
            git_username(&account(ProviderKind::Gitlab, "cid")),
            "oauth2"
        );
        assert_eq!(git_username(&account(ProviderKind::Gitlab, "")), "jo");
        assert_eq!(
            git_username(&account(ProviderKind::Bitbucket, "")),
            "x-bitbucket-api-token-auth"
        );
    }
}
