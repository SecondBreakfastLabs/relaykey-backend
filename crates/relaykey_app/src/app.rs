use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
    Router,
};
use std::sync::Arc; 

use crate::{
    auth::{require_admin, require_virtual_key},
    admin::{virtual_keys, usage, errors},
    health,
    limits::middleware::enforce_limits,
    metrics,
    proxy,
    policies::allowlist::enforce_allowlist,
    x402::{
        noop::NoopProvider,
        stub::StubProvider, 
        registry::ProviderRegistry,
    }
};

pub fn build_router() -> Router<()> {
    let provider_registry = Arc::new(
        ProviderRegistry::new()
            .register("noop", Arc::new(NoopProvider::default()))
            .register("stub", Arc::new(StubProvider))
    );

    let public = Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/metrics", get(metrics::metrics));

    let protected = Router::new()
        .route("/proxy/:partner/*tail", any(proxy::handler))
        .route_layer(middleware::from_fn(crate::x402::middleware::enforce_x402))
        .route_layer(middleware::from_fn(enforce_limits))
        .route_layer(middleware::from_fn(enforce_allowlist))
        .route_layer(middleware::from_fn(require_virtual_key));

    let admin = Router::new()
        .route(
            "/admin/virtual-keys",
            post(virtual_keys::create_virtual_key)
                .get(virtual_keys::list_virtual_keys_handler),
        )
        .route("/admin/usage", get(usage::admin_usage))
        .route("/admin/errors", get(errors::admin_errors))
        .route_layer(middleware::from_fn(require_admin));

    public
        .merge(protected)
        .merge(admin) 
        .layer(axum::Extension(provider_registry))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
}
