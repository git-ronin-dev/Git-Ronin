//! Azure DevOps Services and Server (REST API 7.1), with a personal access
//! token. Issues are work items.

use serde_json::{Value, json};

use crate::http::{Api, encode};
use crate::json::Json;
use crate::{
    Account, Capabilities, Check, CheckState, CiStatus, Comment, Error, HostedRepo, Involvement,
    Issue, MergeMethod, MyPullRequest, NewPullRequest, Person, PrSource, PrState, Provider,
    PullRequest, PullRequestDetail, Result, Review, ReviewState, User,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    repos: true,
    fork: false,
    pull_requests: true,
    approve: true,
    draft: true,
    issues: true,
    ci: true,
    ssh_keys: false,
    device_flow: false,
};

const VERSION: &str = "api-version=7.1";
/// Work item states that mean done in the standard processes.
const DONE_STATES: &str = "'Closed', 'Done', 'Removed', 'Resolved'";

pub struct Azure {
    api: Api,
    /// `https://dev.azure.com/<organization>`.
    web: String,
}

impl Azure {
    pub fn new(api: Api, account: &Account) -> Result<Self> {
        Ok(Self {
            api,
            web: account.url.trim_end_matches('/').to_owned(),
        })
    }

    fn work_items(&self, wiql_path: &str, query: &str) -> Result<Vec<Issue>> {
        let found = self.api.post(
            &format!("{wiql_path}?$top=100&{VERSION}"),
            &json!({ "query": query }),
        )?;
        let ids: Vec<String> = found
            .list("/workItems")
            .iter()
            .take(100)
            .map(|w| w.n("/id").to_string())
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let fields = "System.Title,System.State,System.WorkItemType,System.CreatedBy,\
                      System.ChangedDate,System.TeamProject,System.Tags";
        let items = self.api.get(&format!(
            "/_apis/wit/workitems?ids={}&fields={fields}&{VERSION}",
            ids.join(",")
        ))?;
        Ok(items
            .list("/value")
            .iter()
            .map(|w| self.work_item(w))
            .collect())
    }

    fn work_item(&self, w: &Value) -> Issue {
        let number = w.n("/id");
        let project = w.s("/fields/System.TeamProject");
        let state = w.s("/fields/System.State");
        let mut labels = vec![w.s("/fields/System.WorkItemType")];
        labels.extend(
            w.s("/fields/System.Tags")
                .split(';')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned),
        );
        Issue {
            key: format!("#{number}"),
            number,
            title: w.s("/fields/System.Title"),
            open: !DONE_STATES.contains(&format!("'{state}'")),
            author: w.get("fields").map(|f| person(&f["System.CreatedBy"])),
            labels: labels.into_iter().filter(|l| !l.is_empty()).collect(),
            web_url: format!("{}/{}/_workitems/edit/{number}", self.web, encode(&project)),
            updated: w.time("/fields/System.ChangedDate"),
            repo: project,
        }
    }

    fn pr(&self, v: &Value) -> PullRequest {
        let project = v.s("/repository/project/name");
        let repo = v.s("/repository/name");
        let number = v.n("/pullRequestId");
        PullRequest {
            web_url: format!(
                "{}/{}/_git/{}/pullrequest/{number}",
                self.web,
                encode(&project),
                encode(&repo)
            ),
            repo: format!("{project}/{repo}"),
            number,
            title: v.s("/title"),
            state: match v.s("/status").as_str() {
                "active" => PrState::Open,
                "completed" => PrState::Merged,
                _ => PrState::Closed,
            },
            draft: v.b("/isDraft"),
            author: person(&v["createdBy"]),
            source_branch: branch(&v.s("/sourceRefName")),
            target_branch: branch(&v.s("/targetRefName")),
            source_repo: v
                .opt("/forkSource/repository/name")
                .map(|fork| format!("{project}/{fork}")),
            updated: v.time("/closedDate").max(v.time("/creationDate")),
        }
    }
}

