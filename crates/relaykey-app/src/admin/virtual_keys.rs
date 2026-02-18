use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
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
    pub rps_limit: Option<i32>,
    pub rps_burst: Option<i32>,
    pub monthly_quota: Option<i32>,
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
    pub rps_limit: Option<i32>,
    pub rps_burst: Option<i32>,
    pub monthly_quota: Option<i32>,
    pub created_at: String, // simplest cross-crate representation
}

pub async fn create_virtual_key(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateVirtualKeyRequest>,
) -> impl IntoResponse {
    // Generate raw key (you probably want environment-aware; leaving your current logic intact)
    let raw_key = format!("rk_live_{}", Uuid::new_v4());

    let key_hash = hash_virtual_key(&state.key_salt, &raw_key);

    let id = match insert_virtual_key(
        &state.db,
        &body.name,
        &body.environment,
        &body.tags,
        &key_hash,
        true,
        body.rps_limit,
        body.rps_burst,
        body.monthly_quota,
    )
    .await
    {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Json(CreateVirtualKeyResponse { id, key: raw_key }).into_response()
}

pub async fn list_virtual_keys_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
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
                    rps_limit: k.rps_limit,
                    rps_burst: k.rps_burst,
                    monthly_quota: k.monthly_quota,
                    created_at: k.created_at.to_rfc3339(), // if created_at is chrono::DateTime
                })
                .collect();

            Json(out).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}


