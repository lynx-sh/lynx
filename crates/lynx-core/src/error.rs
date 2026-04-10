use thiserror::Error;

#[derive(Error, Debug)]
pub enum LynxError {
    #[error("Config error: {0}")]
    Config(String),
    #[error("Plugin error: {0}")]
    Plugin(String),
    #[error("Theme error: {0}")]
    Theme(String),
    #[error("Shell error: {0}")]
    Shell(String),
    #[error("Task error: {0}")]
    Task(String),
    #[error("Manifest error: {0}")]
    Manifest(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_display() {
        let cases: &[LynxError] = &[
            LynxError::Config("bad".into()),
            LynxError::Plugin("bad".into()),
            LynxError::Theme("bad".into()),
            LynxError::Shell("bad".into()),
            LynxError::Task("bad".into()),
            LynxError::Manifest("bad".into()),
            LynxError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "x")),
        ];
        for e in cases {
            assert!(!e.to_string().is_empty());
        }
    }
}

pub type Result<T> = std::result::Result<T, LynxError>;
