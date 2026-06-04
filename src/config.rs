//! Configuration management for the Thufir bot.
//!
//! Loads configuration from environment variables and optional TOML file.
//! Environment variables take precedence over TOML settings.
//!
//! Secret environment variables:
//! - `DISCORD_TOKEN`: Discord bot token
//!
//! Non-secret settings may be provided by TOML or `THUFIR_` environment variables.
//! Environment variables override TOML settings.
//!
//! Optional environment variables:
//! - `THUFIR_BOT__LOG_LEVEL`: Log level (default: "info")
//! - `THUFIR_BOT__TIMEZONE`: Timezone (default: "America/New_York")
//! - `THUFIR_COMMANDS__PING__ALLOWED_CHANNELS`: Channel IDs where `/ping` may run
//! - `THUFIR_DISCORD__GUILD_ID`: Discord guild ID
//! - `THUFIR_COMMANDS__TRADE_DASHBOARD__ALLOWED_CHANNELS`: Channel IDs where `/trade-dashboard` may run
//! - `THUFIR_VOLUME_LEADERS__DASHBOARD_DAYS`: Dashboard lookback days (default: 365)
//! - `THUFIR_VOLUME_LEADERS__DASHBOARD_COUNT`: Dashboard top count (default: 10)

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Top-level configuration for the Thufir bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Bot-level settings.
    pub bot: BotConfig,
    /// Discord-specific settings.
    pub discord: DiscordConfig,
    /// Slash command-specific settings.
    #[serde(default)]
    pub commands: CommandsConfig,
    /// Volume leaders dashboard settings.
    pub volume_leaders: VolumeLeadersConfig,
}

/// Bot-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Log level for the application.
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Timezone for the bot.
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

/// Discord-specific configuration.
#[derive(Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Discord bot token loaded only from `DISCORD_TOKEN`.
    #[serde(skip)]
    pub token: String,
    /// Discord guild ID.
    pub guild_id: u64,
}

impl fmt::Debug for DiscordConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordConfig")
            .field("token", &"<redacted>")
            .field("guild_id", &self.guild_id)
            .finish()
    }
}

/// Slash command-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandsConfig {
    /// Settings for the `/ping` command.
    #[serde(default)]
    pub ping: CommandConfig,
    /// Settings for the `/trade-dashboard` command.
    #[serde(default)]
    pub trade_dashboard: CommandConfig,
}

/// Configuration shared by individual slash commands.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandConfig {
    /// Channel IDs where the command may be used. Empty means all channels are allowed.
    #[serde(default)]
    pub allowed_channels: Vec<u64>,
}

/// Volume leaders dashboard configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeLeadersConfig {
    /// Number of days to look back for dashboard data.
    #[serde(default = "default_dashboard_days")]
    pub dashboard_days: u32,
    /// Number of top volume leaders to display.
    #[serde(default = "default_dashboard_count")]
    pub dashboard_count: u32,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_timezone() -> String {
    "America/New_York".to_string()
}

fn default_dashboard_days() -> u32 {
    365
}

fn default_dashboard_count() -> u32 {
    10
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bot: BotConfig {
                log_level: default_log_level(),
                timezone: default_timezone(),
            },
            discord: DiscordConfig {
                token: String::new(),
                guild_id: 0,
            },
            commands: CommandsConfig::default(),
            volume_leaders: VolumeLeadersConfig {
                dashboard_days: default_dashboard_days(),
                dashboard_count: default_dashboard_count(),
            },
        }
    }
}

