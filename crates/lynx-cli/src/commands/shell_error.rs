use anyhow::Result;
use clap::Args;
use lynx_core::error::LynxError;

/// `lx shell-error` — internal command called by the eval bridge when zsh eval fails.
///
/// The shell layer cannot use LynxError directly. When `eval "$(lx ...)"` fails,
/// the eval-bridge captures zsh's raw stderr and delegates here so the error is
/// formatted and displayed via the standard Lynx error renderer instead of leaking
/// raw zsh messages like `(eval):N: unmatched '` to the user.
///
/// This command is internal plumbing — not listed in help or examples.
#[derive(Args)]
#[command(hide = true)]
pub struct ShellErrorArgs {
    /// The raw error message captured from zsh eval stderr
    pub message: String,
}

pub fn run(args: ShellErrorArgs) -> Result<()> {
    Err(LynxError::Shell(args.message).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_error_returns_lynx_shell_error() {
        let args = ShellErrorArgs {
            message: "unmatched '".to_string(),
        };
        let err = run(args).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unmatched '"), "expected message in: {msg}");
        assert!(msg.contains("Fix:"), "expected Fix hint in: {msg}");
    }
}
