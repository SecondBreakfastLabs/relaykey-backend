use axum::extract::DefaultBodyLimit;
use axum::{middleware, routing::any, routing::get, Router};
use std::sync::Arc;

use crate::auth::require_virtual_key;
use crate::health::{health, ready};
use crate::metrics::metrics;
use crate::proxy::proxy_handler;
use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics));

    let protected = Router::new()
        .route("/proxy/:partner/*tail", any(proxy_handler))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_virtual_key,
        ));

    public
        .merge(protected)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .with_state(state)
}
