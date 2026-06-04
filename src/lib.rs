//! Core library for the Thufir Discord bot.
//!
//! Thufir provides a Discord bot for tracking volume leaders and market data.

#![deny(missing_docs)]

/// Configuration management for the bot.
pub mod config;

/// Error types and result type for the crate.
pub mod error;

/// Observability and logging infrastructure.
pub mod observability;

/// Application state management.
pub mod state;

/// Data source integrations.
pub mod data_sources;

/// Dashboard and UI components.
pub mod dashboard;

/// Command handlers and CLI logic.
pub mod commands;

pub use error::{Error, Result};
