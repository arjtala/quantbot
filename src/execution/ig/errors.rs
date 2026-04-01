#[derive(Debug, thiserror::Error)]
pub enum IgError {
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("session expired")]
    SessionExpired,

    #[error("rate limited")]
    RateLimited,

    #[error("IG API error: {status} — {message}")]
    ApiError { status: u16, message: String },

    #[error("epic not found: {0}")]
    EpicNotFound(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
}
