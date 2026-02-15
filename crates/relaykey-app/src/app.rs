use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use axum::extract::DefaultBodyLimit; 

use crate::health::{health, ready};
use crate::metrics::metrics;
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready)) // optional but useful
        .route("/metrics", get(metrics)) // placeholder for now
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(state)
}

pub fn build_public_router() -> Router {
    Router::new()
        .route("/health", axum::routing::get(crate::health::health))
        .route("/metrics", axum::routing::get(crate::metrics::metrics))
}

