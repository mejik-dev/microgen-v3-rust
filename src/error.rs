use thiserror::Error;

/// Unified error type for the Microgen SDK.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MicrogenError {
    /// The API returned a non-success HTTP status.
    #[error("API error ({status}): {message} — {body}")]
    Api {
        status: u16,
        message: String,
        body: String,
    },
    /// An error from the reqwest HTTP client.
    #[error("HTTP request error: {0}")]
    Request(#[from] reqwest::Error),
    /// An error during JSON serialization/deserialization.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    /// A WebSocket protocol error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    /// A low-level WebSocket connection error.
    #[error("WebSocket connection error: {0}")]
    WebSocketConnection(Box<tokio_tungstenite::tungstenite::Error>),
    /// Invalid configuration or arguments.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, MicrogenError>;

impl From<tokio_tungstenite::tungstenite::Error> for MicrogenError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocketConnection(Box::new(e))
    }
}

/// Check an HTTP response status and convert non-success codes into [`MicrogenError::Api`].
pub async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response> {
    let status = resp.status();
    if !status.is_success() {
        let status_code = status.as_u16();
        let reason = status.canonical_reason().unwrap_or("Unknown").to_string();
        let body = resp.text().await.unwrap_or_default();
        return Err(MicrogenError::Api {
            status: status_code,
            message: reason,
            body,
        });
    }
    Ok(resp)
}
