use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

use crate::state::AppState;

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Postgres ping
    if let Err(e) = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("db not ready: {e}"),
        );
    }

    // Redis ping
    {
        let mut conn = state.redis.clone();
        let pong: String = match redis::cmd("PING").query_async(&mut conn).await {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    format!("redis not ready: {e}"),
                )
            }
        };
        let _ = pong;
    }

    (StatusCode::OK, "ready".to_string())
}
