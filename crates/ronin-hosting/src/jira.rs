//! Jira Cloud, Server and Data Center: issues only. Cloud takes the
//! account's email with an API token, Server and Data Center a personal
//! access token.

use serde_json::{Value, json};

use crate::http::{Api, encode};
use crate::json::Json;
use crate::{Capabilities, Error, Issue, Person, Provider, Result, User};

pub const CAPABILITIES: Capabilities = Capabilities {
    repos: false,
    fork: false,
    pull_requests: false,
    approve: false,
    draft: false,
    issues: true,
    ci: false,
    ssh_keys: false,
    device_flow: false,
};

const FIELDS: &str = "summary,status,issuetype,project,updated,reporter,labels";

pub struct Jira {
    api: Api,
    web: String,
    cloud: bool,
}

impl Jira {
    pub fn new(api: Api, web: &str) -> Self {
        let web = web.trim_end_matches('/').to_owned();
        let cloud = ronin_config::accounts::host_of(&web).ends_with(".atlassian.net");
        Self { api, web, cloud }
    }

    fn search(&self, jql: &str) -> Result<Vec<Issue>> {
        // Cloud retired the old search endpoint.
        let endpoint = if self.cloud {
            "/rest/api/3/search/jql"
        } else {
            "/rest/api/2/search"
        };
        let v = self.api.get(&format!(
            "{endpoint}?jql={}&fields={FIELDS}&maxResults=100",
            encode(jql)
        ))?;
        Ok(v.list("/issues").iter().map(|i| self.issue(i)).collect())
    }

    fn issue(&self, v: &Value) -> Issue {
        let key = v.s("/key");
        let mut labels = vec![v.s("/fields/issuetype/name"), v.s("/fields/status/name")];
        labels.extend(
            v.list("/fields/labels")
                .iter()
                .filter_map(|l| l.as_str().map(str::to_owned)),
        );
        Issue {
            repo: v.opt("/fields/project/key").unwrap_or_else(|| {
                key.rsplit_once('-')
                    .map(|(p, _)| p.to_owned())
                    .unwrap_or_default()
            }),
            web_url: format!("{}/browse/{key}", self.web),
            number: v.n("/id"),
            title: v.s("/fields/summary"),
            open: v.s("/fields/status/statusCategory/key") != "done",
            author: v
                .get("fields")
                .and_then(|f| f.get("reporter"))
                .filter(|r| !r.is_null())
                .map(|r| Person {
                    username: r
                        .opt("/name")
                        .or_else(|| r.opt("/emailAddress"))
                        .unwrap_or_else(|| r.s("/accountId")),
                    name: r.s("/displayName"),
                    avatar_url: r.opt("/avatarUrls/48x48"),
                }),
            labels: labels.into_iter().filter(|l| !l.is_empty()).collect(),
            updated: v.time("/fields/updated"),
            key,
        }
    }
}

/// Jira project keys are letters, digits and underscores.
fn project_key(project: &str) -> Result<&str> {
    if !project.is_empty()
        && project
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Ok(project)
    } else {
        Err(Error::Invalid(format!(
            "“{project}” is not a Jira project key"
        )))
    }
}

impl Provider for Jira {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn user(&self) -> Result<User> {
        let v = self.api.get("/rest/api/2/myself")?;
        let username = v
            .opt("/name")
            .or_else(|| v.opt("/emailAddress"))
            .unwrap_or_else(|| v.s("/displayName"));
        Ok(User {
            id: v.opt("/accountId").unwrap_or_else(|| v.s("/key")),
            name: v.opt("/displayName").unwrap_or_else(|| username.clone()),
            slug: username.clone(),
            username,
            avatar_url: v.opt("/avatarUrls/48x48"),
        })
    }

    fn issues(&self, project: &str) -> Result<Vec<Issue>> {
        self.search(&format!(
            "project = \"{}\" AND statusCategory != Done ORDER BY updated DESC",
            project_key(project)?
        ))
    }

    fn my_issues(&self, _me: &User) -> Result<Vec<Issue>> {
        self.search("assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC")
    }

    fn create_issue(&self, project: &str, title: &str, body: &str) -> Result<Issue> {
        let key = project_key(project)?;
        let created = self.api.post(
            "/rest/api/2/issue",
            &json!({ "fields": {
                "project": { "key": key },
                "summary": title,
                "description": body,
                "issuetype": { "name": "Task" },
            }}),
        )?;
        let v = self.api.get(&format!(
            "/rest/api/2/issue/{}?fields={FIELDS}",
            encode(&created.s("/key"))
        ))?;
        Ok(self.issue(&v))
    }
}

#[cfg(test)]
mod tests {
    use ronin_config::ProviderKind;
    use serde_json::json;

    use super::*;
    use crate::http::Method;
    use crate::test_support::{Fake, account, provider};

    fn issue_json() -> Value {
        json!({"id": "10001", "key": "APP-12", "fields": {
            "summary": "Crash on start", "updated": "2024-02-29T12:00:00.000+0000",
            "status": {"name": "In Progress", "statusCategory": {"key": "indeterminate"}},
            "issuetype": {"name": "Bug"}, "project": {"key": "APP"}, "labels": ["mobile"],
            "reporter": {"displayName": "Ann", "accountId": "a-1"}
        }})
    }

    #[test]
    fn cloud_uses_the_new_search() {
        let fake = Fake::new().on(
            Method::Get,
            "/search/jql",
            json!({"issues": [issue_json()]}),
        );
        let mut acct = account(ProviderKind::Jira, "https://acme.atlassian.net");
        acct.login = "me@acme.com".into();
        let jira = provider(&fake, &acct);
        let issues = jira.my_issues(&User::default()).unwrap();
        let issue = &issues[0];
        assert_eq!((issue.key.as_str(), issue.repo.as_str()), ("APP-12", "APP"));
        assert_eq!(issue.web_url, "https://acme.atlassian.net/browse/APP-12");
        assert_eq!(issue.labels, ["Bug", "In Progress", "mobile"]);
        assert_eq!(issue.updated, 1_709_208_000);
        assert!(issue.open);
        assert!(fake.urls()[0].contains("jql=assignee%20%3D%20currentUser%28%29"));
    }

    #[test]
    fn server_searches_and_creates() {
        let fake = Fake::new()
            .on(Method::Get, "/rest/api/2/search", json!({"issues": []}))
            .on(
                Method::Post,
                "/rest/api/2/issue",
                json!({"id": "10001", "key": "APP-12"}),
            )
            .on(Method::Get, "/rest/api/2/issue/APP-12", issue_json());
        let jira = provider(&fake, &account(ProviderKind::Jira, "https://jira.corp"));
        assert!(jira.issues("APP").unwrap().is_empty());
        assert!(jira.issues("APP\" OR 1=1").is_err());
        let created = jira.create_issue("APP", "Crash on start", "").unwrap();
        assert_eq!(created.key, "APP-12");
        let (request, _) = fake.sent(Method::Get, "/search");
        assert!(
            request
                .headers
                .contains(&("Authorization".into(), "Bearer tok".into()))
        );
        assert!(jira.pull_requests("APP").is_err());
    }
}
