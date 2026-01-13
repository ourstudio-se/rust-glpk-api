use thiserror::Error;

/// Result type for GLPK API client operations
pub type Result<T> = std::result::Result<T, GlpkError>;

/// Errors that can occur when using the GLPK API client
#[derive(Error, Debug)]
pub enum GlpkError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// Invalid URL provided
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// API returned an error response
    #[error("API error: {0}")]
    ApiError(String),

    /// Failed to parse response
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Invalid request configuration
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Authentication failed
    #[error("Authentication failed")]
    AuthenticationFailed,
}
