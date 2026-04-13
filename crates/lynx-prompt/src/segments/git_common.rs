use crate::segment::RenderContext;

/// Extract the git state JSON object from the render context cache.
///
/// Shared by all git-related segments (git_status, git_action, git_stash, git_ahead_behind).
pub(crate) fn git_state_obj(ctx: &RenderContext) -> Option<&serde_json::Map<String, serde_json::Value>> {
    match ctx.cache.get(crate::cache_keys::GIT_STATE)? {
        serde_json::Value::Object(obj) => Some(obj),
        _ => None,
    }
}
