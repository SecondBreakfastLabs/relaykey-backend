use relaykey_core::crypto::key_hash::hash_virtual_key;
use relaykey_db::init_db;

use sqlx::Row;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let key_salt = std::env::var("RELAYKEY_KEY_SALT")?;

    let db = init_db(&database_url).await?;

    println!("Seeding RelayKey Phase 1...");

    // ------------------------------------------------
    // 1. Create partner
    // ------------------------------------------------

    let partner_name = "example";
    let partner_base = "https://httpbin.org";

    let partner_id: Uuid = sqlx::query(
        r#"
        INSERT INTO partners (name, base_url)
        VALUES ($1, $2)
        ON CONFLICT (name)
        DO UPDATE SET base_url = EXCLUDED.base_url
        RETURNING id
        "#,
    )
    .bind(partner_name)
    .bind(partner_base)
    .fetch_one(&db)
    .await?
    .get("id");

    println!("Partner OK: {}", partner_name);

    // ------------------------------------------------
    // 2. Create upstream credential
    // ------------------------------------------------

    sqlx::query(
        r#"
        INSERT INTO upstream_credentials (partner_id, header_name, header_value)
        VALUES ($1, 'X-Upstream-Key', 'demo-upstream-secret')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(partner_id)
    .execute(&db)
    .await?;

    println!("Credential OK");

    // ------------------------------------------------
    // 3. Create virtual key
    // ------------------------------------------------

    let raw_key = format!("vk_{}", Uuid::new_v4());
    let key_hash = hash_virtual_key(&key_salt, &raw_key);

    sqlx::query(
        r#"
        INSERT INTO virtual_keys (name, key_hash)
        VALUES ($1, $2)
        "#,
    )
    .bind("dev")
    .bind(&key_hash)
    .execute(&db)
    .await?;

    println!();
    println!("========================================");
    println!("RelayKey seed complete.");
    println!();
    println!("USE THIS KEY:");
    println!();
    println!("{}", raw_key);
    println!();
    println!("Example test:");
    println!();
    println!(
        "curl -H \"X-RelayKey: {}\" http://localhost:8080/proxy/example/get",
        raw_key
    );
    println!("========================================");

    Ok(())
}
