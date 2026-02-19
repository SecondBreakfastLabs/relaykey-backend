use axum::{
    Extension,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

use relaykey_core::crypto::key_hash::hash_virtual_key;
use relaykey_db::queries::admin::{insert_virtual_key, list_virtual_keys};

#[derive(Deserialize)]
pub struct CreateVirtualKeyRequest {
    pub name: String,
    pub environment: String,
    #[serde(default)]
    pub tags: Vec<String>,

    // Phase 4: required - keys must point at a policy
    pub policy_id: Uuid,
}

#[derive(Serialize)]
pub struct CreateVirtualKeyResponse {
    pub id: Uuid,
    pub key: String,
}

#[derive(Serialize)]
pub struct VirtualKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub environment: String,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub policy_id: Uuid,
    pub created_at: String, // Display-based (works across chrono/time)
}

pub async fn create_virtual_key(
    Extension(state): Extension<Arc<AppState>>,
    Json(body): Json<CreateVirtualKeyRequest>,
) -> Response {
    // Make the raw key environment-aware (nice for debugging/tests)
    let raw_key = format!("rk_{}_{}", body.environment, Uuid::new_v4());

    let key_hash = hash_virtual_key(&state.key_salt, &raw_key);

    // Phase 4: limits live on the policy, so pass None for per-key limits
    let id = match insert_virtual_key(
        &state.db,
        &body.name,
        &body.environment,
        &body.tags,
        body.policy_id,
        &key_hash,
        true,
        None, // rps_limit
        None, // rps_burst
        None, // monthly_quota
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "insert_virtual_key failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    (StatusCode::CREATED, Json(CreateVirtualKeyResponse { id, key: raw_key })).into_response()
}

pub async fn list_virtual_keys_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Response {
    match list_virtual_keys(&state.db).await {
        Ok(keys) => {
            let out: Vec<VirtualKeyResponse> = keys
                .into_iter()
                .map(|k| VirtualKeyResponse {
                    id: k.id,
                    name: k.name,
                    environment: k.environment,
                    tags: k.tags,
                    enabled: k.enabled,
                    policy_id: k.policy_id,
                    created_at: k.created_at.to_string(),
                })
                .collect();

            Json(out).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "list_virtual_keys failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
