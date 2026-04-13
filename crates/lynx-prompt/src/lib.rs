mod assemble;
pub mod cache;
pub mod cache_keys;
pub mod color_apply;
pub mod evaluator;
pub mod renderer;
pub mod segment;
pub mod segments;

pub use segment::{RenderContext, RenderedSegment, Segment};
pub use segments::{
    AwsProfileSegment, BackgroundJobsSegment, BatterySegment, CmdDurationSegment, CondaEnvSegment,
    ContextBadgeSegment, CustomSegment, DirSegment, DockerSegment, ExitCodeSegment, GcpSegment,
    GitActionSegment, GitAheadBehindSegment, GitBranchSegment, GitShaSegment, GitStashSegment,
    GitStatusSegment, GitTimeSinceCommitSegment, GolangVersionSegment, HistNumberSegment,
    HostnameSegment, KubectlContextSegment, LangVersionSegment, NewlineSegment, NodeVersionSegment,
    OsSegment, PromptCharSegment, RubyVersionSegment, RustVersionSegment, ShellSegment,
    SshIndicatorSegment, TaskStatusSegment, TerraformSegment, TextSegment, TimeSegment,
    UsernameSegment, VenvSegment, ViModeSegment,
};
