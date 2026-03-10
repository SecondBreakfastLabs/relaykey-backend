use axum::{Extension, Json, http::StatusCode, response::IntoResponse};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

use crate::state::AppState;
use relaykey_db::queries::{
    metrics::{UsageRollupRow, query_usage_rollup},
    x402_metrics::query_payment_intents_by_day,
};

#[derive(Deserialize)]
pub struct UsageQuery {
    pub from: String,
    pub to: String,
    pub customer_id: Option<Uuid>,
    pub virtual_key_id: Option<Uuid>,
    pub partner_name: Option<String>,
}

#[derive(Serialize)]
pub struct UsageRollupJson {
    pub day: String,
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

    // Phase 8.x x402 visibility
    pub x402_intents_created: i64,
    pub x402_verified_count: i64,
    pub x402_unpaid_count: i64,

    // TODO(x402): later extend with:
    // pub x402_revenue_cents: i64,
    // pub x402_settlement_failed_count: i64,
}

fn parse_day(s: &str) -> Result<NaiveDate, ()> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| ())
}

pub async fn admin_usage(
    Extension(state): Extension<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<UsageQuery>,
) -> impl IntoResponse {
    let from_day = match parse_day(&q.from) {
        Ok(d) => d,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "invalid from (expected YYYY-MM-DD)")
                .into_response();
        }
    };

    let to_day = match parse_day(&q.to) {
        Ok(d) => d,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "invalid to (expected YYYY-MM-DD)")
                .into_response();
        }
    };

    let partner = q.partner_name.as_deref();

    let rows: Vec<UsageRollupRow> = match query_usage_rollup(
        &state.db,
        from_day,
        to_day,
        q.customer_id,
        q.virtual_key_id,
        partner,
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let x402_rows = match query_payment_intents_by_day(
        &state.db,
        from_day,
        to_day,
        q.customer_id,
        q.virtual_key_id,
        partner,
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // key = (day, customer_id, virtual_key_id, partner_name)
    // value = (intents_created, verified_count, unpaid_count)
    let mut x402_map: HashMap<(String, Uuid, Uuid, String), (i64, i64, i64)> = HashMap::new();

    for r in x402_rows {
        let key = (
            r.day.to_string(),
            r.customer_id,
            r.virtual_key_id,
            r.partner_name.clone(),
        );

        let entry = x402_map.entry(key).or_insert((0, 0, 0));
        entry.0 += r.count;

        match r.status.as_str() {
            "verified" => entry.1 += r.count,
            "pending" => entry.2 += r.count,
            _ => {}
        }
    }

    let out: Vec<UsageRollupJson> = rows
        .into_iter()
        .map(|r| {
            let key = (
                r.day.to_string(),
                r.customer_id,
                r.virtual_key_id,
                r.partner_name.clone(),
            );

            let (x402_intents_created, x402_verified_count, x402_unpaid_count) =
                x402_map.get(&key).copied().unwrap_or((0, 0, 0));

            UsageRollupJson {
                day: r.day.to_string(),
                customer_id: r.customer_id,
                virtual_key_id: r.virtual_key_id,
                partner_name: r.partner_name,
                total_requests: r.total_requests,
                forwarded_requests: r.forwarded_requests,
                blocked_requests: r.blocked_requests,
                avg_latency_ms: r.avg_latency_ms,
                status_2xx: r.status_2xx,
                status_3xx: r.status_3xx,
                status_4xx: r.status_4xx,
                status_5xx: r.status_5xx,
                x402_intents_created,
                x402_verified_count,
                x402_unpaid_count,
            }
        })
        .collect();

    Json(out).into_response()
}