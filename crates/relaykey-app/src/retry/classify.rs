use axum::http::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryClass {
    NoRetry,
    Retryable,
}

pub fn classify_status(status: StatusCode) -> RetryClass {
    match status {
        StatusCode::REQUEST_TIMEOUT
        | StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => RetryClass::Retryable,

        // 429 is controversial; default to retryable but gate with partner profile.
        StatusCode::TOO_MANY_REQUESTS => RetryClass::Retryable,

        _ => RetryClass::NoRetry,
    }
}

pub fn classify_reqwest_error(err: &reqwest::Error) -> RetryClass {
    if err.is_timeout() || err.is_connect() {
        return RetryClass::Retryable;
    }
    RetryClass::NoRetry
}