use axum::{
    body::{Body, Bytes},
    extract::{Extension, Path},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::{sync::Arc, time::Instant};
use tokio::time::{sleep, timeout, Duration};
use url::Url;

use crate::auth::VirtualKeyCtx;
use crate::state::AppState;
use crate::usage::{insert_usage_event, BlockedReason};

// Adjust if your path differs
use relaykey_db::queries::policies::PolicyRow;
use relaykey_db::queries::virtual_keys::{get_credential_for_partner, get_partner_by_name};

use crate::retry::{
    budget::{allow_retry_dual_budget, RetryBudgets},
    classify::{classify_reqwest_error, classify_status, RetryClass},
    partner::{profile_for_partner, status_retry_allowed},
    policy::RetryPolicy,
};

static HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.contains(&name)
}

fn is_idempotent(method: &Method) -> bool {
    matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

/// Very lightweight allowlist matcher:
/// - exact match: "/get"
/// - prefix glob: "/v1/*" matches "/v1/anything"
/// - single '*' anywhere: supports simple contains/prefix use-cases (not a full glob engine)
fn path_allowed(allowlist: &[String], forwarded_path: &str) -> bool {
    if allowlist.is_empty() {
        // If you want "deny by default", change this to false.
        return true;
    }

    allowlist.iter().any(|pat| match_one(pat, forwarded_path))
}

fn match_one(pattern: &str, path: &str) -> bool {
    // Normalize: ensure forwarded path starts with '/'
    let path = if path.starts_with('/') { path } else { "/" };

    // Fast exact match
    if !pattern.contains('*') {
        return pattern == path;
    }

    // Handle common "/prefix/*"
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path.starts_with(prefix) && (path.len() == prefix.len() || path.as_bytes()[prefix.len()] == b'/');
    }

    // Very simple wildcard: split on '*' and require pieces in order
    // Example: "/v1/*/foo" => pieces ["/v1/", "/foo"]
    let parts: Vec<&str> = pattern.split('*').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return true;
    }

    let mut idx = 0usize;
    for part in parts {
        if let Some(pos) = path[idx..].find(part) {
            idx += pos + part.len();
        } else {
            return false;
        }
    }
    true
}

fn backoff_ms(attempt: usize, base: u64, cap: u64) -> u64 {
    // attempt starts at 1; exponential backoff: base * 2^(attempt-1)
    let shift = (attempt.saturating_sub(1)).min(10) as u32;
    let exp = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
    base.saturating_mul(exp).min(cap)
}

fn cheap_jitter_ms(attempt: usize) -> u64 {
    // deterministic tiny jitter (no rand dependency)
    ((attempt as u64 * 37) % 23) as u64
}

