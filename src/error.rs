//! Error types for the Thufir bot.

/// Errors that can occur in the Thufir bot.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Discord/Poise error.
    #[error("discord error: {0}")]
    Discord(String),

    /// Volume leaders data source error.
    #[error("volume leaders error: {0}")]
    VolumeLeaders(String),

    /// Validation error.
    #[error("validation error: {0}")]
    Validation(String),

    /// Observability/logging error.
    #[error("observability error: {0}")]
    Observability(String),
}

/// Result type for the Thufir bot.
pub type Result<T> = std::result::Result<T, Error>;

impl From<serenity::Error> for Error {
    fn from(err: serenity::Error) -> Self {
        Error::Discord(err.to_string())
    }
}
