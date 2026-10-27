use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use serde::Serialize;

use crate::api::Api;

#[derive(Default)]
pub struct Requests {
    endpoints: Mutex<HashMap<String, BTreeMap<u16, u64>>>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct RequestStats {
    pub total: u64,
    pub statuses: BTreeMap<u16, u64>,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Endpoint {
    pub path: String,
    pub total: u64,
    pub statuses: BTreeMap<u16, u64>,
}

impl Requests {
    pub fn record(&self, path: Option<&str>, status: u16) {
        if !(100..600).contains(&status) {
            return;
        }
        let path = path.unwrap_or("*");
        let mut endpoints = self.endpoints.lock().unwrap();
        match endpoints.get_mut(path) {
            Some(statuses) => *statuses.entry(status).or_default() += 1,
            None => {
                endpoints.insert(path.to_string(), BTreeMap::from([(status, 1)]));
            }
        }
    }

    pub fn stats(&self) -> RequestStats {
        let mut endpoints: Vec<Endpoint> = self
            .endpoints
            .lock()
            .unwrap()
            .iter()
            .map(|(path, statuses)| Endpoint {
                path: path.clone(),
                total: statuses.values().sum(),
                statuses: statuses.clone(),
            })
            .collect();
        endpoints.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.path.cmp(&b.path)));
        let mut statuses = BTreeMap::new();
        for endpoint in &endpoints {
            for (status, count) in &endpoint.statuses {
                *statuses.entry(*status).or_default() += count;
            }
        }
        RequestStats {
            total: endpoints.iter().map(|endpoint| endpoint.total).sum(),
            statuses,
            endpoints,
        }
    }
}

pub async fn count(State(api): State<Arc<Api>>, request: Request, next: Next) -> Response {
    let path = request.extensions().get::<MatchedPath>().cloned();
    let response = next.run(request).await;
    api.requests.record(
        path.as_ref().map(MatchedPath::as_str),
        response.status().as_u16(),
    );
    response
}
