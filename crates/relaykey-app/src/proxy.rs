use axum::{
    body::{Body, Bytes},
    extract::{Extension, Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use std::{sync::Arc, time::Instant};
use url::Url;

use crate::auth::VirtualKeyCtx;
use crate::state::AppState;
use crate::usage::insert_usage_event;
use crate::usage::BlockedReason;
use relaykey_db::queries::virtual_keys::{get_credential_for_partner, get_partner_by_name};

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

pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    Extension(vk): Extension<VirtualKeyCtx>,
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

    // 2) Load credential (Phase 1/2: latest header for partner)
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

    let joined = match base.join(&(forwarded_path + &query)) {
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

    // 4) Build outgoing request
    let mut out = state.http.request(method, joined);

    // Copy safe headers (blocklist approach for Phase 1/2)
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
        if name_str == "authorization" || name_str == "cookie" || name_str.starts_with("proxy-") {
            continue;
        }

        out = out.header(name, value);
    }

    // Inject upstream credential
    let header_name = match HeaderName::from_bytes(cred.header_name.as_bytes()) {
        Ok(h) => h,
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::InvalidCredentialHeaderName),
                None,
                latency_ms,
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid credential header_name",
            )
                .into_response();
        }
    };

    let header_value = match HeaderValue::from_str(&cred.header_value) {
        Ok(v) => v,
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::InvalidCredentialHeaderValue),
                None,
                latency_ms,
            )
            .await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid credential header_value",
            )
                .into_response();
        }
    };

    out = out.header(header_name, header_value);

    // Send upstream request
    let resp = match out.body(body).send().await {
        Ok(r) => r,
        Err(_) => {
            let latency_ms = start.elapsed().as_millis().min(i32::MAX as u128) as i32;
            let _ = insert_usage_event(
                &state.db,
                vk.id,
                &partner_row.name,
                uri.path(),
                false,
                Some(BlockedReason::UpstreamRequestFailed),
                None,
                latency_ms,
            )
            .await;
            return (StatusCode::BAD_GATEWAY, "upstream request failed").into_response();
        }
    };

    // 5) Return upstream response (streaming; filter headers)
    let status = resp.status();

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

    // Emit usage event (forwarded)
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

    // Stream upstream body to client (prevents buffering huge responses in memory).
    let body = Body::from_stream(resp.bytes_stream());

    (status, resp_headers, body).into_response()
}
