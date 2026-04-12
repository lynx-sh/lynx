use serde::{Deserialize, Serialize};

/// Syntax highlighting colors — maps zsh token types to theme colors.
/// Used to generate `ZSH_HIGHLIGHT_STYLES` associative array entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SyntaxHighlight {
    /// Valid external commands
    pub command: Option<String>,
    /// Unknown / invalid commands
    pub unknown: Option<String>,
    /// Shell builtins (cd, echo, etc.)
    pub builtin: Option<String>,
    /// Aliases
    pub alias: Option<String>,
    /// Functions
    pub function: Option<String>,
    /// File paths
    pub path: Option<String>,
    /// Quoted strings
    pub string: Option<String>,
    /// Command arguments
    pub argument: Option<String>,
    /// Flags and options (--flag, -x)
    pub option: Option<String>,
    /// Comments
    pub comment: Option<String>,
    /// Globbing patterns
    pub globbing: Option<String>,
    /// Variable references ($VAR)
    pub variable: Option<String>,
}

/// Auto-suggestion configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AutoSuggestions {
    /// Suggestion text color (typically muted). Supports hex or named colors.
    pub color: Option<String>,
}

/// One entry in the `[ls_colors]` table — colors for a single file-type category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsColorsEntry {
    pub fg: Option<String>,
    pub bg: Option<String>,
    #[serde(default)]
    pub bold: bool,
}

/// The `[ls_colors.columns]` table — eza metadata column colors.
///
/// These map to EZA_COLORS keys and colorize the *content* of each column
/// in `ls -la` output (dates, sizes, permission bits, user/group names).
/// Has no effect on plain `/bin/ls` — only eza reads these keys.
///
/// All fields are optional; absent fields fall back to eza's own defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EzaColumns {
    /// File modification date/time — eza key `da`
    pub date: Option<String>,
    /// Size number (the digits) — eza key `sn`
    pub size_number: Option<String>,
    /// Size unit suffix (B, k, M, G) — eza key `sb`
    pub size_unit: Option<String>,
    /// Owner name when it matches the current user — eza key `uu`
    pub user_you: Option<String>,
    /// Owner name when it does NOT match the current user — eza key `un`
    pub user_other: Option<String>,
    /// Group name when the current user is a member — eza key `gu`
    pub group_you: Option<String>,
    /// Group name when the current user is NOT a member — eza key `gn`
    pub group_other: Option<String>,
    /// Read permission bits (r) for all three tiers — eza keys `ur`, `gr`, `or`
    pub perm_read: Option<String>,
    /// Write permission bits (w) for all three tiers — eza keys `uw`, `gw`, `ow`
    pub perm_write: Option<String>,
    /// Execute permission bits (x) for all three tiers — eza keys `ux`, `gx`, `ox`
    pub perm_exec: Option<String>,
    /// Column header row (when using --header) — eza key `hd`
    pub header: Option<String>,
    /// Symlink target path — eza key `lp`
    pub symlink_path: Option<String>,
}

/// The `[ls_colors]` table — semantic mapping from file-type categories to colors.
/// Absent fields default to no override (terminal/distro default applies).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LsColors {
    pub dir: Option<LsColorsEntry>,
    pub symlink: Option<LsColorsEntry>,
    pub executable: Option<LsColorsEntry>,
    pub archive: Option<LsColorsEntry>,
    pub image: Option<LsColorsEntry>,
    pub audio: Option<LsColorsEntry>,
    pub broken: Option<LsColorsEntry>,
    pub other_writable: Option<LsColorsEntry>,
    /// Eza metadata column colors — only used when eza is the ls backend.
    #[serde(default)]
    pub columns: EzaColumns,
}

impl SyntaxHighlight {
    /// Generate a string of `ZSH_HIGHLIGHT_STYLES[<key>]=fg=<hex>` assignments.
    /// Returns `None` if no syntax highlight colors are configured.
    pub fn to_zsh_highlight_styles(&self) -> Option<String> {
        let mappings: &[(&str, &Option<String>)] = &[
            ("command", &self.command),
            ("unknown-token", &self.unknown),
            ("builtin", &self.builtin),
            ("alias", &self.alias),
            ("function", &self.function),
            ("path", &self.path),
            ("single-quoted-argument", &self.string),
            ("double-quoted-argument", &self.string),
            ("dollar-quoted-argument", &self.string),
            ("default", &self.argument),
            ("single-hyphen-option", &self.option),
            ("double-hyphen-option", &self.option),
            ("comment", &self.comment),
            ("globbing", &self.globbing),
            ("assign", &self.variable),
        ];

        let mut parts: Vec<String> = Vec::new();
        for (key, color) in mappings {
            if let Some(c) = color {
                parts.push(format!("ZSH_HIGHLIGHT_STYLES[{key}]='fg={c}'"));
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }
}

impl AutoSuggestions {
    /// Generate `ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE` assignment.
    /// Returns `None` if no color is configured.
    pub fn to_autosuggest_style(&self) -> Option<String> {
        self.color.as_ref().map(|c| format!("fg={c}"))
    }
}
