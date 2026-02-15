mod app;
mod health;
mod settings;
mod state;
mod telemetry;
mod shutdown; 
mod metrics;
mod auth;
mod proxy;

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

// Helper Function 
fn safe_host(database_url: &str) -> String {
    if let Some(at) = database_url.find('@') {
        let (left, right) = database_url.split_at(at);
        if let Some(scheme_end) = left.find("://") {
            let scheme = &left[..scheme_end + 3];
            return format!("{scheme}***{right}");
        }
    }
    database_url.to_string()
}

// Main Function 
#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    let settings = Settings::from_env()?;
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client init failed: {e}"))?;
    telemetry::init(&settings.log_filter);
    tracing::info!(
        bind_addr = %settings.bind_addr,
        db_host = %safe_host(&settings.database_url),
        redis_url = %settings.redis_url,
        log = %settings.log_filter,
        "Starting relaykey-app"
    );

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
        .with_graceful_shutdown(shutdown::shutdown())
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}
