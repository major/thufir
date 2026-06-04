//! Command handlers and CLI logic for the Thufir bot.
//!
//! This module provides the poise command framework setup, including command registration,
//! error handling, and type aliases for the bot's command context.

pub mod ping;
pub mod trade_dashboard;

/// Shared application state passed to every command via the poise framework.
pub type Data = crate::state::AppState;

/// Error type for command operations.
pub type Error = crate::Error;

/// Context type for poise commands.
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Returns whether a command is allowed in the current channel.
///
/// Empty allow-lists are treated as unrestricted.
///
/// # Arguments
/// * `channel_id` - The Discord channel ID where the command was invoked.
/// * `allowed_channels` - Channel IDs where the command may run.
#[must_use]
pub fn is_channel_allowed(channel_id: u64, allowed_channels: &[u64]) -> bool {
    allowed_channels.is_empty() || allowed_channels.contains(&channel_id)
}

/// Builds the user-facing denial message for a channel-restricted command.
///
/// # Arguments
/// * `allowed_channels` - Channel IDs where the command may run.
#[must_use]
pub fn channel_denial_message(allowed_channels: &[u64]) -> String {
    let allowed = allowed_channels
        .iter()
        .map(|channel_id| format!("<#{channel_id}>"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("This command can only be used in these channels: {allowed}")
}

/// Ensures a command may run in the current channel.
///
/// Sends an ephemeral denial and returns `Ok(false)` when the command is restricted.
///
/// # Arguments
/// * `ctx` - The command context.
/// * `allowed_channels` - Channel IDs where the command may run. Empty means unrestricted.
///
/// # Errors
/// Returns an error if Discord rejects the denial response.
pub async fn ensure_channel_allowed(
    ctx: &Context<'_>,
    allowed_channels: &[u64],
) -> Result<bool, Error> {
    if is_channel_allowed(ctx.channel_id().get(), allowed_channels) {
        return Ok(true);
    }

    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .content(channel_denial_message(allowed_channels)),
    )
    .await?;
    Ok(false)
}

/// Returns the list of all available commands.
///
/// # Returns
/// A vector of poise commands registered with the bot.
pub fn get_commands() -> Vec<poise::Command<Data, Error>> {
    vec![ping::ping(), trade_dashboard::trade_dashboard()]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_channel_allow_list_allows_any_channel() {
        assert!(is_channel_allowed(123, &[]));
    }

    #[test]
    fn populated_channel_allow_list_allows_matching_channel() {
        assert!(is_channel_allowed(222, &[111, 222, 333]));
    }

    #[test]
    fn populated_channel_allow_list_rejects_unknown_channel() {
        assert!(!is_channel_allowed(444, &[111, 222, 333]));
    }

    #[test]
    fn channel_denial_message_mentions_allowed_channels() {
        let message = channel_denial_message(&[111, 222]);

        assert!(message.contains("<#111>"));
        assert!(message.contains("<#222>"));
    }
}
