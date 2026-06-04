//! Trade dashboard command for the Thufir bot.
//!
//! Provides a `/trade-dashboard` slash command that fetches VolumeLeaders data
//! for a given ticker and renders it as a Discord embed.

#![allow(missing_docs)]

use chrono::Utc;
use rusty_volumeleaders::{
    TradeClusterBombsRequest, TradeClustersRequest, TradeLevelsRequest, TradesRequest,
};

use crate::dashboard::build_dashboard_embed;

use super::Context;

/// Validate and normalize a ticker symbol.
///
/// Converts to uppercase and rejects empty strings, strings exceeding 10 chars,
/// or strings containing non-ASCII-alphanumeric characters.
///
/// # Errors
///
/// Returns an error message string if the ticker is invalid.
pub fn normalize_ticker(ticker: &str) -> Result<String, String> {
    let trimmed = ticker.trim();
    if trimmed.is_empty() {
        return Err("Ticker must not be empty.".to_owned());
    }
    if trimmed.len() > 10 {
        return Err("Ticker must be at most 10 characters.".to_owned());
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Ticker must contain only ASCII letters and digits.".to_owned());
    }
    Ok(trimmed.to_ascii_uppercase())
}

/// Validate that `days` is within 1..=3650.
///
/// # Errors
///
/// Returns an error message string if out of range.
pub fn validate_days(days: u32) -> Result<u32, String> {
    if !(1..=3650).contains(&days) {
        return Err(format!("Days must be between 1 and 3650, got {days}."));
    }
    Ok(days)
}

/// Validate that `count` is within 1..=25.
///
/// # Errors
///
/// Returns an error message string if out of range.
pub fn validate_count(count: u32) -> Result<u32, String> {
    if !(1..=25).contains(&count) {
        return Err(format!("Count must be between 1 and 25, got {count}."));
    }
    Ok(count)
}

/// Compute date filter strings from a days-back parameter.
///
/// Returns `(start_date, end_date)` formatted as `MM/DD/YYYY`.
fn compute_date_range(days: u32) -> (String, String) {
    let end = Utc::now();
    let start = end - chrono::Duration::days(i64::from(days));
    (
        start.format("%m/%d/%Y").to_string(),
        end.format("%m/%d/%Y").to_string(),
    )
}

/// Build the common filter list used by trades and clusters requests.
fn ticker_date_filters(ticker: &str, days: u32) -> Vec<(String, String)> {
    let (start, end) = compute_date_range(days);
    vec![
        ("Tickers".to_owned(), ticker.to_owned()),
        ("StartDate".to_owned(), start),
        ("EndDate".to_owned(), end),
        ("Sort".to_owned(), "Dollars".to_owned()),
    ]
}

