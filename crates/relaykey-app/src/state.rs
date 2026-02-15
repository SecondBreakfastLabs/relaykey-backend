use relaykey_db::{Db, RedisConn};

pub struct AppState {
    pub db: Db,
    pub redis: RedisConn,
    pub http: reqwest::Client, 
    pub key_salt: String,
}
