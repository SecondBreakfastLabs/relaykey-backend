use anyhow::{Context, Result};
use redis::AsyncCommands;
use uuid::Uuid;

use crate::state::AppState;
use relaykey_db::queries::policies::{PolicyRow, get_policy_by_id};


const POLICY_CACHE_PREFIX: &str = "rk:policy:";
const POLICY_CACHE_TTL_SECS: usize = 300; // 5 min; tweak as you like

fn cache_key(policy_id: Uuid) -> String {
    format!("{POLICY_CACHE_PREFIX}{policy_id}")
}

/// Loads a PolicyRow with a Redis JSON cache:
/// - Try Redis (rk:policy:{id})
/// - On miss: load from Postgres
/// - If found in DB: cache JSON back into Redis with TTL
///
/// Returns:
/// - Ok(Some(policy)) if found
/// - Ok(None) if policy_id doesn’t exist in DB
pub async fn load_policy_bundle(state: &AppState, policy_id: Uuid) -> Result<Option<PolicyRow>> {
    let key = cache_key(policy_id);

    // 1) Redis lookup (best-effort; fall back to DB on any redis error)
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        let cached: Option<String> = conn
            .get(&key)
            .await
            .unwrap_or(None);

        if let Some(json) = cached {
            match serde_json::from_str::<PolicyRow>(&json) {
                Ok(policy) => return Ok(Some(policy)),
                Err(e) => {
                    tracing::warn!(
                        policy_id = %policy_id,
                        error = %e,
                        "policy cache decode failed; falling back to DB"
                    );
                    // fallthrough to DB fetch
                }
            }
        }
    } else {
        tracing::warn!(policy_id = %policy_id, "policy cache redis unavailable; falling back to DB");
    }

    // 2) DB fetch
    let policy_opt = get_policy_by_id(&state.db, policy_id)
        .await
        .context("get_policy_by_id failed")?;

    // 3) Cache fill (best-effort)
    if let Some(ref policy) = policy_opt {
        if let Ok(json) = serde_json::to_string(policy) {
            if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
                // Set with TTL. Ignore errors (best-effort cache).
                let _: Result<(), _> = conn.set_ex(&key, json, POLICY_CACHE_TTL_SECS as u64).await;
            }
        }
    }

    Ok(policy_opt)
}
