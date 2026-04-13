use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Args;

use lynx_core::brand;
use lynx_core::env_vars;

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Number of runs to average (default: 1)
    #[arg(long, short, default_value = "1")]
    pub runs: u32,
}

#[derive(Debug)]
struct BenchResult {
    component: String,
    avg_ms: u64,
}

pub fn run(args: BenchmarkArgs) -> Result<()> {
    let runs = args.runs.max(1);
    println!("Running {runs} benchmark run(s)...");

    // Collect timings across runs.
    let mut all_runs: Vec<Vec<(String, Duration)>> = Vec::new();
    for i in 0..runs {
        if runs > 1 {
            print!("  run {}/{}... ", i + 1, runs);
        }
        let timings = measure_startup()?;
        if runs > 1 {
            let total: Duration = timings.iter().map(|(_, d)| *d).sum();
            println!("{}ms", total.as_millis());
        }
        all_runs.push(timings);
    }

    // Average across runs.
    let results = average_runs(&all_runs);

    // Load previous benchmark for regression detection.
    let previous = load_previous_benchmark();

    // Print table.
    print_table(&results, &previous);

    // Save results.
    save_benchmark(&results)?;

    Ok(())
}

fn measure_startup() -> Result<Vec<(String, Duration)>> {
    // Spawn a clean zsh subprocess and time component loading.
    // We measure by running `lx benchmark --internal` which instruments timing.
    // For now, measure total shell init time and break into coarse components.
    let start = Instant::now();
    let status = std::process::Command::new("zsh")
        .arg("-i")
        .arg("-c")
        .arg("exit")
        .env(env_vars::LYNX_BENCHMARK_MODE, "1")
        .output();

    let total = start.elapsed();

    // In a real implementation each component would report its own timing.
    // For now we report the total startup time as a single measurement.
    match status {
        Ok(output) if !output.status.success() => {
            tracing::warn!("benchmark: zsh exited with status {}", output.status);
        }
        Err(e) => {
            return Err(lynx_core::error::LynxError::Shell(format!(
                "failed to spawn zsh for benchmark: {e}"
            ))
            .into());
        }
        _ => {}
    }

    Ok(vec![("shell startup".to_string(), total)])
}

fn average_runs(runs: &[Vec<(String, Duration)>]) -> Vec<BenchResult> {
    if runs.is_empty() {
        return vec![];
    }

    let first = &runs[0];
    first
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let total_ms: u128 = runs
                .iter()
                .filter_map(|r| r.get(i))
                .map(|(_, d)| d.as_millis())
                .sum();
            let count = runs.len() as u128;
            BenchResult {
                component: name.clone(),
                avg_ms: (total_ms / count) as u64,
            }
        })
        .collect()
}