/// Show the VolumeLeaders trade dashboard for a ticker.
#[allow(missing_docs)]
#[poise::command(slash_command, guild_only, rename = "trade-dashboard")]
pub async fn trade_dashboard(
    ctx: Context<'_>,
    #[description = "Stock ticker symbol (e.g. AAPL)"] ticker: String,
    #[description = "Number of days to look back (1-3650, default 365)"] days: Option<u32>,
    #[description = "Number of results per section (1-25, default 10)"] count: Option<u32>,
) -> Result<(), crate::Error> {
    // Validate inputs before doing any work.
    let ticker = match normalize_ticker(&ticker) {
        Ok(t) => t,
        Err(msg) => {
            ctx.send(poise::CreateReply::default().ephemeral(true).content(msg))
                .await?;
            return Ok(());
        }
    };

    let days = match validate_days(days.unwrap_or(365)) {
        Ok(d) => d,
        Err(msg) => {
            ctx.send(poise::CreateReply::default().ephemeral(true).content(msg))
                .await?;
            return Ok(());
        }
    };

    let count = match validate_count(count.unwrap_or(10)) {
        Ok(c) => c,
        Err(msg) => {
            ctx.send(poise::CreateReply::default().ephemeral(true).content(msg))
                .await?;
            return Ok(());
        }
    };

    // Defer the response since VolumeLeaders calls take time.
    ctx.defer().await?;

    // Grab the VolumeLeaders manager from app state.
    let data = ctx.data();
    let vl_manager = &data.vl_manager;
    let manager = vl_manager.read().await;

    // Build date filters for all requests.
    let filters = ticker_date_filters(&ticker, days);
    let (start_date, end_date) = compute_date_range(days);

    // Build requests using the builder API.
    let trades_req = TradesRequest::new()
        .with_trade_filters(filters.clone())
        .with_length(count as i32);
    let clusters_req = TradeClustersRequest::new()
        .with_cluster_filters(filters.clone())
        .with_length(count as i32);
    let levels_req = TradeLevelsRequest::new().with_chart_filters(
        &ticker,
        &start_date,
        &end_date,
        count as usize,
    );
    let bombs_req = TradeClusterBombsRequest::new()
        .with_cluster_bomb_filters(filters)
        .with_length(count as i32);

    // Fetch all four data sources.
    let trades = manager.get_trades(&trades_req).await?;
    let clusters = manager.get_trade_clusters(&clusters_req).await?;
    let levels = manager.get_chart0_trade_levels(&levels_req).await?;
    let bombs = manager.get_trade_cluster_bombs(&bombs_req).await?;

    drop(manager);

    // Build the dashboard embed.
    let embed = build_dashboard_embed(
        &ticker,
        &trades.data,
        &clusters.data,
        &levels.data,
        &bombs.data,
        count as usize,
    );

    // Send the embed as a reply.
    let mut reply = poise::CreateReply::default();
    let mut discord_embed = serenity::all::CreateEmbed::default().title(&embed.title);
    for field in &embed.fields {
        discord_embed = discord_embed.field(&field.name, &field.value, field.inline);
    }
    reply = reply.embed(discord_embed);

    ctx.send(reply).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_ticker tests ---

    /// Test that a lowercase ticker is uppercased.
    #[test]
    fn trade_dashboard_valid_request() {
        let result = normalize_ticker("aapl");
        assert_eq!(result.unwrap(), "AAPL");
    }

    /// Test that mixed case is uppercased.
    #[test]
    fn trade_dashboard_mixed_case() {
        assert_eq!(normalize_ticker("AaPl").unwrap(), "AAPL");
    }

    /// Test that already-uppercase is passed through.
    #[test]
    fn trade_dashboard_uppercase_passthrough() {
        assert_eq!(normalize_ticker("MSFT").unwrap(), "MSFT");
    }

    /// Test that an empty ticker is rejected.
    #[test]
    fn trade_dashboard_rejects_empty_ticker() {
        let result = normalize_ticker("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    /// Test that whitespace-only ticker is rejected.
    #[test]
    fn trade_dashboard_rejects_whitespace_ticker() {
        let result = normalize_ticker("   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    /// Test that a ticker exceeding 10 chars is rejected.
    #[test]
    fn trade_dashboard_rejects_invalid_ticker() {
        let result = normalize_ticker("ABCDEFGHIJK");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("10"));
    }

    /// Test that non-alphanumeric chars are rejected.
    #[test]
    fn trade_dashboard_rejects_special_chars() {
        let result = normalize_ticker("AA$PL");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ASCII"));
    }

    /// Test that numeric tickers are allowed.
    #[test]
    fn trade_dashboard_allows_numeric_ticker() {
        assert_eq!(normalize_ticker("6758").unwrap(), "6758");
    }

    /// Test that alphanumeric tickers are allowed.
    #[test]
    fn trade_dashboard_allows_alphanumeric_ticker() {
        assert_eq!(normalize_ticker("brk2").unwrap(), "BRK2");
    }

    // --- validate_days tests ---

    /// Test valid days values.
    #[test]
    fn trade_dashboard_valid_days() {
        assert_eq!(validate_days(1).unwrap(), 1);
        assert_eq!(validate_days(365).unwrap(), 365);
        assert_eq!(validate_days(3650).unwrap(), 3650);
    }

    /// Test that days=0 is rejected.
    #[test]
    fn trade_dashboard_rejects_zero_days() {
        assert!(validate_days(0).is_err());
    }

    /// Test that days over 3650 is rejected.
    #[test]
    fn trade_dashboard_rejects_excessive_days() {
        assert!(validate_days(3651).is_err());
    }

    // --- validate_count tests ---

    /// Test valid count values.
    #[test]
    fn trade_dashboard_valid_count() {
        assert_eq!(validate_count(1).unwrap(), 1);
        assert_eq!(validate_count(10).unwrap(), 10);
        assert_eq!(validate_count(25).unwrap(), 25);
    }

    /// Test that count=0 is rejected.
    #[test]
    fn trade_dashboard_rejects_invalid_count() {
        assert!(validate_count(0).is_err());
    }

    /// Test that count=26 is rejected.
    #[test]
    fn trade_dashboard_rejects_excessive_count() {
        assert!(validate_count(26).is_err());
    }

    /// Test that the default count=10 is valid.
    #[test]
    fn trade_dashboard_default_count_valid() {
        assert_eq!(validate_count(10).unwrap(), 10);
    }

    /// Test that the default days=365 is valid.
    #[test]
    fn trade_dashboard_default_days_valid() {
        assert_eq!(validate_days(365).unwrap(), 365);
    }
}
