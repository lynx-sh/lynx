// `lx doctor` — environment health checks.
//
// Checks are defined in checks.rs. This module owns the output types (Check,
// Status) and the two output formatters (human-readable and JSON).

use anyhow::Result;
use clap::Args;

mod checks;

#[derive(Args)]
pub struct DoctorArgs {
    /// Output results as JSON (for scripting)
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug)]
pub(crate) struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    pub fix: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    pub fn symbol(&self) -> &'static str {
        match self {
            Status::Pass => "✓",
            Status::Warn => "⚠",
            Status::Fail => "✗",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Warn => "warn",
            Status::Fail => "fail",
        }
    }
}

pub async fn run(args: DoctorArgs) -> Result<()> {
    let results = checks::run_all();

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
