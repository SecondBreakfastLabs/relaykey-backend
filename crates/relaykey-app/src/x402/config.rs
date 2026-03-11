use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct X402Config {
    pub enabled: bool,
    pub amount: String,
    pub currency: String,
    pub facilitator_url: String,
    pub recipient: String,
    pub provider: String,

    #[serde(default)]
    pub path_prefixes: Vec<String>,
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn parse_override_map(env_name: &str) -> Option<HashMap<String, X402Config>> {
    let raw = std::env::var(env_name).ok()?;
    serde_json::from_str(&raw).ok()
}

fn path_matches(cfg: &X402Config, path: &str) -> bool {
    if cfg.path_prefixes.is_empty() {
        return true;
    }

    cfg.path_prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn global_default() -> Option<X402Config> {
    if !env_bool("X402_ENABLED") {
        return None;
    }

    let amount = std::env::var("X402_AMOUNT").ok()?;
    let currency = std::env::var("X402_CURRENCY").ok()?;
    let facilitator_url = std::env::var("X402_FACILITATOR_URL").ok()?;
    let recipient = std::env::var("X402_RECIPIENT").ok()?;
    let provider = std::env::var("X402_PROVIDER").unwrap_or_else(|_| "noop".to_string());

    Some(X402Config {
        enabled: true,
        amount,
        currency,
        facilitator_url,
        recipient,
        provider,
        path_prefixes: vec![],
    })
}

/// Scoped resolution precedence:
/// 1. virtual key override
/// 2. customer/org override
/// 3. partner override
/// 4. global default
pub fn resolve_x402_config(
    customer_id: Uuid,
    virtual_key_id: Uuid,
    partner_name: &str,
    path: &str,
) -> Option<X402Config> {
    let vk_key = virtual_key_id.to_string();
    let customer_key = customer_id.to_string();

    // 1) virtual key override
    if let Some(map) = parse_override_map("X402_VIRTUAL_KEY_OVERRIDES") {
        if let Some(cfg) = map.get(&vk_key) {
            if path_matches(cfg, path) {
                return if cfg.enabled { Some(cfg.clone()) } else { None };
            }
        }
    }

    // 2) customer/org override
    if let Some(map) = parse_override_map("X402_CUSTOMER_OVERRIDES") {
        if let Some(cfg) = map.get(&customer_key) {
            if path_matches(cfg, path) {
                return if cfg.enabled { Some(cfg.clone()) } else { None };
            }
        }
    }

    // 3) partner override
    if let Some(map) = parse_override_map("X402_PARTNER_OVERRIDES") {
        if let Some(cfg) = map.get(partner_name) {
            if path_matches(cfg, path) {
                return if cfg.enabled { Some(cfg.clone()) } else { None };
            }
        }
    }

    // 4) global default
    global_default().and_then(|cfg| {
        if path_matches(&cfg, path) {
            Some(cfg)
        } else {
            None
        }
    })
}