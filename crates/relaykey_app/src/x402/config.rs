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

    // monetization controls
    pub is_free: bool,
    pub is_internal: bool,
    pub requires_payment: bool,

    #[serde(default)]
    pub path_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartialX402Config {
    pub enabled: Option<bool>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub facilitator_url: Option<String>,
    pub recipient: Option<String>,
    pub provider: Option<String>,

    pub is_free: Option<bool>,
    pub is_internal: Option<bool>,
    pub requires_payment: Option<bool>,

    pub path_prefixes: Option<Vec<String>>,
}

impl X402Config {
    pub fn amount_as_f64(&self) -> Option<f64> {
        self.amount.parse().ok()
    }
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn parse_override_map(env_name: &str) -> Option<HashMap<String, PartialX402Config>> {
    let raw = std::env::var(env_name).ok()?;
    serde_json::from_str(&raw).ok()
}

fn matches_path_prefix(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| path.starts_with(p))
}

fn merge_config(base: X402Config, override_cfg: PartialX402Config) -> X402Config {
    X402Config {
        enabled: override_cfg.enabled.unwrap_or(base.enabled),

        amount: override_cfg.amount.unwrap_or(base.amount),
        currency: override_cfg.currency.unwrap_or(base.currency),

        facilitator_url: override_cfg
            .facilitator_url
            .unwrap_or(base.facilitator_url),
        recipient: override_cfg.recipient.unwrap_or(base.recipient),
        provider: override_cfg.provider.unwrap_or(base.provider),

        is_free: override_cfg.is_free.unwrap_or(base.is_free),
        is_internal: override_cfg.is_internal.unwrap_or(base.is_internal),
        requires_payment: override_cfg
            .requires_payment
            .unwrap_or(base.requires_payment),

        path_prefixes: override_cfg.path_prefixes.unwrap_or(base.path_prefixes),
    }
}

fn finalize_config(cfg: X402Config, path: &str) -> Option<X402Config> {
    if !cfg.enabled {
        return None;
    }

    if cfg.is_internal {
        return None;
    }

    if cfg.is_free {
        return None;
    }

    if !cfg.requires_payment {
        return None;
    }

    if !cfg.path_prefixes.is_empty() && !matches_path_prefix(path, &cfg.path_prefixes) {
        return None;
    }

    Some(cfg)
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
        is_free: false,
        is_internal: false,
        requires_payment: true,
        path_prefixes: vec![],
    })
}

pub fn resolve_x402_config(
    customer_id: Uuid,
    virtual_key_id: Uuid,
    partner_name: &str,
    path: &str,
) -> Option<X402Config> {
    let base = global_default()?;
    let vk_key = virtual_key_id.to_string();
    let customer_key = customer_id.to_string();

    // 1) partner override on top of global
    let mut resolved = if let Some(map) = parse_override_map("X402_PARTNER_OVERRIDES") {
        if let Some(cfg) = map.get(partner_name) {
            merge_config(base, cfg.clone())
        } else {
            base
        }
    } else {
        base
    };

    // 2) customer/org override on top of partner/global
    if let Some(map) = parse_override_map("X402_CUSTOMER_OVERRIDES") {
        if let Some(cfg) = map.get(&customer_key) {
            resolved = merge_config(resolved, cfg.clone());
        }
    }

    // 3) virtual key override on top of customer/partner/global
    if let Some(map) = parse_override_map("X402_VIRTUAL_KEY_OVERRIDES") {
        if let Some(cfg) = map.get(&vk_key) {
            resolved = merge_config(resolved, cfg.clone());
        }
    }

    finalize_config(resolved, path)
}