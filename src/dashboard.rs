//! Dashboard and UI components for the Thufir bot.
//!
//! Provides purpose-built DTOs and rendering logic for building Discord embeds
//! from VolumeLeaders trade data.

use std::fmt::Display;

use rusty_volumeleaders::{Trade, TradeCluster, TradeClusterBomb, TradeLevel};

/// Maximum characters allowed in a single Discord embed field value.
const FIELD_CHAR_LIMIT: usize = 1024;

/// Maximum total characters across all embed field values.
const TOTAL_CHAR_LIMIT: usize = 6000;

/// Placeholder text for sections with no data.
const NO_DATA: &str = "No data available";

/// Fallback text for missing optional fields.
const NA: &str = "N/A";

/// A Discord embed representing the VolumeLeaders dashboard for a ticker.
#[derive(Debug, Clone)]
pub struct DashboardEmbed {
    /// Embed title, e.g. "VolumeLeaders dashboard: AAPL".
    pub title: String,
    /// Exactly four fields: Trades, Clusters, Levels, Cluster Bombs.
    pub fields: Vec<EmbedField>,
}

/// A single field within a [`DashboardEmbed`].
#[derive(Debug, Clone)]
pub struct EmbedField {
    /// Section name (e.g. "Trades").
    pub name: String,
    /// Formatted rows, capped at the per-field character limit.
    pub value: String,
    /// Whether this field renders inline. Always `false` for dashboard fields.
    pub inline: bool,
}

/// Build a Discord embed from dashboard data.
///
/// Each section is rendered from the first `count` items of the corresponding
/// slice. Field values are truncated to the per-field character limit, and the
/// total text across all fields is capped at the embed-wide character limit.
///
/// # Arguments
///
/// * `ticker` - The stock ticker (rendered uppercase in the title).
/// * `trades` - Trade rows to display.
/// * `clusters` - Trade cluster rows to display.
/// * `levels` - Trade level rows to display.
/// * `bombs` - Trade cluster bomb rows to display.
/// * `count` - Maximum number of rows per section.
pub fn build_dashboard_embed(
    ticker: &str,
    trades: &[Trade],
    clusters: &[TradeCluster],
    levels: &[TradeLevel],
    bombs: &[TradeClusterBomb],
    count: usize,
) -> DashboardEmbed {
    let title = format!("VolumeLeaders dashboard: {}", ticker.to_uppercase());

    let mut fields = vec![
        build_field("Trades", trades, count, format_trade),
        build_field("Clusters", clusters, count, format_cluster),
        build_field("Levels", levels, count, format_level),
        build_field("Cluster Bombs", bombs, count, format_bomb),
    ];

    enforce_total_limit(&mut fields);

    DashboardEmbed { title, fields }
}

/// Build a single embed field from a slice of items.
fn build_field<T>(
    name: &str,
    items: &[T],
    count: usize,
    formatter: fn(&T) -> String,
) -> EmbedField {
    let value = if items.is_empty() {
        NO_DATA.to_owned()
    } else {
        let rows: Vec<String> = items.iter().take(count).map(formatter).collect();
        truncate_rows(&rows, items.len().saturating_sub(count))
    };

    EmbedField {
        name: name.to_owned(),
        value,
        inline: false,
    }
}

/// Join rows with newlines, truncating to [`FIELD_CHAR_LIMIT`].
///
/// If the joined text exceeds the limit, rows are dropped from the end and a
/// `... (+N more)` suffix is appended.
fn truncate_rows(rows: &[String], extra_remaining: usize) -> String {
    let joined = rows.join("\n");
    if joined.len() <= FIELD_CHAR_LIMIT {
        return joined;
    }

    // Find how many rows fit within the limit, reserving space for the suffix.
    let mut included = 0;
    let mut total_len = 0;

    for (i, row) in rows.iter().enumerate() {
        let remaining = rows.len() - (i + 1) + extra_remaining;
        let suffix = format!("\n... (+{remaining} more)");
        let sep = if i > 0 { 1 } else { 0 }; // newline separator
        let needed = total_len + sep + row.len() + suffix.len();

        if needed > FIELD_CHAR_LIMIT {
            break;
        }

        total_len += sep + row.len();
        included += 1;
    }

    // Ensure we include at least one row even if it's long.
    if included == 0 {
        included = 1;
    }

    let kept: Vec<&str> = rows.iter().take(included).map(String::as_str).collect();
    let remaining = rows.len() - included + extra_remaining;
    format!("{}\n... (+{remaining} more)", kept.join("\n"))
}

