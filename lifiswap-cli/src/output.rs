//! Output formatting utilities for table, JSON, and compact modes.

use std::fmt::Display;
use std::str::FromStr;

use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{Cell, ContentArrangement, Table};
use console::Style;
use serde::Serialize;

/// Output format selected via `--output`.
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Compact,
}

impl FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "table" => Ok(Self::Table),
            "json" => Ok(Self::Json),
            "compact" => Ok(Self::Compact),
            other => Err(format!("unknown output format: {other}")),
        }
    }
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Json => write!(f, "json"),
            Self::Compact => write!(f, "compact"),
        }
    }
}

/// Create a styled table with default settings.
pub fn styled_table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);

    let header_style = Style::new().bold().cyan();
    let cells: Vec<Cell> = headers
        .iter()
        .map(|h| Cell::new(header_style.apply_to(h).to_string()))
        .collect();
    table.set_header(cells);
    table
}

/// Print a value as JSON to stdout.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    println!("{json}");
    Ok(())
}

/// Print a labeled key-value pair.
pub fn print_kv(label: &str, value: &str) {
    let label_style = Style::new().bold();
    println!("{}: {value}", label_style.apply_to(label));
}

/// Styles for consistent terminal output.
pub struct Styles;

impl Styles {
    pub fn success() -> Style {
        Style::new().green().bold()
    }

    pub fn error() -> Style {
        Style::new().red().bold()
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn warning() -> Style {
        Style::new().yellow()
    }

    pub fn dim() -> Style {
        Style::new().dim()
    }

    pub fn highlight() -> Style {
        Style::new().cyan().bold()
    }
}
