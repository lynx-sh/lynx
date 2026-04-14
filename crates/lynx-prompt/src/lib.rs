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

/// Return the full ordered list of all built-in segment types.
///
/// This is the single source of truth for segment registration. Adding a new
/// segment type requires updating this function and the `pub use` above.
pub fn all_segments() -> Vec<Box<dyn Segment>> {
    vec![
        Box::new(UsernameSegment),
        Box::new(HostnameSegment),
        Box::new(SshIndicatorSegment),
        Box::new(DirSegment),
        Box::new(GitBranchSegment),
        Box::new(GitStatusSegment),
        Box::new(GitActionSegment),
        Box::new(GitAheadBehindSegment),
        Box::new(GitShaSegment),
        Box::new(GitStashSegment),
        Box::new(GitTimeSinceCommitSegment),
        Box::new(AwsProfileSegment),
        Box::new(BatterySegment),
        Box::new(DockerSegment),
        Box::new(GcpSegment),
        Box::new(HistNumberSegment),
        Box::new(KubectlContextSegment),
        Box::new(NodeVersionSegment),
        Box::new(RubyVersionSegment),
        Box::new(GolangVersionSegment),
        Box::new(RustVersionSegment),
        Box::new(LangVersionSegment),
        Box::new(VenvSegment),
        Box::new(CondaEnvSegment),
        Box::new(TaskStatusSegment),
        Box::new(CmdDurationSegment),
        Box::new(ExitCodeSegment),
        Box::new(BackgroundJobsSegment),
        Box::new(ViModeSegment),
        Box::new(CustomSegment),
        Box::new(TimeSegment),
        Box::new(ContextBadgeSegment),
        Box::new(NewlineSegment),
        Box::new(OsSegment),
        Box::new(PromptCharSegment),
        Box::new(ShellSegment),
        Box::new(TerraformSegment),
        Box::new(TextSegment),
    ]
}
