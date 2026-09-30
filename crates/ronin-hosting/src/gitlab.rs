//! GitLab.com and self-managed GitLab (REST API v4).

use serde_json::{Value, json};

use crate::http::{Api, encode};
use crate::json::Json;
use crate::{
    Capabilities, Check, CheckState, CiStatus, Comment, HostedRepo, Involvement, Issue,
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

pub struct GitLab {
    api: Api,
}

impl GitLab {
    pub fn new(api: Api) -> Self {
        Self { api }
    }
}

/// Projects are addressed by their URL-encoded full path.
fn project(path: &str) -> String {
    format!("/projects/{}", encode(path))
}

fn person(v: &Value) -> Person {
    Person {
        username: v.s("/username"),
        name: v.opt("/name").unwrap_or_else(|| v.s("/username")),
        avatar_url: v.opt("/avatar_url"),
    }
}

fn repo(v: &Value) -> HostedRepo {
    HostedRepo {
        path: v.s("/path_with_namespace"),
        name: v.s("/name"),
        description: v.opt("/description"),
        private: v.s("/visibility") != "public",
        fork: v.get("forked_from_project").is_some_and(|f| !f.is_null()),
        clone_https: v.s("/http_url_to_repo"),
        clone_ssh: v.opt("/ssh_url_to_repo"),
        web_url: v.s("/web_url"),
        default_branch: v.opt("/default_branch"),
        updated: Some(v.time("/last_activity_at")),
    }
}

/// `group/project` from a reference such as `group/project!12` or `group/project#3`.
fn path_of_reference(reference: &str) -> String {
    reference
        .rsplit_once(['!', '#'])
        .map_or(reference, |(path, _)| path)
        .to_owned()
}

fn mr(v: &Value, repo: Option<&str>) -> PullRequest {
    let state = match v.s("/state").as_str() {
        "opened" | "locked" => PrState::Open,
        "merged" => PrState::Merged,
        _ => PrState::Closed,
    };
    let fork = v.n("/source_project_id") != v.n("/target_project_id");
    PullRequest {
        repo: repo.map_or_else(
            || path_of_reference(&v.s("/references/full")),
            str::to_owned,
        ),
        number: v.n("/iid"),
        title: v.s("/title"),
        state,
        draft: v.b("/draft") || v.b("/work_in_progress"),
        author: person(&v["author"]),
        source_branch: v.s("/source_branch"),
        target_branch: v.s("/target_branch"),
        source_repo: fork.then(|| format!("project {}", v.n("/source_project_id"))),
        web_url: v.s("/web_url"),
        updated: v.time("/updated_at"),
    }
}

fn issue(v: &Value, repo: Option<&str>) -> Issue {
    let number = v.n("/iid");
    Issue {
        repo: repo.map_or_else(
            || path_of_reference(&v.s("/references/full")),
            str::to_owned,
        ),
        key: format!("#{number}"),
        number,
        title: v.s("/title"),
        open: v.s("/state") == "opened",
        author: v.get("author").map(person),
        labels: v
            .list("/labels")
            .iter()
            .filter_map(|l| l.as_str().map(str::to_owned).or_else(|| l.opt("/name")))
            .collect(),
        web_url: v.s("/web_url"),
        updated: v.time("/updated_at"),
    }
}

fn check_state(status: &str, allow_failure: bool) -> CheckState {
    match status {
        "success" => CheckState::Success,
        "failed" if allow_failure => CheckState::Neutral,
        "failed" => CheckState::Failure,
        "canceled" | "skipped" | "manual" => CheckState::Neutral,
        _ => CheckState::Pending,
    }
}

impl Provider for GitLab {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        let v = self.api.get("/user")?;
        Ok(User {
            id: v.s("/id"),
            username: v.s("/username"),
            name: v.opt("/name").unwrap_or_else(|| v.s("/username")),
            avatar_url: v.opt("/avatar_url"),
            slug: v.s("/username"),
        })
    }

    fn repos(&self, limit: usize) -> Result<Vec<HostedRepo>> {
        let items = self.api.get_linked(
            "/projects?membership=true&order_by=last_activity_at&sort=desc&per_page=100",
            limit,
        )?;
        Ok(items.iter().map(repo).collect())
    }

    fn fork(&self, path: &str) -> Result<HostedRepo> {
        let v = self
            .api
            .post(&format!("{}/fork", project(path)), &json!({}))?;
        Ok(repo(&v))
    }

    fn pull_requests(&self, path: &str) -> Result<Vec<PullRequest>> {
        let items = self.api.get_linked(
            &format!(
                "{}/merge_requests?state=opened&order_by=updated_at&per_page=100",
                project(path)
            ),
            300,
        )?;
        Ok(items.iter().map(|v| mr(v, Some(path))).collect())
    }

    fn my_pull_requests(&self, me: &User) -> Result<Vec<MyPullRequest>> {
        let mut mine: Vec<MyPullRequest> = Vec::new();
        for (query, role) in [
            ("scope=created_by_me".to_owned(), Involvement::Author),
            (
                format!("scope=all&reviewer_username={}", encode(&me.username)),
                Involvement::Reviewer,
            ),
            ("scope=assigned_to_me".to_owned(), Involvement::Assignee),
        ] {
            let items = self
                .api
                .get(&format!("/merge_requests?state=opened&per_page=50&{query}"))?;
            for v in items.as_array().map_or(&[][..], Vec::as_slice) {
                let found = mr(v, None);
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
        let base = format!("{}/merge_requests/{number}", project(path));
        let v = self.api.get(&base)?;
        let notes = self.api.get_linked(
            &format!("{base}/notes?sort=asc&order_by=created_at&per_page=100"),
            1000,
        )?;
        // Approval rules are a paid feature, but who approved is not.
        let approvals = self.api.get(&format!("{base}/approvals")).ok();

        let comments = notes
            .iter()
            .filter(|n| !n.b("/system"))
            .map(|n| Comment {
                author: person(&n["author"]),
                body: n.s("/body"),
                created: n.time("/created_at"),
            })
            .collect();
        let mut reviews: Vec<Review> = approvals
            .as_ref()
            .map(|a| a.list("/approved_by"))
            .unwrap_or_default()
            .iter()
            .map(|a| Review {
                author: person(&a["user"]),
                state: ReviewState::Approved,
            })
            .collect();
        for reviewer in v.list("/reviewers") {
            let author = person(reviewer);
            if !reviews.iter().any(|r| r.author.username == author.username) {
                reviews.push(Review {
                    author,
                    state: ReviewState::Pending,
                });
            }
        }
        let mergeable = match v.s("/detailed_merge_status").as_str() {
            "mergeable" => Some(true),
            "checking" | "unchecked" | "preparing" | "approvals_syncing" => None,
            "" => match v.s("/merge_status").as_str() {
                "can_be_merged" => Some(true),
                "cannot_be_merged" => Some(false),
                _ => None,
            },
            _ => Some(false),
        };
        Ok(PullRequestDetail {
            pr: mr(&v, Some(path)),
            body: v.s("/description"),
            head_sha: v.opt("/diff_refs/head_sha").or_else(|| v.opt("/sha")),
            base_sha: v.opt("/diff_refs/base_sha"),
            source: PrSource {
                refspec: format!("refs/merge-requests/{number}/head"),
                url: None,
            },
            mergeable,
            reviews,
            comments,
            merge_methods: vec![MergeMethod::Merge, MergeMethod::Squash],
        })
    }

    fn create_pull_request(&self, path: &str, new: &NewPullRequest) -> Result<PullRequest> {
        let title = if new.draft && !new.title.starts_with("Draft:") {
            format!("Draft: {}", new.title)
        } else {
            new.title.clone()
        };
        let mut body = json!({
            "source_branch": new.source_branch,
            "target_branch": new.target_branch,
            "title": title,
            "description": new.body,
            "remove_source_branch": false,
        });
        // From a fork, the request is made on the fork and names the target.
        let on = match &new.source_repo {
            Some(fork) => {
                let target = self.api.get(&project(path))?;
                body["target_project_id"] = target["id"].clone();
                fork.as_str()
            }
            None => path,
        };
        let v = self
            .api
            .post(&format!("{}/merge_requests", project(on)), &body)?;
        Ok(mr(&v, Some(path)))
    }

    fn comment(&self, path: &str, number: u64, body: &str) -> Result<()> {
        self.api.post(
            &format!("{}/merge_requests/{number}/notes", project(path)),
            &json!({ "body": body }),
        )?;
        Ok(())
    }

    fn approve(&self, path: &str, number: u64, _me: &User) -> Result<()> {
        self.api.post(
            &format!("{}/merge_requests/{number}/approve", project(path)),
            &json!({}),
        )?;
        Ok(())
    }

    fn merge(&self, path: &str, number: u64, method: MergeMethod) -> Result<()> {
        self.api.put(
            &format!("{}/merge_requests/{number}/merge", project(path)),
            &json!({ "squash": method == MergeMethod::Squash }),
        )?;
        Ok(())
    }

    fn issues(&self, path: &str) -> Result<Vec<Issue>> {
        let items = self.api.get_linked(
            &format!(
                "{}/issues?state=opened&order_by=updated_at&per_page=100",
                project(path)
            ),
            300,
        )?;
        Ok(items.iter().map(|v| issue(v, Some(path))).collect())
    }

    fn my_issues(&self, _me: &User) -> Result<Vec<Issue>> {
        let items = self
            .api
            .get("/issues?scope=assigned_to_me&state=opened&order_by=updated_at&per_page=50")?;
        Ok(items
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|v| issue(v, None))
            .collect())
    }

    fn create_issue(&self, path: &str, title: &str, body: &str) -> Result<Issue> {
        let v = self.api.post(
            &format!("{}/issues", project(path)),
            &json!({ "title": title, "description": body }),
        )?;
        Ok(issue(&v, Some(path)))
    }

    fn ci_status(&self, path: &str, sha: &str) -> Result<CiStatus> {
        let items = self.api.get_linked(
            &format!(
                "{}/repository/commits/{}/statuses?per_page=100",
                project(path),
                encode(sha)
            ),
            300,
        )?;
        Ok(CiStatus::from_checks(
            items
                .iter()
                .map(|s| Check {
                    name: s.s("/name"),
                    state: check_state(&s.s("/status"), s.b("/allow_failure")),
                    description: s.opt("/description"),
                    url: s.opt("/target_url"),
                })
                .collect(),
        ))
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

    fn mr_json() -> Value {
        json!({
            "iid": 4, "title": "Draft: speed up", "state": "opened", "draft": true,
            "author": {"username": "ann", "name": "Ann"},
            "source_branch": "fast", "target_branch": "main",
            "source_project_id": 1, "target_project_id": 1,
            "references": {"full": "grp/sub/app!4"},
            "web_url": "http://localhost:8929/grp/sub/app/-/merge_requests/4",
            "updated_at": "2024-02-29T12:00:00.000Z",
            "description": "Faster", "detailed_merge_status": "mergeable",
            "diff_refs": {"base_sha": "b", "head_sha": "h"},
            "reviewers": [{"username": "bo"}, {"username": "cy"}]
        })
    }

    #[test]
    fn encodes_nested_project_paths() {
        let fake = Fake::new().on(
            Method::Get,
            "/merge_requests?state=opened",
            json!([mr_json()]),
        );
        let gl = provider(
            &fake,
            &account(ProviderKind::Gitlab, "http://localhost:8929"),
        );
        let list = gl.pull_requests("grp/sub/app").unwrap();
        assert_eq!(list[0].number, 4);
        assert!(list[0].draft);
        assert_eq!(list[0].source_repo, None);
        assert!(fake.urls()[0].starts_with(
            "GET http://localhost:8929/api/v4/projects/grp%2Fsub%2Fapp/merge_requests"
        ));
    }

    #[test]
    fn reads_merge_requests() {
        let fake = Fake::new()
            .on(
                Method::Get,
                "/merge_requests/4/notes",
                json!([
                    {"author": {"username": "bot"}, "body": "added 1 commit", "system": true},
                    {"author": {"username": "bo"}, "body": "Nice", "system": false,
                     "created_at": "2024-03-01T00:00:00Z"}
                ]),
            )
            .on(
                Method::Get,
                "/merge_requests/4/approvals",
                json!({"approved_by": [{"user": {"username": "bo", "name": "Bo"}}]}),
            )
            .on(Method::Get, "/merge_requests/4", mr_json());
        let gl = provider(&fake, &account(ProviderKind::Gitlab, "https://gitlab.com"));
        let d = gl.pull_request("grp/sub/app", 4).unwrap();
        assert_eq!(d.source.refspec, "refs/merge-requests/4/head");
        assert_eq!(d.mergeable, Some(true));
        assert_eq!(
            (d.base_sha.as_deref(), d.head_sha.as_deref()),
            (Some("b"), Some("h"))
        );
        assert_eq!(d.comments.len(), 1);
        let reviews: Vec<_> = d
            .reviews
            .iter()
            .map(|r| (r.author.username.as_str(), r.state))
            .collect();
        assert_eq!(
            reviews,
            [("bo", ReviewState::Approved), ("cy", ReviewState::Pending)]
        );
    }

    #[test]
    fn my_merge_requests_and_issues_find_their_projects() {
        let fake = Fake::new()
            .on(Method::Get, "/merge_requests?", json!([mr_json()]))
            .on(
                Method::Get,
                "/issues?scope=assigned_to_me",
                json!([
                    {"iid": 3, "title": "Bug", "state": "opened", "labels": ["bug"],
                     "references": {"full": "grp/app#3"}}
                ]),
            );
        let gl = provider(&fake, &account(ProviderKind::Gitlab, "https://gitlab.com"));
        let me = User {
            username: "me".into(),
            ..User::default()
        };
        let mine = gl.my_pull_requests(&me).unwrap();
        assert_eq!(mine[0].pr.repo, "grp/sub/app");
        assert_eq!(mine[0].involvement.len(), 3);
        assert!(
            fake.urls()
                .iter()
                .any(|u| u.contains("reviewer_username=me"))
        );
        let issues = gl.my_issues(&me).unwrap();
        assert_eq!(
            (issues[0].repo.as_str(), issues[0].labels[0].as_str()),
            ("grp/app", "bug")
        );
    }

    #[test]
    fn creates_from_forks_and_merges() {
        let fake = Fake::new()
            .on(Method::Post, "/merge_requests", mr_json())
            .on(Method::Get, "/projects/grp%2Fapp", json!({"id": 77}))
            .on(Method::Put, "/merge", json!({}));
        let gl = provider(&fake, &account(ProviderKind::Gitlab, "https://gitlab.com"));
        gl.create_pull_request(
            "grp/app",
            &NewPullRequest {
                title: "Speed up".into(),
                body: String::new(),
                source_branch: "fast".into(),
                target_branch: "main".into(),
                source_repo: Some("me/app".into()),
                draft: true,
            },
        )
        .unwrap();
        let (request, body) = fake.sent(Method::Post, "/merge_requests");
        assert!(request.url.contains("/projects/me%2Fapp/merge_requests"));
        assert_eq!(body["target_project_id"], 77);
        assert_eq!(body["title"], "Draft: Speed up");
        gl.merge("grp/app", 4, MergeMethod::Squash).unwrap();
        assert_eq!(fake.sent(Method::Put, "/merge").1["squash"], true);
    }

    #[test]
    fn maps_job_statuses() {
        let fake = Fake::new().on(
            Method::Get,
            "/statuses",
            json!([
                {"name": "test", "status": "success"},
                {"name": "flaky", "status": "failed", "allow_failure": true},
                {"name": "deploy", "status": "running"}
            ]),
        );
        let gl = provider(&fake, &account(ProviderKind::Gitlab, "https://gitlab.com"));
        let s = gl.ci_status("grp/app", "abc").unwrap();
        assert_eq!(s.state, Some(CheckState::Pending));
        assert_eq!(s.checks[1].state, CheckState::Neutral);
    }
}
