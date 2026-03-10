use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PaymentIntentDailyRow {
    pub day: NaiveDate,
    pub virtual_key_id: Uuid,
    pub customer_id: Uuid,
    pub partner_name: String,
    pub provider: String,
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct X402ErrorDailyRow {
    pub day: NaiveDate,
    pub virtual_key_id: Uuid,
    pub customer_id: Uuid,
    pub partner_name: String,
    pub error_bucket: String,
    pub count: i64,
}

pub async fn query_payment_intents_by_day(
    db: &PgPool,
    from_day: NaiveDate,
    to_day: NaiveDate,
    customer_id: Option<Uuid>,
    virtual_key_id: Option<Uuid>,
    partner_name: Option<&str>,
) -> Result<Vec<PaymentIntentDailyRow>, sqlx::Error> {
    sqlx::query_as!(
        PaymentIntentDailyRow,
        r#"
        SELECT
            date_trunc('day', pi.ts)::date AS "day!",
            pi.virtual_key_id AS "virtual_key_id!",
            vk.customer_id AS "customer_id!",
            pi.partner_name AS "partner_name!",
            pi.provider AS "provider!",
            pi.status AS "status!",
            COUNT(*)::bigint AS "count!"
        FROM payment_intents pi
        JOIN virtual_keys vk
          ON pi.virtual_key_id = vk.id
        WHERE date_trunc('day', pi.ts)::date >= $1
          AND date_trunc('day', pi.ts)::date < $2
          AND ($3::uuid IS NULL OR vk.customer_id = $3)
          AND ($4::uuid IS NULL OR pi.virtual_key_id = $4)
          AND ($5::text IS NULL OR pi.partner_name = $5)
        GROUP BY 1, 2, 3, 4, 5, 6
        ORDER BY 1 DESC, 7 DESC
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
) -> Result<Vec<X402ErrorDailyRow>, sqlx::Error> {
    sqlx::query_as!(
        X402ErrorDailyRow,
        r#"
        SELECT
            date_trunc('day', pi.ts)::date AS "day!",
            pi.virtual_key_id AS "virtual_key_id!",
            vk.customer_id AS "customer_id!",
            pi.partner_name AS "partner_name!",
            CASE
                WHEN pi.status = 'failed' THEN 'x402_verify_failed'
                WHEN pi.status = 'expired' THEN 'x402_expired'
                WHEN pi.status = 'pending' THEN 'x402_pending'
                WHEN pi.status = 'verified' THEN 'x402_verified'
                ELSE 'x402_other'
            END AS "error_bucket!",
            COUNT(*)::bigint AS "count!"
        FROM payment_intents pi
        JOIN virtual_keys vk
          ON pi.virtual_key_id = vk.id
        WHERE date_trunc('day', pi.ts)::date >= $1
          AND date_trunc('day', pi.ts)::date < $2
          AND ($3::uuid IS NULL OR vk.customer_id = $3)
          AND ($4::uuid IS NULL OR pi.virtual_key_id = $4)
          AND ($5::text IS NULL OR pi.partner_name = $5)
        GROUP BY 1, 2, 3, 4, 5
        ORDER BY 1 DESC, 6 DESC
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