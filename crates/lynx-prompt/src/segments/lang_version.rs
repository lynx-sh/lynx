use serde::Deserialize;
use std::path::Path;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Unified language version segment — detects project language from marker files in cwd,
/// shows the appropriate Nerd Font icon and version string.
///
/// No plugin required. Reads the filesystem directly. Supports:
/// - Rust       — Cargo.toml → rust-toolchain.toml / rust-toolchain
/// - Node.js    — package.json → "version" field
/// - Go         — go.mod → "go X.Y" directive
/// - Python     — pyproject.toml / requirements.txt
/// - Ruby       — Gemfile
/// - PHP        — composer.json → "require.php"
/// - Java       — pom.xml / build.gradle
///
/// TOML config:
/// ```toml
/// [segment.lang_version]
/// color = { fg = "#394260", bg = "#212736" }
/// # icon = " "  # override auto-detected icon
/// ```
pub struct LangVersionSegment;

#[derive(Deserialize, Default)]
struct LangVersionConfig {
    /// Override the auto-detected icon. Default: Nerd Font icon for the detected language.
    icon: Option<String>,
}

struct Detection {
    icon: &'static str,
    /// Human-readable language name — shown when no version can be determined.
    name: &'static str,
    version: String,
}

impl Segment for LangVersionSegment {
    fn name(&self) -> &'static str {
        "lang_version"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cwd = Path::new(&ctx.cwd);
        let cfg: LangVersionConfig = config.clone().try_into().unwrap_or_default();

        let detection = detect(cwd)?;
        let icon = cfg.icon.as_deref().unwrap_or(detection.icon);
        let label = if detection.version.is_empty() {
            detection.name.to_string()
        } else {
            detection.version.clone()
        };
        let text = format!("{icon}{label}");
        Some(RenderedSegment::new(text))
    }
}

