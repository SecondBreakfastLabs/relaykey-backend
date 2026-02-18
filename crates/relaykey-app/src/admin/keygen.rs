use rand::{rngs::OsRng, RngCore};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

pub fn generate_virtual_key(environment: &str) -> String {

    let mut bytes = [0u8; 24];

    OsRng.fill_bytes(&mut bytes);

    let suffix = URL_SAFE_NO_PAD.encode(bytes);

    format!("rk_{}_{}", environment, suffix)
}
