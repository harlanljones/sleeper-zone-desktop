//! Typed errors for the Sleeper client.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SleeperError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Sleeper API returned status {status}: {body}")]
    Api { status: u16, body: String },

    #[error("failed to decode Sleeper response: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SleeperError>;
