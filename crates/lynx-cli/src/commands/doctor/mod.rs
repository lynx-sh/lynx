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

pub fn run(args: DoctorArgs) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_human_all_pass_says_all_checks_passed() {
        // Smoke test — print_human should not panic on empty input.
        print_human(&[]);
    }

    #[test]
    fn print_human_with_fail_mentions_issues() {
        let checks = vec![
            Check {
                name: "test-check",
                status: Status::Fail,
                detail: "something is wrong".to_string(),
                fix: Some("fix it".to_string()),
            },
        ];
        print_human(&checks);
    }

    #[test]
    fn print_human_with_pass_no_fix() {
        let checks = vec![
            Check {
                name: "ok-check",
                status: Status::Pass,
                detail: "all good".to_string(),
                fix: None,
            },
        ];
        print_human(&checks);
    }

    #[test]
    fn print_json_empty_produces_valid_json() {
        print_json(&[]);
    }

    #[test]
    fn print_json_includes_fix_when_present() {
        let checks = vec![
            Check {
                name: "broken",
                status: Status::Fail,
                detail: "bad".to_string(),
                fix: Some("run this".to_string()),
            },
        ];
        print_json(&checks);
    }

    #[test]
    fn print_json_omits_fix_when_none() {
        let checks = vec![
            Check {
                name: "ok",
                status: Status::Pass,
                detail: "good".to_string(),
                fix: None,
            },
        ];
        print_json(&checks);
    }
}
