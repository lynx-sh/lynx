use lynx_core::error::Result;
use tracing::info;

use crate::schema::{LynxConfig, CURRENT_SCHEMA_VERSION};

/// A migration function: transforms a config from version N to N+1.
/// Must be idempotent — safe to re-run.
type MigrationFn = fn(&mut LynxConfig) -> Result<()>;

/// Migrations indexed by the version they migrate FROM.
/// Entry [0] migrates v0 → v1, entry [1] migrates v1 → v2, etc.
const VERSION_MIGRATIONS: &[MigrationFn] = &[migrate_v0_to_v1, migrate_v1_to_v2];

/// Migrate a config to the current schema version, applying all pending
/// migrations in order. Each migration logs what it changed. Idempotent.
pub fn migrate(config: &mut LynxConfig) -> Result<()> {
    let start = config.schema_version as usize;
    let target = CURRENT_SCHEMA_VERSION as usize;

    if start >= target {
        return Ok(());
    }

    info!("migrating config schema from v{} to v{}", start, target);

    for version in start..target {
        if let Some(migration) = VERSION_MIGRATIONS.get(version) {
            info!("applying migration v{} → v{}", version, version + 1);
            migration(config)?;
            config.schema_version = (version + 1) as u32;
        }
    }

    Ok(())
}

// ── Individual migrations ──────────────────────────────────────────────────

/// v0 → v1: initial schema; set default theme if empty.
fn migrate_v0_to_v1(config: &mut LynxConfig) -> Result<()> {
    if config.active_theme.is_empty() {
        info!("migration v0→v1: active_theme was empty, setting to 'default'");
        config.active_theme = "default".to_string();
    }
    Ok(())
}

/// v1 → v2: aliases and paths fields added; serde defaults handle forward compat.
/// Migration is a no-op — just advances schema_version.
fn migrate_v1_to_v2(_config: &mut LynxConfig) -> Result<()> {
    info!("migration v1→v2: aliases and paths fields added (serde defaults apply)");
    Ok(())
}

/// Describe what migration(s) would be applied to `config` without saving.
/// Returns a list of human-readable change descriptions.
pub fn dry_run(config: &LynxConfig) -> Vec<String> {
    let start = config.schema_version as usize;
    let target = CURRENT_SCHEMA_VERSION as usize;

    if start >= target {
        return vec!["config is up-to-date — no migrations needed".to_string()];
    }

    (start..target)
        .map(|v| format!("would apply migration v{v} → v{}", v + 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::CURRENT_SCHEMA_VERSION;
    use lynx_core::types::Context;

    fn config_at(version: u32) -> LynxConfig {
        LynxConfig {
            schema_version: version,
            active_theme: if version == 0 {
                String::new()
            } else {
                "default".into()
            },
            active_context: Context::Interactive,
            enabled_plugins: vec![],
            ..Default::default()
        }
    }

    #[test]
    fn migrates_from_zero_to_current() {
        let mut cfg = config_at(0);
        migrate(&mut cfg).unwrap();
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn already_current_is_noop() {
        let mut cfg = config_at(CURRENT_SCHEMA_VERSION);
        cfg.active_theme = "minimal".into();
        migrate(&mut cfg).unwrap();
        assert_eq!(cfg.active_theme, "minimal");
        assert_eq!(cfg.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn idempotent_reruns() {
        let mut cfg = config_at(0);
        migrate(&mut cfg).unwrap();
        let theme_after_first = cfg.active_theme.clone();
        cfg.schema_version = 0; // force re-run
        migrate(&mut cfg).unwrap();
        assert_eq!(cfg.active_theme, theme_after_first);
    }

    #[test]
    fn dry_run_lists_pending() {
        let cfg = config_at(0);
        let changes = dry_run(&cfg);
        assert!(!changes.is_empty());
        assert!(changes[0].contains("v0"));
    }

    #[test]
    fn dry_run_empty_when_current() {
        let cfg = config_at(CURRENT_SCHEMA_VERSION);
        let changes = dry_run(&cfg);
        assert_eq!(changes.len(), 1);
        assert!(changes[0].contains("up-to-date"));
    }
}
