//! Which hosted repository a git remote points at, so a local repository
//! can show its pull requests, issues and checks.

use ronin_config::accounts::host_of;
use ronin_config::{Account, ProviderKind};

/// A remote URL split into host and path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Lowercase, without port or user.
    pub host: String,
    /// Path segments, percent-decoded, without a trailing `.git`.
    pub segments: Vec<String>,
    /// Whether it's an ssh address (whose path has no web prefix).
    pub ssh: bool,
}

/// Parses `https://host/owner/repo.git`, `ssh://git@host:22/owner/repo`,
/// `git@host:owner/repo.git` and the like. Local paths are `None`.
pub fn parse(url: &str) -> Option<Location> {
    let url = url.trim();
    let (host, path, ssh) = if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if scheme == "file" {
            return None;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        (host_of(authority), path, scheme.contains("ssh"))
    } else {
        // scp-like syntax: [user@]host:path, with no slash before the colon.
        let (authority, path) = url.split_once(':')?;
        if authority.contains('/') || authority.contains('\\') || authority.len() < 2 {
            return None;
        }
        (host_of(authority), path, true)
    };
    if host.is_empty() {
        return None;
    }
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            percent_encoding::percent_decode_str(s)
                .decode_utf8_lossy()
                .into_owned()
        })
        .collect();
    Some(Location {
        host,
        segments,
        ssh,
    })
}

/// The repository `url` names on `account`'s service, as
/// [`crate::HostedRepo::path`].
pub fn repo_path(account: &Account, url: &str) -> Option<String> {
    let location = parse(url)?;
    let host = account.host();
    let segments = &location.segments;
    let same_host = || {
        location.host == host
            // github.com's ssh-over-https host.
            || (host == "github.com" && location.host == "ssh.github.com")
            || (host == "bitbucket.org" && location.host == "altssh.bitbucket.org")
    };
    match account.kind {
        ProviderKind::Github | ProviderKind::Bitbucket => {
            (same_host() && segments.len() == 2).then(|| segments.join("/"))
        }
        ProviderKind::Gitlab => {
            if !same_host() {
                return None;
            }
            // A GitLab under a sub-path serves https remotes below it.
            let prefix = parse(&account.url).map(|l| l.segments).unwrap_or_default();
            let segments = if !location.ssh && segments.starts_with(&prefix) {
                &segments[prefix.len()..]
            } else {
                &segments[..]
            };
            (segments.len() >= 2).then(|| segments.join("/"))
        }
        ProviderKind::BitbucketServer => {
            if !same_host() {
                return None;
            }
            let rest = if location.ssh {
                segments.as_slice()
            } else {
                let scm = segments.iter().position(|s| s == "scm")?;
                &segments[scm + 1..]
            };
            match rest {
                [project, repo] => Some(format!("{}/{repo}", project.to_ascii_uppercase())),
                _ => None,
            }
        }
        ProviderKind::AzureDevops => azure_path(account, &location),
        ProviderKind::Jira => None,
    }
}

/// The organization of an Azure DevOps account URL.
pub fn azure_organization(url: &str) -> Option<String> {
    let location = parse(url)?;
    if location.host == "dev.azure.com" {
        location.segments.first().cloned()
    } else {
        location
            .host
            .strip_suffix(".visualstudio.com")
            .map(str::to_owned)
    }
}