fn detect(cwd: &Path) -> Option<Detection> {
    // Priority order: most specific first.
    if cwd.join("Cargo.toml").exists() {
        return Some(Detection {
            icon: " \u{e7a8} ",  // Nerd Fonts Rust (seti range)
            name: "rust",
            version: rust_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("go.mod").exists() {
        return Some(Detection {
            icon: " \u{e627} ",  // Nerd Fonts Go
            name: "go",
            version: go_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("package.json").exists() {
        return Some(Detection {
            icon: " \u{e718} ",  // Nerd Fonts Node
            name: "node",
            version: node_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("requirements.txt").exists() {
        return Some(Detection {
            icon: " \u{e73c} ",  // Nerd Fonts Python
            name: "python",
            version: python_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("Gemfile").exists() {
        return Some(Detection {
            icon: " \u{e791} ",  // Nerd Fonts Ruby
            name: "ruby",
            version: ruby_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("composer.json").exists() {
        return Some(Detection {
            icon: " \u{e73d} ",  // Nerd Fonts PHP
            name: "php",
            version: php_version(cwd).unwrap_or_default(),
        });
    }
    if cwd.join("pom.xml").exists() || cwd.join("build.gradle").exists() {
        return Some(Detection {
            icon: " \u{e738} ",  // Nerd Fonts Java
            name: "java",
            version: String::new(),
        });
    }
    None
}

/// Read Rust version from rust-toolchain.toml or rust-toolchain file.
fn rust_version(cwd: &Path) -> Option<String> {
    let toml_path = cwd.join("rust-toolchain.toml");
    if toml_path.exists() {
        let content = std::fs::read_to_string(toml_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("channel") {
                return line.split('"').nth(1).map(str::to_string);
            }
        }
    }
    let legacy = cwd.join("rust-toolchain");
    if legacy.exists() {
        return std::fs::read_to_string(legacy)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }
    // No toolchain file — show nothing rather than invoking rustup.
    None
}

/// Read Go version from go.mod ("go X.Y" directive).
fn go_version(cwd: &Path) -> Option<String> {
    let content = std::fs::read_to_string(cwd.join("go.mod")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("go ") {
            return Some(line[3..].trim().to_string());
        }
    }
    None
}

/// Read Node version from package.json "engines.node" or ".nvmrc" / ".node-version".
fn node_version(cwd: &Path) -> Option<String> {
    // .nvmrc takes priority (explicit pin).
    for name in &[".nvmrc", ".node-version"] {
        let p = cwd.join(name);
        if p.exists() {
            if let Ok(v) = std::fs::read_to_string(p) {
                let v = v.trim().trim_start_matches('v').to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    // Fall back to package.json engines.node.
    let content = std::fs::read_to_string(cwd.join("package.json")).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    val.get("engines")
        .and_then(|e| e.get("node"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
}

/// Read Python version from .python-version or pyproject.toml requires-python.
fn python_version(cwd: &Path) -> Option<String> {
    let pv = cwd.join(".python-version");
    if pv.exists() {
        if let Ok(v) = std::fs::read_to_string(pv) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    let pp = cwd.join("pyproject.toml");
    if pp.exists() {
        let content = std::fs::read_to_string(pp).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("requires-python") {
                return line.split('"').nth(1).map(str::to_string);
            }
        }
    }
    None
}

/// Read Ruby version from .ruby-version or Gemfile ruby directive.
fn ruby_version(cwd: &Path) -> Option<String> {
    let rv = cwd.join(".ruby-version");
    if rv.exists() {
        if let Ok(v) = std::fs::read_to_string(rv) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    let content = std::fs::read_to_string(cwd.join("Gemfile")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("ruby '") || line.starts_with("ruby \"") {
            return line.split('\'').nth(1)
                .or_else(|| line.split('"').nth(1))
                .map(str::to_string);
        }
    }
    None
}

/// Read PHP version from composer.json require.php.
fn php_version(cwd: &Path) -> Option<String> {
    let content = std::fs::read_to_string(cwd.join("composer.json")).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    val.get("require")
        .and_then(|r| r.get("php"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ctx(cwd: &str) -> RenderContext {
        RenderContext {
            cwd: cwd.to_string(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn no_marker_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let seg = LangVersionSegment;
        let cfg = toml::Value::Table(toml::map::Map::new());
        assert!(seg.render(&cfg, &ctx(dir.path().to_str().unwrap())).is_none());
    }

    #[test]
    fn detects_rust_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"test\"").unwrap();
        let seg = LangVersionSegment;
        let cfg = toml::Value::Table(toml::map::Map::new());
        let r = seg.render(&cfg, &ctx(dir.path().to_str().unwrap())).unwrap();
        // Icon contains Rust nerd font char \u{e7a8}
        assert!(r.text.contains('\u{e7a8}'), "expected rust icon in: {:?}", r.text);
    }

    #[test]
    fn detects_go_project_with_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module foo\n\ngo 1.22\n").unwrap();
        let seg = LangVersionSegment;
        let cfg = toml::Value::Table(toml::map::Map::new());
        let r = seg.render(&cfg, &ctx(dir.path().to_str().unwrap())).unwrap();
        assert!(r.text.contains('\u{e627}'), "expected go icon");
        assert!(r.text.contains("1.22"), "expected version");
    }

    #[test]
    fn detects_node_project_with_nvmrc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join(".nvmrc"), "v20.11.0\n").unwrap();
        let seg = LangVersionSegment;
        let cfg = toml::Value::Table(toml::map::Map::new());
        let r = seg.render(&cfg, &ctx(dir.path().to_str().unwrap())).unwrap();
        assert!(r.text.contains('\u{e718}'), "expected node icon");
        assert!(r.text.contains("20.11.0"), "expected version");
    }

    #[test]
    fn rust_version_from_toolchain_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.78.0\"\n",
        )
        .unwrap();
        let r = detect(dir.path()).unwrap();
        assert_eq!(r.version, "1.78.0");
    }

    #[test]
    fn custom_icon_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        let seg = LangVersionSegment;
        let cfg: toml::Value = toml::from_str(r#"icon = "RS ""#).unwrap();
        let r = seg.render(&cfg, &ctx(dir.path().to_str().unwrap())).unwrap();
        assert!(r.text.starts_with("RS "), "text: {}", r.text);
    }
}
