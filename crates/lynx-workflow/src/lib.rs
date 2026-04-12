//! Lynx Workflow Engine — TOML-defined workflows with runners (D-031).
//!
//! Workflows are data, not code. Each step declares a runner and a command.
//! Lynx orchestrates execution order, concurrency, signals, and logging.

pub mod schema;

pub use schema::{
    OnFail, ParamType, RunnerType, Step, Workflow, WorkflowMeta, WorkflowParam,
};
pub use schema::{parse, validate};
