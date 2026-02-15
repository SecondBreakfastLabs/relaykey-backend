use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri},
    response::IntoResponse,
};
use std::sync::Arc;
use url::Url;

use crate::state::AppState;
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
    Path((partner, tail)): Path<(String, String)>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Reject potentially dangerous methods that can enable tunneling or reflection.
    if method == Method::CONNECT || method == Method::TRACE {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    // 1) Load partner
    let partner_row = match get_partner_by_name(&state.db, &partner).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, "unknown partner").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response(),
    };

    // 2) Load credential (single for Phase 1)
    let cred = match get_credential_for_partner(&state.db, partner_row.id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "missing upstream credential",
            )
                .into_response()
        }
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response(),
    };

    // 3) Build upstream URL safely (SSRF protection)
    let base = match Url::parse(&partner_row.base_url) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid partner base_url",
            )
                .into_response()
        }
    };

    // Reconstruct path+query from incoming URI:
    let path_and_query = match uri.path_and_query() {
        Some(pq) => pq.as_str(),
        None => uri.path(),
    };

    // If someone tries to smuggle an absolute URL in the path, reject it.
    // (Defense-in-depth; should already be safe with our join strategy.)
    let tail_lc = tail.to_lowercase();
    if tail_lc.starts_with("http://") || tail_lc.starts_with("https://") {
        return (StatusCode::BAD_REQUEST, "blocked by SSRF guard").into_response();
    }

    // Ensure request path starts with /proxy/{partner}/ and we forward only the tail
    // We'll reconstruct as "/{tail}" and preserve query string from original uri.
    let forwarded_path = if tail.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", tail)
    };

    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let joined = match base.join(&(forwarded_path + &query)) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid upstream path").into_response(),
    };

    // SSRF guard: enforce origin unchanged (host/scheme must match base)
    if joined.scheme() != base.scheme()
        || joined.host_str() != base.host_str()
        || joined.port_or_known_default() != base.port_or_known_default()
    {
        return (StatusCode::BAD_REQUEST, "blocked by SSRF guard").into_response();
    }

    // 4) Build outgoing request
    let mut out = state.http.request(method, joined);

    // Copy safe headers
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();

        // never forward these
        if name_str == "host" || name_str == "x-relaykey" {
            continue;
        }
        if is_hop_by_hop(&name_str) {
            continue;
        }

        out = out.header(name, value);
    }

    // Inject upstream credential
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
    out = out.header(header_name, header_value);

    // Set body
    let resp = match out.body(body).send().await {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_GATEWAY, "upstream request failed").into_response(),
    };

    // 5) Return upstream response (filter hop-by-hop headers)
    let status = resp.status();
    let mut resp_headers = axum::http::HeaderMap::new();
    for (name, value) in resp.headers().iter() {
        let name_str = name.as_str().to_lowercase();
        if is_hop_by_hop(&name_str) {
            continue;
        }
        resp_headers.insert(name, value.clone());
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => {
            return (StatusCode::BAD_GATEWAY, "failed to read upstream response").into_response()
        }
    };

    (status, resp_headers, bytes).into_response()
}
