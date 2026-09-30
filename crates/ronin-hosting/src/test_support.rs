//! A [`Transport`] that answers from a list of canned responses and records
//! what was asked.

use std::sync::{Arc, Mutex};

use ronin_config::{Account, ProviderKind};
use serde_json::Value;

use crate::http::{Method, Request, Response, Transport};
use crate::{Credential, Provider, Result};

#[derive(Default)]
pub struct Fake {
    routes: Mutex<Vec<(Method, String, Response)>>,
    pub requests: Mutex<Vec<Request>>,
}

impl Fake {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Answers requests whose URL contains `pattern` (the first route added
    /// that matches wins) with `body` and status 200.
    pub fn on(self: &Arc<Self>, method: Method, pattern: &str, body: Value) -> Arc<Self> {
        self.on_status(method, pattern, 200, body, &[])
    }

    pub fn on_status(
        self: &Arc<Self>,
        method: Method,
        pattern: &str,
        status: u16,
        body: Value,
        headers: &[(&str, &str)],
    ) -> Arc<Self> {
        let body = if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).unwrap()
        };
        self.routes.lock().unwrap().push((
            method,
            pattern.to_owned(),
            Response {
                status,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.to_ascii_lowercase(), (*v).to_owned()))
                    .collect(),
                body,
            },
        ));
        self.clone()
    }

    /// The last request that matched `pattern`, with its JSON body.
    pub fn sent(&self, method: Method, pattern: &str) -> (Request, Value) {
        let requests = self.requests.lock().unwrap();
        let request = requests
            .iter()
            .rev()
            .find(|r| r.method == method && r.url.contains(pattern))
            .unwrap_or_else(|| panic!("no {method:?} request to {pattern}; sent: {requests:#?}"))
            .clone();
        let body = request
            .body
            .as_deref()
            .map(|b| serde_json::from_slice(b).unwrap_or(Value::Null))
            .unwrap_or(Value::Null);
        (request, body)
    }

    pub fn urls(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| format!("{} {}", r.method.as_str(), r.url))
            .collect()
    }
}

impl Transport for Fake {
    fn send(&self, request: Request) -> Result<Response> {
        self.requests.lock().unwrap().push(request.clone());
        let routes = self.routes.lock().unwrap();
        Ok(routes
            .iter()
            .find(|(m, p, _)| *m == request.method && request.url.contains(p.as_str()))
            .map(|(_, _, r)| r.clone())
            .unwrap_or(Response {
                status: 404,
                headers: vec![],
                body: br#"{"message":"no fake route"}"#.to_vec(),
            }))
    }
}

pub fn account(kind: ProviderKind, url: &str) -> Account {
    Account {
        id: "test".into(),
        kind,
        url: url.into(),
        username: "me".into(),
        user_id: "42".into(),
        ..Account::default()
    }
}

pub fn provider(fake: &Arc<Fake>, account: &Account) -> Box<dyn Provider> {
    crate::connect(account, &Credential::token("tok"), fake.clone()).unwrap()
}
