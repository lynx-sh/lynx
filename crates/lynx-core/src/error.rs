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
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, LynxError>;