/// `/Project/_apis/git/repositories/Repo`.
fn repo_url(path: &str) -> Result<String> {
    match path.split_once('/') {
        Some((project, repo)) if !project.is_empty() && !repo.is_empty() => Ok(format!(
            "/{}/_apis/git/repositories/{}",
            encode(project),
            encode(repo)
        )),
        _ => Err(Error::Invalid(format!(
            "“{path}” is not project/repository"
        ))),
    }
}

fn branch(name: &str) -> String {
    name.strip_prefix("refs/heads/").unwrap_or(name).to_owned()
}

fn person(v: &Value) -> Person {
    Person {
        username: v.opt("/uniqueName").unwrap_or_else(|| v.s("/displayName")),
        name: v.s("/displayName"),
        avatar_url: v.opt("/imageUrl"),
    }
}

fn repo(v: &Value) -> HostedRepo {
    HostedRepo {
        path: format!("{}/{}", v.s("/project/name"), v.s("/name")),
        name: v.s("/name"),
        description: None,
        private: v.s("/project/visibility") != "public",
        fork: v.b("/isFork"),
        clone_https: v.s("/remoteUrl"),
        clone_ssh: v.opt("/sshUrl"),
        web_url: v.s("/webUrl"),
        default_branch: v.opt("/defaultBranch").map(|b| branch(&b)),
        updated: None,
    }
}

