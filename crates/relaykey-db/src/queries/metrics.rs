use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UsageRollupRow {
    pub day: NaiveDate,
    pub customer_id: Uuid,
    pub virtual_key_id: Uuid,
    pub partner_name: String,

    pub total_requests: i64,
    pub forwarded_requests: i64,
    pub blocked_requests: i64,

    pub avg_latency_ms: f64,

    pub status_2xx: i64,
    pub status_3xx: i64,
    pub status_4xx: i64,
    pub status_5xx: i64,
}

#[derive(Debug, Clone)]
pub struct ErrorRollupRow {
    pub day: NaiveDate,
    pub customer_id: Uuid,
    pub virtual_key_id: Uuid,
    pub partner_name: String,

    pub error_bucket: String,
    pub count: i64,
}

/// Roll up raw usage_events into daily usage_rollup_daily for [from, to)
pub async fn rollup_usage_daily(
    db: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    // NOTE: usage_events does NOT store customer_id in your schema.
    // We join virtual_keys to get customer_id at rollup time.
    //
    // Phase 7 (x402): extend rollups by joining x402 intent tables by:
    // - virtual_key_id (or customer_id)
    // - day
    // and add:
    // - x402_intents_count
    // - x402_settled_count
    // - x402_revenue_cents
    sqlx::query!(
        r#"
        INSERT INTO usage_rollup_daily (
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          total_requests,
          forwarded_requests,
          blocked_requests,
          avg_latency_ms,
          status_2xx,
          status_3xx,
          status_4xx,
          status_5xx
        )
        SELECT
          date_trunc('day', ue.ts)::date AS day,
          vk.customer_id AS customer_id,
          ue.virtual_key_id AS virtual_key_id,
          ue.partner_name AS partner_name,

          count(*)::bigint AS total_requests,
          count(*) FILTER (WHERE ue.forwarded = true)::bigint AS forwarded_requests,
          count(*) FILTER (WHERE ue.blocked_reason IS NOT NULL)::bigint AS blocked_requests,

          avg(ue.latency_ms)::double precision AS avg_latency_ms,

          count(*) FILTER (WHERE ue.status_code BETWEEN 200 AND 299)::bigint AS status_2xx,
          count(*) FILTER (WHERE ue.status_code BETWEEN 300 AND 399)::bigint AS status_3xx,
          count(*) FILTER (WHERE ue.status_code BETWEEN 400 AND 499)::bigint AS status_4xx,
          count(*) FILTER (WHERE ue.status_code BETWEEN 500 AND 599)::bigint AS status_5xx
        FROM usage_events ue
        JOIN virtual_keys vk ON vk.id = ue.virtual_key_id
        WHERE ue.ts >= $1 AND ue.ts < $2
        GROUP BY 1,2,3,4
        ON CONFLICT (day, customer_id, virtual_key_id, partner_name)
        DO UPDATE SET
          total_requests = EXCLUDED.total_requests,
          forwarded_requests = EXCLUDED.forwarded_requests,
          blocked_requests = EXCLUDED.blocked_requests,
          avg_latency_ms = EXCLUDED.avg_latency_ms,
          status_2xx = EXCLUDED.status_2xx,
          status_3xx = EXCLUDED.status_3xx,
          status_4xx = EXCLUDED.status_4xx,
          status_5xx = EXCLUDED.status_5xx
        "#,
        from,
        to
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Roll up into error_rollup_daily for [from, to)
pub async fn rollup_error_daily(
    db: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    // Bucketing rules:
    // - blocked:<blocked_reason>
    // - upstream_timeout (504)
    // - upstream_bad_gateway (502)
    // - upstream_5xx
    // - upstream_4xx
    // - other
    //
    // Phase 7 (x402): add x402-specific buckets for:
    // - x402_intent_failed
    // - x402_settlement_failed
    sqlx::query!(
        r#"
        INSERT INTO error_rollup_daily (
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          error_bucket,
          count
        )
        SELECT
          date_trunc('day', ue.ts)::date AS day,
          vk.customer_id AS customer_id,
          ue.virtual_key_id AS virtual_key_id,
          ue.partner_name AS partner_name,

          CASE
            WHEN ue.blocked_reason IS NOT NULL THEN 'blocked:' || ue.blocked_reason
            WHEN ue.status_code = 504 THEN 'upstream_timeout'
            WHEN ue.status_code = 502 THEN 'upstream_bad_gateway'
            WHEN ue.status_code BETWEEN 500 AND 599 THEN 'upstream_5xx'
            WHEN ue.status_code BETWEEN 400 AND 499 THEN 'upstream_4xx'
            ELSE 'other'
          END AS error_bucket,

          count(*)::bigint AS count
        FROM usage_events ue
        JOIN virtual_keys vk ON vk.id = ue.virtual_key_id
        WHERE ue.ts >= $1 AND ue.ts < $2
          AND (
            ue.blocked_reason IS NOT NULL OR
            ue.status_code IS NULL OR
            ue.status_code >= 400
          )
        GROUP BY 1,2,3,4,5
        ON CONFLICT (day, customer_id, virtual_key_id, partner_name, error_bucket)
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

pub async fn query_usage_rollup(
    db: &PgPool,
    from_day: NaiveDate,
    to_day: NaiveDate,
    customer_id: Option<Uuid>,
    virtual_key_id: Option<Uuid>,
    partner_name: Option<&str>,
) -> Result<Vec<UsageRollupRow>, sqlx::Error> {
    // Using a plain query_as with optional filters
    // (COALESCE-style filter avoids dynamic SQL)
    sqlx::query_as!(
        UsageRollupRow,
        r#"
        SELECT
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          total_requests,
          forwarded_requests,
          blocked_requests,
          avg_latency_ms,
          status_2xx,
          status_3xx,
          status_4xx,
          status_5xx
        FROM usage_rollup_daily
        WHERE day >= $1
          AND day < $2
          AND ($3::uuid IS NULL OR customer_id = $3)
          AND ($4::uuid IS NULL OR virtual_key_id = $4)
          AND ($5::text IS NULL OR partner_name = $5)
        ORDER BY day DESC, partner_name ASC
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

pub async fn query_error_rollup(
    db: &PgPool,
    from_day: NaiveDate,
    to_day: NaiveDate,
    customer_id: Option<Uuid>,
    virtual_key_id: Option<Uuid>,
    partner_name: Option<&str>,
) -> Result<Vec<ErrorRollupRow>, sqlx::Error> {
    sqlx::query_as!(
        ErrorRollupRow,
        r#"
        SELECT
          day,
          customer_id,
          virtual_key_id,
          partner_name,
          error_bucket,
          count
        FROM error_rollup_daily
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