//! Hosting service integrations for Git Ronin.
//!
//! Every service implements [`Provider`]: the signed-in user, repositories
//! (list, fork), pull requests (list, view, create, comment, approve,
//! merge), issues (list, create) and CI status of commits. What a service
//! can't do returns [`Error::Unsupported`], and [`Capabilities`] tells the
//! UI in advance.
//!
//! Requests go through a [`Transport`], so tests replay recorded responses
//! instead of talking to the network. The crate never stores anything:
//! tokens come in as a [`Credential`] and the caller keeps them (in the OS
//! keyring).

mod azure;
mod bitbucket;
mod bitbucket_server;
mod github;
mod gitlab;
pub mod http;
mod jira;
mod json;
pub mod oauth;
pub mod remote;

#[cfg(test)]
mod test_support;

use std::sync::Arc;

pub use ronin_config::{Account, ProviderKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use crate::http::{Transport, UreqTransport};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("could not reach the service: {0}")]
    Network(String),
    #[error("the service refused the token ({0}); sign in to the account again")]
    Unauthorized(String),
    #[error("not allowed: {0}")]
    Forbidden(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{message} (HTTP {status})")]
    Api { status: u16, message: String },
    #[error("unexpected answer from the service: {0}")]
    Decode(String),
    #[error("{0} is not supported by this service")]
    Unsupported(&'static str),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A token and, for OAuth tokens that expire, how to renew it. Kept as
/// JSON in the OS keyring.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Credential {
    pub token: String,
    pub refresh_token: String,
    /// Seconds since the Unix epoch; 0 if the token doesn't expire.
    pub expires_at: u64,
}

impl Credential {
    pub fn token(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            ..Self::default()
        }
    }

    /// Whether the token expires within a minute of `now`.
    pub fn expiring(&self, now: u64) -> bool {
        self.expires_at != 0 && self.expires_at <= now + 60
    }
}

