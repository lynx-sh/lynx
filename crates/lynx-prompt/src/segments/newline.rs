use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Emits a literal newline character, enabling two-line prompt layouts.
///
/// Place between segment groups in the `left` order to break the prompt onto
/// a new line. The renderer preserves this newline as-is — it is not stripped,
/// filtered, or counted as a space.
pub struct NewlineSegment;

impl Segment for NewlineSegment {
    fn name(&self) -> &'static str {
        "newline"
    }

    fn render(&self, _config: &toml::Value, _ctx: &RenderContext) -> Option<RenderedSegment> {
        Some(RenderedSegment::new("\n"))
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
    fn emits_newline() {
        let r = NewlineSegment.render(&empty_config(), &ctx()).unwrap();
        assert_eq!(r.text, "\n");
    }
}
