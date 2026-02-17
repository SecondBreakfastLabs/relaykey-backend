use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub enum BlockedReason {
    RateLimitExceeded,
    MonthlyQuotaExceeded,
}

impl BlockedReason {
    pub fn code(self) -> &'static str {
        match self {
            BlockedReason::RateLimitExceeded => "rate_limit_exceeded",
            BlockedReason::MonthlyQuotaExceeded => "monthly_quota_exceeded",
        }
    }
}

pub async fn insert_usage_event(
    db: &PgPool,
    virtual_key_id: Uuid,
    partner_name: &str,
    path: &str,
    forwarded: bool,
    blocked_reason: Option<BlockedReason>,
    status_code: Option<u16>,
    latency_ms: i32,
) -> Result<(), sqlx::Error> {
    let blocked_reason_str = blocked_reason.map(|r| r.code().to_string());
    let status_code_i32 = status_code.map(|s| s as i32);

    sqlx::query!(
        r#"
        INSERT INTO usage_events (
            virtual_key_id,
            partner_name,
            path,
            forwarded,
            blocked_reason,
            status_code,
            latency_ms
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        virtual_key_id,
        partner_name,
        path,
        forwarded,
        blocked_reason_str,
        status_code_i32,
        latency_ms
    )
    .execute(db)
    .await?;

    Ok(())
}
