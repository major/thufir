//! Observability and logging infrastructure for the Thufir bot.

use tracing_subscriber::EnvFilter;

/// Initialize the global tracing subscriber with JSON formatting and environment-based filtering.
///
/// # Arguments
///
/// * `log_level` - A log level string (e.g., "info", "debug", "warn") that will be used to create an EnvFilter.
///
/// # Errors
///
/// Returns `crate::Error::Observability` if:
/// - The log level string is invalid and cannot be parsed into an EnvFilter
/// - The tracing subscriber fails to initialize (e.g., already initialized)
///
/// # Examples
///
/// ```no_run
/// # use thufir::observability::init_observability;
/// # fn main() -> thufir::Result<()> {
/// init_observability("info")?;
/// # Ok(())
/// # }
/// ```
pub fn init_observability(log_level: &str) -> crate::Result<()> {
    let filter = EnvFilter::try_new(log_level)
        .map_err(|e| crate::Error::Observability(format!("invalid log level: {}", e)))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .try_init()
        .map_err(|e| crate::Error::Observability(format!("failed to init tracing: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that a valid log level (e.g., "info") can be parsed into an EnvFilter.
    #[test]
    fn observability_valid_log_level() {
        let result = EnvFilter::try_new("info");
        assert!(result.is_ok());
    }

    /// Test that an invalid log level returns an error instead of panicking.
    #[test]
    fn observability_invalid_log_level() {
        // EnvFilter::try_new with an invalid directive syntax should fail
        // Use a directive that's syntactically invalid (e.g., contains invalid characters)
        let result = EnvFilter::try_new("not_a_valid_level_xyz_12345=invalid");
        assert!(result.is_err());
    }
}
