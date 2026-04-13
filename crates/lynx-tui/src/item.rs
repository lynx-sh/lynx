//! Core types for the interactive list component.

use ratatui::style::Color;

/// A single item displayable in the interactive list.
///
/// Implement this trait for any data type you want to browse interactively.
/// The list component calls these methods to render each row and the preview pane.
pub trait ListItem {
    /// Primary display text (shown in the list). Keep short — one line.
    fn title(&self) -> &str;

    /// Secondary text shown next to/below the title (e.g. description snippet).
    /// Returns empty string if none.
    fn subtitle(&self) -> String {
        String::new()
    }

    /// Multi-line detail text for the preview pane. Shown when this item is highlighted.
    fn detail(&self) -> String {
        String::new()
    }

    /// Category or type tag (e.g. "plugin", "theme", "builtin").
    /// Used for grouping and filtering.
    fn category(&self) -> Option<&str> {
        None
    }

    /// Tags for search/filter matching beyond title/subtitle.
    fn tags(&self) -> Vec<&str> {
        vec![]
    }

    /// Whether this item is currently active/selected (e.g. active theme, enabled plugin).
    fn is_active(&self) -> bool {
        false
    }
}

/// What to do when the user presses enter on an item.
pub enum ListAction {
    /// No action — just close the list.
    None,
    /// Run a named action with the item's title as argument.
    Run(String),
}

/// Theme colors for TUI chrome. Sourced from the active theme's [colors] table (D-041).
///
/// All fields are ratatui `Color` values, pre-parsed from hex strings.
/// Use `TuiColors::default()` for Tokyo Night fallback.
#[derive(Debug, Clone, Copy)]
pub struct TuiColors {
    /// Primary UI color — highlights, active items, borders, search input.
    pub accent: Color,
    /// Positive states — active marker, success confirmations.
    pub success: Color,
    /// Caution — search match highlight.
    pub warning: Color,
    /// Negative states — errors, empty state text.
    pub error: Color,
    /// De-emphasized — borders, status bar, inactive text.
    pub muted: Color,
}

impl Default for TuiColors {
    fn default() -> Self {
        Self {
            accent: hex_to_color(crate::defaults::ACCENT),
            success: hex_to_color(crate::defaults::SUCCESS),
            warning: hex_to_color(crate::defaults::WARNING),
            error: hex_to_color(crate::defaults::ERROR),
            muted: hex_to_color(crate::defaults::MUTED),
        }
    }
}

impl TuiColors {
    /// Build from a theme's [colors] HashMap. Missing keys fall back to defaults.
    pub fn from_palette(colors: &std::collections::HashMap<String, String>) -> Self {
        let def = Self::default();
        Self {
            accent: colors.get("accent").map(|s| hex_to_color(s)).unwrap_or(def.accent),
            success: colors.get("success").map(|s| hex_to_color(s)).unwrap_or(def.success),
            warning: colors.get("warning").map(|s| hex_to_color(s)).unwrap_or(def.warning),
            error: colors.get("error").map(|s| hex_to_color(s)).unwrap_or(def.error),
            muted: colors.get("muted").map(|s| hex_to_color(s)).unwrap_or(def.muted),
        }
    }
}

/// Parse a "#RRGGBB" hex string into a ratatui Color.
fn hex_to_color(hex: &str) -> Color {
    let s = hex.trim_start_matches('#');
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::White
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItem {
        name: String,
        desc: String,
    }

    impl ListItem for TestItem {
        fn title(&self) -> &str {
            &self.name
        }
        fn subtitle(&self) -> String {
            self.desc.clone()
        }
    }

    #[test]
    fn default_colors_are_tokyo_night() {
        let c = TuiColors::default();
        assert_eq!(c.accent, Color::Rgb(122, 162, 247));
        assert_eq!(c.success, Color::Rgb(158, 206, 106));
        assert_eq!(c.error, Color::Rgb(247, 118, 142));
    }

    #[test]
    fn from_palette_overrides() {
        let mut pal = std::collections::HashMap::new();
        pal.insert("accent".into(), "#ff0000".into());
        let c = TuiColors::from_palette(&pal);
        assert_eq!(c.accent, Color::Rgb(255, 0, 0));
        // Others fall back to defaults
        assert_eq!(c.success, Color::Rgb(158, 206, 106));
    }

    #[test]
    fn from_empty_palette_is_default() {
        let pal = std::collections::HashMap::new();
        let c = TuiColors::from_palette(&pal);
        let d = TuiColors::default();
        assert_eq!(c.accent, d.accent);
        assert_eq!(c.muted, d.muted);
    }

    #[test]
    fn hex_to_color_parses() {
        assert_eq!(hex_to_color("#7aa2f7"), Color::Rgb(122, 162, 247));
        assert_eq!(hex_to_color("#000000"), Color::Rgb(0, 0, 0));
        assert_eq!(hex_to_color("invalid"), Color::White);
    }

    #[test]
    fn list_item_defaults() {
        let item = TestItem {
            name: "test".into(),
            desc: "desc".into(),
        };
        assert_eq!(item.title(), "test");
        assert_eq!(item.subtitle(), "desc");
        assert_eq!(item.detail(), "");
        assert_eq!(item.category(), None);
        assert!(item.tags().is_empty());
        assert!(!item.is_active());
    }
}
