//! Bitbucket Cloud (REST API 2.0). Sign in with an Atlassian API token and
//! the account's email, or a repository/workspace access token alone.

use serde_json::{Value, json};

use crate::http::{Api, encode};
use crate::json::Json;
use crate::{
    Capabilities, Check, CheckState, CiStatus, Comment, Error, HostedRepo, Involvement, Issue,
    MergeMethod, MyPullRequest, NewPullRequest, Person, PrSource, PrState, Provider, PullRequest,
    PullRequestDetail, Result, Review, ReviewState, User,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    repos: true,
    fork: true,
    pull_requests: true,
    approve: true,
    draft: true,
    issues: true,
    ci: true,
    ssh_keys: true,
    device_flow: false,
};

pub struct Bitbucket {
    api: Api,
}

impl Bitbucket {
    pub fn new(api: Api) -> Self {
        Self { api }
    }

    /// Follows `next` links until `limit` values were read.
    fn paged(&self, path: &str, limit: usize) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut next = Some(path.to_owned());
        while let Some(url) = next.take() {
            let page = self.api.get(&url)?;
            items.extend(page.list("/values").iter().cloned());
            if items.len() >= limit {
                items.truncate(limit);
                break;
            }
            next = page.opt("/next");
        }
        Ok(items)
    }

    /// The slugs of the workspaces the user can see. Bitbucket dropped its
    /// cross-workspace listings, so repositories and pull requests are read
    /// one workspace at a time.
    fn workspaces(&self) -> Result<Vec<String>> {
        let items = self.paged("/user/workspaces?pagelen=100", 1000)?;
        Ok(items
            .iter()
            .map(|w| w.s("/workspace/slug"))
            .filter(|slug| !slug.is_empty())
            .collect())
    }
}

fn repo_url(path: &str) -> Result<String> {
    match path.split_once('/') {
        Some((ws, slug)) if !ws.is_empty() && !slug.is_empty() => {
            Ok(format!("/repositories/{}/{}", encode(ws), encode(slug)))
        }
        _ => Err(Error::Invalid(format!(
            "“{path}” is not workspace/repository"
        ))),
    }
}

fn person(v: &Value) -> Person {
    let username = v
        .opt("/nickname")
        .or_else(|| v.opt("/username"))
        .unwrap_or_else(|| v.s("/display_name"));
    Person {
        name: v.opt("/display_name").unwrap_or_else(|| username.clone()),
        username,
        avatar_url: v.opt("/links/avatar/href"),
    }
}

fn clone_link(v: &Value, name: &str) -> Option<String> {
    v.list("/links/clone")
        .iter()
        .find(|l| l.s("/name") == name)
        .and_then(|l| l.opt("/href"))
}

fn repo(v: &Value) -> HostedRepo {
    HostedRepo {
        path: v.s("/full_name"),
        name: v.s("/name"),
        description: v.opt("/description"),
        private: v.b("/is_private"),
        fork: v.get("parent").is_some_and(|p| !p.is_null()),
        clone_https: clone_link(v, "https")
            .map(|u| strip_user(&u))
            .unwrap_or_else(|| format!("https://bitbucket.org/{}.git", v.s("/full_name"))),
        clone_ssh: clone_link(v, "ssh"),
        web_url: v.s("/links/html/href"),
        default_branch: v.opt("/mainbranch/name"),
        updated: Some(v.time("/updated_on")),
    }
}

/// Bitbucket puts the requesting user into clone links; git asks for the
/// right one anyway.
fn strip_user(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = authority.rsplit('@').next().unwrap_or(authority);
    format!("{scheme}://{host}/{path}")
}

fn pr(v: &Value) -> PullRequest {
    let target = v.s("/destination/repository/full_name");
    let source = v.s("/source/repository/full_name");
    PullRequest {
        number: v.n("/id"),
        title: v.s("/title"),
        state: match v.s("/state").as_str() {
            "OPEN" => PrState::Open,
            "MERGED" => PrState::Merged,
            _ => PrState::Closed,
        },
        draft: v.b("/draft"),
        author: person(&v["author"]),
        source_branch: v.s("/source/branch/name"),
        target_branch: v.s("/destination/branch/name"),
        source_repo: (!source.is_empty() && source != target).then_some(source),
        repo: target,
        web_url: v.s("/links/html/href"),
        updated: v.time("/updated_on"),
    }
}

fn issue(v: &Value, repo: &str) -> Issue {
    let number = v.n("/id");
    Issue {
        repo: repo.to_owned(),
        key: format!("#{number}"),
        number,
        title: v.s("/title"),
        open: matches!(v.s("/state").as_str(), "new" | "open"),
        author: v.get("reporter").filter(|r| !r.is_null()).map(person),
        labels: [v.s("/kind"), v.s("/component/name")]
            .into_iter()
            .filter(|l| !l.is_empty())
            .collect(),
        web_url: v.s("/links/html/href"),
        updated: v.time("/updated_on"),
    }
}

