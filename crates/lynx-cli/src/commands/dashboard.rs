use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct DashboardArgs {}

pub async fn run(_args: DashboardArgs) -> Result<()> {
    lynx_dashboard::run().await
}
