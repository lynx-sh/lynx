// `lx doctor` — environment health checks.
//
// All check logic lives in the lynx-doctor library crate.
// This module owns only the CLI args and output formatters.

use anyhow::Result;
use clap::Args;
use lynx_doctor::{Check, Status};

#[derive(Args)]
pub struct DoctorArgs {
    /// Output results as JSON (for scripting)
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let results = lynx_doctor::run_all();

    if args.json {
        print_json(&results);
    } else {
        print_human(&results);
    }

    Ok(())
}

fn print_human(checks: &[Check]) {
    let mut any_fail = false;
    for c in checks {
        println!("  {} {}  {}", c.status.symbol(), c.name, c.detail);
        if let Some(fix) = &c.fix {
            println!("    Fix: {fix}");
        }
        if c.status == Status::Fail {
            any_fail = true;
        }
    }
    println!();
    if any_fail {
        println!("Issues found. Run the Fix commands above to resolve them.");
    } else {
        println!("All checks passed.");
    }
}

fn print_json(checks: &[Check]) {
    let items: Vec<serde_json::Value> = checks
        .iter()
        .map(|c| {
            let mut obj = serde_json::json!({
                "name": c.name,
                "status": c.status.label(),
                "detail": c.detail,
            });
            if let Some(fix) = &c.fix {
                obj["fix"] = serde_json::Value::String(fix.clone());
            }
            obj
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&items).unwrap_or_default()
    );
}
