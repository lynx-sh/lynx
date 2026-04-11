/// Canonical cache key constants for segment state stored in `RenderContext::cache`.
///
/// These keys are the contract between the shell environment (env vars set by plugins)
/// and the prompt renderer (segments that read from the cache map). Both sides must
/// use these constants — never raw strings.

/// Git state JSON — branch, dirty flag, stash count, etc.
pub const GIT_STATE: &str = "git_state";

/// Kubectl context JSON — current context name and namespace.
pub const KUBECTL_STATE: &str = "kubectl_state";

/// Active profile state JSON — currently active profile name.
pub const PROFILE_STATE: &str = "profile_state";
