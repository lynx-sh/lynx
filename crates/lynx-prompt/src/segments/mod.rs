pub mod cmd_duration;
pub mod context_badge;
pub mod dir;
pub mod git;
pub mod tasks;

pub use cmd_duration::CmdDurationSegment;
pub use context_badge::ContextBadgeSegment;
pub use dir::DirSegment;
pub use git::{GitBranchSegment, GitStatusSegment};
pub use tasks::TaskStatusSegment;
