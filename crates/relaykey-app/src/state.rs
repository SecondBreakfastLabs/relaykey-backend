use relaykey_db::{Db, RedisConn};

pub struct AppState {
    pub db: Db,
    pub redis: RedisConn,
}
