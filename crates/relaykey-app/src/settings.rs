use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Settings {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub redis_url: String,
    pub log_filter: String,
    pub key_salt: String,
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = std::env::var("RELAYKEY_BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse::<SocketAddr>()
            .map_err(|e| format!("Invalid RELAYKEY_BIND_ADDR: {e}"))?;

        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?;

        let redis_url =
            std::env::var("REDIS_URL").map_err(|_| "REDIS_URL is required".to_string())?;

        let log_filter = std::env::var("RELAYKEY_LOG").unwrap_or_else(|_| "info".to_string());
        let key_salt = std::env::var("RELAYKEY_KEY_SALT")
            .map_err(|_| "RELAYKEY_KEY_SALT is required".to_string())?;

        Ok(Self {
            bind_addr,
            database_url,
            redis_url,
            log_filter,
            key_salt,
        })
    }
}
