use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;
use relaykey_db::queries::{
    metrics::{query_error_rollup, ErrorRollupRow},
    x402_metrics::{query_x402_error_rollup, X402ErrorDailyRow},
};

#[derive(Deserialize)]
pub struct ErrorsQuery {
    pub from: String,
    pub to: String,
    pub customer_id: Option<Uuid>,
    pub virtual_key_id: Option<Uuid>,
    pub partner_name: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorRollupJson {
    pub day: String,
    pub customer_id: Uuid,
    pub virtual_key_id: Uuid,
    pub partner_name: String,
    pub error_bucket: String,
    pub count: i64,

    // lets the UI / caller distinguish core gateway errors vs x402 errors
    pub source: String,
    // TODO(x402): later add richer classification if needed
    // pub x402_error_class: Option<String>,
}

fn parse_day(s: &str) -> Result<NaiveDate, ()> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| ())
}

pub async fn admin_errors(
    Extension(state): Extension<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<ErrorsQuery>,
) -> impl IntoResponse {
    let from_day = match parse_day(&q.from) {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                "invalid from (expected YYYY-MM-DD)",
            )
                .into_response();
        }
    };

    let to_day = match parse_day(&q.to) {
        Ok(d) => d,
        Err(_) => {
            return (StatusCode::BAD_REQUEST, "invalid to (expected YYYY-MM-DD)").into_response();
        }
    };

    let partner = q.partner_name.as_deref();

    let rows: Vec<ErrorRollupRow> = match query_error_rollup(
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

    let x402_rows: Vec<X402ErrorDailyRow> = match query_x402_error_rollup(
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

    let mut out: Vec<ErrorRollupJson> = rows
        .into_iter()
        .map(|r| ErrorRollupJson {
            day: r.day.to_string(),
            customer_id: r.customer_id,
            virtual_key_id: r.virtual_key_id,
            partner_name: r.partner_name,
            error_bucket: r.error_bucket,
            count: r.count,
            source: "gateway".to_string(),
        })
        .collect();

    out.extend(x402_rows.into_iter().map(|r| ErrorRollupJson {
        day: r.day.to_string(),
        customer_id: r.customer_id,
        virtual_key_id: r.virtual_key_id,
        partner_name: r.partner_name,
        error_bucket: r.error_bucket,
        count: r.count,
        source: "x402".to_string(),
    }));

    Json(out).into_response()
}
