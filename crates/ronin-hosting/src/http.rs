//! HTTP for the providers: a [`Transport`] that sends requests (ureq, or
//! recorded answers in tests) and [`Api`], which adds the service's address
//! and authentication and turns failures into [`Error`]s.

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use ronin_config::{Account, ProviderKind};
use serde_json::Value;

use crate::{Credential, Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// Header names are lowercase.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Result<Value> {
        if self.body.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&self.body).map_err(|e| Error::Decode(e.to_string()))
    }
}

/// Sends HTTP requests. Any status is a response; only failing to get one
/// is an error.
pub trait Transport: Send + Sync {
    fn send(&self, request: Request) -> Result<Response>;
}

/// The real network, trusting the platform's certificate store (so
/// self-hosted servers with a company CA work) and honouring proxy
/// variables.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        let tls = ureq::tls::TlsConfig::builder()
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("GitRonin/", env!("CARGO_PKG_VERSION")))
            .tls_config(tls)
            .build()
            .new_agent();
        Self { agent }
    }
}

impl Transport for UreqTransport {
    fn send(&self, request: Request) -> Result<Response> {
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let network = |e: ureq::Error| Error::Network(e.to_string());
        let body = request.body.unwrap_or_default();
        let http = builder
            .body(body)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let mut response = self.agent.run(http).map_err(network)?;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_ascii_lowercase(),
                    v.to_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        let mut body = Vec::new();
        response
            .body_mut()
            .with_config()
            .limit(32 * 1024 * 1024)
            .reader()
            .read_to_end(&mut body)
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Response {
            status: response.status().as_u16(),
            headers,
            body,
        })
    }
}

/// A service's API, authenticated as one account.
#[derive(Clone)]
pub struct Api {
    transport: Arc<dyn Transport>,
    /// API root, without a trailing slash.
    pub base: String,
    headers: Vec<(String, String)>,
}

impl Api {
    pub fn new(
        transport: Arc<dyn Transport>,
        account: &Account,
        credential: &Credential,
    ) -> Result<Self> {
        let url = account.url.trim_end_matches('/');
        let host = account.host();
        let base = match account.kind {
            ProviderKind::Github if host == "github.com" => "https://api.github.com".to_owned(),
            ProviderKind::Github => format!("{url}/api/v3"),
            ProviderKind::Gitlab => format!("{url}/api/v4"),
            ProviderKind::Bitbucket => "https://api.bitbucket.org/2.0".to_owned(),
            ProviderKind::BitbucketServer => format!("{url}/rest"),
            ProviderKind::AzureDevops | ProviderKind::Jira => url.to_owned(),
        };
        let token = &credential.token;
        if token.is_empty() {
            return Err(Error::Unauthorized("no token".into()));
        }
        let authorization = match account.kind {
            // Azure DevOps takes a personal access token as a password
            // with an empty user name.
            ProviderKind::AzureDevops => basic("", token),
            _ if !account.login.is_empty() => basic(&account.login, token),
            _ => format!("Bearer {token}"),
        };
        let mut headers = vec![
            ("Authorization".to_owned(), authorization),
            ("Accept".to_owned(), "application/json".to_owned()),
        ];
        if account.kind == ProviderKind::Github {
            headers[1].1 = "application/vnd.github+json".into();
            headers.push(("X-GitHub-Api-Version".into(), "2022-11-28".into()));
        }
        Ok(Self {
            transport,
            base,
            headers,
        })
    }

    /// An unauthenticated client for `base` (OAuth endpoints).
    pub fn anonymous(transport: Arc<dyn Transport>, base: &str) -> Self {
        Self {
            transport,
            base: base.trim_end_matches('/').to_owned(),
            headers: vec![("Accept".into(), "application/json".into())],
        }
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with("https://") || path.starts_with("http://") {
            path.to_owned()
        } else {
            format!("{}{path}", self.base)
        }
    }

