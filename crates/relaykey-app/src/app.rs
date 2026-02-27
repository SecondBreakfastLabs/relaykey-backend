use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
    Router,
};

use crate::{
    auth::{require_admin, require_virtual_key},
    admin::virtual_keys,
    health,
    limits::middleware::enforce_limits,
    metrics,
    proxy,
    policies::allowlist::enforce_allowlist,
};


pub fn build_router() -> Router<()> {
    let public = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(metrics::metrics));

    let protected = Router::new()
        .route("/proxy/:partner/*tail", any(proxy::handler))
        .route_layer(middleware::from_fn(enforce_limits))
        .route_layer(middleware::from_fn(enforce_allowlist))
        .route_layer(middleware::from_fn(require_virtual_key));

    let admin = Router::new()
        .route(
            "/admin/virtual-keys",
            post(virtual_keys::create_virtual_key)
                .get(virtual_keys::list_virtual_keys_handler),
        )
        .route_layer(middleware::from_fn(require_admin));

    public
        .merge(protected)
        .merge(admin) 
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
