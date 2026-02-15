use axum::{routing::get, routing::any, Router};
use axum::middleware;
use axum::extract::DefaultBodyLimit;
use std::sync::Arc;

use crate::auth::require_virtual_key;
use crate::health::{health, ready};
use crate::metrics::metrics;
use crate::proxy::proxy_handler;
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/proxy/:partner/*tail", any(proxy_handler))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_virtual_key))
        .with_state(state)
}

pub fn build_public_router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(crate::health::health))
        .route("/metrics", axum::routing::get(crate::metrics::metrics))
}

