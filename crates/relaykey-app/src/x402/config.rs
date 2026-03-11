use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct X402Config {
    pub enabled: bool,
    pub amount: String,
    pub currency: String,
    pub facilitator_url: String,
    pub recipient: String,
    pub provider: String,
}

fn env_bool(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

// Phase 8 Skeleton:
// Later this will resolve in the following order:
// 1. virtual key override
// 2. customer/org override
// 3. partner override
// 4. global default
//
// For now it returns a global default if enabled
pub fn resolve_x402_config(
    _customer_id: Uuid,
    _virtual_key_id: Uuid,
    _partner_name: &str,
    _path: &str,
) -> Option<X402Config> {
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
    })
}
