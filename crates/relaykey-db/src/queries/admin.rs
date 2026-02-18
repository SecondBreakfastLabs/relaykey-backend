use sqlx::PgPool;
use uuid::Uuid;
use crate::queries::virtual_keys::VirtualKeyRow;

pub async fn insert_virtual_key(
    db: &PgPool,
    name: &str,
    environment: &str,
    tags: &[String],
    key_hash: &str,
    enabled: bool,
    rps_limit: Option<i32>,
    rps_burst: Option<i32>,
    monthly_quota: Option<i32>,
) -> Result<Uuid, sqlx::Error> {

    let rec = sqlx::query!(
        r#"
        INSERT INTO virtual_keys
        (name, environment, tags, key_hash, enabled, rps_limit, rps_burst, monthly_quota)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
        RETURNING id
        "#,
        name,
        environment,
        tags,
        key_hash,
        enabled,
        rps_limit,
        rps_burst,
        monthly_quota
    )
    .fetch_one(db)
    .await?;

    Ok(rec.id)
}

pub async fn list_virtual_keys(
    db: &PgPool
) -> Result<Vec<VirtualKeyRow>, sqlx::Error> {

    sqlx::query_as!(
        VirtualKeyRow,
        r#"
        SELECT
        id,
        name,
        environment,
        tags,
        key_hash, 
        enabled,
        rps_limit,
        rps_burst,
        monthly_quota,
        created_at
        FROM virtual_keys
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(db)
    .await
}
