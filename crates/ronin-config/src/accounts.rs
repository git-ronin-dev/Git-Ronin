//! Accounts on hosting services. Only what identifies an account is kept
//! here; its token lives in the OS keyring, keyed by [`Account::id`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProviderKind {
    /// github.com or GitHub Enterprise Server.
    Github,
    /// gitlab.com or a self-managed GitLab.
    Gitlab,
    /// Bitbucket Cloud (bitbucket.org).
    Bitbucket,
    /// Bitbucket Server or Data Center.
    BitbucketServer,
    AzureDevops,
    /// Jira Cloud, Server or Data Center; issues only.
    Jira,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Account {
    /// Stable identifier; also names the account's keyring entry.
    pub id: String,
    pub kind: ProviderKind,
    /// Web address of the service, e.g. `https://github.com`, or
    /// `https://dev.azure.com/<organization>` for Azure DevOps.
    pub url: String,
    /// The account's user name on the service.
    pub username: String,
    /// Display name.
    pub name: String,
    pub avatar_url: String,
    /// The service's own id for the user (Bitbucket and Azure DevOps need it).
    pub user_id: String,
    /// For services that take a user name with the token (Bitbucket Cloud
    /// and Jira Cloud use the Atlassian email); empty for bearer tokens.
    pub login: String,
    /// The profile the account belongs to.
    pub profile: String,
    /// The OAuth application the token came from; empty for personal
    /// access tokens. Needed to refresh expiring tokens.
    pub oauth_client_id: String,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: ProviderKind::Github,
            url: String::new(),
            username: String::new(),
            name: String::new(),
            avatar_url: String::new(),
            user_id: String::new(),
            login: String::new(),
            profile: String::new(),
            oauth_client_id: String::new(),
        }
    }
}

impl Account {
    /// The host part of [`Account::url`], lowercased.
    pub fn host(&self) -> String {
        host_of(&self.url)
    }
}

/// The host of a URL such as `https://Git.Example.com:8443/x`, lowercased
/// and without the port.
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = if host.starts_with('[') {
        host.split(']')
            .next()
            .map(|h| format!("{h}]"))
            .unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or_default().to_owned()
    };
    host.to_ascii_lowercase()
}

/// An id for a new account that isn't taken: `<kind>-<user>@<host>`.
pub fn account_id(kind: ProviderKind, host: &str, username: &str, taken: &[Account]) -> String {
    let kind = serde_json_kind(kind);
    let base: String = format!("{kind}-{username}@{host}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let free = |id: &str| !taken.iter().any(|a| a.id == id);
    if free(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}-{i}"))
        .find(|id| free(id))
        .expect("an unused id exists")
}

fn serde_json_kind(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Github => "github",
        ProviderKind::Gitlab => "gitlab",
        ProviderKind::Bitbucket => "bitbucket",
        ProviderKind::BitbucketServer => "bitbucketServer",
        ProviderKind::AzureDevops => "azureDevops",
        ProviderKind::Jira => "jira",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_ignore_scheme_user_port_and_case() {
        assert_eq!(host_of("https://GitHub.com"), "github.com");
        assert_eq!(
            host_of("http://git.example.com:8929/sub"),
            "git.example.com"
        );
        assert_eq!(host_of("https://me@dev.azure.com/org"), "dev.azure.com");
        assert_eq!(host_of("https://[::1]:8080/"), "[::1]");
        assert_eq!(host_of("gitlab.example.com"), "gitlab.example.com");
    }

    #[test]
    fn ids_are_unique_and_file_safe() {
        let first = account_id(ProviderKind::Gitlab, "localhost", "jo doe", &[]);
        assert_eq!(first, "gitlab-jo_doe@localhost");
        let taken = [Account {
            id: first.clone(),
            ..Account::default()
        }];
        assert_eq!(
            account_id(ProviderKind::Gitlab, "localhost", "jo doe", &taken),
            "gitlab-jo_doe@localhost-2"
        );
    }
}
