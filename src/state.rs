//! Application state management for the Thufir bot.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::data_sources::volumeleaders::VolumeLeadersManager;

/// Shared application state for the Thufir bot.
///
/// Holds long-lived service clients behind `Arc<RwLock<...>>` so they
/// can be shared across async tasks and re-authenticated when needed.
pub struct AppState {
    /// VolumeLeaders API session manager.
    pub vl_manager: Arc<RwLock<VolumeLeadersManager>>,
}