pub async fn handler(
    Extension(state): Extension<Arc<AppState>>,
    Extension(vk): Extension<VirtualKeyCtx>,
    Extension(policy): Extension<PolicyRow>,
    Path((partner, tail)): Path<(String, String)>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let start = Instant::now();

    // Reject potentially dangerous methods that can enable tunneling or reflection.
    if method == Method::CONNECT || method == Method::TRACE {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    // 1) Load partner
    let partner_row = match get_partner_by_name(&state.db, &partner).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner,
                uri.path(),
                false,
                Some(BlockedReason::UnknownPartner),
                None,
                latency_ms,
            )
            .await;
            return (StatusCode::NOT_FOUND, "unknown partner").into_response();
        }
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner,
                uri.path(),
                false,
                Some(BlockedReason::DbError),
                None,
                latency_ms,
            )
            .await;
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    // 2) Load credential (latest header for partner)
    let cred = match get_credential_for_partner(&state.db, partner_row.id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::MissingUpstreamCredential),
                None,
                latency_ms,
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "missing upstream credential",
            )
                .into_response();
        }
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::DbError),
                None,
                latency_ms,
            )
            .await;
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    // 3) Build upstream URL safely (SSRF protection)
    let base = match Url::parse(&partner_row.base_url) {
        Ok(u) => u,
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::InvalidPartnerBaseUrl),
                None,
                latency_ms,
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid partner base_url",
            )
                .into_response();
        }
    };

    // Defense-in-depth: reject URL-looking tails explicitly.
    let tail_lc = tail.to_lowercase();
    if tail_lc.starts_with("http://") || tail_lc.starts_with("https://") {
        let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let _ = insert_usage_event(
            &state.db,
            vk.id,
            &partner_row.name,
            uri.path(),
            false,
            Some(BlockedReason::SsrfBlocked),
            None,
            latency_ms,
        )
        .await;
        return (StatusCode::BAD_REQUEST, "blocked by SSRF guard").into_response();
    }

    // Forward only the captured tail; preserve query string.
    let forwarded_path = if tail.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", tail)
    };
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();

    // Phase 4: endpoint allowlist check uses forwarded path (NOT /proxy/..)
    if !path_allowed(&policy.endpoint_allowlist, &forwarded_path) {
        let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let _ = insert_usage_event(
            &state.db,
            vk.id,
            &partner_row.name,
            uri.path(),
            false,
            Some(BlockedReason::EndpointNotAllowed),
            None,
            latency_ms,
        )
        .await;

        return (StatusCode::FORBIDDEN, "Endpoint not allowed").into_response();
    }

    let joined = match base.join(&(forwarded_path.clone() + &query)) {
        Ok(u) => u,
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::InvalidUpstreamPath),
                None,
                latency_ms,
            )
            .await;
            return (StatusCode::BAD_REQUEST, "invalid upstream path").into_response();
        }
    };

    // SSRF guard: enforce origin unchanged (host/scheme/port must match base)
    if joined.scheme() != base.scheme()
        || joined.host_str() != base.host_str()
        || joined.port_or_known_default() != base.port_or_known_default()
    {
        let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
        let _ = insert_usage_event(
            &state.db,
            vk.id,
            &partner_row.name,
            uri.path(),
            false,
            Some(BlockedReason::SsrfBlocked),
            None,
            latency_ms,
        )
        .await;
        return (StatusCode::BAD_REQUEST, "blocked by SSRF guard").into_response();
    }

    // Prepare parsed credential header once
    let header_name = match HeaderName::from_bytes(cred.header_name.as_bytes()) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid credential header_name",
            )
                .into_response()
        }
    };

    let header_value = match HeaderValue::from_str(&cred.header_value) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid credential header_value",
            )
                .into_response()
        }
    };

    // -------------------------
    // Phase 5: retry loop
    // -------------------------
    let retry_policy = RetryPolicy::default();
    let partner_profile = profile_for_partner(&partner_row.name);
    let allow_retries = is_idempotent(&method);

    // Total request budget from policy
    let total_budget_ms: u64 = policy.timeout_ms.max(1) as u64;
    let deadline = Instant::now() + Duration::from_millis(total_budget_ms);

    // Retry budget defaults (can move into DB later)
    let budgets = RetryBudgets::default();

    // Helper: build reqwest request fresh each attempt (builders are one-shot)
    let build_reqwest = || {
        let mut out = state.http.request(method.clone(), joined.clone());

        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();

            // never forward these
            if name_str == "host" || name_str == "x-relaykey" || name_str == "x-request-id" {
                continue;
            }

            // drop hop-by-hop headers
            if is_hop_by_hop(&name_str) {
                continue;
            }

            // drop sensitive end-user / proxy headers
            if name_str == "authorization" || name_str == "cookie" || name_str.starts_with("proxy-")
            {
                continue;
            }

            out = out.header(name, value);
        }

        out = out.header(header_name.clone(), header_value.clone());
        out
    };

    let mut attempt: usize = 0;
    let mut retries_used: usize = 0;
    let mut budget_blocked: bool = false;

    loop {
        attempt += 1;

        // Remaining total time budget
        let now = Instant::now();
        if now >= deadline {
            return (StatusCode::GATEWAY_TIMEOUT, "upstream request timed out").into_response();
        }
        let remaining = deadline - now;

        // Send attempt with remaining budget as timeout
        let send_fut = build_reqwest().body(body.clone()).send();
        let resp_result = timeout(remaining, send_fut).await;

        match resp_result {
            // Completed with an HTTP response
            Ok(Ok(resp)) => {
                let status = resp.status();
                let axum_status =
                    StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

                let class = classify_status(axum_status);

                let can_retry_status = allow_retries
                    && class == RetryClass::Retryable
                    && status_retry_allowed(&partner_profile, axum_status)
                    && attempt < retry_policy.max_attempts;

                if can_retry_status {
                    // ---- Budget gate (BOTH partner + vk) ----
                    let decision =
                        allow_retry_dual_budget(&state.redis, &budgets, &partner_row.name, vk.id)
                            .await;

                    if !decision.allowed {
                        budget_blocked = true;
                        tracing::warn!(
                            partner = %partner_row.name,
                            vk_id = %vk.id,
                            status = %status.as_u16(),
                            attempt,
                            max_attempts = retry_policy.max_attempts,
                            partner_remaining = ?decision.partner_remaining,
                            vk_remaining = ?decision.vk_remaining,
                            reason = ?decision.reason,
                            "retry blocked by budget"
                        );
                        // Return this response without retrying further
                    } else {
                        retries_used += 1;

                        let sleep_ms = backoff_ms(
                            attempt,
                            retry_policy.base_backoff_ms,
                            retry_policy.max_backoff_ms,
                        ) + cheap_jitter_ms(attempt);

                        tracing::warn!(
                            partner = %partner_row.name,
                            vk_id = %vk.id,
                            status = %status.as_u16(),
                            attempt,
                            max_attempts = retry_policy.max_attempts,
                            backoff_ms = sleep_ms,
                            partner_remaining = ?decision.partner_remaining,
                            vk_remaining = ?decision.vk_remaining,
                            "retrying upstream request (status)"
                        );

                        sleep(Duration::from_millis(sleep_ms)).await;
                        continue;
                    }
                }

                // Return upstream response (streaming; filter headers)
                let mut resp_headers = axum::http::HeaderMap::new();
                for (name, value) in resp.headers().iter() {
                    let name_str = name.as_str().to_lowercase();
                    if is_hop_by_hop(&name_str) {
                        continue;
                    }
                    // Don't leak upstream cookies to callers.
                    if name_str == "set-cookie" {
                        continue;
                    }
                    resp_headers.insert(name, value.clone());
                }

                let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;

                let _ = insert_usage_event(
                    &state.db,
                    vk.id,
                    &partner_row.name,
                    uri.path(),
                    true,
                    None,
                    Some(status.as_u16()),
                    latency_ms,
                )
                .await;

                tracing::info!(
                    partner = %partner_row.name,
                    vk_id = %vk.id,
                    attempts = attempt,
                    retries_used = retries_used,
                    budget_blocked = budget_blocked,
                    status = status.as_u16(),
                    "proxy completed"
                );

                let body_stream = Body::from_stream(resp.bytes_stream());
                return (status, resp_headers, body_stream).into_response();
            }

            // Completed with a reqwest error
            Ok(Err(e)) => {
                let class = classify_reqwest_error(&e);

                let can_retry_err = allow_retries
                    && class == RetryClass::Retryable
                    && attempt < retry_policy.max_attempts;

                if can_retry_err {
                    // ---- Budget gate (BOTH partner + vk) ----
                    let decision =
                        allow_retry_dual_budget(&state.redis, &budgets, &partner_row.name, vk.id)
                            .await;

                    if !decision.allowed {
                        budget_blocked = true;
                        tracing::warn!(
                            partner = %partner_row.name,
                            vk_id = %vk.id,
                            attempt,
                            max_attempts = retry_policy.max_attempts,
                            error = %e,
                            partner_remaining = ?decision.partner_remaining,
                            vk_remaining = ?decision.vk_remaining,
                            reason = ?decision.reason,
                            "retry blocked by budget (reqwest error)"
                        );
                        // fall through to final error response
                    } else {
                        retries_used += 1;

                        let sleep_ms = backoff_ms(
                            attempt,
                            retry_policy.base_backoff_ms,
                            retry_policy.max_backoff_ms,
                        ) + cheap_jitter_ms(attempt);

                        tracing::warn!(
                            partner = %partner_row.name,
                            vk_id = %vk.id,
                            attempt,
                            max_attempts = retry_policy.max_attempts,
                            error = %e,
                            backoff_ms = sleep_ms,
                            partner_remaining = ?decision.partner_remaining,
                            vk_remaining = ?decision.vk_remaining,
                            "upstream error (retrying)"
                        );

                        sleep(Duration::from_millis(sleep_ms)).await;
                        continue;
                    }
                }

                tracing::warn!(
                    partner = %partner_row.name,
                    vk_id = %vk.id,
                    attempts = attempt,
                    retries_used = retries_used,
                    budget_blocked = budget_blocked,
                    error = %e,
                    "upstream request failed"
                );

                return (StatusCode::BAD_GATEWAY, "upstream request failed").into_response();
            }

            // tokio timeout elapsed (hit the remaining budget for this attempt)
            Err(_elapsed) => {
                // since we use remaining budget, this effectively means total budget expired
                tracing::warn!(
                    partner = %partner_row.name,
                    vk_id = %vk.id,
                    attempts = attempt,
                    retries_used = retries_used,
                    budget_blocked = budget_blocked,
                    "upstream request timed out"
                );
                return (StatusCode::GATEWAY_TIMEOUT, "upstream request timed out").into_response();
            }
        }
    }
}