    /// Sends a request; a status other than 2xx is an error.
    pub fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Response> {
        let mut headers = self.headers.clone();
        let body = body.map(|b| {
            headers.push(("Content-Type".into(), "application/json".into()));
            serde_json::to_vec(b).expect("JSON values serialize")
        });
        self.send_raw(method, path, headers, body)
    }

    /// Sends a form (OAuth token endpoints).
    pub fn send_form(&self, path: &str, fields: &[(&str, &str)]) -> Result<Response> {
        let response = self.form_unchecked(path, fields)?;
        if (200..300).contains(&response.status) {
            Ok(response)
        } else {
            Err(status_error(&response, &self.url(path)))
        }
    }

    /// Sends a form and returns the answer whatever its status.
    pub fn form_unchecked(&self, path: &str, fields: &[(&str, &str)]) -> Result<Response> {
        let body = fields
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut headers = self.headers.clone();
        headers.push((
            "Content-Type".into(),
            "application/x-www-form-urlencoded".into(),
        ));
        self.transport.send(Request {
            method: Method::Post,
            url: self.url(path),
            headers,
            body: Some(body.into_bytes()),
        })
    }

    /// Sends with a custom content type (Azure's JSON Patch).
    pub fn send_typed(
        &self,
        method: Method,
        path: &str,
        content_type: &str,
        body: &Value,
    ) -> Result<Value> {
        let mut headers = self.headers.clone();
        headers.push(("Content-Type".into(), content_type.into()));
        let body = serde_json::to_vec(body).expect("JSON values serialize");
        self.send_raw(method, path, headers, Some(body))?.json()
    }

    fn send_raw(
        &self,
        method: Method,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    ) -> Result<Response> {
        let url = self.url(path);
        let response = self.transport.send(Request {
            method,
            url: url.clone(),
            headers,
            body,
        })?;
        if (200..300).contains(&response.status) {
            Ok(response)
        } else {
            Err(status_error(&response, &url))
        }
    }

    pub fn get(&self, path: &str) -> Result<Value> {
        self.send(Method::Get, path, None)?.json()
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::Post, path, Some(body))?.json()
    }

    pub fn put(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::Put, path, Some(body))?.json()
    }

    pub fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        self.send(Method::Patch, path, Some(body))?.json()
    }

    /// Follows `Link: <…>; rel="next"` headers (GitHub, GitLab) until
    /// `limit` items were read.
    pub fn get_linked(&self, path: &str, limit: usize) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut next = Some(path.to_owned());
        while let Some(url) = next.take() {
            let response = self.send(Method::Get, &url, None)?;
            match response.json()? {
                Value::Array(page) => items.extend(page),
                other => return Err(Error::Decode(format!("expected a list, got {other}"))),
            }
            if items.len() >= limit {
                items.truncate(limit);
                break;
            }
            next = response.header("link").and_then(next_link);
        }
        Ok(items)
    }
}

