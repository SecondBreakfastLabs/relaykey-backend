use redis::AsyncCommands;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RetryBudgets {
    pub partner_retries_per_min: u32,
    pub vk_retries_per_min: u32,
}

impl Default for RetryBudgets {
    fn default() -> Self {
        Self {
            partner_retries_per_min: 300,
            vk_retries_per_min: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BudgetDecision {
    pub allowed: bool,
    pub reason: Option<&'static str>,
    pub partner_remaining: Option<i64>,
    pub vk_remaining: Option<i64>,
}

/// Decrement a "retries remaining this minute" counter.
/// - key is namespaced
/// - resets each minute via TTL
/// - returns remaining after decrement
async fn take_one_with_ttl(
    conn: &mut redis::aio::MultiplexedConnection,
    key: &str,
    limit: u32,
    ttl_secs: usize,
) -> redis::RedisResult<i64> {
    // Simple atomic-ish pattern:
    // - INCR to count used
    // - if first use, set EXPIRE
    // - allow if used <= limit
    //
    // This returns "remaining", not "used".
    let used: i64 = conn.incr(key, 1).await?;
    if used == 1 {
        let _: () = conn.expire(key, ttl_secs as i64).await?;
    }
    let remaining = (limit as i64) - used;
    Ok(remaining)
}

/// Require BOTH partner and vk budgets to allow a retry.
/// Fail-open: if Redis errors, we allow.
pub async fn allow_retry_dual_budget(
    redis_client: &redis::Client,
    budgets: &RetryBudgets,
    partner_name: &str,
    vk_id: Uuid,
) -> BudgetDecision {
    let ttl_secs = 60usize;

    let mut conn = match redis_client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(_e) => {
            return BudgetDecision {
                allowed: true,
                reason: Some("redis_unavailable_fail_open"),
                partner_remaining: None,
                vk_remaining: None,
            };
        }
    };

    // Keys bucketed by partner and by vk
    let partner_key = format!("rk:retry_budget:partner:{}:m", partner_name);
    let vk_key = format!("rk:retry_budget:vk:{}:m", vk_id);

    // Take from both
    let partner_remaining = match take_one_with_ttl(
        &mut conn,
        &partner_key,
        budgets.partner_retries_per_min,
        ttl_secs,
    )
    .await
    {
        Ok(r) => r,
        Err(_e) => {
            return BudgetDecision {
                allowed: true,
                reason: Some("redis_error_partner_fail_open"),
                partner_remaining: None,
                vk_remaining: None,
            };
        }
    };

    let vk_remaining = match take_one_with_ttl(
        &mut conn,
        &vk_key,
        budgets.vk_retries_per_min,
        ttl_secs,
    )
    .await
    {
        Ok(r) => r,
        Err(_e) => {
            return BudgetDecision {
                allowed: true,
                reason: Some("redis_error_vk_fail_open"),
                partner_remaining: Some(partner_remaining),
                vk_remaining: None,
            };
        }
    };

    // If either is < 0, budget exceeded (we already decremented).
    // That’s fine: this is a budget, not a limiter—exceed means no more retries.
    let allowed = partner_remaining >= 0 && vk_remaining >= 0;

    BudgetDecision {
        allowed,
        reason: if allowed { None } else { Some("retry_budget_exhausted") },
        partner_remaining: Some(partner_remaining),
        vk_remaining: Some(vk_remaining),
    }
}