/// Enforce the total embed character limit across all fields.
///
/// If the sum of all field values exceeds [`TOTAL_CHAR_LIMIT`], the longest
/// field is re-truncated until the total fits.
fn enforce_total_limit(fields: &mut [EmbedField]) {
    loop {
        let total: usize = fields.iter().map(|f| f.value.len()).sum();
        if total <= TOTAL_CHAR_LIMIT {
            break;
        }

        // Find the longest field.
        let longest_idx = fields
            .iter()
            .enumerate()
            .max_by_key(|(_, f)| f.value.len())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let field = &mut fields[longest_idx];
        let overshoot = total - TOTAL_CHAR_LIMIT;
        let target = field.value.len().saturating_sub(overshoot);

        // Truncate at a newline boundary if possible.
        let truncated = if let Some(pos) = field.value[..target].rfind('\n') {
            let kept = &field.value[..pos];
            // Count remaining lines.
            let total_lines = field.value.lines().count();
            let kept_lines = kept.lines().count();
            let remaining = total_lines - kept_lines;
            if remaining > 0 {
                format!("{kept}\n... (+{remaining} more)")
            } else {
                kept.to_owned()
            }
        } else {
            // Single line or no newline found - hard truncate.
            let cut = target.min(field.value.len());
            let s = &field.value[..cut];
            format!("{s}...")
        };

        field.value = truncated;
    }
}

/// Format a single trade row.
fn format_trade(trade: &Trade) -> String {
    let rank = trade
        .trade_rank
        .map_or_else(|| NA.to_owned(), |r| format!("#{r}"));
    let time = trade.full_time_string_24.as_deref().unwrap_or(NA);
    let price = format_decimal(trade.price.as_ref());
    let dollars = format_decimal(trade.dollars.as_ref());
    format!("{rank} {time} ${price} ${dollars}M")
}

/// Format a single cluster row.
fn format_cluster(cluster: &TradeCluster) -> String {
    let rank = cluster
        .trade_cluster_rank
        .map_or_else(|| NA.to_owned(), |r| format!("#{r}"));
    let min_time = cluster.min_full_time_string_24.as_deref().unwrap_or(NA);
    let max_time = cluster.max_full_time_string_24.as_deref().unwrap_or(NA);
    let price = format_decimal(cluster.price.as_ref());
    let dollars = format_decimal(cluster.dollars.as_ref());
    format!("{rank} {min_time}-{max_time} ${price} ${dollars}M")
}

/// Format a single level row.
fn format_level(level: &TradeLevel) -> String {
    let rank = level
        .trade_level_rank
        .map_or_else(|| NA.to_owned(), |r| format!("#{r}"));
    let price = format_decimal(level.price.as_ref());
    let dollars = format_decimal(level.dollars.as_ref());
    let touches = level
        .trade_level_touches
        .map_or_else(|| NA.to_owned(), |t| t.to_string());
    format!("{rank} ${price} ${dollars}M ({touches} touches)")
}

/// Format a single cluster bomb row.
fn format_bomb(bomb: &TradeClusterBomb) -> String {
    let rank = bomb
        .trade_cluster_bomb_rank
        .map_or_else(|| NA.to_owned(), |r| format!("#{r}"));
    let price = format_decimal(bomb.close_price.as_ref());
    let dollars = format_decimal(bomb.dollars.as_ref());
    format!("{rank} ${price} ${dollars}M")
}

