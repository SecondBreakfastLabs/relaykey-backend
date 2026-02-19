use sqlx::PgPool; 
use uuid::Uuid; 
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRow{
    pub id: Uuid, 
    pub name: String, 
    pub endpoint_allowlist: Vec<String>, 
    pub rps_limit: Option<i32>, 
    pub rps_burst: Option<i32>, 
    pub monthly_quota: Option<i32>, 
    pub timeout_ms: i32, 
}

pub async fn get_policy_by_id(db: &PgPool, id: Uuid) -> Result<Option<PolicyRow>, sqlx::Error> {
    sqlx::query_as!(
        PolicyRow, 
        r#"
        SELECT 
            id, 
            name, 
            endpoint_allowlist as "endpoint_allowlist!",
            rps_limit, 
            rps_burst, 
            monthly_quota,
            timeout_ms
        FROM policies 
        WHERE id = $1 
        "#, 
        id
    )
    .fetch_optional(db)
    .await
}