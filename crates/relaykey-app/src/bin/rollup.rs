use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use relaykey_app::settings::Settings;
use relaykey_db::init_db;

fn arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn parse_day(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| format!("invalid date: {s} (expected YYYY-MM-DD)"))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    let settings = Settings::from_env()?;

    // Usage:
    // cargo run -p relaykey-app --bin rollup -- --from 2026-03-01 --to 2026-03-05
    let args: Vec<String> = std::env::args().collect();
    let from_s = arg_value(&args, "--from").ok_or_else(|| "missing --from YYYY-MM-DD".to_string())?;
    let to_s = arg_value(&args, "--to").ok_or_else(|| "missing --to YYYY-MM-DD".to_string())?;

    let from_day = parse_day(&from_s)?;
    let to_day = parse_day(&to_s)?;

    // Convert day range to timestamptz range [from, to)
    let from: DateTime<Utc> = Utc.from_utc_datetime(&from_day.and_hms_opt(0, 0, 0).unwrap());
    let to: DateTime<Utc> = Utc.from_utc_datetime(&to_day.and_hms_opt(0, 0, 0).unwrap());

    let db = init_db(&settings.database_url)
        .await
        .map_err(|e| format!("DB init failed: {e}"))?;

    // Run rollups
    relaykey_db::queries::metrics::rollup_usage_daily(&db, from, to)
        .await
        .map_err(|e| format!("usage rollup failed: {e}"))?;

    relaykey_db::queries::metrics::rollup_error_daily(&db, from, to)
        .await
        .map_err(|e| format!("error rollup failed: {e}"))?;

    println!("rollup complete: [{} .. {})", from_day, to_day);
    Ok(())
}