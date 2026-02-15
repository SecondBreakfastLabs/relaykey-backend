use relaykey_core::crypto::key_hash::hash_virtual_key;
use uuid::Uuid;

fn main() {
    let salt = std::env::var("RELAYKEY_KEY_SALT").expect("RELAYKEY_KEY_SALT is required");

    let raw = format!("vk_{}", Uuid::new_v4());
    let hash = hash_virtual_key(&salt, &raw);

    println!("RAW_VK={raw}");
    println!("VK_HASH={hash}");
}
