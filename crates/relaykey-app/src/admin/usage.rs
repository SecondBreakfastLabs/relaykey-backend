use axum::{Extension, http::StatusCode, response::IntoResponse, Json}; 
use chrono::NaiveDate; 
use serde::{Serialize, Deserialize};
use std::sync::Arc; 
use uuid::Uuid; 

use crate::state::AppState;
use relaykey_db::queries::metrics::{query_usage_rollup, UsageRollupRow};

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
    // Phase 7 (x402)
    // pub x402_intents: i64, 
    // pub x402_settled: i64, 
    // pub x402_revenue_cents: i64, 
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
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid from (expected YYYY-MM-DD)").into_response(),
    }; 

    let to_day = match parse_day(&q.to) {
        Ok(d) => d, 
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid to (expected YYYY-MM-DD)").into_response(),
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
    .await {
        Ok(v) => v, 
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }; 

    let out: Vec<UsageRollupJson> = rows.into_iter().map(|r| UsageRollupJson {
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
    }).collect(); 

    Json(out).into_response()
}