fn basic(user: &str, password: &str) -> String {
    format!("Basic {}", base64(format!("{user}:{password}").as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Percent-encodes a path segment or query value.
pub fn encode(value: &str) -> String {
    use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
    const SET: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    utf8_percent_encode(value, SET).to_string()
}

/// The `rel="next"` target of a `Link` header.
fn next_link(header: &str) -> Option<String> {
    header.split(',').find_map(|part| {
        let (target, params) = part.split_once(';')?;
        params
            .split(';')
            .any(|p| {
                let p = p.trim().replace(' ', "");
                p == "rel=\"next\"" || p == "rel=next"
            })
            .then(|| target.trim().trim_matches(['<', '>']).to_owned())
    })
}

pub(crate) fn status_error(response: &Response, url: &str) -> Error {
    let message = response
        .json()
        .ok()
        .as_ref()
        .and_then(error_message)
        .unwrap_or_else(|| {
            let text = String::from_utf8_lossy(&response.body);
            let text = text.trim();
            if text.is_empty() || text.starts_with('<') {
                format!("HTTP {}", response.status)
            } else {
                text.chars().take(300).collect()
            }
        });
    match response.status {
        401 => Error::Unauthorized(message),
        403 => Error::Forbidden(message),
        404 => Error::NotFound(format!("{message} ({})", strip_query(url))),
        status => Error::Api { status, message },
    }
}

fn strip_query(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// The message in a service's error answer, whichever shape it has.
fn error_message(body: &Value) -> Option<String> {
    let text = |v: &Value| -> Option<String> {
        match v {
            Value::String(s) if !s.is_empty() => Some(s.clone()),
            Value::Array(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .filter_map(|i| {
                        i.as_str()
                            .map(str::to_owned)
                            .or_else(|| i.get("message").and_then(Value::as_str).map(str::to_owned))
                    })
                    .collect();
                (!parts.is_empty()).then(|| parts.join("; "))
            }
            Value::Object(map) if !map.is_empty() => Some(
                map.iter()
                    .map(|(k, v)| match v {
                        Value::String(s) => format!("{k}: {s}"),
                        Value::Array(a) => format!(
                            "{k}: {}",
                            a.iter()
                                .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_owned))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        other => format!("{k}: {other}"),
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            _ => None,
        }
    };
    let mut message = None;
    for key in [
        "message",
        "error_description",
        "errorMessages",
        "errors",
        "error",
    ] {
        let Some(value) = body.get(key) else {
            continue;
        };
        // Bitbucket: {"error": {"message": …}}.
        let value = value.get("message").unwrap_or(value);
        if let Some(found) = text(value) {
            message = Some(found);
            break;
        }
    }
    // GitHub adds the failing fields: {"message": "Validation Failed", "errors": [...]}.
    if let (Some(m), Some(errors)) = (&message, body.get("errors"))
        && body.get("message").is_some()
        && let Some(detail) = text(errors)
    {
        return Some(format!("{m}: {detail}"));
    }
    message
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b":pat"), "OnBhdA==");
    }

    #[test]
    fn finds_next_links() {
        let h = r#"<https://api.github.com/user/repos?page=2>; rel="next", <https://api.github.com/user/repos?page=5>; rel="last""#;
        assert_eq!(
            next_link(h).as_deref(),
            Some("https://api.github.com/user/repos?page=2")
        );
        assert_eq!(next_link(r#"<https://x/?page=1>; rel="prev""#), None);
    }

    #[test]
    fn reads_error_messages_of_every_service() {
        let m = |v: Value| error_message(&v).unwrap();
        assert_eq!(m(json!({"message": "Bad credentials"})), "Bad credentials");
        assert_eq!(
            m(
                json!({"message": "Validation Failed", "errors": [{"message": "A pull request already exists"}]})
            ),
            "Validation Failed: A pull request already exists"
        );
        assert_eq!(
            m(json!({"message": {"title": ["can't be blank"]}})),
            "title: can't be blank"
        );
        assert_eq!(
            m(json!({"type": "error", "error": {"message": "Repository not found"}})),
            "Repository not found"
        );
        assert_eq!(
            m(json!({"errors": [{"message": "Authentication failed"}]})),
            "Authentication failed"
        );
        assert_eq!(
            m(json!({"errorMessages": ["Issue does not exist"], "errors": {}})),
            "Issue does not exist"
        );
        assert_eq!(
            m(json!({"error": "invalid_grant", "error_description": "The grant is invalid"})),
            "The grant is invalid"
        );
    }

    #[test]
    fn maps_statuses_to_errors() {
        let response = |status: u16, body: &str| Response {
            status,
            headers: vec![],
            body: body.as_bytes().to_vec(),
        };
        assert!(matches!(
            status_error(&response(401, r#"{"message":"Bad credentials"}"#), "u"),
            Error::Unauthorized(m) if m == "Bad credentials"
        ));
        assert!(matches!(
            status_error(&response(404, ""), "https://x/y?token=1"),
            Error::NotFound(m) if m == "HTTP 404 (https://x/y)"
        ));
        assert!(matches!(
            status_error(&response(502, "<html>"), "u"),
            Error::Api { status: 502, message } if message == "HTTP 502"
        ));
    }
}