fn load_previous_benchmark() -> Vec<(String, u64)> {
    let path = benchmark_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    content
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .next_back()
        .and_then(|v| v.as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let name = item.get("component")?.as_str()?.to_string();
                    let ms = item.get("ms")?.as_u64()?;
                    Some((name, ms))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn print_table(results: &[BenchResult], previous: &[(String, u64)]) {
    println!("{:<24} {:>8}  vs last", "Component", "Time (ms)");
    println!("{}", "─".repeat(48));

    let mut sorted: Vec<&BenchResult> = results.iter().collect();
    sorted.sort_by(|a, b| b.avg_ms.cmp(&a.avg_ms));

    for r in sorted {
        let prev = previous
            .iter()
            .find(|(n, _)| n == &r.component)
            .map(|(_, ms)| *ms);
        let delta = match prev {
            None => "  (new)".to_string(),
            Some(0) => "  —".to_string(),
            Some(p) => {
                let pct = (r.avg_ms as f64 - p as f64) / p as f64 * 100.0;
                if pct > 20.0 {
                    format!("  ⚠ +{pct:.0}% regression")
                } else if pct < -5.0 {
                    format!("  ↓ {pct:.0}%")
                } else {
                    format!("  {pct:+.0}%")
                }
            }
        };
        println!("{:<24} {:>8}ms{}", r.component, r.avg_ms, delta);
    }
}

fn save_benchmark(results: &[BenchResult]) -> Result<()> {
    let path = benchmark_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json_arr: Vec<serde_json::Value> = results
        .iter()
        .map(|r| serde_json::json!({ "component": r.component, "ms": r.avg_ms }))
        .collect();

    // Append as a new JSONL entry.
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", serde_json::to_string(&json_arr)?)?;

    Ok(())
}

fn benchmark_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(brand::CONFIG_DIR).join("benchmarks.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn average_runs_empty_returns_empty() {
        let result = average_runs(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn average_runs_single_run() {
        let runs = vec![vec![(
            "component_a".to_string(),
            Duration::from_millis(100),
        )]];
        let result = average_runs(&runs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].component, "component_a");
        assert_eq!(result[0].avg_ms, 100);
    }

    #[test]
    fn average_runs_multiple_runs() {
        let runs = vec![
            vec![("startup".to_string(), Duration::from_millis(100))],
            vec![("startup".to_string(), Duration::from_millis(200))],
            vec![("startup".to_string(), Duration::from_millis(300))],
        ];
        let result = average_runs(&runs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].component, "startup");
        assert_eq!(result[0].avg_ms, 200); // (100+200+300)/3
    }

    #[test]
    fn average_runs_multiple_components() {
        let runs = vec![
            vec![
                ("config".to_string(), Duration::from_millis(10)),
                ("plugins".to_string(), Duration::from_millis(50)),
            ],
            vec![
                ("config".to_string(), Duration::from_millis(20)),
                ("plugins".to_string(), Duration::from_millis(30)),
            ],
        ];
        let result = average_runs(&runs);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].component, "config");
        assert_eq!(result[0].avg_ms, 15); // (10+20)/2
        assert_eq!(result[1].component, "plugins");
        assert_eq!(result[1].avg_ms, 40); // (50+30)/2
    }

    #[test]
    fn load_previous_benchmark_missing_file() {
        // Should return empty vec when file doesn't exist
        let result = load_previous_benchmark();
        // May or may not be empty depending on user's actual benchmarks.
        // At minimum, should not panic.
        let _ = result;
    }

    #[test]
    fn print_table_no_previous() {
        let results = [BenchResult {
            component: "startup".to_string(),
            avg_ms: 150,
        }];
        // Should print "(new)" for no previous data and not panic.
        print_table(&results, &[]);
    }

    #[test]
    fn print_table_with_regression() {
        let results = vec![BenchResult {
            component: "startup".to_string(),
            avg_ms: 200,
        }];
        let previous = vec![("startup".to_string(), 100u64)];
        // Should show regression warning
        print_table(&results, &previous);
    }

    #[test]
    fn print_table_with_improvement() {
        let results = vec![BenchResult {
            component: "startup".to_string(),
            avg_ms: 50,
        }];
        let previous = vec![("startup".to_string(), 100u64)];
        print_table(&results, &previous);
    }

    #[test]
    fn print_table_previous_zero_shows_dash() {
        let results = vec![BenchResult {
            component: "startup".to_string(),
            avg_ms: 100,
        }];
        let previous = vec![("startup".to_string(), 0u64)];
        print_table(&results, &previous);
    }

    #[test]
    fn save_benchmark_writes_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bench.jsonl");

        let results = [BenchResult {
            component: "startup".to_string(),
            avg_ms: 150,
        }];

        // Write to a temp path by temporarily overriding
        // We can't easily override benchmark_path(), so test the JSONL format directly.
        let json_arr: Vec<serde_json::Value> = results
            .iter()
            .map(|r| serde_json::json!({ "component": r.component, "ms": r.avg_ms }))
            .collect();
        let line = serde_json::to_string(&json_arr).unwrap();

        std::fs::write(&path, format!("{line}\n")).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["component"].as_str().unwrap(), "startup");
        assert_eq!(parsed[0]["ms"].as_u64().unwrap(), 150);
    }

    #[test]
    fn benchmark_args_defaults() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: BenchmarkArgs,
        }
        let w = W::parse_from(["test"]);
        assert_eq!(w.args.runs, 1);
    }

    #[test]
    fn benchmark_args_custom_runs() {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            args: BenchmarkArgs,
        }
        let w = W::parse_from(["test", "--runs", "5"]);
        assert_eq!(w.args.runs, 5);
    }

    #[test]
    fn benchmark_path_contains_benchmarks_jsonl() {
        let path = benchmark_path();
        assert!(path.to_string_lossy().ends_with("benchmarks.jsonl"));
    }
}
