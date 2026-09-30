//! GitHub and GitHub Enterprise Server (REST API v3).

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
    device_flow: true,
};

pub struct GitHub {
    api: Api,
}

impl GitHub {
    pub fn new(api: Api) -> Self {
        Self { api }
    }

    fn repo_url(repo: &str) -> Result<String> {
        match repo.split_once('/') {
            Some((owner, name)) if !owner.is_empty() && !name.is_empty() && !name.contains('/') => {
                Ok(format!("/repos/{}/{}", encode(owner), encode(name)))
            }
            _ => Err(Error::Invalid(format!("“{repo}” is not owner/name"))),
        }
    }

    /// Open pull requests found by an issue search.
    fn search_prs(&self, query: &str) -> Result<Vec<PullRequest>> {
        let found = self.api.get(&format!(
            "/search/issues?q={}&per_page=50&sort=updated",
            encode(query)
        ))?;
        Ok(found.list("/items").iter().map(search_pr).collect())
    }
}

fn person(v: &Value) -> Person {
    Person {
        username: v.s("/login"),
        name: v.opt("/name").unwrap_or_else(|| v.s("/login")),
        avatar_url: v.opt("/avatar_url"),
    }
}

fn repo(v: &Value) -> HostedRepo {
    HostedRepo {
        path: v.s("/full_name"),
        name: v.s("/name"),
        description: v.opt("/description"),
        private: v.b("/private"),
        fork: v.b("/fork"),
        clone_https: v.s("/clone_url"),
        clone_ssh: v.opt("/ssh_url"),
        web_url: v.s("/html_url"),
        default_branch: v.opt("/default_branch"),
        updated: Some(v.time("/pushed_at").max(v.time("/updated_at"))),
    }
}

fn pr(v: &Value) -> PullRequest {
    let state = if v.opt("/merged_at").is_some() {
        PrState::Merged
    } else if v.s("/state") == "open" {
        PrState::Open
    } else {
        PrState::Closed
    };
    let base_repo = v.s("/base/repo/full_name");
    let head_repo = v.s("/head/repo/full_name");
    PullRequest {
        repo: base_repo.clone(),
        number: v.n("/number"),
        title: v.s("/title"),
        state,
        draft: v.b("/draft"),
        author: person(&v["user"]),
        source_branch: v.s("/head/ref"),
        target_branch: v.s("/base/ref"),
        source_repo: (!head_repo.is_empty() && head_repo != base_repo).then_some(head_repo),
        web_url: v.s("/html_url"),
        updated: v.time("/updated_at"),
    }
}

/// A pull request as the issue search returns it: no branches.
fn search_pr(v: &Value) -> PullRequest {
    // repository_url: https://api.github.com/repos/owner/name
    let repo_url = v.s("/repository_url");
    let repo = repo_url
        .rsplitn(3, '/')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("/");
    PullRequest {
        repo,
        number: v.n("/number"),
        title: v.s("/title"),
        state: if v.s("/state") == "open" {
            PrState::Open
        } else {
            PrState::Closed
        },
        draft: v.b("/draft"),
        author: person(&v["user"]),
        source_branch: String::new(),
        target_branch: String::new(),
        source_repo: None,
        web_url: v.s("/html_url"),
        updated: v.time("/updated_at"),
    }
}

fn issue(v: &Value, repo: &str) -> Issue {
    let number = v.n("/number");
    Issue {
        repo: repo.to_owned(),
        key: format!("#{number}"),
        number,
        title: v.s("/title"),
        open: v.s("/state") == "open",
        author: v.get("user").map(person),
        labels: v.list("/labels").iter().map(|l| l.s("/name")).collect(),
        web_url: v.s("/html_url"),
        updated: v.time("/updated_at"),
    }
}

fn check_state(status: &str, conclusion: &str) -> CheckState {
    match (status, conclusion) {
        (_, "success") => CheckState::Success,
        (_, "failure" | "timed_out" | "cancelled" | "action_required" | "startup_failure") => {
            CheckState::Failure
        }
        (_, "neutral" | "skipped" | "stale") => CheckState::Neutral,
        ("completed", _) => CheckState::Neutral,
        // Commit statuses.
        ("success", _) => CheckState::Success,
        ("failure" | "error", _) => CheckState::Failure,
        _ => CheckState::Pending,
    }
}

