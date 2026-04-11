pub mod benchmark;
pub mod config;
pub mod context;
pub mod daemon;
pub mod doctor;
pub mod event;
pub mod examples;
pub mod init;
pub mod install;
pub mod migrate;
pub mod plugin;
pub mod profile;
pub mod prompt;
pub mod rollback;
pub mod sync;
pub mod task;
pub mod theme;
pub mod uninstall;
pub mod update;

use crate::cli::{Cli, Command};
use anyhow::Result;

pub async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => init::run(args).await,
        Command::Install(args) => install::run(args).await,
        Command::Event(args) => event::run(args).await,
        Command::Plugin(args) => plugin::run(args).await,
        Command::Theme(args) => theme::run(args).await,
        Command::Task(args) => task::run(args).await,
        Command::Daemon(args) => daemon::run(args).await,
        Command::Context(args) => context::run(args).await,
        Command::Doctor(args) => doctor::run(args).await,
        Command::Benchmark(args) => benchmark::run(args).await,
        Command::Rollback(args) => rollback::run(args).await,
        Command::Sync(args) => sync::run(args).await,
        Command::Config(args) => config::run(args).await,
        Command::Migrate(args) => migrate::run(args).await,
        Command::Update(args) => update::run(args).await,
        Command::Uninstall(args) => uninstall::run(args).await,
        Command::Examples(args) => examples::run(args).await,
        Command::Profile(args) => profile::run(args).await,
        Command::Prompt(args) => prompt::run(args).await,
    }
}
