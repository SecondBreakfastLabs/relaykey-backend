use axum::{http::StatusCode, response::IntoResponse, Extension};
use std::sync::Arc;

use crate::state::AppState;

pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn ready(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
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
    let mut conn = match state.redis.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("redis not ready: {e}"),
            )
        }
    };

    if let Err(e) = redis::cmd("PING").query_async::<_, String>(&mut conn).await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("redis not ready: {e}"),
        );
    }

    (StatusCode::OK, "ready".to_string())
}