/// Someone on the service.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Person {
    pub username: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// The signed-in user.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct User {
    /// The service's own id.
    pub id: String,
    pub username: String,
    pub name: String,
    pub avatar_url: Option<String>,
    /// Some services (Bitbucket Server) key users by a slug.
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostedRepo {
    /// How the service names it: `owner/name` (GitHub, Bitbucket),
    /// `group/subgroup/name` (GitLab), `PROJECT/slug` (Bitbucket Server) or
    /// `project/name` (Azure DevOps).
    pub path: String,
    pub name: String,
    pub description: Option<String>,
    pub private: bool,
    pub fork: bool,
    pub clone_https: String,
    pub clone_ssh: Option<String>,
    pub web_url: String,
    pub default_branch: Option<String>,
    /// Seconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub updated: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullRequest {
    /// The target repository, as [`HostedRepo::path`].
    pub repo: String,
    #[ts(type = "number")]
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub draft: bool,
    pub author: Person,
    pub source_branch: String,
    pub target_branch: String,
    /// The repository the changes come from, when it's a fork.
    pub source_repo: Option<String>,
    pub web_url: String,
    #[ts(type = "number")]
    pub updated: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    /// Asked to review, no answer yet.
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Review {
    pub author: Person,
    pub state: ReviewState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Comment {
    pub author: Person,
    pub body: String,
    #[ts(type = "number")]
    pub created: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// Where git can fetch a pull request's commits from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrSource {
    /// A ref on the target repository (`refs/pull/1/head`), else the source
    /// branch.
    pub refspec: String,
    /// Fetch from this URL instead of the target repository's remote (the
    /// source branch of a fork).
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullRequestDetail {
    pub pr: PullRequest,
    pub body: String,
    pub head_sha: Option<String>,
    pub base_sha: Option<String>,
    pub source: PrSource,
    /// `None` while the service is still working it out.
    pub mergeable: Option<bool>,
    pub reviews: Vec<Review>,
    pub comments: Vec<Comment>,
    pub merge_methods: Vec<MergeMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewPullRequest {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
    /// The fork the source branch is on, if it isn't the target repository.
    pub source_repo: Option<String>,
    pub draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Issue {
    /// The repository (or Jira/Azure project) it belongs to.
    pub repo: String,
    /// What people call it: `#12`, `PROJ-12`.
    pub key: String,
    #[ts(type = "number")]
    pub number: u64,
    pub title: String,
    pub open: bool,
    pub author: Option<Person>,
    pub labels: Vec<String>,
    pub web_url: String,
    #[ts(type = "number")]
    pub updated: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CheckState {
    // Ordered by precedence when combining: any failure fails the lot.
    Success,
    Neutral,
    Pending,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Check {
    pub name: String,
    pub state: CheckState,
    pub description: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CiStatus {
    /// All checks combined; `None` when there are none.
    pub state: Option<CheckState>,
    pub checks: Vec<Check>,
}

impl CiStatus {
    pub fn from_checks(checks: Vec<Check>) -> Self {
        let state = checks
            .iter()
            .map(|c| c.state)
            .filter(|s| *s != CheckState::Neutral)
            .max()
            .or_else(|| checks.first().map(|_| CheckState::Neutral));
        Self { state, checks }
    }
}

/// How the signed-in user is involved in a pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Involvement {
    Author,
    Reviewer,
    Assignee,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MyPullRequest {
    pub pr: PullRequest,
    pub involvement: Vec<Involvement>,
}

/// What a service supports, so the UI only offers what works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    pub repos: bool,
    pub fork: bool,
    pub pull_requests: bool,
    pub approve: bool,
    pub draft: bool,
    pub issues: bool,
    pub ci: bool,
    pub ssh_keys: bool,
    /// Sign in through the browser (OAuth device flow) as well as with a token.
    pub device_flow: bool,
}

/// A hosting service, signed in as one account.
pub trait Provider: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    /// Who the token belongs to; also checks it works.
    fn user(&self) -> Result<User>;

    /// Repositories the user can access, most recently updated first.
    fn repos(&self, _limit: usize) -> Result<Vec<HostedRepo>> {
        Err(Error::Unsupported("listing repositories"))
    }

    /// Forks `repo` to the user's namespace.
    fn fork(&self, _repo: &str) -> Result<HostedRepo> {
        Err(Error::Unsupported("forking"))
    }

    /// Open pull requests of `repo`.
    fn pull_requests(&self, _repo: &str) -> Result<Vec<PullRequest>> {
        Err(Error::Unsupported("pull requests"))
    }

    /// Open pull requests the user wrote, is asked to review or is assigned to.
    fn my_pull_requests(&self, _me: &User) -> Result<Vec<MyPullRequest>> {
        Err(Error::Unsupported("pull requests"))
    }

    fn pull_request(&self, _repo: &str, _number: u64) -> Result<PullRequestDetail> {
        Err(Error::Unsupported("pull requests"))
    }

    fn create_pull_request(&self, _repo: &str, _pr: &NewPullRequest) -> Result<PullRequest> {
        Err(Error::Unsupported("pull requests"))
    }

    fn comment(&self, _repo: &str, _number: u64, _body: &str) -> Result<()> {
        Err(Error::Unsupported("comments"))
    }

    fn approve(&self, _repo: &str, _number: u64, _me: &User) -> Result<()> {
        Err(Error::Unsupported("approving"))
    }

    fn merge(&self, _repo: &str, _number: u64, _method: MergeMethod) -> Result<()> {
        Err(Error::Unsupported("merging"))
    }

    /// Open issues of `repo`.
    fn issues(&self, _repo: &str) -> Result<Vec<Issue>> {
        Err(Error::Unsupported("issues"))
    }

    /// Open issues assigned to the user.
    fn my_issues(&self, _me: &User) -> Result<Vec<Issue>> {
        Err(Error::Unsupported("issues"))
    }

    fn create_issue(&self, _repo: &str, _title: &str, _body: &str) -> Result<Issue> {
        Err(Error::Unsupported("issues"))
    }

    /// Checks reported for commit `sha` of `repo`.
    fn ci_status(&self, _repo: &str, _sha: &str) -> Result<CiStatus> {
        Err(Error::Unsupported("CI status"))
    }

    /// Adds a public key (an OpenSSH line) to the user's account.
    fn add_ssh_key(&self, _me: &User, _title: &str, _key: &str) -> Result<()> {
        Err(Error::Unsupported("adding SSH keys"))
    }
}

/// Connects to the service `account` is on, with `credential`.
pub fn connect(
    account: &Account,
    credential: &Credential,
    transport: Arc<dyn Transport>,
) -> Result<Box<dyn Provider>> {
    let api = http::Api::new(transport, account, credential)?;
    Ok(match account.kind {
        ProviderKind::Github => Box::new(github::GitHub::new(api)),
        ProviderKind::Gitlab => Box::new(gitlab::GitLab::new(api)),
        ProviderKind::Bitbucket => Box::new(bitbucket::Bitbucket::new(api)),
        ProviderKind::BitbucketServer => {
            Box::new(bitbucket_server::BitbucketServer::new(api, &account.url))
        }
        ProviderKind::AzureDevops => Box::new(azure::Azure::new(api, account)?),
        ProviderKind::Jira => Box::new(jira::Jira::new(api, &account.url)),
    })
}

/// What an account's service supports, without connecting.
pub fn capabilities(kind: ProviderKind) -> Capabilities {
    match kind {
        ProviderKind::Github => github::CAPABILITIES,
        ProviderKind::Gitlab => gitlab::CAPABILITIES,
        ProviderKind::Bitbucket => bitbucket::CAPABILITIES,
        ProviderKind::BitbucketServer => bitbucket_server::CAPABILITIES,
        ProviderKind::AzureDevops => azure::CAPABILITIES,
        ProviderKind::Jira => jira::CAPABILITIES,
    }
}

/// Normalises the address a user typed for a service: adds `https://`,
/// drops a trailing slash, and fills in the public service's address.
pub fn normalize_url(kind: ProviderKind, url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    let url = if url.is_empty() {
        match kind {
            ProviderKind::Github => "https://github.com",
            ProviderKind::Gitlab => "https://gitlab.com",
            ProviderKind::Bitbucket => "https://bitbucket.org",
            _ => return Err(Error::Invalid("enter the address of the server".into())),
        }
        .to_owned()
    } else if url.contains("://") {
        url.to_owned()
    } else {
        format!("https://{url}")
    };
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::Invalid(format!("“{url}” is not a web address")));
    }
    if kind == ProviderKind::AzureDevops {
        let host = ronin_config::accounts::host_of(&url);
        let has_org = url.split('/').nth(3).is_some_and(|s| !s.is_empty());
        if host == "dev.azure.com" && !has_org {
            return Err(Error::Invalid(
                "include the organization: https://dev.azure.com/<organization>".into(),
            ));
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_check_states() {
        let check = |state| Check {
            name: String::new(),
            state,
            description: None,
            url: None,
        };
        let s = |states: &[CheckState]| {
            CiStatus::from_checks(states.iter().map(|s| check(*s)).collect()).state
        };
        assert_eq!(s(&[]), None);
        assert_eq!(s(&[CheckState::Neutral]), Some(CheckState::Neutral));
        assert_eq!(
            s(&[CheckState::Success, CheckState::Neutral]),
            Some(CheckState::Success)
        );
        assert_eq!(
            s(&[CheckState::Success, CheckState::Pending]),
            Some(CheckState::Pending)
        );
        assert_eq!(
            s(&[CheckState::Failure, CheckState::Pending]),
            Some(CheckState::Failure)
        );
    }

    #[test]
    fn normalizes_service_addresses() {
        let n = |k, u| normalize_url(k, u);
        assert_eq!(n(ProviderKind::Github, "").unwrap(), "https://github.com");
        assert_eq!(
            n(ProviderKind::Gitlab, "gitlab.example.com/").unwrap(),
            "https://gitlab.example.com"
        );
        assert_eq!(
            n(ProviderKind::Gitlab, "http://localhost:8929").unwrap(),
            "http://localhost:8929"
        );
        assert!(n(ProviderKind::Jira, "").is_err());
        assert!(n(ProviderKind::AzureDevops, "dev.azure.com").is_err());
        assert!(n(ProviderKind::AzureDevops, "https://dev.azure.com/contoso").is_ok());
        assert!(n(ProviderKind::Github, "ftp://x").is_err());
    }
}
