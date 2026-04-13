//! Lynx Workflow Engine — TOML-defined workflows with runners (D-031).
//!
//! Workflows are data, not code. Each step declares a runner and a command.
//! Lynx orchestrates execution order, concurrency, signals, and logging.

pub mod context;
pub mod executor;
pub mod pty_runner;
pub mod job;
pub mod jobs;
pub mod params;
pub mod runner;
pub mod schema;
pub mod store;

pub use schema::{parse, validate};
pub use schema::{OnFail, ParamType, RunnerType, Step, Workflow, WorkflowMeta, WorkflowParam};
