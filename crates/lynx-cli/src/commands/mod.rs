pub mod plugin;
pub mod theme;
pub mod task;
pub mod sync;

use anyhow::Result;
use crate::cli::{Cli, Command};

pub async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init          => init().await,
        Command::Plugin(args)  => plugin::run(args).await,
        Command::Theme(args)   => theme::run(args).await,
        Command::Task(args)    => task::run(args).await,
        Command::Context{name} => context(name).await,
        Command::Doctor        => doctor().await,
        Command::Benchmark     => benchmark().await,
        Command::Rollback      => rollback().await,
        Command::Sync(args)    => sync::run(args).await,
    }
}

async fn init()              -> Result<()> { println!("init"); Ok(()) }
async fn context(name: String) -> Result<()> { println!("context: {name}"); Ok(()) }
async fn doctor()            -> Result<()> { println!("doctor"); Ok(()) }
async fn benchmark()         -> Result<()> { println!("benchmark"); Ok(()) }
async fn rollback()          -> Result<()> { println!("rollback"); Ok(()) }
