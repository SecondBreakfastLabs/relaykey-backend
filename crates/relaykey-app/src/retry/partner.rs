use axum::http::StatusCode;

#[derive(Debug, Clone)]
pub struct PartnerRetryProfile {
    pub retry_429: bool,
}

impl Default for PartnerRetryProfile {
    fn default() -> Self {
        Self { retry_429: false }
    }
}

pub fn profile_for_partner(_partner_name: &str) -> PartnerRetryProfile {
    // Later: load from DB. For now: safe default.
    PartnerRetryProfile::default()
}

pub fn status_retry_allowed(profile: &PartnerRetryProfile, status: StatusCode) -> bool {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return profile.retry_429;
    }
    true
}