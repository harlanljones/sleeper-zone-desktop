//! Typed errors for the ESPN client.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EspnError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("ESPN API returned status {status}: {body}")]
    Api { status: u16, body: String },

    #[error("failed to decode ESPN response: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, EspnError>;
