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

use relaykey_db::queries::payment_intents::{insert_payment_intent, mark_payment_intent_verified};

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
    // memo/expires later if you want
}

fn x402_enabled() -> bool {
    std::env::var("X402_ENABLED")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn x402_config() -> Option<(String, String, String, String)> {
    // amount, currency, facilitator_url, recipient
    let amount = std::env::var("X402_AMOUNT").ok()?;
    let currency = std::env::var("X402_CURRENCY").ok()?;
    let facilitator = std::env::var("X402_FACILITATOR_URL").ok()?;
    let recipient = std::env::var("X402_RECIPIENT").ok()?;
    Some((amount, currency, facilitator, recipient))
}

fn extract_payment_headers(req: &Request<Body>) -> (Option<String>, Option<String>) {
    let pid = req
        .headers()
        .get("x-payment-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let ptoken = req
        .headers()
        .get("x-payment-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (pid, ptoken)
}

/// x402 middleware:
/// - assumes VirtualKeyCtx already attached (so it must run AFTER require_virtual_key)
/// - should run AFTER enforce_limits (quota check) per Phase 7 scope
pub async fn enforce_x402(
    Extension(state): Extension<Arc<AppState>>,
    Extension(vk): Extension<VirtualKeyCtx>,
    Extension(provider): Extension<Arc<dyn PaymentProvider>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    if !x402_enabled() {
        return next.run(req).await;
    }

    let Some((amount, currency, facilitator_url, recipient)) = x402_config() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "x402 misconfigured (missing env vars)",
        )
            .into_response();
    };

    // NOTE: for now x402 protects *all* proxy calls. Later you can scope it by policy/partner/path.
    let (payment_id, payment_token) = extract_payment_headers(&req);

    // If the client provided proof, verify via provider hook
    if payment_id.is_some() || payment_token.is_some() {
        let input = VerifyInput {
            payment_id: payment_id.as_deref(),
            payment_token: payment_token.as_deref(),
            amount: &amount,
            currency: &currency,
            recipient: &recipient,
            facilitator_url: &facilitator_url,
        };

        match provider.verify(input).await {
            Ok(out) if out.verified => {
                // Optional: if you want, mark a matching intent verified later.
                // For now we just let the request through.
                return next.run(req).await;
            }
            Ok(_out) => {
                return (StatusCode::PAYMENT_REQUIRED, "payment not verified").into_response();
            }
            Err(e) => {
                tracing::error!(error = %e, "x402 verify hook failed");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "x402 verify failed",
                )
                    .into_response();
            }
        }
    }

    // No payment proof → create a payment intent and return 402 instructions.
    // We need the raw body bytes to hash; for now, hash empty if we can’t buffer safely.
    // Since this middleware runs on proxy routes, you *already* cap body size globally.
    let (parts, body) = req.into_parts();
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => {
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

    // Insert intent
    let intent_id = match insert_payment_intent(
        &state.db,
        vk.id,
        "-", // partner_name unknown here unless you parse it from the path; optional
        parts.uri.path(),
        &request_hash,
        &amount,
        &currency,
        &facilitator_url,
        &recipient,
        provider.name(),
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = %e, "failed to insert payment intent");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };

    // Rebuild req if you ever want to continue later (not needed; we return 402)
    let _ = mark_payment_intent_verified; // keep compiler quiet if unused during iteration
    let _intent_id = intent_id;

    let instructions = PaymentInstructions {
        typ: "x402",
        amount: &amount,
        currency: &currency,
        facilitator: &facilitator_url,
        recipient: &recipient,
    };

    let mut resp = (StatusCode::PAYMENT_REQUIRED, Json(instructions)).into_response();

    // Helpful x402 headers :contentReference[oaicite:4]{index=4}
    let headers = resp.headers_mut();
    headers.insert("x-payment-required", "x402".parse().unwrap());
    headers.insert("x-payment-amount", amount.parse().unwrap());
    headers.insert("x-payment-currency", currency.parse().unwrap());
    headers.insert("x-payment-facilitator", facilitator_url.parse().unwrap());

    resp
}