impl Config {
    /// Load configuration from environment and optional TOML file.
    ///
    /// # Arguments
    ///
    /// * `config_path` - Optional path to a TOML configuration file. If not provided,
    ///   only environment variables are used.
    ///
    /// # Errors
    ///
    /// Returns a configuration error if:
    /// - `DISCORD_TOKEN` environment variable is not set
    /// - Discord guild ID is not set in TOML or `THUFIR_DISCORD__GUILD_ID`
    /// - The TOML file cannot be parsed
    /// - Environment variables cannot be parsed
    pub fn load(config_path: Option<&str>) -> crate::Result<Self> {
        // Start with defaults
        let mut figment = Figment::from(Serialized::defaults(Config::default()));

        // Merge TOML file if provided
        if let Some(path) = config_path {
            figment = figment.merge(Toml::file(path));
        }

        // Merge environment variables with THUFIR_ prefix
        figment = figment.merge(Env::prefixed("THUFIR_").split("__"));

        // Extract the config, but we still need to handle DISCORD_TOKEN separately
        let mut config: Config = figment
            .extract()
            .map_err(|e| crate::Error::Config(e.to_string()))?;

        // Load DISCORD_TOKEN from environment (not THUFIR_ prefixed)
        config.discord.token = std::env::var("DISCORD_TOKEN").map_err(|_| {
            crate::Error::Config("DISCORD_TOKEN environment variable not set".to_string())
        })?;

        // Validate that guild_id was set through TOML or THUFIR_ env config.
        if config.discord.guild_id == 0 {
            return Err(crate::Error::Config(
                "discord.guild_id must be set in TOML or THUFIR_DISCORD__GUILD_ID".to_string(),
            ));
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify environment variables
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn config_env_only_loads() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Set required environment variables
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token_12345");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "987654321");
        }

        let config = Config::load(None);
        assert!(config.is_ok(), "Config should load with env vars");

        let config = config.unwrap();
        assert_eq!(config.discord.token, "test_token_12345");
        assert_eq!(config.discord.guild_id, 987654321);
        assert_eq!(config.bot.log_level, "info");
        assert_eq!(config.bot.timezone, "America/New_York");
        assert!(config.commands.ping.allowed_channels.is_empty());
        assert!(config.commands.trade_dashboard.allowed_channels.is_empty());
        assert_eq!(config.volume_leaders.dashboard_days, 365);
        assert_eq!(config.volume_leaders.dashboard_count, 10);

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_missing_discord_token() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Remove DISCORD_TOKEN if it exists
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "987654321");
        }

        let config = Config::load(None);
        assert!(config.is_err(), "Config should fail without DISCORD_TOKEN");

        let err = config.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("DISCORD_TOKEN"),
            "Error message should mention DISCORD_TOKEN, got: {}",
            err_msg
        );

        // Clean up
        unsafe {
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_missing_guild_id() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Set DISCORD_TOKEN but not guild ID
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token_12345");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }

        let config = Config::load(None);
        assert!(
            config.is_err(),
            "Config should fail without discord.guild_id"
        );

        let err = config.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("discord.guild_id"),
            "Error message should mention discord.guild_id, got: {}",
            err_msg
        );

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
        }
    }

    #[test]
    fn config_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();

        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "123");
        }

        let config = Config::load(None).expect("Config should load");

        assert_eq!(config.bot.log_level, "info");
        assert_eq!(config.bot.timezone, "America/New_York");
        assert!(config.commands.ping.allowed_channels.is_empty());
        assert!(config.commands.trade_dashboard.allowed_channels.is_empty());
        assert_eq!(config.volume_leaders.dashboard_days, 365);
        assert_eq!(config.volume_leaders.dashboard_count, 10);

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_env_overrides_toml() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Create a temporary TOML file with guild_id=111
        let toml_content = r#"
[discord]
guild_id = 111

