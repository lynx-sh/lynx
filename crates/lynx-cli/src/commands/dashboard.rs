use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct DashboardArgs {}

pub async fn run(_args: DashboardArgs) -> Result<()> {
    lynx_dashboard::run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_args_struct_exists() {
        // DashboardArgs is a unit struct with no fields
        let _args = DashboardArgs {};
    }
}
