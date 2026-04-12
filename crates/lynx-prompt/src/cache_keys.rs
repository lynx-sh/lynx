/// Canonical cache key constants for segment state stored in `RenderContext::cache`.
///
/// These keys are the contract between the shell environment (env vars set by plugins)
/// and the prompt renderer (segments that read from the cache map). Both sides must
/// use these constants — never raw strings.

/// Git state JSON — branch, dirty flag, stash count, etc.
pub const GIT_STATE: &str = "git_state";

/// Kubectl context JSON — current context name and namespace.
pub const KUBECTL_STATE: &str = "kubectl_state";

/// Node.js version state JSON — set by the node plugin from .node-version/.nvmrc.
pub const NODE_STATE: &str = "node_state";

/// Ruby version state JSON — set by the ruby plugin from .ruby-version.
pub const RUBY_STATE: &str = "ruby_state";

/// Go version state JSON — set by the golang plugin from go.mod.
pub const GOLANG_STATE: &str = "golang_state";

/// Rust toolchain state JSON — set by the rust-ver plugin from rust-toolchain.toml.
pub const RUST_STATE: &str = "rust_state";
