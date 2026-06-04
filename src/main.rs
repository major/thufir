//! Thufir Discord bot entry point.
//!
//! Handles startup, configuration loading, and the poise framework lifecycle.

use std::sync::Arc;

use clap::Parser;
use poise::serenity_prelude as serenity;
use tokio::sync::RwLock;
use tracing::info;

use thufir::commands;
use thufir::config::Config;
use thufir::data_sources::volumeleaders::VolumeLeadersManager;
use thufir::observability::init_observability;
use thufir::state::AppState;

/// Thufir - a Discord bot for tracking VolumeLeaders market data.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Path to an optional TOML configuration file.
    #[arg(long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> thufir::Result<()> {
    // Load .env file if present (ignore errors when missing).
    dotenvy::dotenv().ok();

    // Parse CLI args first so --help works without any secrets.
    let cli = Cli::parse();

    // Load configuration from environment variables and optional TOML file.
    let config = Config::load(cli.config.as_deref())?;

    // Initialize tracing subscriber.
    init_observability(&config.bot.log_level)?;

    info!("Starting Thufir bot");

    // Resolve VolumeLeaders credentials and perform startup login.
    let creds = rusty_volumeleaders::resolve_credentials()
        .map_err(|e| thufir::Error::VolumeLeaders(e.to_string()))?;
    let manager = VolumeLeadersManager::new(
        creds.credentials().username().to_owned(),
        creds.credentials().password().to_owned(),
    )
    .await?;

    info!("VolumeLeaders login successful");

    // Build shared application state.
    let app_state = AppState {
        vl_manager: Arc::new(RwLock::new(manager)),
    };

    let guild_id = serenity::GuildId::new(config.discord.guild_id);

    // Build the poise framework with guild-only command registration.
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::get_commands(),
            on_error: |error| Box::pin(commands::on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id)
                    .await?;
                info!("Commands registered in guild {guild_id}");
                Ok(app_state)
            })
        })
        .build();

    // Build the serenity client with minimal gateway intents.
    let intents = serenity::GatewayIntents::non_privileged();
    let mut client = serenity::ClientBuilder::new(&config.discord.token, intents)
        .framework(framework)
        .await
        .map_err(|e| thufir::Error::Discord(e.to_string()))?;

    // Run the bot with graceful shutdown on Ctrl+C.
    tokio::select! {
        biased;
        result = client.start() => {
            result.map_err(|e| thufir::Error::Discord(e.to_string()))?;
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down");
            client.shard_manager.shutdown_all().await;
        }
    }

    Ok(())
}
