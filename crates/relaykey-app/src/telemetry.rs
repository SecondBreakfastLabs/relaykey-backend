use tracing_subscriber::{fmt, EnvFilter};

pub fn init(log_filter: &str) {
    // Prefer RUST_LOG, fall back to RELAYKEY_LOG passed in.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_filter));

    // JSON logs are nice for later; switch to .compact() if you prefer.
    fmt()
        .with_env_filter(env_filter)
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .init();
}
