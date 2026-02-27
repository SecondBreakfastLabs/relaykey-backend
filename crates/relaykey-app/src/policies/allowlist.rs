use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
}; 
use relaykey_db::queries::policies::PolicyRow; 

fn matches(pattern: &str, path: &str) -> bool {
    // Minimal glob: 
    // - "/v1/*" matches "/v1/x", "/v1/x/y", etc.
    // - Exact matches if no wildcard
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path == prefix || path.starts_with(&(prefix.to_string() + "/"));
    }
    path == pattern
}

pub async fn enforce_allowlist(
    req: Request<axum::body::Body>, 
    next: Next, 
) -> Response {
    let policy = match req.extensions().get::<PolicyRow>() {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR, 
                "Missing policy in request extensions", 
            )
            .into_response();
        }
    };

    // Check "tail path"
    let full_path = req.uri().path(); 

    let upstream_path = full_path  
        .splitn(4, '/' )
        .nth(3)
        .map(|rest| format!("/{}", rest))
        .unwrap_or_else(|| "/".to_string()); 

    let allowed = policy 
        .endpoint_allowlist 
        .iter()
        .any(|pat| matches(pat, &upstream_path)); 

    if !allowed {
        return (StatusCode::FORBIDDEN, "Endpoint not allowed").into_response();
    }

    next.run(req).await
}