/// Format an optional `Decimal` value with two decimal places, or `N/A`.
fn format_decimal<T: Display>(d: Option<&T>) -> String {
    d.map_or_else(|| NA.to_owned(), |v| format!("{v:.2}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deserialize a Trade from a JSON fragment with only the specified fields.
    fn make_trade(rank: i64, time: &str, price: i64, dollars: i64) -> Trade {
        serde_json::from_str(&format!(
            r#"{{"TradeRank":{rank},"FullTimeString24":"{time}","Price":{price},"Dollars":{dollars}}}"#
        ))
        .expect("valid trade JSON")
    }

    /// Deserialize an empty Trade (all None).
    fn empty_trade() -> Trade {
        serde_json::from_str("{}").expect("valid empty trade JSON")
    }

    /// Deserialize a TradeCluster from a JSON fragment.
    fn make_cluster(
        rank: i64,
        min_time: &str,
        max_time: &str,
        price: i64,
        dollars: i64,
    ) -> TradeCluster {
        serde_json::from_str(&format!(
            r#"{{"TradeClusterRank":{rank},"MinFullTimeString24":"{min_time}","MaxFullTimeString24":"{max_time}","Price":{price},"Dollars":{dollars}}}"#
        ))
        .expect("valid cluster JSON")
    }

    /// Deserialize an empty TradeCluster (all None).
    fn empty_cluster() -> TradeCluster {
        serde_json::from_str("{}").expect("valid empty cluster JSON")
    }

    /// Deserialize a TradeLevel from a JSON fragment.
    fn make_level(rank: i64, price: i64, dollars: i64, touches: i64) -> TradeLevel {
        serde_json::from_str(&format!(
            r#"{{"TradeLevelRank":{rank},"Price":{price},"Dollars":{dollars},"TradeLevelTouches":{touches}}}"#
        ))
        .expect("valid level JSON")
    }

    /// Deserialize an empty TradeLevel (all None).
    fn empty_level() -> TradeLevel {
        serde_json::from_str("{}").expect("valid empty level JSON")
    }

    /// Deserialize a TradeClusterBomb from a JSON fragment.
    fn make_bomb(rank: i64, price: i64, dollars: i64) -> TradeClusterBomb {
        serde_json::from_str(&format!(
            r#"{{"TradeClusterBombRank":{rank},"ClosePrice":{price},"Dollars":{dollars}}}"#
        ))
        .expect("valid bomb JSON")
    }

    /// Deserialize an empty TradeClusterBomb (all None).
    fn empty_bomb() -> TradeClusterBomb {
        serde_json::from_str("{}").expect("valid empty bomb JSON")
    }

    #[test]
    fn trade_dashboard_embed_happy_path() {
        let trades = vec![
            make_trade(1, "09:30:00", 150, 5000000),
            make_trade(2, "10:15:30", 151, 3000000),
            make_trade(3, "14:00:00", 149, 2000000),
        ];
        let clusters = vec![
            make_cluster(1, "09:30:00", "09:45:00", 150, 8000000),
            make_cluster(2, "13:00:00", "13:30:00", 152, 6000000),
        ];
        let levels = vec![
            make_level(1, 150, 10000000, 5),
            make_level(2, 155, 7000000, 3),
        ];
        let bombs = vec![make_bomb(1, 148, 12000000), make_bomb(2, 152, 9000000)];

        let embed = build_dashboard_embed("aapl", &trades, &clusters, &levels, &bombs, 10);

        assert_eq!(embed.title, "VolumeLeaders dashboard: AAPL");
        assert_eq!(embed.fields.len(), 4);

        for field in &embed.fields {
            assert!(!field.value.is_empty());
            assert_ne!(field.value, NO_DATA);
            assert!(
                field.value.len() <= FIELD_CHAR_LIMIT,
                "field '{}' exceeds {FIELD_CHAR_LIMIT} chars: {}",
                field.name,
                field.value.len()
            );
            assert!(!field.inline);
        }

        assert_eq!(embed.fields[0].name, "Trades");
        assert_eq!(embed.fields[1].name, "Clusters");
        assert_eq!(embed.fields[2].name, "Levels");
        assert_eq!(embed.fields[3].name, "Cluster Bombs");
    }

    #[test]
    fn trade_dashboard_embed_truncates() {
        // Create 100 rows per section with long-ish strings.
        let trades: Vec<Trade> = (1..=100)
            .map(|i| make_trade(i, "16:20:51", 999999, 888888888))
            .collect();
        let clusters: Vec<TradeCluster> = (1..=100)
            .map(|i| make_cluster(i, "09:30:00", "16:00:00", 999999, 888888888))
            .collect();
        let levels: Vec<TradeLevel> = (1..=100)
            .map(|i| make_level(i, 999999, 888888888, 99))
            .collect();
        let bombs: Vec<TradeClusterBomb> =
            (1..=100).map(|i| make_bomb(i, 999999, 888888888)).collect();

        let embed = build_dashboard_embed("aapl", &trades, &clusters, &levels, &bombs, 100);

        let mut has_more = false;
        let mut total_chars = 0;
        for field in &embed.fields {
            assert!(
                field.value.len() <= FIELD_CHAR_LIMIT,
                "field '{}' is {} chars (limit {FIELD_CHAR_LIMIT})",
                field.name,
                field.value.len()
            );
            total_chars += field.value.len();
            if field.value.contains("more)") {
                has_more = true;
            }
        }

        assert!(
            total_chars <= TOTAL_CHAR_LIMIT,
            "total {total_chars} > {TOTAL_CHAR_LIMIT}"
        );
        assert!(
            has_more,
            "expected at least one field with '+N more' suffix"
        );
    }

    #[test]
    fn trade_dashboard_embed_missing_fields() {
        // All-None fields should render N/A, never panic.
        let trades = vec![empty_trade(), empty_trade()];
        let clusters = vec![empty_cluster()];
        let levels = vec![empty_level(), empty_level(), empty_level()];
        let bombs = vec![empty_bomb()];

        let embed = build_dashboard_embed("test", &trades, &clusters, &levels, &bombs, 10);

        assert_eq!(embed.fields.len(), 4);
        for field in &embed.fields {
            assert!(
                !field.value.is_empty(),
                "field '{}' has empty value",
                field.name
            );
            assert!(
                field.value.contains(NA) || field.value.contains(NO_DATA),
                "field '{}' should contain N/A or No data available: {}",
                field.name,
                field.value
            );
        }
    }

    #[test]
    fn trade_dashboard_embed_empty_data() {
        let embed = build_dashboard_embed("xyz", &[], &[], &[], &[], 10);

        assert_eq!(embed.fields.len(), 4);
        for field in &embed.fields {
            assert_eq!(
                field.value, NO_DATA,
                "field '{}' should be '{NO_DATA}'",
                field.name
            );
        }
    }
}
