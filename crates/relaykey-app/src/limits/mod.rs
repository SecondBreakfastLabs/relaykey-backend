use redis::{aio::MultiplexedConnection, Script};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_ms() -> i64 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_millis()) as i64
}

/// Seconds until the start of next month UTC.
/// Good enough for quotas. (UTC-based)
fn seconds_until_next_month_utc() -> i64 {
    use time::{Date, Month, OffsetDateTime};

    let now = OffsetDateTime::now_utc();
    let year = now.year();
    let month = now.month();

    let (ny, nm) = match month {
        Month::December => (year + 1, Month::January),
        _ => (year, Month::try_from((month as u8) + 1).unwrap()),
    };

    let next = OffsetDateTime::from_unix_timestamp(0)
        .unwrap()
        .replace_date(Date::from_calendar_date(ny, nm, 1).unwrap())
        .replace_time(time::Time::MIDNIGHT);

    let diff = next - now;
    diff.whole_seconds().max(60) // at least 60s to avoid weirdness
}

pub fn yyyymm_utc() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_utc();
    format!("{:04}{:02}", now.year(), now.month() as u8)
}

pub async fn token_bucket_allow(
    redis_conn: &mut MultiplexedConnection,
    vk_id: Uuid,
    rate_per_sec: i32,
    capacity: i32,
) -> Result<bool, redis::RedisError> {
    // Atomic refill+consume: returns {allowed(0/1), remaining_tokens}
    static LUA: &str = r#"
local key = KEYS[1]
local now_ms = tonumber(ARGV[1])
local rate = tonumber(ARGV[2])
local cap = tonumber(ARGV[3])

local data = redis.call("HMGET", key, "tokens", "ts_ms")
local tokens = tonumber(data[1])
local last_ms = tonumber(data[2])

if tokens == nil then tokens = cap end
if last_ms == nil then last_ms = now_ms end

local delta = math.max(0, now_ms - last_ms) / 1000.0
tokens = math.min(cap, tokens + (delta * rate))

local allowed = 0
if tokens >= 1.0 then
  allowed = 1
  tokens = tokens - 1.0
end

redis.call("HMSET", key, "tokens", tokens, "ts_ms", now_ms)
redis.call("EXPIRE", key, 60 * 60 * 24 * 7)

return {allowed, tokens}
"#;

    let key = format!("rl:{}", vk_id);
    let script = Script::new(LUA);

    let (allowed, _remaining): (i64, f64) = script
        .key(key)
        .arg(now_ms())
        .arg(rate_per_sec)
        .arg(capacity)
        .invoke_async(redis_conn)
        .await?;

    Ok(allowed == 1)
}

/// Atomically checks monthly quota and increments if allowed.
/// Returns true if allowed; false if quota exceeded.
pub async fn monthly_quota_allow_and_incr(
    redis_conn: &mut MultiplexedConnection,
    vk_id: Uuid,
    monthly_limit: i32,
) -> Result<bool, redis::RedisError> {
    static LUA: &str = r#"
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local ttl = tonumber(ARGV[2])

local current = tonumber(redis.call("GET", key))
if current == nil then current = 0 end

if current >= limit then
  return {0, current}
end

local nextv = redis.call("INCR", key)
if nextv == 1 then
  redis.call("EXPIRE", key, ttl)
end

return {1, nextv}
"#;

    let yyyymm = yyyymm_utc();
    let key = format!("quota:{}:{}", vk_id, yyyymm);
    let ttl = seconds_until_next_month_utc();

    let script = Script::new(LUA);
    let (allowed, _count): (i64, i64) = script
        .key(key)
        .arg(monthly_limit)
        .arg(ttl)
        .invoke_async(redis_conn)
        .await?;

    Ok(allowed == 1)
}
