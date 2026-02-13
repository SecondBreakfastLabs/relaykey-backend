use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tracing::info;

pub type Db = PgPool;
pub type RedisConn = MultiplexedConnection;

pub async fn init_db(database_url: &str) -> Result<Db, sqlx::Error> {
    // Keep it simple in phase 0.
    let pool = PgPool::connect(database_url).await?;
    info!("Connected to Postgres");
    Ok(pool)
}

pub async fn init_redis(redis_url: &str) -> Result<RedisConn, redis::RedisError> {
    let client = redis::Client::open(redis_url)?;
    let conn = client.get_multiplexed_tokio_connection().await?;
    info!("Connected to Redis");
    Ok(conn)
}
