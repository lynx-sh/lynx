use anyhow::Result;
use clap::Args;
use lynx_config::load as load_config;
use lynx_shell::{context::detect_context, init::{generate_init_script, InitParams}};
use lynx_core::types::Context;

#[derive(Args)]
pub struct InitArgs {
    /// Override the detected context (interactive | agent | minimal)
    #[arg(long)]
    pub context: Option<String>,
}

pub async fn run(args: InitArgs) -> Result<()> {
    let config = load_config()?;

    let detected = detect_context();
    let context = match args.context.as_deref() {
        Some("agent") => Context::Agent,
        Some("minimal") => Context::Minimal,
        Some("interactive") => Context::Interactive,
        Some(other) => {
            eprintln!("lx: unknown context '{}', falling back to auto-detect", other);
            detected
        }
        None => detected,
    };

    let lynx_dir = resolve_lynx_dir();
    let plugin_dir = format!("{}/plugins", lynx_dir);

    let script = generate_init_script(&InitParams {
        context: &context,
        lynx_dir: &lynx_dir,
        plugin_dir: &plugin_dir,
        enabled_plugins: &config.enabled_plugins,
    });

    print!("{}", script);
    Ok(())
}

/// Resolve LYNX_DIR: env override → default install location.
fn resolve_lynx_dir() -> String {
    if let Ok(dir) = std::env::var("LYNX_DIR") {
        return dir;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    format!("{}/.local/share/lynx", home)
}
