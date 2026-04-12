use crate::schema::{IntroConfig, LynxConfig, SyncConfig, CURRENT_SCHEMA_VERSION};
use lynx_core::types::Context;

impl Default for LynxConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            active_theme: "default".to_string(),
            active_context: Context::Interactive,
            enabled_plugins: vec![],
            sync: SyncConfig { remote: None },
            active_profile: None,
            intro: IntroConfig::default(),
        }
    }
}
