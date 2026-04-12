use serde::{Deserialize, Serialize};

/// Top-level intro definition, stored at `<lynx_dir>/intros/<slug>.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intro {
    #[serde(default)]
    pub meta: IntroMeta,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct IntroMeta {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Show on every new interactive shell startup.
    #[serde(default = "default_true")]
    pub on_startup: bool,
    /// Show when opening a new terminal tab (detected via TERM_SESSION_ID or similar).
    #[serde(default)]
    pub on_new_tab: bool,
    /// Show when connecting via SSH.
    #[serde(default = "default_true")]
    pub on_ssh: bool,
    /// Minimum seconds between displays (0 = always show).
    #[serde(default)]
    pub cooldown_sec: u64,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            on_startup: true,
            on_new_tab: false,
            on_ssh: true,
            cooldown_sec: 0,
        }
    }
}

fn default_true() -> bool {
    true
}

/// A renderable block within an intro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A free-form text block. Supports `{{TOKEN}}` substitution.
    Text {
        content: String,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        bold: bool,
    },
    /// A two-column aligned key/value table. Supports `{{TOKEN}}` in values.
    #[serde(rename = "keyval")]
    KeyVal {
        /// Each item is a [key, value] pair.
        items: Vec<[String; 2]>,
        #[serde(default)]
        color_key: Option<String>,
        #[serde(default)]
        color_val: Option<String>,
    },
    /// A horizontal separator line.
    Separator {
        #[serde(default = "default_dash")]
        char: String,
        #[serde(default = "default_separator_width")]
        width: usize,
        #[serde(default)]
        color: Option<String>,
    },
    /// An ASCII art block generated from a bundled figlet font.
    AsciiLogo {
        font: String,
        text: String,
        #[serde(default)]
        color: Option<String>,
    },
}

fn default_dash() -> String {
    "─".to_string()
}

fn default_separator_width() -> usize {
    40
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_text_block() {
        let block = Block::Text {
            content: "Hello, {{username}}!".to_string(),
            color: Some("green".to_string()),
            bold: true,
        };
        let toml_str = toml::to_string(&block).unwrap();
        let parsed: Block = toml::from_str(&toml_str).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn roundtrip_keyval_block() {
        let block = Block::KeyVal {
            items: vec![
                ["OS".to_string(), "{{os}}".to_string()],
                ["CPU".to_string(), "{{cpu_model}}".to_string()],
            ],
            color_key: Some("muted".to_string()),
            color_val: Some("accent".to_string()),
        };
        let toml_str = toml::to_string(&block).unwrap();
        let parsed: Block = toml::from_str(&toml_str).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn roundtrip_separator_block() {
        let block = Block::Separator {
            char: "─".to_string(),
            width: 40,
            color: None,
        };
        let toml_str = toml::to_string(&block).unwrap();
        let parsed: Block = toml::from_str(&toml_str).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn roundtrip_ascii_logo_block() {
        let block = Block::AsciiLogo {
            font: "slant".to_string(),
            text: "LYNX".to_string(),
            color: Some("#00ff88".to_string()),
        };
        let toml_str = toml::to_string(&block).unwrap();
        let parsed: Block = toml::from_str(&toml_str).unwrap();
        assert_eq!(block, parsed);
    }

    #[test]
    fn roundtrip_full_intro() {
        let intro = Intro {
            meta: IntroMeta {
                name: "test".to_string(),
                description: "A test intro".to_string(),
                author: "proxikal".to_string(),
            },
            display: DisplayConfig {
                on_startup: true,
                on_new_tab: false,
                on_ssh: true,
                cooldown_sec: 300,
            },
            blocks: vec![
                Block::AsciiLogo {
                    font: "slant".to_string(),
                    text: "LYNX".to_string(),
                    color: Some("green".to_string()),
                },
                Block::Separator {
                    char: "─".to_string(),
                    width: 40,
                    color: None,
                },
                Block::Text {
                    content: "Welcome, {{username}}".to_string(),
                    color: None,
                    bold: false,
                },
            ],
        };
        let toml_str = toml::to_string(&intro).unwrap();
        let parsed: Intro = toml::from_str(&toml_str).unwrap();
        assert_eq!(intro, parsed);
    }

    #[test]
    fn intro_defaults_on_empty() {
        let intro: Intro = toml::from_str("").unwrap();
        assert!(!intro.display.on_new_tab);
        assert!(intro.display.on_startup);
        assert_eq!(intro.display.cooldown_sec, 0);
        assert!(intro.blocks.is_empty());
    }
}