impl Provider for Azure {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        let v = self.api.get(&format!("/_apis/connectionData?{VERSION}"))?;
        let me = &v["authenticatedUser"];
        let id = me.s("/id");
        if id.is_empty() || id == "aa44a8a5-0000-0000-0000-000000000000" {
            return Err(Error::Unauthorized("the token was not accepted".into()));
        }
        let email = me.s("/properties/Account/$value");
        let name = me.s("/providerDisplayName");
        Ok(User {
            id,
            username: if email.is_empty() {
                name.clone()
            } else {
                email
            },
            name,
            avatar_url: None,
            slug: String::new(),
        })
    }

    fn repos(&self, limit: usize) -> Result<Vec<HostedRepo>> {
        let v = self.api.get(&format!(
            "/_apis/git/repositories?includeHidden=false&{VERSION}"
        ))?;
        let mut repos: Vec<HostedRepo> = v.list("/value").iter().map(repo).collect();
        repos.sort_by_key(|r| r.path.to_lowercase());
        repos.truncate(limit);
        Ok(repos)
    }

    fn pull_requests(&self, path: &str) -> Result<Vec<PullRequest>> {
        let v = self.api.get(&format!(
            "{}/pullrequests?searchCriteria.status=active&$top=200&{VERSION}",
            repo_url(path)?
        ))?;
        Ok(v.list("/value").iter().map(|p| self.pr(p)).collect())
    }

    fn my_pull_requests(&self, me: &User) -> Result<Vec<MyPullRequest>> {
        let projects = self
            .api
            .get(&format!("/_apis/projects?$top=50&{VERSION}"))?;
        let mut mine: Vec<MyPullRequest> = Vec::new();
        for project in projects.list("/value") {
            let project = encode(&project.s("/name"));
            for (criterion, role) in [
                ("creatorId", Involvement::Author),
                ("reviewerId", Involvement::Reviewer),
            ] {
                let v = self.api.get(&format!(
                    "/{project}/_apis/git/pullrequests?searchCriteria.status=active&\
                     searchCriteria.{criterion}={}&$top=100&{VERSION}",
                    encode(&me.id)
                ))?;
                for p in v.list("/value") {
                    let found = self.pr(p);
                    match mine.iter_mut().find(|m| m.pr.web_url == found.web_url) {
                        Some(m) => m.involvement.push(role),
                        None => mine.push(MyPullRequest {
                            pr: found,
                            involvement: vec![role],
                        }),
                    }
                }
            }
        }
        mine.sort_by_key(|m| std::cmp::Reverse(m.pr.updated));
        Ok(mine)
    }

    fn pull_request(&self, path: &str, number: u64) -> Result<PullRequestDetail> {
        let base = format!("{}/pullrequests/{number}", repo_url(path)?);
        let v = self.api.get(&format!("{base}?{VERSION}"))?;
        let threads = self.api.get(&format!("{base}/threads?{VERSION}"))?;
        let mut comments: Vec<Comment> = threads
            .list("/value")
            .iter()
            .filter(|t| !t.b("/isDeleted"))
            .flat_map(|t| t.list("/comments"))
            .filter(|c| c.s("/commentType") == "text" && !c.b("/isDeleted"))
            .map(|c| Comment {
                author: person(&c["author"]),
                body: c.s("/content"),
                created: c.time("/publishedDate"),
            })
            .collect();
        comments.sort_by_key(|c| c.created);
        let reviews = v
            .list("/reviewers")
            .iter()
            .map(|r| Review {
                author: person(r),
                // Votes: 10 approved, 5 with suggestions, -5 waiting, -10 rejected.
                state: match r.pointer("/vote").and_then(Value::as_i64).unwrap_or(0) {
                    5.. => ReviewState::Approved,
                    ..0 => ReviewState::ChangesRequested,
                    _ => ReviewState::Pending,
                },
            })
            .collect();
        Ok(PullRequestDetail {
            pr: self.pr(&v),
            body: v.s("/description"),
            head_sha: v.opt("/lastMergeSourceCommit/commitId"),
            base_sha: v.opt("/lastMergeTargetCommit/commitId"),
            source: PrSource {
                refspec: v.s("/sourceRefName"),
                url: None,
            },
            mergeable: match v.s("/mergeStatus").as_str() {
                "succeeded" => Some(true),
                "conflicts" | "failure" | "rejectedByPolicy" => Some(false),
                _ => None,
            },
            reviews,
            comments,
            merge_methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
        })
    }

    fn create_pull_request(&self, path: &str, new: &NewPullRequest) -> Result<PullRequest> {
        if new.source_repo.is_some() {
            return Err(Error::Unsupported("pull requests from forks"));
        }
        let v = self.api.post(
            &format!("{}/pullrequests?{VERSION}", repo_url(path)?),
            &json!({
                "sourceRefName": format!("refs/heads/{}", new.source_branch),
                "targetRefName": format!("refs/heads/{}", new.target_branch),
                "title": new.title,
                "description": new.body,
                "isDraft": new.draft,
            }),
        )?;
        Ok(self.pr(&v))
    }

    fn comment(&self, path: &str, number: u64, body: &str) -> Result<()> {
        self.api.post(
            &format!(
                "{}/pullrequests/{number}/threads?{VERSION}",
                repo_url(path)?
            ),
            &json!({
                "comments": [{ "parentCommentId": 0, "content": body, "commentType": 1 }],
                "status": 1,
            }),
        )?;
        Ok(())
    }

    fn approve(&self, path: &str, number: u64, me: &User) -> Result<()> {
        self.api.put(
            &format!(
                "{}/pullrequests/{number}/reviewers/{}?{VERSION}",
                repo_url(path)?,
                encode(&me.id)
            ),
            &json!({ "vote": 10 }),
        )?;
        Ok(())
    }

    fn merge(&self, path: &str, number: u64, method: MergeMethod) -> Result<()> {
        let base = format!("{}/pullrequests/{number}?{VERSION}", repo_url(path)?);
        // Completing needs the source commit the merge is for.
        let current = self.api.get(&base)?;
        let strategy = match method {
            MergeMethod::Merge => "noFastForward",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
        };
        self.api.patch(
            &base,
            &json!({
                "status": "completed",
                "lastMergeSourceCommit": { "commitId": current.s("/lastMergeSourceCommit/commitId") },
                "completionOptions": { "mergeStrategy": strategy },
            }),
        )?;
        Ok(())
    }

    fn issues(&self, path: &str) -> Result<Vec<Issue>> {
        let project = path.split('/').next().unwrap_or_default();
        self.work_items(
            &format!("/{}/_apis/wit/wiql", encode(project)),
            &format!(
                "SELECT [System.Id] FROM WorkItems WHERE [System.TeamProject] = @project \
                 AND [System.State] NOT IN ({DONE_STATES}) ORDER BY [System.ChangedDate] DESC"
            ),
        )
    }

    fn my_issues(&self, _me: &User) -> Result<Vec<Issue>> {
        self.work_items(
            "/_apis/wit/wiql",
            &format!(
                "SELECT [System.Id] FROM WorkItems WHERE [System.AssignedTo] = @Me \
                 AND [System.State] NOT IN ({DONE_STATES}) ORDER BY [System.ChangedDate] DESC"
            ),
        )
    }

    fn create_issue(&self, path: &str, title: &str, body: &str) -> Result<Issue> {
        let project = path.split('/').next().unwrap_or_default();
        let v = self.api.send_typed(
            crate::http::Method::Post,
            &format!("/{}/_apis/wit/workitems/$Task?{VERSION}", encode(project)),
            "application/json-patch+json",
            &json!([
                { "op": "add", "path": "/fields/System.Title", "value": title },
                { "op": "add", "path": "/fields/System.Description", "value": body },
            ]),
        )?;
        Ok(self.work_item(&v))
    }

    fn ci_status(&self, path: &str, sha: &str) -> Result<CiStatus> {
        let v = self.api.get(&format!(
            "{}/commits/{}/statuses?latestOnly=true&{VERSION}",
            repo_url(path)?,
            encode(sha)
        ))?;
        Ok(CiStatus::from_checks(
            v.list("/value")
                .iter()
                .map(|s| {
                    let genre = s.s("/context/genre");
                    let name = s.s("/context/name");
                    Check {
                        name: if genre.is_empty() {
                            name
                        } else {
                            format!("{genre}/{name}")
                        },
                        state: match s.s("/state").as_str() {
                            "succeeded" => CheckState::Success,
                            "failed" | "error" => CheckState::Failure,
                            "notApplicable" | "notSet" => CheckState::Neutral,
                            _ => CheckState::Pending,
                        },
                        description: s.opt("/description"),
                        url: s.opt("/targetUrl"),
                    }
                })
                .collect(),
        ))
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
            "pullRequestId": 12, "title": "Upgrade", "status": "active", "isDraft": false,
            "createdBy": {"displayName": "Ann", "uniqueName": "ann@contoso.com"},
            "sourceRefName": "refs/heads/upgrade", "targetRefName": "refs/heads/main",
            "repository": {"name": "web", "project": {"name": "My Project"}},
            "creationDate": "2024-02-29T12:00:00Z", "mergeStatus": "succeeded",
            "lastMergeSourceCommit": {"commitId": "h"}, "lastMergeTargetCommit": {"commitId": "b"},
            "reviewers": [{"displayName": "Bo", "vote": 10}, {"displayName": "Cy", "vote": -10},
                          {"displayName": "Di", "vote": 0}]
        })
    }

    fn azure(fake: &std::sync::Arc<Fake>) -> Box<dyn Provider> {
        provider(
            fake,
            &account(ProviderKind::AzureDevops, "https://dev.azure.com/contoso"),
        )
    }

    #[test]
    fn authenticates_with_basic_auth_and_reads_the_user() {
        let fake = Fake::new().on(
            Method::Get,
            "/_apis/connectionData",
            json!({
                "authenticatedUser": {"id": "u-1", "providerDisplayName": "Me",
                    "properties": {"Account": {"$value": "me@contoso.com"}}}
            }),
        );
        let user = azure(&fake).user().unwrap();
        assert_eq!(
            (user.id.as_str(), user.username.as_str()),
            ("u-1", "me@contoso.com")
        );
        let (request, _) = fake.sent(Method::Get, "connectionData");
        assert_eq!(
            request.url,
            "https://dev.azure.com/contoso/_apis/connectionData?api-version=7.1"
        );
        assert_eq!(request.headers[0].1, "Basic OnRvaw==");
    }

    #[test]
    fn reads_pull_requests() {
        let fake = Fake::new()
            .on(Method::Get, "/pullrequests/12/threads", json!({"value": [
                {"comments": [{"author": {"displayName": "Bo"}, "content": "Hi", "commentType": "text",
                               "publishedDate": "2024-03-01T00:00:00Z"}]},
                {"comments": [{"content": "Policy", "commentType": "system"}]}
            ]}))
            .on(Method::Get, "/pullrequests/12", pr_json());
        let d = azure(&fake).pull_request("My Project/web", 12).unwrap();
        assert!(
            fake.urls()[0].contains("/My%20Project/_apis/git/repositories/web/pullrequests/12")
        );
        assert_eq!(d.pr.repo, "My Project/web");
        assert_eq!(
            d.pr.web_url,
            "https://dev.azure.com/contoso/My%20Project/_git/web/pullrequest/12"
        );
        assert_eq!(d.source.refspec, "refs/heads/upgrade");
        assert_eq!(d.pr.source_branch, "upgrade");
        assert_eq!(d.comments.len(), 1);
        let states: Vec<_> = d.reviews.iter().map(|r| r.state).collect();
        assert_eq!(
            states,
            [
                ReviewState::Approved,
                ReviewState::ChangesRequested,
                ReviewState::Pending
            ]
        );
    }

    #[test]
    fn completes_with_the_source_commit() {
        let fake = Fake::new()
            .on(Method::Get, "/pullrequests/12", pr_json())
            .on(Method::Patch, "/pullrequests/12", json!({}));
        azure(&fake)
            .merge("P/web", 12, MergeMethod::Squash)
            .unwrap();
        let (_, body) = fake.sent(Method::Patch, "/pullrequests/12");
        assert_eq!(body["lastMergeSourceCommit"]["commitId"], "h");
        assert_eq!(body["completionOptions"]["mergeStrategy"], "squash");
    }

    #[test]
    fn work_items_are_issues() {
        let fake = Fake::new()
            .on(Method::Post, "/wiql", json!({"workItems": [{"id": 5}, {"id": 6}]}))
            .on(Method::Get, "/_apis/wit/workitems?ids=5,6", json!({"value": [
                {"id": 5, "fields": {"System.Title": "Broken", "System.State": "Active",
                  "System.WorkItemType": "Bug", "System.TeamProject": "P", "System.Tags": "ui; urgent"}},
                {"id": 6, "fields": {"System.Title": "Old", "System.State": "Closed",
                  "System.WorkItemType": "Task", "System.TeamProject": "P"}}
            ]}))
            .on(Method::Post, "/workitems/$Task", json!({"id": 7, "fields": {
                "System.Title": "New", "System.State": "To Do", "System.TeamProject": "P"}}));
        let az = azure(&fake);
        let issues = az.my_issues(&User::default()).unwrap();
        assert_eq!(issues[0].labels, ["Bug", "ui", "urgent"]);
        assert!(issues[0].open);
        assert!(!issues[1].open);
        assert_eq!(
            issues[0].web_url,
            "https://dev.azure.com/contoso/P/_workitems/edit/5"
        );
        let created = az.create_issue("P/web", "New", "").unwrap();
        assert_eq!(created.key, "#7");
        let (request, body) = fake.sent(Method::Post, "/workitems/$Task");
        assert!(
            request
                .headers
                .contains(&("Content-Type".into(), "application/json-patch+json".into()))
        );
        assert_eq!(body[0]["path"], "/fields/System.Title");
    }
}
