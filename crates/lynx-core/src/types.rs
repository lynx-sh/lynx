use serde::{Deserialize, Serialize};

/// The context Lynx is running in — determines what gets loaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Context {
    #[default]
    Interactive,  // normal shell use
    Agent,        // AI agentic coding (Claude Code, Cursor, etc.)
    Minimal,      // bare minimum — scripts, SSH, CI
}

/// Load strategy for a plugin or module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LoadStrategy {
    #[default]
    Eager,   // load at startup
    Lazy,    // defer until first use
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct CtxWrapper {
        ctx: Context,
    }

    #[derive(Serialize, Deserialize)]
    struct LsWrapper {
        ls: LoadStrategy,
    }

    #[test]
    fn context_serde_roundtrip() {
        for ctx in [Context::Interactive, Context::Agent, Context::Minimal] {
            let w = CtxWrapper { ctx: ctx.clone() };
            let s = toml::to_string(&w).unwrap();
            let back: CtxWrapper = toml::from_str(&s).unwrap();
            assert_eq!(ctx, back.ctx);
        }
    }

    #[test]
    fn load_strategy_serde_roundtrip() {
        for ls in [LoadStrategy::Eager, LoadStrategy::Lazy] {
            let w = LsWrapper { ls: ls.clone() };
            let s = toml::to_string(&w).unwrap();
            let back: LsWrapper = toml::from_str(&s).unwrap();
            assert_eq!(ls, back.ls);
        }
    }

    #[test]
    fn defaults() {
        assert_eq!(Context::default(), Context::Interactive);
        assert_eq!(LoadStrategy::default(), LoadStrategy::Eager);
    }
}
