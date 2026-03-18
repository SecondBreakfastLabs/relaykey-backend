use axum::http::Method;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

pub fn compute_request_hash(method: &Method, path_and_query: &str, body: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(method.as_str().as_bytes());
    h.update(b"\n");
    h.update(path_and_query.as_bytes());
    h.update(b"\n");
    h.update(body);
    let out = h.finalize();
    base64::Engine::encode(&URL_SAFE_NO_PAD, &out)
}
