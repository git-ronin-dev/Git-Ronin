//! Bitbucket Server and Data Center (REST API 1.0), with an HTTP access
//! token. Issues live in Jira, so there are none here.

use serde_json::{Value, json};

use crate::http::{Api, Method, encode};
use crate::json::Json;
use crate::{
    Capabilities, Check, CheckState, CiStatus, Comment, Error, HostedRepo, Involvement,
    MergeMethod, MyPullRequest, NewPullRequest, Person, PrSource, PrState, Provider, PullRequest,
    PullRequestDetail, Result, Review, ReviewState, User,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    repos: true,
    fork: true,
    pull_requests: true,
    approve: true,
    draft: true,
    issues: false,
    ci: true,
    ssh_keys: true,
    device_flow: false,
};

pub struct BitbucketServer {
    api: Api,
    web: String,
}

impl BitbucketServer {
    pub fn new(api: Api, web: &str) -> Self {
        Self {
            api,
            web: web.trim_end_matches('/').to_owned(),
        }
    }

    /// Reads `start`/`isLastPage` pages until `limit` values were read.
    fn paged(&self, path: &str, limit: usize) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut start = 0;
        let separator = if path.contains('?') { '&' } else { '?' };
        loop {
            let page = self.api.get(&format!("{path}{separator}start={start}"))?;
            items.extend(page.list("/values").iter().cloned());
            if items.len() >= limit {
                items.truncate(limit);
                break;
            }
            if page.b("/isLastPage") || page.get("nextPageStart").is_none_or(Value::is_null) {
                break;
            }
            start = page.n("/nextPageStart");
        }
        Ok(items)
    }
}

/// `/api/1.0/projects/PROJ/repos/slug`.
fn repo_url(path: &str) -> Result<String> {
    match path.split_once('/') {
        Some((project, slug)) if !project.is_empty() && !slug.is_empty() => Ok(format!(
            "/api/1.0/projects/{}/repos/{}",
            encode(project),
            encode(slug)
        )),
        _ => Err(Error::Invalid(format!(
            "“{path}” is not PROJECT/repository"
        ))),
    }
}

fn path_of(repo: &Value) -> String {
    format!("{}/{}", repo.s("/project/key"), repo.s("/slug"))
}

fn person(v: &Value) -> Person {
    Person {
        username: v.opt("/slug").unwrap_or_else(|| v.s("/name")),
        name: v.opt("/displayName").unwrap_or_else(|| v.s("/name")),
        avatar_url: None,
    }
}

fn link(v: &Value, list: &str, name: Option<&str>) -> Option<String> {
    v.list(list)
        .iter()
        .find(|l| name.is_none_or(|n| l.s("/name") == n))
        .and_then(|l| l.opt("/href"))
}

fn repo(v: &Value) -> HostedRepo {
    HostedRepo {
        path: path_of(v),
        name: v.s("/name"),
        description: v.opt("/description"),
        private: !v.b("/public") && !v.b("/project/public"),
        fork: v.get("origin").is_some_and(|o| !o.is_null()),
        clone_https: link(v, "/links/clone", Some("http")).unwrap_or_default(),
        clone_ssh: link(v, "/links/clone", Some("ssh")),
        web_url: link(v, "/links/self", None).unwrap_or_default(),
        default_branch: None,
        updated: None,
    }
}

fn pr(v: &Value) -> PullRequest {
    let target = path_of(&v["toRef"]["repository"]);
    let source = path_of(&v["fromRef"]["repository"]);
    PullRequest {
        number: v.n("/id"),
        title: v.s("/title"),
        state: match v.s("/state").as_str() {
            "OPEN" => PrState::Open,
            "MERGED" => PrState::Merged,
            _ => PrState::Closed,
        },
        draft: v.b("/draft"),
        author: person(&v["author"]["user"]),
        source_branch: v.s("/fromRef/displayId"),
        target_branch: v.s("/toRef/displayId"),
        source_repo: (source != target).then_some(source),
        repo: target,
        web_url: link(v, "/links/self", None).unwrap_or_default(),
        updated: v.time("/updatedDate"),
    }
}

fn reviewer(r: &Value) -> Review {
    Review {
        author: person(&r["user"]),
        state: match r.s("/status").as_str() {
            "APPROVED" => ReviewState::Approved,
            "NEEDS_WORK" => ReviewState::ChangesRequested,
            _ => ReviewState::Pending,
        },
    }
}

fn ref_json(path: &str, branch: &str) -> Result<Value> {
    let (project, slug) = path
        .split_once('/')
        .ok_or_else(|| Error::Invalid(format!("“{path}” is not PROJECT/repository")))?;
    Ok(json!({
        "id": format!("refs/heads/{branch}"),
        "repository": { "slug": slug, "project": { "key": project } },
    }))
}

