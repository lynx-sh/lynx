use anyhow::{Context as _, Result};
use lynx_core::error::LynxError;
use lynx_theme::loader::user_theme_dir;

/// Convert an OMZ .zsh-theme or Oh-My-Posh .omp.json theme to Lynx TOML format.
pub async fn run(source: &str, name: Option<&str>, force: bool) -> Result<()> {
    let resolved = lynx_convert::fetch::resolve_source(source)
        .context("failed to resolve theme source")?;

    let theme_name = name
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            let stem = std::path::Path::new(source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("converted")
                .to_string();
            stem.strip_suffix(".omp")
                .or_else(|| stem.strip_suffix(".zsh-theme"))
                .unwrap_or(&stem)
                .to_string()
        });

    let out_path = user_theme_dir().join(format!("{theme_name}.toml"));
    if out_path.exists() && !force {
        return Err(LynxError::Theme(format!(
            "theme '{}' already exists at {}. Use --force to overwrite.",
            theme_name,
            out_path.display()
        ))
        .into());
    }

    let content = lynx_convert::fetch::fetch_content(&resolved)
        .context("failed to fetch theme content")?;

    let is_omp = content.trim_start().starts_with('{');

    std::fs::create_dir_all(user_theme_dir())?;

    if is_omp {
        let theme = lynx_convert::omp::parse(&content)
            .map_err(|e| anyhow::Error::from(lynx_core::error::LynxError::Theme(e.to_string())))?;
        let toml_str = lynx_convert::emit::omp_to_lynx_toml(&theme, &theme_name);
        std::fs::write(&out_path, &toml_str)?;

        println!("Converted OMP theme → {}", out_path.display());
        if theme.two_line {
            println!("  Layout: two-line");
        }
        let seg_count = theme.top.len() + theme.top_right.len() + theme.left.len();
        println!("  Segments: {seg_count} mapped");
        if !theme.palette.is_empty() {
            println!("  Palette: {} colors extracted", theme.palette.len());
        }
        for note in &theme.notes {
            println!("  ⚠ {note}");
        }
    } else {
        let ir = lynx_convert::omz::parse(&content);
        let toml_str = lynx_convert::emit::to_lynx_toml(&ir, &theme_name);
        std::fs::write(&out_path, &toml_str)?;

        println!("Converted OMZ theme → {}", out_path.display());
        println!("  Segments (left):  {}", ir.left.join(", "));
        if !ir.right.is_empty() {
            println!("  Segments (right): {}", ir.right.join(", "));
        }
        if ir.two_line {
            println!("  Two-line layout detected");
        }
        for note in &ir.notes {
            println!("  ⚠ {note}");
        }
    }
    println!("\nActivate with: lx theme set {theme_name}");

    Ok(())
}
