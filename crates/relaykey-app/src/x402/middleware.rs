use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::VirtualKeyCtx;
use crate::state::AppState;
use crate::x402::config::resolve_x402_config;

use relaykey_db::queries::payment_intents::{
    insert_payment_intent,
    mark_payment_intent_verified,
};

use super::{
    hash::compute_request_hash,
    provider::{PaymentProvider, VerifyInput},
};

#[derive(Serialize)]
struct PaymentInstructions<'a> {
    #[serde(rename = "type")]
    typ: &'a str, // "x402"
    amount: &'a str,
    currency: &'a str,
    facilitator: &'a str,
    recipient: &'a str,
    // TODO(x402): later add memo / expires_at / settlement metadata
}

fn extract_payment_headers(req: &Request<Body>) -> (Option<String>, Option<String>) {
    let payment_id = req
        .headers()
        .get("x-payment-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let payment_token = req
        .headers()
        .get("x-payment-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (payment_id, payment_token)
}

fn parse_partner_from_path(path: &str) -> String {
    // expected route shape: /proxy/{partner}/...
    let mut it = path.split('/');
    let _ = it.next(); // ""
    let p1 = it.next().unwrap_or("");
    if p1 != "proxy" {
        return "-".to_string();
    }
    it.next().unwrap_or("-").to_string()
}

/// x402 middleware:
/// - must run AFTER require_virtual_key
/// - should run AFTER enforce_limits (quota check)
/// - remains optional via resolve_x402_config(...)
pub async fn enforce_x402(
    Extension(state): Extension<Arc<AppState>>,
    Extension(vk): Extension<VirtualKeyCtx>,
    Extension(provider): Extension<Arc<dyn PaymentProvider>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let partner_name = parse_partner_from_path(req.uri().path());

    // Phase 8.x scoped enablement:
    // If config resolution returns None, x402 is disabled for this request.
    let Some(cfg) = resolve_x402_config(
        vk.customer_id,
        vk.id,
        &partner_name,
        req.uri().path(),
    ) else {
        return next.run(req).await;
    };

    let (payment_id, payment_token) = extract_payment_headers(&req);

    // If payment proof was supplied, verify it via the provider hook.
    if payment_id.is_some() || payment_token.is_some() {
        let input = VerifyInput {
            payment_id: payment_id.as_deref(),
            payment_token: payment_token.as_deref(),
            amount: &cfg.amount,
            currency: &cfg.currency,
            recipient: &cfg.recipient,
            facilitator_url: &cfg.facilitator_url,
        };

        match provider.verify(input).await {
            Ok(out) if out.verified => {
                // TODO(x402): once we correlate proof -> intent more directly,
                // mark the matching intent verified here instead of just passing through.
                //
                // Example future flow:
                // - lookup latest pending intent by (vk.id, request_hash) or payment_id
                // - call mark_payment_intent_verified(...)
                let _ = mark_payment_intent_verified; // keep compiler happy during iteration
                return next.run(req).await;
            }
            Ok(out) => {
                tracing::warn!(
                    vk_id = %vk.id,
                    customer_id = %vk.customer_id,
                    partner = %partner_name,
                    reason = ?out.reason,
                    "x402 payment proof not verified"
                );
                return (StatusCode::PAYMENT_REQUIRED, "payment not verified").into_response();
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    vk_id = %vk.id,
                    customer_id = %vk.customer_id,
                    partner = %partner_name,
                    "x402 verify hook failed"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "x402 verify failed",
                )
                    .into_response();
            }
        }
    }

    // No payment proof -> create a payment intent and return 402 instructions.
    // Since this middleware runs on proxy routes and body size is globally capped,
    // buffering for request hashing is acceptable here.
    let (parts, body) = req.into_parts();

    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(
                error = %e,
                vk_id = %vk.id,
                customer_id = %vk.customer_id,
                partner = %partner_name,
                "failed to read request body for x402 hashing"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to read request body",
            )
                .into_response();
        }
    };

    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(parts.uri.path());

    let request_hash = compute_request_hash(&parts.method, path_and_query, &body_bytes);

    let intent_id = match insert_payment_intent(
        &state.db,
        vk.id,
        &partner_name,
        parts.uri.path(),
        &request_hash,
        &cfg.amount,
        &cfg.currency,
        &cfg.facilitator_url,
        &cfg.recipient,
        &cfg.provider,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(
                error = %e,
                vk_id = %vk.id,
                customer_id = %vk.customer_id,
                partner = %partner_name,
                "failed to insert payment intent"
            );
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    tracing::info!(
        intent_id = %intent_id,
        vk_id = %vk.id,
        customer_id = %vk.customer_id,
        partner = %partner_name,
        path = %parts.uri.path(),
        provider = %cfg.provider,
        "x402 payment intent created"
    );

    let instructions = PaymentInstructions {
        typ: "x402",
        amount: &cfg.amount,
        currency: &cfg.currency,
        facilitator: &cfg.facilitator_url,
        recipient: &cfg.recipient,
    };

    let mut resp = (StatusCode::PAYMENT_REQUIRED, Json(instructions)).into_response();

    let headers = resp.headers_mut();
    headers.insert("x-payment-required", "x402".parse().unwrap());
    headers.insert("x-payment-amount", cfg.amount.parse().unwrap());
    headers.insert("x-payment-currency", cfg.currency.parse().unwrap());
    headers.insert(
        "x-payment-facilitator",
        cfg.facilitator_url.parse().unwrap(),
    );
    headers.insert("x-payment-provider", cfg.provider.parse().unwrap());

    // Optional/debug-friendly header so callers can correlate the challenge
    headers.insert("x-payment-intent-id", intent_id.to_string().parse().unwrap());

    resp
}