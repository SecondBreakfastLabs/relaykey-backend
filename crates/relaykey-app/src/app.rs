use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::get,
    Router,
};
use std::sync::Arc;
use crate::state::AppState;
use crate::{
    auth::require_virtual_key,
    health,
    limits::middleware::enforce_limits,
};

pub fn build_router() -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready));

    let protected = Router::new()
        // .route("/proxy/:partner/*tail", any(proxy::handler))
        .route_layer(middleware::from_fn(enforce_limits))
        .route_layer(middleware::from_fn(require_virtual_key));

    public
        .merge(protected)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