impl Provider for Bitbucket {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        let v = self.api.get("/user")?;
        let username = v
            .opt("/username")
            .or_else(|| v.opt("/nickname"))
            .unwrap_or_else(|| v.s("/account_id"));
        Ok(User {
            id: v.s("/uuid"),
            name: v.opt("/display_name").unwrap_or_else(|| username.clone()),
            slug: username.clone(),
            username,
            avatar_url: v.opt("/links/avatar/href"),
        })
    }

    fn repos(&self, limit: usize) -> Result<Vec<HostedRepo>> {
        let mut repos = Vec::new();
        for ws in self.workspaces()? {
            let items = self.paged(
                &format!(
                    "/repositories/{}?role=member&sort=-updated_on&pagelen=100",
                    encode(&ws)
                ),
                limit,
            )?;
            repos.extend(items.iter().map(repo));
        }
        repos.sort_by_key(|r| std::cmp::Reverse(r.updated));
        repos.truncate(limit);
        Ok(repos)
    }

    fn fork(&self, path: &str) -> Result<HostedRepo> {
        let v = self
            .api
            .post(&format!("{}/forks", repo_url(path)?), &json!({}))?;
        Ok(repo(&v))
    }

    fn pull_requests(&self, path: &str) -> Result<Vec<PullRequest>> {
        let items = self.paged(
            &format!("{}/pullrequests?state=OPEN&pagelen=50", repo_url(path)?),
            300,
        )?;
        Ok(items.iter().map(pr).collect())
    }

    fn my_pull_requests(&self, me: &User) -> Result<Vec<MyPullRequest>> {
        // Bitbucket can list what a user wrote, not what they review.
        let mut prs = Vec::new();
        for ws in self.workspaces()? {
            let items = self.paged(
                &format!(
                    "/workspaces/{}/pullrequests/{}?state=OPEN&pagelen=50",
                    encode(&ws),
                    encode(&me.id)
                ),
                100,
            )?;
            prs.extend(items.iter().map(|v| MyPullRequest {
                pr: pr(v),
                involvement: vec![Involvement::Author],
            }));
        }
        prs.sort_by_key(|p| std::cmp::Reverse(p.pr.updated));
        prs.truncate(100);
        Ok(prs)
    }

    fn pull_request(&self, path: &str, number: u64) -> Result<PullRequestDetail> {
        let base = format!("{}/pullrequests/{number}", repo_url(path)?);
        let v = self.api.get(&base)?;
        let notes = self.paged(&format!("{base}/comments?pagelen=100"), 1000)?;
        let comments = notes
            .iter()
            .filter(|c| !c.b("/deleted"))
            .map(|c| Comment {
                author: person(&c["user"]),
                body: c.s("/content/raw"),
                created: c.time("/created_on"),
            })
            .collect();
        let reviews = v
            .list("/participants")
            .iter()
            .filter_map(|p| {
                let state = match (p.s("/state").as_str(), p.b("/approved")) {
                    ("changes_requested", _) => ReviewState::ChangesRequested,
                    (_, true) => ReviewState::Approved,
                    _ if p.s("/role") == "REVIEWER" => ReviewState::Pending,
                    _ => return None,
                };
                Some(Review {
                    author: person(&p["user"]),
                    state,
                })
            })
            .collect();
        let pr = pr(&v);
        let source_url = pr
            .source_repo
            .as_ref()
            .map(|fork| format!("https://bitbucket.org/{fork}.git"));
        Ok(PullRequestDetail {
            body: v.s("/description"),
            head_sha: v.opt("/source/commit/hash"),
            base_sha: v.opt("/destination/commit/hash"),
            source: PrSource {
                refspec: format!("refs/heads/{}", pr.source_branch),
                url: source_url,
            },
            mergeable: None,
            reviews,
            comments,
            merge_methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
            pr,
        })
    }

    fn create_pull_request(&self, path: &str, new: &NewPullRequest) -> Result<PullRequest> {
        let mut source = json!({ "branch": { "name": new.source_branch } });
        if let Some(fork) = &new.source_repo {
            source["repository"] = json!({ "full_name": fork });
        }
        let v = self.api.post(
            &format!("{}/pullrequests", repo_url(path)?),
            &json!({
                "title": new.title,
                "description": new.body,
                "source": source,
                "destination": { "branch": { "name": new.target_branch } },
                "draft": new.draft,
            }),
        )?;
        Ok(pr(&v))
    }

    fn comment(&self, path: &str, number: u64, body: &str) -> Result<()> {
        self.api.post(
            &format!("{}/pullrequests/{number}/comments", repo_url(path)?),
            &json!({ "content": { "raw": body } }),
        )?;
        Ok(())
    }

    fn approve(&self, path: &str, number: u64, _me: &User) -> Result<()> {
        self.api.post(
            &format!("{}/pullrequests/{number}/approve", repo_url(path)?),
            &json!({}),
        )?;
        Ok(())
    }

    fn merge(&self, path: &str, number: u64, method: MergeMethod) -> Result<()> {
        let strategy = match method {
            MergeMethod::Merge => "merge_commit",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "fast_forward",
        };
        self.api.post(
            &format!("{}/pullrequests/{number}/merge", repo_url(path)?),
            &json!({ "merge_strategy": strategy }),
        )?;
        Ok(())
    }

    fn issues(&self, path: &str) -> Result<Vec<Issue>> {
        let query = encode("state = \"new\" OR state = \"open\"");
        let items = self.paged(
            &format!(
                "{}/issues?q={query}&sort=-updated_on&pagelen=50",
                repo_url(path)?
            ),
            300,
        )?;
        Ok(items.iter().map(|v| issue(v, path)).collect())
    }

    fn create_issue(&self, path: &str, title: &str, body: &str) -> Result<Issue> {
        let v = self.api.post(
            &format!("{}/issues", repo_url(path)?),
            &json!({ "title": title, "content": { "raw": body } }),
        )?;
        Ok(issue(&v, path))
    }

    fn ci_status(&self, path: &str, sha: &str) -> Result<CiStatus> {
        let items = self.paged(
            &format!(
                "{}/commit/{}/statuses?pagelen=100",
                repo_url(path)?,
                encode(sha)
            ),
            300,
        )?;
        Ok(CiStatus::from_checks(
            items
                .iter()
                .map(|s| Check {
                    name: s.opt("/name").unwrap_or_else(|| s.s("/key")),
                    state: match s.s("/state").as_str() {
                        "SUCCESSFUL" => CheckState::Success,
                        "FAILED" => CheckState::Failure,
                        "STOPPED" => CheckState::Neutral,
                        _ => CheckState::Pending,
                    },
                    description: s.opt("/description"),
                    url: s.opt("/url"),
                })
                .collect(),
        ))
    }

    fn add_ssh_key(&self, me: &User, title: &str, key: &str) -> Result<()> {
        self.api.post(
            &format!("/users/{}/ssh-keys", encode(&me.id)),
            &json!({ "key": key.trim(), "label": title }),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ronin_config::ProviderKind;
    use serde_json::json;

    use super::*;
    use crate::http::Method;
    use crate::test_support::{Fake, account, provider};

    fn pr_json() -> Value {
        json!({
            "id": 8, "title": "Fix", "state": "OPEN", "draft": false,
            "author": {"display_name": "Ann Lee", "nickname": "ann"},
            "source": {"branch": {"name": "fix"}, "commit": {"hash": "h"},
                       "repository": {"full_name": "ann/app"}},
            "destination": {"branch": {"name": "main"}, "commit": {"hash": "b"},
                            "repository": {"full_name": "team/app"}},
            "links": {"html": {"href": "https://bitbucket.org/team/app/pull-requests/8"}},
            "updated_on": "2024-02-29T12:00:00.000000+00:00",
            "participants": [
                {"user": {"nickname": "bo"}, "role": "REVIEWER", "approved": true, "state": "approved"},
                {"user": {"nickname": "cy"}, "role": "REVIEWER", "approved": false, "state": null},
                {"user": {"nickname": "di"}, "role": "PARTICIPANT", "approved": false, "state": null}
            ]
        })
    }

    #[test]
    fn signs_in_with_email_and_token() {
        let fake = Fake::new().on(
            Method::Get,
            "/user",
            json!({
                "uuid": "{u-1}", "nickname": "me", "display_name": "Me",
                "links": {"avatar": {"href": "https://a/me.png"}}
            }),
        );
        let mut acct = account(ProviderKind::Bitbucket, "https://bitbucket.org");
        acct.login = "me@example.com".into();
        let bb = provider(&fake, &acct);
        let user = bb.user().unwrap();
        assert_eq!((user.id.as_str(), user.username.as_str()), ("{u-1}", "me"));
        let (request, _) = fake.sent(Method::Get, "/user");
        assert_eq!(request.url, "https://api.bitbucket.org/2.0/user");
        let auth = &request.headers[0].1;
        assert!(auth.starts_with("Basic "), "{auth}");
    }

    #[test]
    fn follows_next_and_reads_repos() {
        let fake = Fake::new()
            .on(
                Method::Get,
                "page=2",
                json!({"values": [{"full_name": "t/b"}]}),
            )
            .on(
                Method::Get,
                "/user/workspaces",
                json!({"values": [{"workspace": {"slug": "t"}}]}),
            )
            .on(
                Method::Get,
                "/repositories/t?role=member",
                json!({
                    "values": [{"full_name": "t/a", "name": "a", "is_private": true,
                        "links": {"clone": [
                            {"name": "https", "href": "https://me@bitbucket.org/t/a.git"},
                            {"name": "ssh", "href": "git@bitbucket.org:t/a.git"}],
                            "html": {"href": "https://bitbucket.org/t/a"}},
                        "mainbranch": {"name": "main"}}],
                    "next": "https://api.bitbucket.org/2.0/repositories/t?role=member&page=2"
                }),
            );
        let bb = provider(
            &fake,
            &account(ProviderKind::Bitbucket, "https://bitbucket.org"),
        );
        let repos = bb.repos(100).unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].clone_https, "https://bitbucket.org/t/a.git");
        assert_eq!(
            repos[0].clone_ssh.as_deref(),
            Some("git@bitbucket.org:t/a.git")
        );
        assert_eq!(repos[1].clone_https, "https://bitbucket.org/t/b.git");
    }

    #[test]
    fn lists_my_pull_requests_per_workspace() {
        let fake = Fake::new()
            .on(
                Method::Get,
                "/user/workspaces",
                json!({"values": [{"workspace": {"slug": "a"}}, {"workspace": {"slug": "b"}}]}),
            )
            .on(
                Method::Get,
                "/workspaces/a/",
                json!({"values": [pr_json()]}),
            )
            .on(Method::Get, "/workspaces/b/", json!({"values": []}));
        let bb = provider(
            &fake,
            &account(ProviderKind::Bitbucket, "https://bitbucket.org"),
        );
        let me = User {
            id: "{u-1}".into(),
            ..User::default()
        };
        let mine = bb.my_pull_requests(&me).unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].pr.number, 8);
        let (request, _) = fake.sent(Method::Get, "/workspaces/a/");
        assert!(
            request
                .url
                .ends_with("/2.0/workspaces/a/pullrequests/%7Bu-1%7D?state=OPEN&pagelen=50"),
            "{}",
            request.url
        );
    }

    #[test]
    fn reads_and_writes_pull_requests() {
        let fake = Fake::new()
            .on(
                Method::Get,
                "/pullrequests/8/comments",
                json!({"values": [
                    {"user": {"nickname": "bo"}, "content": {"raw": "ok"}, "deleted": false},
                    {"user": {"nickname": "bo"}, "content": {"raw": ""}, "deleted": true}
                ]}),
            )
            .on(Method::Get, "/pullrequests/8", pr_json())
            .on(Method::Post, "/pullrequests/8/merge", json!({}))
            .on(Method::Post, "/pullrequests", pr_json());
        let bb = provider(
            &fake,
            &account(ProviderKind::Bitbucket, "https://bitbucket.org"),
        );
        let d = bb.pull_request("team/app", 8).unwrap();
        assert_eq!(d.pr.source_repo.as_deref(), Some("ann/app"));
        assert_eq!(
            d.source.url.as_deref(),
            Some("https://bitbucket.org/ann/app.git")
        );
        assert_eq!(d.source.refspec, "refs/heads/fix");
        assert_eq!(d.comments.len(), 1);
        assert_eq!(d.reviews.len(), 2);
        assert_eq!(d.reviews[0].state, ReviewState::Approved);

        bb.merge("team/app", 8, MergeMethod::Rebase).unwrap();
        assert_eq!(
            fake.sent(Method::Post, "/merge").1["merge_strategy"],
            "fast_forward"
        );
        bb.create_pull_request(
            "team/app",
            &NewPullRequest {
                title: "Fix".into(),
                body: String::new(),
                source_branch: "fix".into(),
                target_branch: "main".into(),
                source_repo: None,
                draft: false,
            },
        )
        .unwrap();
        let (_, body) = fake.sent(Method::Post, "/repositories/team/app/pullrequests");
        assert_eq!(body["destination"]["branch"]["name"], "main");
        assert!(body["source"].get("repository").is_none());
    }

    #[test]
    fn strips_users_from_clone_links() {
        assert_eq!(
            strip_user("https://me@bitbucket.org/t/a.git"),
            "https://bitbucket.org/t/a.git"
        );
        assert_eq!(
            strip_user("https://bitbucket.org/t/a.git"),
            "https://bitbucket.org/t/a.git"
        );
    }
}
