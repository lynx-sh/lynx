use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows the current OS platform icon (Nerd Font).
/// Always visible — the OS doesn't change mid-session.
///
/// TOML config:
/// ```toml
/// [segment.os]
/// color = { fg = "#b2bec3" }
/// # icon = ""  # override auto-detected icon
/// ```
pub struct OsSegment;

#[derive(Deserialize, Default)]
struct OsConfig {
    /// Override the auto-detected icon.
    icon: Option<String>,
}

impl Segment for OsSegment {
    fn name(&self) -> &'static str {
        "os"
    }

    fn render(&self, config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: OsConfig = config.clone().try_into().unwrap_or_default();
        let icon = cfg.icon.unwrap_or_else(|| detect_os_icon().to_string());
        Some(RenderedSegment::new(icon))
    }
}

fn detect_os_icon() -> &'static str {
    match std::env::consts::OS {
        "macos" => "\u{e711}",  // nf-md-apple
        "linux" => "\u{e712}",  // nf-md-linux
        "windows" => "\u{e70f}", // nf-md-windows
        "freebsd" => "\u{f30c}", // nf-linux-freebsd
        _ => "\u{f108}",        // nf-fa-desktop (fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn renders_os_icon() {
        let r = OsSegment.render(&empty_config(), &ctx()).unwrap();
        assert!(!r.text.is_empty(), "os segment should always render");
    }

    #[test]
    fn custom_icon_overrides() {
        let cfg: toml::Value = toml::from_str(r#"icon = "🍎""#).unwrap();
        let r = OsSegment.render(&cfg, &ctx()).unwrap();
        assert_eq!(r.text, "🍎");
    }

    #[test]
    fn detect_returns_known_icon() {
        let icon = detect_os_icon();
        // Should be a non-empty string for any supported platform
        assert!(!icon.is_empty());
    }
}
