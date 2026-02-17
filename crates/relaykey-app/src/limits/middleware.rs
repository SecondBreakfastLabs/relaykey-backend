use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::{sync::Arc, time::Instant};
use tracing::warn;

use crate::auth::VirtualKeyCtx;
use crate::state::AppState;
use crate::usage::{insert_usage_event, BlockedReason};

use super::{monthly_quota_allow_and_incr, token_bucket_allow};

#[derive(Serialize)]
struct BlockedResp<'a> {
    code: &'a str,
}

fn parse_partner_from_path(path: &str) -> String {
    // expected: /proxy/{partner}/...
    let mut it = path.split('/');
    let _ = it.next(); // ""
    let p1 = it.next().unwrap_or("");
    if p1 != "proxy" {
        return "-".to_string();
    }
    it.next().unwrap_or("-").to_string()
}

pub async fn enforce_limits(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    let vk = match req.extensions().get::<VirtualKeyCtx>() {
        Some(vk) => vk.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "missing vk context").into_response(),
    };

    let path = req.uri().path().to_string();
    let partner_name = parse_partner_from_path(&path);

    // MultiplexedConnection is Clone; this creates a new handle.
    let mut redis_conn = state.redis.clone();

    // -----------------------
    // 1) RPS limiter (fail-open on Redis errors)
    // -----------------------
    if let Some(rps) = vk.rps_limit {
        let cap = vk.rps_burst.unwrap_or(rps).max(1);

        match token_bucket_allow(&mut redis_conn, vk.id, rps, cap).await {
            Ok(true) => {
                // allowed
            }
            Ok(false) => {
                // blocked
                let latency_ms = start.elapsed().as_millis().min(i64::from(i32::MAX)) as i32;

                let _ = insert_usage_event(
                    &state.db,
                    vk.id,
                    &partner_name,
                    &path,
                    false,
                    Some(BlockedReason::RateLimitExceeded),
                    None,
                    latency_ms,
                )
                .await;

                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(BlockedResp {
                        code: "rate_limit_exceeded",
                    }),
                )
                    .into_response();
            }
            Err(e) => {
                // FAIL-OPEN: allow request to proceed if Redis limiter is unavailable.
                warn!(
                    error = %e,
                    vk_id = %vk.id,
                    partner = %partner_name,
                    path = %path,
                    "rate limiter unavailable (fail-open)"
                );
                // no return; fall through
            }
        }
    }

    // -----------------------
    // 2) Monthly quota (fail-open on Redis errors)
    // increments only when allowed to forward
    // -----------------------
    if let Some(limit) = vk.monthly_quota {
        match monthly_quota_allow_and_incr(&mut redis_conn, vk.id, limit).await {
            Ok(true) => {
                // allowed
            }
            Ok(false) => {
                // blocked
                let latency_ms = start.elapsed().as_millis().min(i64::from(i32::MAX)) as i32;

                let _ = insert_usage_event(
                    &state.db,
                    vk.id,
                    &partner_name,
                    &path,
                    false,
                    Some(BlockedReason::MonthlyQuotaExceeded),
                    None,
                    latency_ms,
                )
                .await;

                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(BlockedResp {
                        code: "monthly_quota_exceeded",
                    }),
                )
                    .into_response();
            }
            Err(e) => {
                // FAIL-OPEN: allow request to proceed if Redis quota is unavailable.
                warn!(
                    error = %e,
                    vk_id = %vk.id,
                    partner = %partner_name,
                    path = %path,
                    "monthly quota limiter unavailable (fail-open)"
                );
                // no return; fall through
            }
        }
    }

    next.run(req).await
}
