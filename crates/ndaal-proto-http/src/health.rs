//! Probes and metrics (API_ET_PROTOCOLES.md section 8): `/health/live`,
//! `/health/ready`, `/metrics`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use ndaal_observe::metrics::{Counter, Registry};

/// Request counter name; `route` is a fixed route or `other`, so unknown
/// paths cannot create series without bound.
const REQUESTS_METRIC: &str = "ndaal_http_requests_total";
const REQUESTS_HELP: &str = "HTTP requests by route and status.";

/// Prometheus text exposition format 0.0.4.
const METRICS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

/// The request counters, created once: no registry lookup per request.
struct Requests {
    live: Counter,
    ready: Counter,
    not_ready: Counter,
    metrics: Counter,
    other: Counter,
}

impl Requests {
    fn new(registry: &Registry) -> Self {
        let counter = |route: &str, status: &str| {
            registry
                .counter(
                    REQUESTS_METRIC,
                    REQUESTS_HELP,
                    &[("route", route), ("status", status)],
                )
                .expect("constant metric name and labels are valid and registered as a counter")
        };
        Self {
            live: counter("/health/live", "200"),
            ready: counter("/health/ready", "200"),
            not_ready: counter("/health/ready", "503"),
            metrics: counter("/metrics", "200"),
            other: counter("other", "404"),
        }
    }
}

/// What the endpoints read: readiness and the metrics registry.
#[derive(Clone)]
pub struct NodeState(Arc<Inner>);

struct Inner {
    ready: AtomicBool,
    registry: Registry,
    requests: Requests,
}

impl NodeState {
    /// A node that is not ready yet, publishing `registry` on `/metrics`.
    pub fn new(registry: Registry) -> Self {
        Self(Arc::new(Inner {
            ready: AtomicBool::new(false),
            requests: Requests::new(&registry),
            registry,
        }))
    }

    /// From now on, `/health/ready` answers 200.
    pub fn set_ready(&self) {
        self.0.ready.store(true, Ordering::Release);
    }
}

/// The routes of this module.
pub fn router(state: NodeState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
        .fallback(not_found)
        .with_state(state)
}

async fn live(State(state): State<NodeState>) -> impl IntoResponse {
    state.0.requests.live.increment(1);
    (StatusCode::OK, "ok\n")
}

async fn ready(State(state): State<NodeState>) -> impl IntoResponse {
    if state.0.ready.load(Ordering::Acquire) {
        state.0.requests.ready.increment(1);
        (StatusCode::OK, "ok\n")
    } else {
        state.0.requests.not_ready.increment(1);
        (StatusCode::SERVICE_UNAVAILABLE, "not ready\n")
    }
}

async fn metrics(State(state): State<NodeState>) -> impl IntoResponse {
    state.0.requests.metrics.increment(1);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, METRICS_CONTENT_TYPE)],
        state.0.registry.render(),
    )
}

async fn not_found(State(state): State<NodeState>) -> impl IntoResponse {
    state.0.requests.other.increment(1);
    (StatusCode::NOT_FOUND, "not found\n")
}
