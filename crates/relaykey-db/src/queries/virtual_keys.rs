use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct VirtualKeyRow {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PartnerRow {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct CredentialRow {
    pub header_name: String,
    pub header_value: String,
}

pub async fn get_virtual_key_by_hash(
    db: &PgPool,
    key_hash: &str,
) -> Result<Option<VirtualKeyRow>, sqlx::Error> {
    let row = sqlx::query_as!(
        VirtualKeyRow,
        r#"
        SELECT id, name, enabled
        FROM virtual_keys
        WHERE key_hash = $1
        "#,
        key_hash
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn get_partner_by_name(
    db: &PgPool,
    partner_name: &str,
) -> Result<Option<PartnerRow>, sqlx::Error> {
    let row = sqlx::query_as!(
        PartnerRow,
        r#"
        SELECT id, name, base_url
        FROM partners
        WHERE name = $1
        "#,
        partner_name
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn get_credential_for_partner(
    db: &PgPool,
    partner_id: Uuid,
) -> Result<Option<CredentialRow>, sqlx::Error> {
    let row = sqlx::query_as!(
        CredentialRow,
        r#"
        SELECT header_name, header_value
        FROM upstream_credentials
        WHERE partner_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        partner_id
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}
