use figlet_rs::{FIGlet, Toilet};

/// All bundled font slugs. These correspond to the built-in fonts in figlet-rs.
pub const BUNDLED_FONTS: &[&str] = &[
    // FIGlet fonts
    "standard", "slant", "small", "big", // Toilet fonts
    "block", "future", "wideterm", "mono12", "mono9",
];

/// List available bundled font slugs.
pub fn list_fonts() -> Vec<&'static str> {
    BUNDLED_FONTS.to_vec()
}

/// Render `text` using the given font slug.
///
/// Returns an error if the font slug is unknown or the text contains no renderable characters.
pub fn render_ascii(font: &str, text: &str) -> anyhow::Result<String> {
    let figure_str = match font {
        "standard" => {
            let f = FIGlet::standard().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_figlet(&f, text, font)?
        }
        "slant" => {
            let f = FIGlet::slant().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_figlet(&f, text, font)?
        }
        "small" => {
            let f = FIGlet::small().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_figlet(&f, text, font)?
        }
        "big" => {
            let f = FIGlet::big().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_figlet(&f, text, font)?
        }
        "block" => {
            let f = Toilet::smblock().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_toilet(&f, text, font)?
        }
        "future" => {
            let f = Toilet::future().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_toilet(&f, text, font)?
        }
        "wideterm" => {
            let f = Toilet::wideterm().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_toilet(&f, text, font)?
        }
        "mono12" => {
            let f = Toilet::mono12().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_toilet(&f, text, font)?
        }
        "mono9" => {
            let f = Toilet::mono9().map_err(|e| {
                anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
                    "figlet: failed to load font '{font}': {e}"
                )))
            })?;
            convert_toilet(&f, text, font)?
        }
        unknown => {
            let available = BUNDLED_FONTS.join(", ");
            return Err(lynx_core::error::LynxError::NotFound {
                item_type: "Font".into(),
                name: unknown.to_string(),
                hint: format!("available fonts: {available}"),
            }
            .into());
        }
    };
    Ok(figure_str)
}

fn convert_figlet(font: &FIGlet, text: &str, font_name: &str) -> anyhow::Result<String> {
    font.convert(text).map(|fig| fig.as_str()).ok_or_else(|| {
        anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
            "figlet: font '{font_name}' could not render '{text}' (no renderable characters)"
        )))
    })
}

fn convert_toilet(font: &Toilet, text: &str, font_name: &str) -> anyhow::Result<String> {
    font.convert(text).map(|fig| fig.as_str()).ok_or_else(|| {
        anyhow::Error::from(lynx_core::error::LynxError::Theme(format!(
            "figlet: font '{font_name}' could not render '{text}' (no renderable characters)"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_fonts_returns_all_bundled() {
        let fonts = list_fonts();
        assert_eq!(fonts.len(), BUNDLED_FONTS.len());
        assert!(fonts.contains(&"slant"));
        assert!(fonts.contains(&"standard"));
        assert!(fonts.contains(&"big"));
        assert!(fonts.contains(&"small"));
        assert!(fonts.contains(&"block"));
        assert!(fonts.contains(&"future"));
    }

    #[test]
    fn render_slant_produces_multiline() {
        let out = render_ascii("slant", "LYNX").unwrap();
        assert!(out.lines().count() > 1, "expected multiple lines");
        assert!(!out.trim().is_empty());
    }

    #[test]
    fn render_standard_produces_multiline() {
        let out = render_ascii("standard", "TEST").unwrap();
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn render_big_produces_multiline() {
        let out = render_ascii("big", "HI").unwrap();
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn render_small_produces_multiline() {
        let out = render_ascii("small", "lx").unwrap();
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn render_block_toilet_produces_multiline() {
        let out = render_ascii("block", "OK").unwrap();
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn render_future_toilet_produces_multiline() {
        let out = render_ascii("future", "LX").unwrap();
        assert!(out.lines().count() > 1);
    }

    #[test]
    fn unknown_font_returns_error_with_list() {
        let err = render_ascii("nonexistent", "test").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("nonexistent"), "error should mention bad font");
        assert!(msg.contains("slant"), "error should list available fonts");
    }
}
