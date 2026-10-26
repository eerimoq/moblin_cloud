use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use axum::extract::{MatchedPath, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use serde::Serialize;

use crate::api::Api;

type Counts = Box<[AtomicU64; 500]>;

#[derive(Default)]
pub struct Requests {
    endpoints: RwLock<HashMap<String, Counts>>,
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
        let index = usize::from(status.wrapping_sub(100));
        if index >= 500 {
            return;
        }
        let path = path.unwrap_or("*");
        if let Some(counts) = self.endpoints.read().unwrap().get(path) {
            counts[index].fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.endpoints
            .write()
            .unwrap()
            .entry(path.to_string())
            .or_insert_with(|| Box::new(std::array::from_fn(|_| AtomicU64::new(0))))[index]
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn stats(&self) -> RequestStats {
        let mut endpoints: Vec<Endpoint> = self
            .endpoints
            .read()
            .unwrap()
            .iter()
            .map(|(path, counts)| {
                let statuses: BTreeMap<u16, u64> = counts
                    .iter()
                    .enumerate()
                    .filter_map(|(index, count)| {
                        let count = count.load(Ordering::Relaxed);
                        (count > 0).then(|| (index as u16 + 100, count))
                    })
                    .collect();
                Endpoint {
                    path: path.clone(),
                    total: statuses.values().sum(),
                    statuses,
                }
            })
            .filter(|endpoint| endpoint.total > 0)
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
