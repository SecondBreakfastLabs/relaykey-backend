use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug, Clone)]
pub struct X402UsageRollupRow {
    pub day: NaiveDate,
    pub customer_id: Uuid,
    pub virtual_key_id: Uuid,
    pub partner_name: String,
    pub provider: String,
    pub intents_created: i64,
    pub verified_count: i64,
    pub failed_count: i64,
    pub expired_count: i64,
    pub unpaid_count: i64,
    pub revenue_cents: i64,
}

#[derive(Debug, Clone)]
pub struct X402ErrorRollupRow {
    pub day: NaiveDate,
    pub customer_id: Uuid,
    pub virtual_key_id: Uuid,
    pub partner_name: String,
    pub provider: String,
    pub error_bucket: String,
    pub count: i64,
}

pub async fn insert_x402_event(
    db: &PgPool,
    customer_id: Uuid,
    virtual_key_id: Uuid,
    partner_name: &str,
    provider: &str,
    path: &str,
    event_type: &str,
    detail: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO x402_events (
          customer_id,
          virtual_key_id,
          partner_name,
          provider,
          path,
          event_type,
          detail
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7)
        "#,
        customer_id,
        virtual_key_id,
        partner_name,
        provider,
        path,
        event_type,
        detail
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn rollup_x402_usage_daily(
    db: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO x402_rollup_daily (
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          provider,
          intents_created,
          verified_count,
          failed_count,
          expired_count,
          unpaid_count,
          revenue_cents
        )
        SELECT
          date_trunc('day', pi.ts)::date AS day,
          vk.customer_id AS customer_id,
          pi.virtual_key_id AS virtual_key_id,
          pi.partner_name AS partner_name,
          pi.provider AS provider,
          COUNT(*)::bigint AS intents_created,
          COUNT(*) FILTER (WHERE pi.status = 'verified')::bigint AS verified_count,
          COUNT(*) FILTER (WHERE pi.status = 'failed')::bigint AS failed_count,
          COUNT(*) FILTER (WHERE pi.status = 'expired')::bigint AS expired_count,
          COUNT(*) FILTER (WHERE pi.status = 'pending')::bigint AS unpaid_count,
          0::bigint AS revenue_cents
        FROM payment_intents pi
        JOIN virtual_keys vk
          ON vk.id = pi.virtual_key_id
        WHERE pi.ts >= $1 AND pi.ts < $2
        GROUP BY 1,2,3,4,5
        ON CONFLICT (day, customer_id, virtual_key_id, partner_name, provider)
        DO UPDATE SET
          intents_created = EXCLUDED.intents_created,
          verified_count = EXCLUDED.verified_count,
          failed_count = EXCLUDED.failed_count,
          expired_count = EXCLUDED.expired_count,
          unpaid_count = EXCLUDED.unpaid_count,
          revenue_cents = EXCLUDED.revenue_cents
        "#,
        from,
        to
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn rollup_x402_error_daily(
    db: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO x402_error_rollup_daily (
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          provider,
          error_bucket,
          count
        )
        SELECT
          date_trunc('day', xe.ts)::date AS day,
          xe.customer_id,
          xe.virtual_key_id,
          xe.partner_name,
          xe.provider,
          xe.event_type AS error_bucket,
          COUNT(*)::bigint AS count
        FROM x402_events xe
        WHERE xe.ts >= $1 AND xe.ts < $2
        GROUP BY 1,2,3,4,5,6
        ON CONFLICT (day, customer_id, virtual_key_id, partner_name, provider, error_bucket)
        DO UPDATE SET
          count = EXCLUDED.count
        "#,
        from,
        to
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn query_x402_usage_rollup(
    db: &PgPool,
    from_day: NaiveDate,
    to_day: NaiveDate,
    customer_id: Option<Uuid>,
    virtual_key_id: Option<Uuid>,
    partner_name: Option<&str>,
) -> Result<Vec<X402UsageRollupRow>, sqlx::Error> {
    sqlx::query_as!(
        X402UsageRollupRow,
        r#"
        SELECT
          day AS "day!",
          customer_id AS "customer_id!",
          virtual_key_id AS "virtual_key_id!",
          partner_name AS "partner_name!",
          provider AS "provider!",
          intents_created AS "intents_created!",
          verified_count AS "verified_count!",
          failed_count AS "failed_count!",
          expired_count AS "expired_count!",
          unpaid_count AS "unpaid_count!",
          revenue_cents AS "revenue_cents!"
        FROM x402_rollup_daily
        WHERE day >= $1
          AND day < $2
          AND ($3::uuid IS NULL OR customer_id = $3)
          AND ($4::uuid IS NULL OR virtual_key_id = $4)
          AND ($5::text IS NULL OR partner_name = $5)
        ORDER BY day DESC, intents_created DESC
        "#,
        from_day,
        to_day,
        customer_id,
        virtual_key_id,
        partner_name
    )
    .fetch_all(db)
    .await
}

pub async fn query_x402_error_rollup(
    db: &PgPool,
    from_day: NaiveDate,
    to_day: NaiveDate,
    customer_id: Option<Uuid>,
    virtual_key_id: Option<Uuid>,
    partner_name: Option<&str>,
) -> Result<Vec<X402ErrorRollupRow>, sqlx::Error> {
    sqlx::query_as!(
        X402ErrorRollupRow,
        r#"
        SELECT
          day AS "day!",
          customer_id AS "customer_id!",
          virtual_key_id AS "virtual_key_id!",
          partner_name AS "partner_name!",
          provider AS "provider!",
          error_bucket AS "error_bucket!",
          count AS "count!"
        FROM x402_error_rollup_daily
        WHERE day >= $1
          AND day < $2
          AND ($3::uuid IS NULL OR customer_id = $3)
          AND ($4::uuid IS NULL OR virtual_key_id = $4)
          AND ($5::text IS NULL OR partner_name = $5)
        ORDER BY day DESC, count DESC
        "#,
        from_day,
        to_day,
        customer_id,
        virtual_key_id,
        partner_name
    )
    .fetch_all(db)
    .await
}