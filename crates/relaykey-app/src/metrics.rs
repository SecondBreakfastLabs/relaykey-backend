use axum::{http::StatusCode, response::IntoResponse};

pub async fn metrics() -> impl IntoResponse {
    // Placeholder for now
    (StatusCode::OK, "Metrics not implemented yet")
}
