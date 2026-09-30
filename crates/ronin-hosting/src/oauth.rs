//! OAuth 2.0 device authorization (RFC 8628): the app shows a code, the
//! user enters it in the browser, and the app polls for the token. Needs
//! no server and no client secret, only the public client id of an OAuth
//! application registered with the service. GitHub and GitLab support it.

use std::sync::Arc;

use ronin_config::ProviderKind;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::http::{Api, Transport};
use crate::json::Json;
use crate::{Credential, Error, Result};

/// Client ids built in at compile time, for the public services. Anyone
/// can register their own and enter its id when signing in.
pub fn default_client_id(kind: ProviderKind, url: &str) -> Option<&'static str> {
    let host = ronin_config::accounts::host_of(url);
    let id = match (kind, host.as_str()) {
        (ProviderKind::Github, "github.com") => option_env!("RONIN_GITHUB_CLIENT_ID"),
        (ProviderKind::Gitlab, "gitlab.com") => option_env!("RONIN_GITLAB_CLIENT_ID"),
        _ => None,
    };
    id.filter(|id| !id.is_empty())
}

/// What the user needs to authorize the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceCode {
    #[serde(skip)]
    #[ts(skip)]
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    /// The verification page with the code already filled in, if offered.
    pub verification_uri_complete: Option<String>,
    #[ts(type = "number")]
    pub expires_in: u64,
    /// Seconds to wait between polls.
    #[ts(type = "number")]
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    /// Not authorized yet; wait `interval` seconds (it grows on `slow_down`).
    Pending {
        interval: u64,
    },
    Done(Credential),
}

struct Endpoints {
    device: String,
    token: String,
    scope: &'static str,
}

fn endpoints(kind: ProviderKind, url: &str) -> Result<Endpoints> {
    let url = url.trim_end_matches('/');
    match kind {
        ProviderKind::Github => Ok(Endpoints {
            device: format!("{url}/login/device/code"),
            token: format!("{url}/login/oauth/access_token"),
            scope: "repo read:org read:user write:public_key",
        }),
        ProviderKind::Gitlab => Ok(Endpoints {
            device: format!("{url}/oauth/authorize_device"),
            token: format!("{url}/oauth/token"),
            scope: "api",
        }),
        _ => Err(Error::Unsupported("signing in through the browser")),
    }
}

/// Asks the service for a code the user enters at `verification_uri`.
pub fn start(
    transport: Arc<dyn Transport>,
    kind: ProviderKind,
    url: &str,
    client_id: &str,
) -> Result<DeviceCode> {
    let endpoints = endpoints(kind, url)?;
    let api = Api::anonymous(transport, "");
    let answer = api
        .send_form(
            &endpoints.device,
            &[("client_id", client_id), ("scope", endpoints.scope)],
        )?
        .json()?;
    if let Some(error) = oauth_error(&answer) {
        return Err(error);
    }
    let code = DeviceCode {
        device_code: answer.s("/device_code"),
        user_code: answer.s("/user_code"),
        verification_uri: answer
            .opt("/verification_uri")
            .or_else(|| answer.opt("/verification_url"))
            .unwrap_or_default(),
        verification_uri_complete: answer.opt("/verification_uri_complete"),
        expires_in: answer.n("/expires_in").max(60),
        interval: answer.n("/interval").max(1),
    };
    if code.device_code.is_empty() || code.user_code.is_empty() {
        return Err(Error::Decode("no device code in the answer".into()));
    }
    Ok(code)
}

/// Asks once whether the user has authorized the app.
pub fn poll(
    transport: Arc<dyn Transport>,
    kind: ProviderKind,
    url: &str,
    client_id: &str,
    code: &DeviceCode,
    now: u64,
) -> Result<Poll> {
    let endpoints = endpoints(kind, url)?;
    let api = Api::anonymous(transport, "");
    let form = [
        ("client_id", client_id),
        ("device_code", code.device_code.as_str()),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ];
    // GitLab answers a pending authorization with 400, GitHub with 200;
    // either way the OAuth error is in the body.
    let response = api.form_unchecked(&endpoints.token, &form)?;
    let answer = match response.json() {
        Ok(answer) if answer.get("error").is_some() || response.status < 300 => answer,
        _ => return Err(crate::http::status_error(&response, &endpoints.token)),
    };
    match answer.opt("/error").as_deref() {
        Some("authorization_pending") => Ok(Poll::Pending {
            interval: code.interval,
        }),
        Some("slow_down") => Ok(Poll::Pending {
            interval: answer.n("/interval").max(code.interval + 5),
        }),
        Some(_) => Err(oauth_error(&answer).expect("an error field is present")),
        None => credential(&answer, now).map(Poll::Done),
    }
}