impl Provider for BitbucketServer {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        // The server names the token's user in a header of any answer.
        let response = self
            .api
            .send(Method::Get, "/api/1.0/application-properties", None)?;
        let slug = response
            .header("x-ausername")
            .map(str::to_owned)
            .ok_or_else(|| Error::Unauthorized("the server did not accept the token".into()))?;
        let v = self.api.get(&format!("/api/1.0/users/{}", encode(&slug)))?;
        Ok(User {
            id: v.s("/id"),
            username: v.opt("/name").unwrap_or_else(|| slug.clone()),
            name: v.opt("/displayName").unwrap_or_else(|| slug.clone()),
            avatar_url: Some(format!("{}/users/{}/avatar.png", self.web, encode(&slug))),
            slug,
        })
    }

    fn repos(&self, limit: usize) -> Result<Vec<HostedRepo>> {
        let items = self.paged("/api/1.0/repos?permission=REPO_READ&limit=100", limit)?;
        Ok(items.iter().map(repo).collect())
    }

    fn fork(&self, path: &str) -> Result<HostedRepo> {
        Ok(repo(&self.api.post(&repo_url(path)?, &json!({}))?))
    }

    fn pull_requests(&self, path: &str) -> Result<Vec<PullRequest>> {
        let items = self.paged(
            &format!(
                "{}/pull-requests?state=OPEN&order=NEWEST&limit=100",
                repo_url(path)?
            ),
            300,
        )?;
        Ok(items.iter().map(pr).collect())
    }

    fn my_pull_requests(&self, _me: &User) -> Result<Vec<MyPullRequest>> {
        let mut mine: Vec<MyPullRequest> = Vec::new();
        for (role, involvement) in [
            ("AUTHOR", Involvement::Author),
            ("REVIEWER", Involvement::Reviewer),
        ] {
            let items = self.paged(
                &format!("/api/1.0/dashboard/pull-requests?state=OPEN&role={role}&limit=50"),
                100,
            )?;
            for v in &items {
                let found = pr(v);
                match mine.iter_mut().find(|m| m.pr.web_url == found.web_url) {
                    Some(m) => m.involvement.push(involvement),
                    None => mine.push(MyPullRequest {
                        pr: found,
                        involvement: vec![involvement],
                    }),
                }
            }
        }
        mine.sort_by_key(|m| std::cmp::Reverse(m.pr.updated));
        Ok(mine)
    }

    fn pull_request(&self, path: &str, number: u64) -> Result<PullRequestDetail> {
        let base = format!("{}/pull-requests/{number}", repo_url(path)?);
        let v = self.api.get(&base)?;
        let activities = self.paged(&format!("{base}/activities?limit=100"), 1000)?;
        let mut comments: Vec<Comment> = activities
            .iter()
            .filter(|a| a.s("/action") == "COMMENTED")
            .map(|a| Comment {
                author: person(&a["comment"]["author"]),
                body: a.s("/comment/text"),
                created: a.time("/comment/createdDate"),
            })
            .collect();
        comments.sort_by_key(|c| c.created);
        let mergeable = self
            .api
            .get(&format!("{base}/merge"))
            .ok()
            .and_then(|m| m.get("canMerge").and_then(Value::as_bool));
        Ok(PullRequestDetail {
            pr: pr(&v),
            body: v.s("/description"),
            head_sha: v.opt("/fromRef/latestCommit"),
            base_sha: v.opt("/toRef/latestCommit"),
            source: PrSource {
                refspec: format!("refs/pull-requests/{number}/from"),
                url: None,
            },
            mergeable,
            reviews: v.list("/reviewers").iter().map(reviewer).collect(),
            comments,
            merge_methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
        })
    }

    fn create_pull_request(&self, path: &str, new: &NewPullRequest) -> Result<PullRequest> {
        let from = new.source_repo.as_deref().unwrap_or(path);
        let v = self.api.post(
            &format!("{}/pull-requests", repo_url(path)?),
            &json!({
                "title": new.title,
                "description": new.body,
                "draft": new.draft,
                "fromRef": ref_json(from, &new.source_branch)?,
                "toRef": ref_json(path, &new.target_branch)?,
            }),
        )?;
        Ok(pr(&v))
    }

    fn comment(&self, path: &str, number: u64, body: &str) -> Result<()> {
        self.api.post(
            &format!("{}/pull-requests/{number}/comments", repo_url(path)?),
            &json!({ "text": body }),
        )?;
        Ok(())
    }

    fn approve(&self, path: &str, number: u64, me: &User) -> Result<()> {
        self.api.put(
            &format!(
                "{}/pull-requests/{number}/participants/{}",
                repo_url(path)?,
                encode(&me.slug)
            ),
            &json!({ "user": { "name": me.slug }, "approved": true, "status": "APPROVED" }),
        )?;
        Ok(())
    }

    fn merge(&self, path: &str, number: u64, method: MergeMethod) -> Result<()> {
        let base = format!("{}/pull-requests/{number}", repo_url(path)?);
        // Merging needs the version the user saw; take the latest.
        let version = self.api.get(&base)?.n("/version");
        let body = match method {
            // The repository's default strategy.
            MergeMethod::Merge => json!({}),
            MergeMethod::Squash => json!({ "strategyId": "squash" }),
            MergeMethod::Rebase => json!({ "strategyId": "rebase-ff-only" }),
        };
        self.api
            .post(&format!("{base}/merge?version={version}"), &body)?;
        Ok(())
    }

    fn ci_status(&self, _path: &str, sha: &str) -> Result<CiStatus> {
        let items = self.paged(
            &format!("/build-status/1.0/commits/{}?limit=100", encode(sha)),
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
                        "CANCELLED" | "UNKNOWN" => CheckState::Neutral,
                        _ => CheckState::Pending,
                    },
                    description: s.opt("/description"),
                    url: s.opt("/url"),
                })
                .collect(),
        ))
    }

    fn add_ssh_key(&self, _me: &User, title: &str, key: &str) -> Result<()> {
        self.api.post(
            "/ssh/1.0/keys",
            &json!({ "text": key.trim(), "label": title }),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ronin_config::ProviderKind;
    use serde_json::json;

    use super::*;
    use crate::test_support::{Fake, account, provider};

    fn pr_json(version: u64) -> Value {
        let repo = json!({"slug": "app", "project": {"key": "PROJ"}});
        json!({
            "id": 3, "version": version, "title": "Change", "state": "OPEN",
            "author": {"user": {"name": "ann", "slug": "ann", "displayName": "Ann"}},
            "fromRef": {"displayId": "feature", "latestCommit": "h", "repository": repo},
            "toRef": {"displayId": "main", "latestCommit": "b", "repository": repo},
            "links": {"self": [{"href": "https://bb.corp/projects/PROJ/repos/app/pull-requests/3"}]},
            "updatedDate": 1_709_208_000_000_i64,
            "reviewers": [{"user": {"slug": "bo"}, "status": "NEEDS_WORK"}]
        })
    }

    #[test]
    fn finds_the_user_from_the_header() {
        let fake = Fake::new()
            .on_status(
                Method::Get,
                "/application-properties",
                200,
                json!({"version": "8.19"}),
                &[("X-AUSERNAME", "jo")],
            )
            .on(
                Method::Get,
                "/api/1.0/users/jo",
                json!({"id": 5, "name": "jo", "displayName": "Jo"}),
            );
        let bbs = provider(
            &fake,
            &account(ProviderKind::BitbucketServer, "https://bb.corp"),
        );
        let user = bbs.user().unwrap();
        assert_eq!(
            (user.id.as_str(), user.slug.as_str(), user.name.as_str()),
            ("5", "jo", "Jo")
        );
        assert!(fake.urls()[0].starts_with("GET https://bb.corp/rest/api/1.0/"));
    }

    #[test]
    fn pages_by_start() {
        let fake = Fake::new()
            .on(Method::Get, "start=25", json!({"values": [{"slug": "b", "project": {"key": "P"}}], "isLastPage": true}))
            .on(Method::Get, "start=0", json!({
                "values": [{"slug": "a", "name": "a", "project": {"key": "P"},
                    "links": {"clone": [{"name": "ssh", "href": "ssh://git@bb.corp:7999/p/a.git"},
                                        {"name": "http", "href": "https://bb.corp/scm/p/a.git"}]}}],
                "isLastPage": false, "nextPageStart": 25
            }));
        let bbs = provider(
            &fake,
            &account(ProviderKind::BitbucketServer, "https://bb.corp"),
        );
        let repos = bbs.repos(100).unwrap();
        let paths: Vec<_> = repos.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["P/a", "P/b"]);
        assert_eq!(repos[0].clone_https, "https://bb.corp/scm/p/a.git");
    }

    #[test]
    fn pull_requests_merge_with_the_latest_version() {
        let fake = Fake::new()
            .on(Method::Get, "/activities", json!({"values": [
                {"action": "COMMENTED", "comment": {"author": {"slug": "bo"}, "text": "why?", "createdDate": 1}},
                {"action": "APPROVED"}
            ], "isLastPage": true}))
            .on(Method::Get, "/pull-requests/3/merge", json!({"canMerge": false}))
            .on(Method::Get, "/pull-requests/3", pr_json(7))
            .on(Method::Post, "/merge?version=7", json!({}))
            .on(Method::Put, "/participants/me", json!({}));
        let bbs = provider(
            &fake,
            &account(ProviderKind::BitbucketServer, "https://bb.corp"),
        );
        let d = bbs.pull_request("PROJ/app", 3).unwrap();
        assert_eq!(d.pr.updated, 1_709_208_000);
        assert_eq!(d.mergeable, Some(false));
        assert_eq!(d.comments[0].body, "why?");
        assert_eq!(d.reviews[0].state, ReviewState::ChangesRequested);
        assert_eq!(d.source.refspec, "refs/pull-requests/3/from");
        bbs.merge("PROJ/app", 3, MergeMethod::Squash).unwrap();
        assert_eq!(
            fake.sent(Method::Post, "/merge?version=7").1["strategyId"],
            "squash"
        );
        let me = User {
            slug: "me".into(),
            ..User::default()
        };
        bbs.approve("PROJ/app", 3, &me).unwrap();
        assert!(bbs.issues("PROJ/app").is_err());
    }
}
