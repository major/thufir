//! Ping command for the Thufir bot.

#![allow(missing_docs)]

use super::{Context, Error, ensure_channel_allowed};

/// Returns the ping response message.
///
/// # Returns
/// A static string containing the ping response.
pub fn ping_response() -> &'static str {
    "Pong!"
}

/// Ping command that responds with "Pong!".
///
/// # Arguments
/// * `ctx` - The command context.
///
/// # Returns
/// A result indicating success or an error.
#[allow(missing_docs)]
#[poise::command(slash_command, guild_only)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    if !ensure_channel_allowed(&ctx, &ctx.data().commands.ping.allowed_channels).await? {
        return Ok(());
    }

    ctx.say(ping_response()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that ping_response returns the expected message.
    #[test]
    fn ping_response_test() {
        assert!(ping_response().starts_with("Pong!"));
    }

    /// Test that the ping command is registered in the command list.
    #[test]
    fn command_list_contains_ping() {
        let commands = super::super::get_commands();
        assert_eq!(commands.len(), 2);
        assert!(commands.iter().any(|c| c.name == "ping"));
    }
}
