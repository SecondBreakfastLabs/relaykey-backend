use axum::{
    Extension,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use uuid::Uuid;
use relaykey_core::crypto::key_hash::hash_virtual_key;
use relaykey_db::queries::virtual_keys::get_virtual_key_by_hash;

use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct VirtualKeyCtx {
    pub id: Uuid,
    pub name: String,
    pub environment: String, 
    pub tags: Vec<String>, 
    pub rps_limit: Option<i32>,
    pub rps_burst: Option<i32>,
    pub monthly_quota: Option<i32>,
}

pub async fn require_virtual_key(
    Extension(state): Extension<Arc<AppState>>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let raw = match req
        .headers()
        .get("x-relaykey")
        .and_then(|v| v.to_str().ok())
    {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => return (StatusCode::UNAUTHORIZED, "missing x-relaykey").into_response(),
    };

    let key_hash = hash_virtual_key(&state.key_salt, &raw);

    let vk = match get_virtual_key_by_hash(&state.db, &key_hash).await {
        Ok(Some(vk)) if vk.enabled => vk,
        Ok(Some(_)) => return (StatusCode::UNAUTHORIZED, "virtual key disabled").into_response(),
        Ok(None) => return (StatusCode::UNAUTHORIZED, "invalid virtual key").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response(),
    };

    req.extensions_mut().insert(VirtualKeyCtx {
        id: vk.id,
        name: vk.name,
        environment: vk.environment, 
        tags: vk.tags, 
        rps_limit: vk.rps_limit,
        rps_burst: vk.rps_burst,
        monthly_quota: vk.monthly_quota,
    });

    next.run(req).await
}

pub async fn require_admin(
    Extension(state): Extension<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
  ) -> Response {
    let expected = std::env::var("ADMIN_TOKEN").ok();
    let Some(expected) = expected else {
      return (StatusCode::INTERNAL_SERVER_ERROR, "admin not configured").into_response();
    };
  
    let token = req.headers()
      .get("x-admin-token")
      .and_then(|v| v.to_str().ok())
      .unwrap_or("");
  
    if token != expected {
      return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
  
    next.run(req).await
  }
  