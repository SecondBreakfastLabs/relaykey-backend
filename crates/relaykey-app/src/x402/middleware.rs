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
use crate::x402::{config::resolve_x402_config, registry::ProviderRegistry};

use relaykey_db::queries::payment_intents::{
    expire_stale_payment_intents, find_latest_pending_intent_by_request_hash,
    insert_payment_intent, mark_payment_intent_failed, mark_payment_intent_verified,
};

use super::{hash::compute_request_hash, provider::VerifyInput};

#[derive(Serialize)]
struct PaymentInstructions<'a> {
    #[serde(rename = "type")]
    typ: &'a str,
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
    Extension(provider_registry): Extension<Arc<ProviderRegistry>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let partner_name = parse_partner_from_path(req.uri().path());

    // Scoped enablement: if no config, x402 is disabled for this request.
    let Some(cfg) = resolve_x402_config(vk.customer_id, vk.id, &partner_name, req.uri().path())
    else {
        return next.run(req).await;
    };

    let provider = match provider_registry.require(&cfg.provider) {
        Ok(provider) => provider,
        Err(e) => {
            tracing::error!(
                error = %e,
                provider = %cfg.provider,
                vk_id = %vk.id,
                customer_id = %vk.customer_id,
                partner = %partner_name,
                "x402 provider not registered"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "x402 provider not configured",
            )
                .into_response();
        }
    };

    // Expire stale pending intents opportunistically.
    if let Err(e) = expire_stale_payment_intents(&state.db).await {
        tracing::warn!(
            error = %e,
            vk_id = %vk.id,
            customer_id = %vk.customer_id,
            "failed to expire stale payment intents"
        );
    }

    let (payment_id, payment_token) = extract_payment_headers(&req);

    // Buffer body once so we can:
    // - compute request hash
    // - look up matching pending intent
    // - rebuild request for upstream if verified
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

    // If payment proof was supplied, verify it and reconcile the matching pending intent.
    if payment_id.is_some() || payment_token.is_some() {
        let pending_intent = match find_latest_pending_intent_by_request_hash(
            &state.db,
            vk.id,
            &partner_name,
            parts.uri.path(),
            &request_hash,
        )
        .await
        {
            Ok(intent) => intent,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    vk_id = %vk.id,
                    customer_id = %vk.customer_id,
                    partner = %partner_name,
                    "failed to lookup pending payment intent"
                );
                return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
            }
        };

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
                if let Some(intent) = pending_intent {
                    if let Err(e) = mark_payment_intent_verified(
                        &state.db,
                        intent.id,
                        payment_id.as_deref(),
                        payment_token.as_deref(),
                    )
                    .await
                    {
                        tracing::error!(
                            error = %e,
                            intent_id = %intent.id,
                            vk_id = %vk.id,
                            customer_id = %vk.customer_id,
                            partner = %partner_name,
                            "failed to mark payment intent verified"
                        );
                        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
                    }

                    tracing::info!(
                        intent_id = %intent.id,
                        vk_id = %vk.id,
                        customer_id = %vk.customer_id,
                        partner = %partner_name,
                        provider = %cfg.provider,
                        "x402 payment intent verified"
                    );
                } else {
                    tracing::warn!(
                        vk_id = %vk.id,
                        customer_id = %vk.customer_id,
                        partner = %partner_name,
                        provider = %cfg.provider,
                        "payment verified but no matching pending intent found"
                    );
                }

                let req = Request::from_parts(parts, Body::from(body_bytes));
                return next.run(req).await;
            }
            Ok(out) => {
                if let Some(intent) = pending_intent {
                    if let Err(e) = mark_payment_intent_failed(
                        &state.db,
                        intent.id,
                        payment_id.as_deref(),
                        payment_token.as_deref(),
                    )
                    .await
                    {
                        tracing::error!(
                            error = %e,
                            intent_id = %intent.id,
                            vk_id = %vk.id,
                            customer_id = %vk.customer_id,
                            partner = %partner_name,
                            "failed to mark payment intent failed"
                        );
                        return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
                    }

                    tracing::warn!(
                        intent_id = %intent.id,
                        vk_id = %vk.id,
                        customer_id = %vk.customer_id,
                        partner = %partner_name,
                        provider = %cfg.provider,
                        reason = ?out.reason,
                        "x402 payment proof not verified; intent marked failed"
                    );
                } else {
                    tracing::warn!(
                        vk_id = %vk.id,
                        customer_id = %vk.customer_id,
                        partner = %partner_name,
                        provider = %cfg.provider,
                        reason = ?out.reason,
                        "x402 payment proof not verified; no matching pending intent found"
                    );
                }

                return (StatusCode::PAYMENT_REQUIRED, "payment not verified").into_response();
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    vk_id = %vk.id,
                    customer_id = %vk.customer_id,
                    partner = %partner_name,
                    provider = %cfg.provider,
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

    // No payment proof -> create a pending payment intent and return 402 instructions.
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
                provider = %cfg.provider,
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

    // Optional/debug-friendly header so callers can correlate the challenge.
    headers.insert(
        "x-payment-intent-id",
        intent_id.to_string().parse().unwrap(),
    );

    resp
}