use axum::{
  body::Body,
  http::{Request, StatusCode},
  middleware::Next,
  response::{IntoResponse, Response},
  Extension,
};
use std::sync::Arc;
use uuid::Uuid;

use relaykey_core::crypto::key_hash::hash_virtual_key;
use relaykey_db::queries::virtual_keys::get_virtual_key_by_hash;
use relaykey_db::queries::policies::PolicyRow;
use crate::policies::cache::load_policy_bundle;

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
  pub policy_id: Uuid,
  pub policy: PolicyRow,
}

pub async fn require_virtual_key(
  Extension(state): Extension<Arc<AppState>>,
  mut req: Request<axum::body::Body>,
  next: Next,
) -> Response {
  // 1) Read raw key
  let raw = match req
      .headers()
      .get("x-relaykey")
      .and_then(|v| v.to_str().ok())
      .map(str::trim)
  {
      Some(v) if !v.is_empty() => v.to_string(),
      _ => return (StatusCode::UNAUTHORIZED, "missing x-relaykey").into_response(),
  };

  // 2) Hash and lookup virtual key
  let key_hash = hash_virtual_key(&state.key_salt, &raw);

  let vk = match get_virtual_key_by_hash(&state.db, &key_hash).await {
      Ok(Some(vk)) if vk.enabled => vk,
      Ok(Some(_)) => return (StatusCode::UNAUTHORIZED, "virtual key disabled").into_response(),
      Ok(None) => return (StatusCode::UNAUTHORIZED, "invalid virtual key").into_response(),
      Err(e) => {
          tracing::error!(error = %e, "db error loading virtual key");
          return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
      }
  };

  // 3) Enforce: virtual key must have a policy_id in Phase 4
  let policy_id = vk.policy_id;

  // 4) Load policy bundle (cache -> DB)
  let policy = match load_policy_bundle(&state, policy_id).await {
      Ok(Some(p)) => p,
      Ok(None) => {
          return (StatusCode::INTERNAL_SERVER_ERROR, "policy not found").into_response()
      }
      Err(e) => {
          tracing::error!(error = %e, policy_id = %policy_id, "policy load failed");
          return (StatusCode::INTERNAL_SERVER_ERROR, "policy load failed").into_response();
      }
  };

  // 5) Attach both the policy and the VK context to request extensions
  // (In handlers/middleware you can extract either one, whichever is cleaner.)
  req.extensions_mut().insert(policy.clone());

  req.extensions_mut().insert(VirtualKeyCtx {
      id: vk.id,
      name: vk.name,
      environment: vk.environment,
      tags: vk.tags,
      rps_limit: policy.rps_limit,
      rps_burst: policy.rps_burst,
      monthly_quota: policy.monthly_quota,
      policy_id,
      policy,
  });

  next.run(req).await
}

pub async fn require_admin(
  Extension(_state): Extension<Arc<AppState>>,
  req: Request<Body>,
  next: Next,
) -> Response {
  let expected = std::env::var("ADMIN_TOKEN").ok();
  let Some(expected) = expected else {
      return (StatusCode::INTERNAL_SERVER_ERROR, "admin not configured").into_response();
  };

  let token = req
      .headers()
      .get("x-admin-token")
      .and_then(|v| v.to_str().ok())
      .unwrap_or("");

  if token != expected {
      return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
  }

  next.run(req).await
}
