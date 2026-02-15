use relaykey_db::init_db;

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?;

    let db = init_db(&database_url)
        .await
        .map_err(|e| format!("DB init failed: {e}"))?;

    // Runs migrations in ./migrations at the workspace root
    sqlx::migrate!("../../migrations")
        .run(&db)
        .await
        .map_err(|e| format!("Migration failed: {e}"))?;

    println!("Migrations applied.");
    Ok(())
}
