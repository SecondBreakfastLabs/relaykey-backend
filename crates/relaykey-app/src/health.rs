use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

use crate::state::AppState;

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

// "ready" checks dependencies; keep it light.
pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Postgres ping
    if let Err(e) = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        return (StatusCode::SERVICE_UNAVAILABLE, format!("db not ready: {e}"));
    }

    // Redis ping
    if let Err(e) = redis::cmd("PING").query_async::<_, String>(&mut state.redis.clone()).await {
        return (StatusCode::SERVICE_UNAVAILABLE, format!("redis not ready: {e}"));
    }

    (StatusCode::OK, "ready".to_string())
}
