use sqlx::PgPool; 
use uuid::Uuid; 

#[derive(Debug, Clone)]
pub struct PaymentIntentRow {
    pub id: Uuid, 
}

#[derive(Debug, Clone)]
pub struct PaymentIntentLookupRow {
    pub id: Uuid, 
    pub virtual_key_id: Uuid, 
    pub partner_name: String, 
    pub path: String, 
    pub request_hash: String, 
    pub status: String, 
}

pub async fn insert_payment_intent(
    db: &PgPool,
    virtual_key_id: Uuid,
    partner_name: &str,
    path: &str,
    request_hash: &str,
    amount: &str,
    currency: &str,
    facilitator_url: &str,
    recipient: &str,
    provider: &str,
) -> Result<Uuid, sqlx::Error> {
    let rec = sqlx::query!(
        r#"
        INSERT INTO payment_intents (
            virtual_key_id,
            partner_name,
            path,
            request_hash,
            amount,
            currency,
            facilitator_url,
            recipient,
            provider,
            status, 
            expires_at
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending', now() + interval '15 minutes'
        )
        RETURNING id
        "#,
        virtual_key_id,
        partner_name,
        path,
        request_hash,
        amount,
        currency,
        facilitator_url,
        recipient,
        provider,
    )
    .fetch_one(db)
    .await?;

    Ok(rec.id)
}

pub async fn mark_payment_intent_verified(
    db: &PgPool, 
    intent_id: Uuid, 
    payment_id: Option<&str>, 
    payment_token: Option<&str>, 
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE payment_intents 
        SET status = 'verified', 
            payment_id = COALESCE($2, payment_id),
            payment_token = COALESCE($3, payment_token)
        WHERE id = $1
        "#,
        intent_id,
        payment_id,
        payment_token
    )
    .execute(db)
    .await?; 

    Ok(())
}

pub async fn mark_payment_intent_failed(
    db: &PgPool, 
    intent_id: Uuid, 
    payment_id: Option<&str>, 
    payment_token: Option<&str>, 
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE payment_intents 
        SET status = 'failed', 
            payment_id = COALESCE($2, payment_id),
            payment_token = COALESCE($3, payment_token)
        WHERE id = $1
        "#,
        intent_id,
        payment_id,
        payment_token
    )
    .execute(db)
    .await?;  

    Ok(())
}

pub async fn expire_stale_payment_intents(
    db: &PgPool, 
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query!(
        r#"
        UPDATE payment_intents
        SET status = 'expired'
        WHERE status = 'pending'
            AND expires_at IS NOT NULL 
            AND expires_at < NOW()
        "#
    )
    .execute(db)
    .await?;

    Ok(res.rows_affected())
}

pub async fn find_latest_pending_intent_by_request_hash(
    db: &PgPool, 
    virtual_key_id: Uuid, 
    partner_name: &str, 
    path: &str, 
    request_hash: &str, 
) -> Result<Option<PaymentIntentLookupRow>, sqlx::Error> {
    sqlx::query_as!(
        PaymentIntentLookupRow, 
        r#"
        SELECT 
            id, 
            virtual_key_id,
            partner_name,
            path,
            request_hash,
            status
        FROM payment_intents
        WHERE virtual_key_id = $1
          AND partner_name = $2
          AND path = $3
          AND request_hash = $4
          AND status = 'pending'
        ORDER BY ts DESC
        LIMIT 1
        "#,
        virtual_key_id,
        partner_name,
        path,
        request_hash
    )
    .fetch_optional(db)
    .await
}