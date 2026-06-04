//! Application state management for the Thufir bot.

use std::fmt;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::config::CommandsConfig;
use crate::data_sources::volumeleaders::VolumeLeadersManager;

/// Shared application state for the Thufir bot.
///
/// Holds long-lived service clients behind `Arc<RwLock<...>>` so they
/// can be shared across async tasks and re-authenticated when needed.
pub struct AppState {
    /// Slash command-specific settings.
    pub commands: CommandsConfig,
    /// VolumeLeaders API session manager.
    pub vl_manager: Arc<RwLock<VolumeLeadersManager>>,
}

impl fmt::Debug for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
            .field("commands", &self.commands)
            .field("vl_manager", &"<VolumeLeadersManager>")
            .finish()
    }
}
