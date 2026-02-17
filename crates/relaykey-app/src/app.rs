use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get},
    Router,
};

use crate::{
    auth::require_virtual_key,
    health,
    limits::middleware::enforce_limits,
    metrics,
    proxy,
};

pub fn build_router() -> Router<()> {
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(metrics::metrics));

    let protected = Router::new()
        .route("/proxy/:partner/*tail", any(proxy::handler))
        .route_layer(middleware::from_fn(enforce_limits))
        .route_layer(middleware::from_fn(require_virtual_key));

    public
        .merge(protected)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