impl Provider for GitHub {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        let v = self.api.get("/user")?;
        Ok(User {
            id: v.s("/id"),
            username: v.s("/login"),
            name: v.opt("/name").unwrap_or_else(|| v.s("/login")),
            avatar_url: v.opt("/avatar_url"),
            slug: v.s("/login"),
        })
    }

    fn repos(&self, limit: usize) -> Result<Vec<HostedRepo>> {
        let items = self.api.get_linked(
            "/user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member",
            limit,
        )?;
        Ok(items.iter().map(repo).collect())
    }

    fn fork(&self, path: &str) -> Result<HostedRepo> {
        let v = self
            .api
            .post(&format!("{}/forks", Self::repo_url(path)?), &json!({}))?;
        Ok(repo(&v))
    }

    fn pull_requests(&self, path: &str) -> Result<Vec<PullRequest>> {
        let items = self.api.get_linked(
            &format!(
                "{}/pulls?state=open&per_page=100&sort=updated&direction=desc",
                Self::repo_url(path)?
            ),
            300,
        )?;
        Ok(items.iter().map(pr).collect())
    }

    fn my_pull_requests(&self, _me: &User) -> Result<Vec<MyPullRequest>> {
        let mut mine: Vec<MyPullRequest> = Vec::new();
        for (query, role) in [
            (
                "is:open is:pr archived:false author:@me",
                Involvement::Author,
            ),
            (
                "is:open is:pr archived:false review-requested:@me",
                Involvement::Reviewer,
            ),
            (
                "is:open is:pr archived:false assignee:@me",
                Involvement::Assignee,
            ),
        ] {
            for found in self.search_prs(query)? {
                match mine.iter_mut().find(|m| m.pr.web_url == found.web_url) {
                    Some(m) => m.involvement.push(role),
                    None => mine.push(MyPullRequest {
                        pr: found,
                        involvement: vec![role],
                    }),
                }
            }
        }
        mine.sort_by_key(|m| std::cmp::Reverse(m.pr.updated));
        Ok(mine)
    }

    fn pull_request(&self, path: &str, number: u64) -> Result<PullRequestDetail> {
        let base = format!("{}/pulls/{number}", Self::repo_url(path)?);
        let v = self.api.get(&base)?;
        let issue_comments = self.api.get_linked(
            &format!(
                "{}/issues/{number}/comments?per_page=100",
                Self::repo_url(path)?
            ),
            500,
        )?;
        let reviews = self
            .api
            .get_linked(&format!("{base}/reviews?per_page=100"), 500)?;

        let mut comments: Vec<Comment> = issue_comments
            .iter()
            .map(|c| Comment {
                author: person(&c["user"]),
                body: c.s("/body"),
                created: c.time("/created_at"),
            })
            .collect();
        // A review with a summary reads like a comment.
        comments.extend(
            reviews
                .iter()
                .filter(|r| r.opt("/body").is_some())
                .map(|r| Comment {
                    author: person(&r["user"]),
                    body: r.s("/body"),
                    created: r.time("/submitted_at"),
                }),
        );
        comments.sort_by_key(|c| c.created);

        // Each reviewer's latest decision counts.
        let mut latest: Vec<Review> = Vec::new();
        for r in &reviews {
            let state = match r.s("/state").as_str() {
                "APPROVED" => ReviewState::Approved,
                "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
                "COMMENTED" => ReviewState::Commented,
                _ => continue,
            };
            let author = person(&r["user"]);
            match latest
                .iter_mut()
                .find(|l| l.author.username == author.username)
            {
                // A comment doesn't withdraw an approval.
                Some(l) if state != ReviewState::Commented => l.state = state,
                Some(_) => {}
                None => latest.push(Review { author, state }),
            }
        }
        for requested in v.list("/requested_reviewers") {
            let author = person(requested);
            if !latest.iter().any(|l| l.author.username == author.username) {
                latest.push(Review {
                    author,
                    state: ReviewState::Pending,
                });
            }
        }

        let pr = pr(&v);
        Ok(PullRequestDetail {
            body: v.s("/body"),
            head_sha: v.opt("/head/sha"),
            base_sha: v.opt("/base/sha"),
            source: PrSource {
                refspec: format!("refs/pull/{number}/head"),
                url: None,
            },
            mergeable: v.get("mergeable").and_then(Value::as_bool),
            reviews: latest,
            comments,
            merge_methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
            pr,
        })
    }

    fn create_pull_request(&self, path: &str, new: &NewPullRequest) -> Result<PullRequest> {
        let head = match &new.source_repo {
            Some(fork) => format!(
                "{}:{}",
                fork.split('/').next().unwrap_or_default(),
                new.source_branch
            ),
            None => new.source_branch.clone(),
        };
        let v = self.api.post(
            &format!("{}/pulls", Self::repo_url(path)?),
            &json!({
                "title": new.title,
                "body": new.body,
                "head": head,
                "base": new.target_branch,
                "draft": new.draft,
            }),
        )?;
        Ok(pr(&v))
    }

    fn comment(&self, path: &str, number: u64, body: &str) -> Result<()> {
        self.api.post(
            &format!("{}/issues/{number}/comments", Self::repo_url(path)?),
            &json!({ "body": body }),
        )?;
        Ok(())
    }

    fn approve(&self, path: &str, number: u64, _me: &User) -> Result<()> {
        self.api.post(
            &format!("{}/pulls/{number}/reviews", Self::repo_url(path)?),
            &json!({ "event": "APPROVE" }),
        )?;
        Ok(())
    }

    fn merge(&self, path: &str, number: u64, method: MergeMethod) -> Result<()> {
        let method = match method {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
        };
        self.api.put(
            &format!("{}/pulls/{number}/merge", Self::repo_url(path)?),
            &json!({ "merge_method": method }),
        )?;
        Ok(())
    }

    fn issues(&self, path: &str) -> Result<Vec<Issue>> {
        let items = self.api.get_linked(
            &format!(
                "{}/issues?state=open&per_page=100&sort=updated",
                Self::repo_url(path)?
            ),
            300,
        )?;
        Ok(items
            .iter()
            // The issues list includes pull requests.
            .filter(|i| i.get("pull_request").is_none())
            .map(|i| issue(i, path))
            .collect())
    }

    fn my_issues(&self, _me: &User) -> Result<Vec<Issue>> {
        let found = self.api.get(&format!(
            "/search/issues?q={}&per_page=50&sort=updated",
            encode("is:open is:issue archived:false assignee:@me")
        ))?;
        Ok(found
            .list("/items")
            .iter()
            .map(|i| {
                let repo_url = i.s("/repository_url");
                let mut parts = repo_url.rsplit('/');
                let name = parts.next().unwrap_or_default();
                let owner = parts.next().unwrap_or_default();
                issue(i, &format!("{owner}/{name}"))
            })
            .collect())
    }

    fn create_issue(&self, path: &str, title: &str, body: &str) -> Result<Issue> {
        let v = self.api.post(
            &format!("{}/issues", Self::repo_url(path)?),
            &json!({ "title": title, "body": body }),
        )?;
        Ok(issue(&v, path))
    }

    fn ci_status(&self, path: &str, sha: &str) -> Result<CiStatus> {
        let base = format!("{}/commits/{}", Self::repo_url(path)?, encode(sha));
        let mut checks = Vec::new();
        // Tokens without the checks permission can still read statuses.
        let runs = self.api.get(&format!("{base}/check-runs?per_page=100"));
        if let Ok(runs) = &runs {
            checks.extend(runs.list("/check_runs").iter().map(|r| Check {
                name: r.s("/name"),
                state: check_state(&r.s("/status"), &r.s("/conclusion")),
                description: r.opt("/output/title"),
                url: r.opt("/html_url").or_else(|| r.opt("/details_url")),
            }));
        }
        let statuses = self.api.get(&format!("{base}/status"));
        if let Ok(combined) = &statuses {
            checks.extend(combined.list("/statuses").iter().map(|s| Check {
                name: s.s("/context"),
                state: check_state(&s.s("/state"), ""),
                description: s.opt("/description"),
                url: s.opt("/target_url"),
            }));
        }
        match (runs, statuses) {
            (Err(e), Err(_)) => Err(e),
            _ => Ok(CiStatus::from_checks(checks)),
        }
    }

    fn add_ssh_key(&self, _me: &User, title: &str, key: &str) -> Result<()> {
        self.api
            .post("/user/keys", &json!({ "title": title, "key": key.trim() }))?;
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

    fn pr_json(number: u64) -> Value {
        json!({
            "number": number, "title": "Add feature", "state": "open", "draft": false,
            "user": {"login": "ann", "avatar_url": "https://a/ann.png"},
            "head": {"ref": "feature", "sha": "abc", "repo": {"full_name": "ann/app"}},
            "base": {"ref": "main", "sha": "def", "repo": {"full_name": "org/app"}},
            "html_url": format!("https://github.com/org/app/pull/{number}"),
            "updated_at": "2024-02-29T12:00:00Z",
            "merged_at": null, "mergeable": true, "body": "Please review",
            "requested_reviewers": [{"login": "cy"}]
        })
    }

    #[test]
    fn uses_the_right_api_address_and_headers() {
        let fake = Fake::new().on(Method::Get, "/user", json!({"id": 7, "login": "me"}));
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));
        let user = gh.user().unwrap();
        assert_eq!((user.id.as_str(), user.name.as_str()), ("7", "me"));
        let (request, _) = fake.sent(Method::Get, "/user");
        assert_eq!(request.url, "https://api.github.com/user");
        assert!(
            request
                .headers
                .contains(&("Authorization".into(), "Bearer tok".into()))
        );

        let ghe = provider(&fake, &account(ProviderKind::Github, "https://git.corp"));
        ghe.user().unwrap();
        assert!(
            fake.urls()
                .contains(&"GET https://git.corp/api/v3/user".to_owned())
        );
    }

    #[test]
    fn reads_pull_requests_with_reviews_and_comments() {
        let fake = Fake::new()
            .on(Method::Get, "/pulls/5/reviews", json!([
                {"user": {"login": "bo"}, "state": "CHANGES_REQUESTED", "body": "", "submitted_at": "2024-03-01T00:00:00Z"},
                {"user": {"login": "bo"}, "state": "APPROVED", "body": "Looks good", "submitted_at": "2024-03-02T00:00:00Z"},
                {"user": {"login": "bo"}, "state": "COMMENTED", "body": "", "submitted_at": "2024-03-03T00:00:00Z"}
            ]))
            .on(Method::Get, "/issues/5/comments", json!([
                {"user": {"login": "ann"}, "body": "Ready", "created_at": "2024-02-29T13:00:00Z"}
            ]))
            .on(Method::Get, "/pulls/5", pr_json(5))
            .on(Method::Get, "/pulls?state=open", json!([pr_json(5), pr_json(6)]));
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));

        let list = gh.pull_requests("org/app").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].source_repo.as_deref(), Some("ann/app"));
        assert_eq!(list[0].updated, 1_709_208_000);

        let detail = gh.pull_request("org/app", 5).unwrap();
        assert_eq!(detail.pr.author.username, "ann");
        assert_eq!(detail.source.refspec, "refs/pull/5/head");
        assert_eq!(detail.mergeable, Some(true));
        let reviews: Vec<_> = detail
            .reviews
            .iter()
            .map(|r| (r.author.username.as_str(), r.state))
            .collect();
        assert_eq!(
            reviews,
            [("bo", ReviewState::Approved), ("cy", ReviewState::Pending)]
        );
        let comments: Vec<_> = detail.comments.iter().map(|c| c.body.as_str()).collect();
        assert_eq!(comments, ["Ready", "Looks good"]);
    }

    #[test]
    fn follows_pages() {
        let fake = Fake::new()
            .on(Method::Get, "page=2", json!([{"full_name": "o/b", "name": "b"}]))
            .on_status(
                Method::Get,
                "/user/repos",
                200,
                json!([{"full_name": "o/a", "name": "a", "private": true,
                        "clone_url": "https://github.com/o/a.git", "ssh_url": "git@github.com:o/a.git"}]),
                &[("Link", "<https://api.github.com/user/repos?page=2>; rel=\"next\"")],
            );
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));
        let repos = gh.repos(1000).unwrap();
        let names: Vec<_> = repos.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(names, ["o/a", "o/b"]);
        assert!(repos[0].private);
        assert_eq!(gh.repos(1).unwrap().len(), 1);
    }

    #[test]
    fn writes() {
        let fake = Fake::new()
            .on(Method::Post, "/pulls", pr_json(9))
            .on(Method::Post, "/issues/9/comments", json!({}))
            .on(Method::Post, "/pulls/9/reviews", json!({}))
            .on(Method::Put, "/pulls/9/merge", json!({"merged": true}))
            .on(
                Method::Post,
                "/issues",
                json!({"number": 3, "title": "Bug", "state": "open",
                                                "html_url": "https://github.com/org/app/issues/3"}),
            )
            .on(Method::Post, "/user/keys", json!({}));
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));
        let me = User::default();
        let created = gh
            .create_pull_request(
                "org/app",
                &NewPullRequest {
                    title: "T".into(),
                    body: "B".into(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                    source_repo: Some("ann/app".into()),
                    draft: true,
                },
            )
            .unwrap();
        assert_eq!(created.number, 9);
        let (_, body) = fake.sent(Method::Post, "/repos/org/app/pulls");
        assert_eq!(body["head"], "ann:feature");
        assert_eq!(body["draft"], true);

        gh.comment("org/app", 9, "hi").unwrap();
        gh.approve("org/app", 9, &me).unwrap();
        assert_eq!(fake.sent(Method::Post, "/reviews").1["event"], "APPROVE");
        gh.merge("org/app", 9, MergeMethod::Squash).unwrap();
        assert_eq!(fake.sent(Method::Put, "/merge").1["merge_method"], "squash");
        let issue = gh.create_issue("org/app", "Bug", "").unwrap();
        assert_eq!((issue.key.as_str(), issue.repo.as_str()), ("#3", "org/app"));
        gh.add_ssh_key(&me, "laptop", "ssh-ed25519 AAAA x\n")
            .unwrap();
        assert_eq!(
            fake.sent(Method::Post, "/user/keys").1["key"],
            "ssh-ed25519 AAAA x"
        );
        assert!(gh.pull_request("bad", 1).is_err());
    }

    #[test]
    fn combines_check_runs_and_statuses() {
        let fake = Fake::new()
            .on(
                Method::Get,
                "/check-runs",
                json!({"check_runs": [
                    {"name": "build", "status": "completed", "conclusion": "success"},
                    {"name": "lint", "status": "in_progress", "conclusion": null}
                ]}),
            )
            .on(
                Method::Get,
                "/status",
                json!({"statuses": [
                    {"context": "ci/deploy", "state": "failure", "target_url": "https://ci/1"}
                ]}),
            );
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));
        let status = gh.ci_status("org/app", "abc").unwrap();
        assert_eq!(status.state, Some(CheckState::Failure));
        let states: Vec<_> = status.checks.iter().map(|c| c.state).collect();
        assert_eq!(
            states,
            [
                CheckState::Success,
                CheckState::Pending,
                CheckState::Failure
            ]
        );
    }

    #[test]
    fn launchpad_merges_roles() {
        let item = json!({"number": 1, "title": "x", "state": "open",
            "repository_url": "https://api.github.com/repos/org/app",
            "html_url": "https://github.com/org/app/pull/1", "user": {"login": "me"}});
        let fake = Fake::new().on(Method::Get, "/search/issues", json!({"items": [item]}));
        let gh = provider(&fake, &account(ProviderKind::Github, "https://github.com"));
        let mine = gh.my_pull_requests(&User::default()).unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].pr.repo, "org/app");
        assert_eq!(
            mine[0].involvement,
            [
                Involvement::Author,
                Involvement::Reviewer,
                Involvement::Assignee
            ]
        );
    }
}
