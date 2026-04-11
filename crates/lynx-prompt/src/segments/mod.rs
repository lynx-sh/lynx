pub mod cmd_duration;
pub mod context_badge;
pub mod dir;
pub mod git;
pub mod kubectl;
pub mod profile_badge;
pub mod tasks;

pub use cmd_duration::CmdDurationSegment;
pub use context_badge::ContextBadgeSegment;
pub use dir::DirSegment;
pub use git::{GitBranchSegment, GitStatusSegment};
pub use kubectl::KubectlContextSegment;
pub use profile_badge::ProfileBadgeSegment;
pub use tasks::TaskStatusSegment;
