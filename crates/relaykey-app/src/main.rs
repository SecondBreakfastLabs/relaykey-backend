mod app;
mod health;
mod settings;
mod state;
mod telemetry;

use axum::http::Request;
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
    timeout::TimeoutLayer,
};

use relaykey_db::{init_db, init_redis};
use settings::Settings;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    let settings = Settings::from_env()?;
    telemetry::init(&settings.log_filter);

    let db = init_db(&settings.database_url)
        .await
        .map_err(|e| format!("DB init failed: {e}"))?;

    let redis = init_redis(&settings.redis_url)
        .await
        .map_err(|e| format!("Redis init failed: {e}"))?;

    let state = Arc::new(AppState { db, redis });

    let middleware = ServiceBuilder::new()
        // set a request id if missing
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        // propagate it to responses
        .layer(PropagateRequestIdLayer::x_request_id())
        // tracing for requests
        .layer(
            TraceLayer::new_for_http()
            .make_span_with(|req: &Request<_>| {
                let request_id = req
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-");
                tracing::info_span!(
                    "http_request",
                    method = %req.method(),
                    uri = %req.uri(),
                    request_id = %request_id
                )
            }),
        )
        .layer(TimeoutLayer::new(Duration::from_secs(30)));

    let app = app::build_router(state).layer(middleware);

    let listener = tokio::net::TcpListener::bind(settings.bind_addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {e}", settings.bind_addr))?;

    tracing::info!("Listening on {}", settings.bind_addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}
