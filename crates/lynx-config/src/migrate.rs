use crate::schema::{LynxConfig, CURRENT_SCHEMA_VERSION};
use lynx_core::error::Result;

/// Migrate a config to the current schema version.
///
/// Currently a no-op stub — ready for future version migrations.
/// Logs a warning when schema_version doesn't match current.
pub fn migrate(config: &mut LynxConfig) -> Result<()> {
    if config.schema_version != CURRENT_SCHEMA_VERSION {
        // Log warning but do not panic — tolerate stale configs gracefully.
        eprintln!(
            "[lynx] warn: config schema_version {} is not current ({}); \
             consider regenerating your config",
            config.schema_version, CURRENT_SCHEMA_VERSION
        );
        // Future: match on version and apply transformations here.
        config.schema_version = CURRENT_SCHEMA_VERSION;
    }
    Ok(())
}
