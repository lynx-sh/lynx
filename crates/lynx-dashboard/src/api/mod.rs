//! API endpoint handlers for the Lynx Dashboard.
//!
//! Each submodule groups related endpoints. All mutations call library
//! functions — the dashboard implements NO business logic.

pub mod config;
pub mod cron;
pub mod intros;
pub mod plugins;
pub mod registry;
pub mod system;
pub mod themes;
pub mod workflows;
