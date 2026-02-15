use hmac::{Hmac, Mac}; 
use sha2::Sha256; 

type HmacSha256 = Hmac<Sha256>;

pub fn hash_virtual_key(server_secret: &str, raw_key: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(server_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(raw_key.as_bytes());
    let bytes = mac.finalize().into_bytes();
    hex::encode(bytes)
}