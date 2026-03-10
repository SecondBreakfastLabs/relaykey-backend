use sqlx::PgPool; 
use uuid::Uuid; 

#[derive(Debug, Clone)]
pub struct PaymentIntentRow {
    pub id: Uuid, 
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
            status
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending'
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