fn azure_path(account: &Account, location: &Location) -> Option<String> {
    let org = azure_organization(&account.url)?.to_ascii_lowercase();
    let segments = &location.segments;
    let (remote_org, project, repo) = match location.host.as_str() {
        "ssh.dev.azure.com" | "vs-ssh.visualstudio.com" => match segments.as_slice() {
            [v3, org, project, repo] if v3 == "v3" => (org.clone(), project, repo),
            _ => return None,
        },
        "dev.azure.com" => match segments.as_slice() {
            [org, project, git, repo] if git == "_git" => (org.clone(), project, repo),
            _ => return None,
        },
        host => {
            let org = host.strip_suffix(".visualstudio.com")?.to_owned();
            let git = segments.iter().position(|s| s == "_git")?;
            match (segments.get(git.checked_sub(1)?), segments.get(git + 1)) {
                (Some(project), Some(repo)) if segments.len() == git + 2 => (org, project, repo),
                _ => return None,
            }
        }
    };
    (remote_org.to_ascii_lowercase() == org).then(|| format!("{project}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(kind: ProviderKind, url: &str) -> Account {
        Account {
            kind,
            url: url.into(),
            ..Account::default()
        }
    }

    #[test]
    fn parses_remote_urls() {
        let l = parse("git@github.com:Owner/Repo.git").unwrap();
        assert_eq!((l.host.as_str(), l.ssh), ("github.com", true));
        assert_eq!(l.segments, ["Owner", "Repo"]);
        let l = parse("https://user:pw@GitLab.example.com:8443/a/b/c.git/").unwrap();
        assert_eq!(l.host, "gitlab.example.com");
        assert_eq!(l.segments, ["a", "b", "c"]);
        assert!(!l.ssh);
        assert_eq!(
            parse("ssh://git@host:7999/proj/my%20repo.git")
                .unwrap()
                .segments,
            ["proj", "my repo"]
        );
        assert_eq!(parse("/srv/git/repo.git"), None);
        assert_eq!(parse("C:\\repos\\x"), None);
        assert_eq!(parse("file:///srv/git/repo"), None);
    }

    #[test]
    fn matches_github_and_bitbucket() {
        let gh = account(ProviderKind::Github, "https://github.com");
        let p = |a: &Account, u| repo_path(a, u);
        assert_eq!(p(&gh, "git@github.com:o/r.git").as_deref(), Some("o/r"));
        assert_eq!(
            p(&gh, "ssh://git@ssh.github.com:443/o/r.git").as_deref(),
            Some("o/r")
        );
        assert_eq!(p(&gh, "https://gitlab.com/o/r"), None);
        assert_eq!(p(&gh, "https://github.com/o/r/extra"), None);
        let ghe = account(ProviderKind::Github, "https://git.corp.example");
        assert_eq!(
            p(&ghe, "https://git.corp.example/team/app.git").as_deref(),
            Some("team/app")
        );
        let bb = account(ProviderKind::Bitbucket, "https://bitbucket.org");
        assert_eq!(
            p(&bb, "https://me@bitbucket.org/ws/repo.git").as_deref(),
            Some("ws/repo")
        );
    }

    #[test]
    fn matches_gitlab_groups_and_sub_paths() {
        let gl = account(ProviderKind::Gitlab, "http://localhost:8929");
        assert_eq!(
            repo_path(&gl, "ssh://git@localhost:2224/g/sub/r.git").as_deref(),
            Some("g/sub/r")
        );
        let sub = account(ProviderKind::Gitlab, "https://example.com/gitlab");
        assert_eq!(
            repo_path(&sub, "https://example.com/gitlab/g/r.git").as_deref(),
            Some("g/r")
        );
        assert_eq!(
            repo_path(&sub, "git@example.com:g/r.git").as_deref(),
            Some("g/r")
        );
        assert_eq!(repo_path(&gl, "http://localhost:8929/solo"), None);
    }

    #[test]
    fn matches_bitbucket_server() {
        let bbs = account(ProviderKind::BitbucketServer, "https://bb.corp/bitbucket");
        assert_eq!(
            repo_path(&bbs, "https://bb.corp/bitbucket/scm/proj/app.git").as_deref(),
            Some("PROJ/app")
        );
        assert_eq!(
            repo_path(&bbs, "ssh://git@bb.corp:7999/proj/app.git").as_deref(),
            Some("PROJ/app")
        );
        assert_eq!(repo_path(&bbs, "https://bb.corp/projects/PROJ"), None);
    }

    #[test]
    fn matches_azure_devops() {
        let az = account(ProviderKind::AzureDevops, "https://dev.azure.com/Contoso");
        let p = |u| repo_path(&az, u);
        assert_eq!(
            p("https://contoso@dev.azure.com/contoso/My%20Project/_git/web").as_deref(),
            Some("My Project/web")
        );
        assert_eq!(
            p("git@ssh.dev.azure.com:v3/contoso/Proj/web").as_deref(),
            Some("Proj/web")
        );
        assert_eq!(
            p("https://contoso.visualstudio.com/DefaultCollection/Proj/_git/web").as_deref(),
            Some("Proj/web")
        );
        assert_eq!(p("https://dev.azure.com/other/Proj/_git/web"), None);
        assert_eq!(
            azure_organization("https://fabrikam.visualstudio.com").as_deref(),
            Some("fabrikam")
        );
    }
}
