use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::health::{health, ready};
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready)) // optional but useful
        .with_state(state)
}
