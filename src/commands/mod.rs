//! Command handlers and CLI logic for the Thufir bot.
//!
//! This module provides the poise command framework setup, including command registration,
//! error handling, and type aliases for the bot's command context.

pub mod ping;
pub mod trade_dashboard;

/// Empty data type for the command framework. Will be extended in Task 10 with AppState.
pub type Data = ();

/// Error type for command operations.
pub type Error = crate::Error;

/// Context type for poise commands.
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Returns the list of all available commands.
///
/// # Returns
/// A vector of poise commands registered with the bot.
pub fn get_commands() -> Vec<poise::Command<Data, Error>> {
    vec![ping::ping()]
}

/// Handles errors that occur during command execution.
///
/// Logs the error and attempts to send a user-safe error message if a context is available.
///
/// # Arguments
/// * `error` - The framework error that occurred.
pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    tracing::error!("Command error: {:?}", error);
    // Try to send user-facing error message if context is available
    if let Some(ctx) = error.ctx() {
        let _ = ctx.say("An error occurred. Please try again.").await;
    }
}
