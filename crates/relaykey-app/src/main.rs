use axum::{http::Request, Extension};
use std::{sync::Arc, time::Duration};

use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use relaykey_db::{init_db, init_redis};
use relaykey_app::settings::Settings;
use relaykey_app::state::AppState;

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    let settings = Settings::from_env()?;

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client init failed: {e}"))?;

    relaykey_app::telemetry::init(&settings.log_filter);

    let db = init_db(&settings.database_url)
        .await
        .map_err(|e| format!("DB init failed: {e}"))?;

    let redis = init_redis(&settings.redis_url)
        .await
        .map_err(|e| format!("Redis init failed: {e}"))?;

    let state = Arc::new(AppState {
        db,
        redis,
        http,
        key_salt: settings.key_salt.clone(),
    });

    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http().make_span_with(|req: &Request<_>| {
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

    // build Router<()> then attach Extension(state)
    let app = relaykey_app::app::build_router()
        .layer(Extension(state))
        .layer(middleware);

    let listener = tokio::net::TcpListener::bind(settings.bind_addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {e}", settings.bind_addr))?;

    tracing::info!("Listening on {}", settings.bind_addr);

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(relaykey_app::shutdown::shutdown())
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}
