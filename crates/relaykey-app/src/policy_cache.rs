use std::sync::Arc; 
use uuid::Uuid; 
use redis::AsyncCommands; 
use relaykey_db::queries::policies::{get_policy_by_id, PolicyRow};
use crate::state::AppState;

const POLICY_CACHE_TTL_SECONDS: usize = 60; 

pub async fn load_policy_bundle(
    state: &Arc<AppState>, 
    policy_id: Uuid
) -> Result<Option<PolicyRow>, sqlx::Error> {
    let cache_key = format!("rk:policy:{policy_id}"); 

    // Try Redis 
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        let cached: Option<String> = conn.get(&cache_key).await.unwrap_or(None); 
        if let Some(json) = cached {
            if let Ok(p) = serde_json::from_str::<PolicyRow>(&json) {
                return Ok(Some(p)); 
            }
        } 
    }

    // Fallback to DB 
    let policy = get_policy_by_id(&state.db, policy_id).await?; 

    // Best effort cache write 
    if let Some(ref p) = policy {
        if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
            if let Ok(json) = serde_json::to_string(p) {
                let _: Result<(), _> = conn 
                    .set_ex(&cache_key, json, POLICY_CACHE_TTL_SECONDS)
                    .await; 
            }
        }
    }

    Ok(policy)
}