/// Renews an expiring token with its refresh token (GitLab's tokens last
/// two hours).
pub fn refresh(
    transport: Arc<dyn Transport>,
    kind: ProviderKind,
    url: &str,
    client_id: &str,
    old: &Credential,
    now: u64,
) -> Result<Credential> {
    let endpoints = endpoints(kind, url)?;
    let api = Api::anonymous(transport, "");
    let form = [
        ("client_id", client_id),
        ("refresh_token", old.refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];
    let answer = api
        .send_form(&endpoints.token, &form)
        .map_err(|e| match e {
            Error::Api { status: 400, .. } => Error::Unauthorized("the sign-in expired".into()),
            e => e,
        })?
        .json()?;
    if let Some(error) = oauth_error(&answer) {
        return Err(error);
    }
    let mut new = credential(&answer, now)?;
    if new.refresh_token.is_empty() {
        new.refresh_token = old.refresh_token.clone();
    }
    Ok(new)
}

fn credential(answer: &Value, now: u64) -> Result<Credential> {
    let token = answer.s("/access_token");
    if token.is_empty() {
        return Err(Error::Decode("no access token in the answer".into()));
    }
    let expires_in = answer.n("/expires_in");
    Ok(Credential {
        token,
        refresh_token: answer.s("/refresh_token"),
        expires_at: if expires_in == 0 { 0 } else { now + expires_in },
    })
}

fn oauth_error(answer: &Value) -> Option<Error> {
    let error = answer.opt("/error")?;
    let description = answer.opt("/error_description");
    Some(match error.as_str() {
        "access_denied" => Error::Invalid("the authorization was declined".into()),
        "expired_token" => Error::Invalid("the code expired; start again".into()),
        "incorrect_client_credentials" | "invalid_client" => Error::Invalid(format!(
            "the service doesn't know this OAuth application{}",
            description.map(|d| format!(": {d}")).unwrap_or_default()
        )),
        "device_flow_disabled" => Error::Invalid(
            "the OAuth application doesn't allow device sign-in; enable “Device Flow” in its settings"
                .into(),
        ),
        _ => Error::Invalid(description.unwrap_or(error)),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::http::Method;
    use crate::test_support::Fake;

    #[test]
    fn github_device_flow() {
        let fake = Fake::new().on(
            Method::Post,
            "/login/device/code",
            json!({"device_code": "dc", "user_code": "ABCD-1234",
                   "verification_uri": "https://github.com/login/device",
                   "expires_in": 900, "interval": 5}),
        );
        let code = start(
            fake.clone(),
            ProviderKind::Github,
            "https://github.com",
            "cid",
        )
        .unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        let (request, _) = fake.sent(Method::Post, "/login/device/code");
        let body = String::from_utf8(request.body.unwrap()).unwrap();
        assert!(
            body.starts_with("client_id=cid&scope=repo%20read%3Aorg"),
            "{body}"
        );

        let pending = Fake::new().on(
            Method::Post,
            "/login/oauth/access_token",
            json!({"error": "slow_down", "interval": 10}),
        );
        let p = poll(
            pending,
            ProviderKind::Github,
            "https://github.com",
            "cid",
            &code,
            0,
        );
        assert_eq!(p.unwrap(), Poll::Pending { interval: 10 });

        let done = Fake::new().on(
            Method::Post,
            "/login/oauth/access_token",
            json!({"access_token": "gho_x", "token_type": "bearer", "scope": "repo"}),
        );
        let p = poll(
            done,
            ProviderKind::Github,
            "https://github.com",
            "cid",
            &code,
            0,
        );
        assert_eq!(p.unwrap(), Poll::Done(Credential::token("gho_x")));

        let denied = Fake::new().on(
            Method::Post,
            "/login/oauth/access_token",
            json!({"error": "access_denied"}),
        );
        let p = poll(
            denied,
            ProviderKind::Github,
            "https://github.com",
            "cid",
            &code,
            0,
        );
        assert!(p.unwrap_err().to_string().contains("declined"));
    }

    #[test]
    fn gitlab_device_flow_and_refresh() {
        let code = DeviceCode {
            device_code: "dc".into(),
            user_code: "X".into(),
            verification_uri: String::new(),
            verification_uri_complete: None,
            expires_in: 300,
            interval: 5,
        };
        let pending = Fake::new().on_status(
            Method::Post,
            "/oauth/token",
            400,
            json!({"error": "authorization_pending", "error_description": "…"}),
            &[],
        );
        let url = "http://localhost:8929";
        let p = poll(pending, ProviderKind::Gitlab, url, "cid", &code, 0);
        assert_eq!(p.unwrap(), Poll::Pending { interval: 5 });

        let done = Fake::new().on(
            Method::Post,
            "/oauth/token",
            json!({"access_token": "a", "refresh_token": "r", "expires_in": 7200}),
        );
        let p = poll(done, ProviderKind::Gitlab, url, "cid", &code, 1000).unwrap();
        let Poll::Done(credential) = p else {
            panic!("not done")
        };
        assert_eq!(credential.expires_at, 8200);
        assert!(credential.expiring(8150));
        assert!(!credential.expiring(1000));

        let renewed = Fake::new().on(
            Method::Post,
            "/oauth/token",
            json!({"access_token": "b", "expires_in": 7200}),
        );
        let new = refresh(
            renewed.clone(),
            ProviderKind::Gitlab,
            url,
            "cid",
            &credential,
            9000,
        )
        .unwrap();
        assert_eq!((new.token.as_str(), new.refresh_token.as_str()), ("b", "r"));
        let (request, _) = renewed.sent(Method::Post, "/oauth/token");
        let body = String::from_utf8(request.body.unwrap()).unwrap();
        assert!(body.contains("grant_type=refresh_token"), "{body}");
    }
}