[bot]
log_level = "debug"
"#;
        let temp_path = "/tmp/test_config_override.toml";
        std::fs::write(temp_path, toml_content).expect("Failed to write temp TOML");

        // Set env var to override TOML value
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token_override");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "999");
            std::env::set_var("THUFIR_BOT__LOG_LEVEL", "warn");
        }

        let config = Config::load(Some(temp_path)).expect("Config should load with TOML and env");

        // Env vars should override TOML values
        assert_eq!(
            config.discord.guild_id, 999,
            "Env var should override TOML guild_id"
        );
        assert_eq!(
            config.bot.log_level, "warn",
            "Env var should override TOML log_level"
        );
        assert_eq!(config.discord.token, "test_token_override");

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
            std::env::remove_var("THUFIR_BOT__LOG_LEVEL");
        }
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn config_invalid_guild_id() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Set guild_id to a non-numeric value
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "not-a-number");
        }

        let config = Config::load(None);
        assert!(
            config.is_err(),
            "Config should fail with non-numeric guild_id"
        );

        let err = config.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("guild_id") || err_msg.contains("invalid"),
            "Error message should mention guild_id or invalid, got: {}",
            err_msg
        );

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_absent_config_file() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Set required env vars
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "555");
        }

        // Try to load with a nonexistent TOML file path
        let config = Config::load(Some("/nonexistent/path/config.toml"));
        assert!(
            config.is_ok(),
            "Config should load even with absent TOML file (optional)"
        );

        let config = config.unwrap();
        assert_eq!(config.discord.guild_id, 555);
        assert_eq!(config.discord.token, "test_token");

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_dashboard_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Set only required env vars, no dashboard config
        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token");
            std::env::set_var("THUFIR_DISCORD__GUILD_ID", "777");
        }

        let config = Config::load(None).expect("Config should load");

        // Verify dashboard defaults are applied
        assert_eq!(
            config.volume_leaders.dashboard_days, 365,
            "dashboard_days should default to 365"
        );
        assert_eq!(
            config.volume_leaders.dashboard_count, 10,
            "dashboard_count should default to 10"
        );

        // Clean up
        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }
    }

    #[test]
    fn config_loads_command_channel_allow_lists_from_toml() {
        let _guard = ENV_LOCK.lock().unwrap();

        let toml_content = r#"
[discord]
guild_id = 4242

[commands.ping]
allowed_channels = [111, 222]

[commands.trade_dashboard]
allowed_channels = [333]
"#;
        let temp_path = format!("/tmp/thufir_command_channels_{}.toml", std::process::id());
        std::fs::write(&temp_path, toml_content).expect("Failed to write temp TOML");

        unsafe {
            std::env::set_var("DISCORD_TOKEN", "test_token");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }

        let config = Config::load(Some(&temp_path)).expect("Config should load");

        assert_eq!(config.discord.guild_id, 4242);
        assert_eq!(config.commands.ping.allowed_channels, vec![111, 222]);
        assert_eq!(config.commands.trade_dashboard.allowed_channels, vec![333]);

        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
        }
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn config_ignores_discord_token_from_toml() {
        let _guard = ENV_LOCK.lock().unwrap();

        let toml_content = r#"
[discord]
token = "toml_token_should_not_be_used"
guild_id = 4242
"#;
        let temp_path = format!("/tmp/thufir_token_source_{}.toml", std::process::id());
        std::fs::write(&temp_path, toml_content).expect("Failed to write temp TOML");

        unsafe {
            std::env::set_var("DISCORD_TOKEN", "env_token");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }

        let config = Config::load(Some(&temp_path)).expect("Config should load");

        assert_eq!(config.discord.token, "env_token");
        assert_eq!(config.discord.guild_id, 4242);

        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
        }
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn config_env_overrides_command_channel_allow_lists() {
        let _guard = ENV_LOCK.lock().unwrap();

        let toml_content = r#"
[discord]
guild_id = 4242

[commands.ping]
allowed_channels = [111]
"#;
        let temp_path = format!("/tmp/thufir_channel_env_{}.toml", std::process::id());
        std::fs::write(&temp_path, toml_content).expect("Failed to write temp TOML");

        unsafe {
            std::env::set_var("DISCORD_TOKEN", "env_token");
            std::env::set_var("THUFIR_COMMANDS__PING__ALLOWED_CHANNELS", "[222, 333]");
            std::env::remove_var("THUFIR_DISCORD__GUILD_ID");
        }

        let config = Config::load(Some(&temp_path)).expect("Config should load");

        assert_eq!(config.commands.ping.allowed_channels, vec![222, 333]);

        unsafe {
            std::env::remove_var("DISCORD_TOKEN");
            std::env::remove_var("THUFIR_COMMANDS__PING__ALLOWED_CHANNELS");
        }
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn discord_config_debug_redacts_token() {
        let config = DiscordConfig {
            token: "secret_token".to_owned(),
            guild_id: 4242,
        };

        let debug = format!("{config:?}");

        assert!(!debug.contains("secret_token"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("4242"));
    }
}
