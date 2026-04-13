use std::collections::HashMap;

use chrono::Local;
use sysinfo::System;

/// Build the full `{{TOKEN}}` substitution map for intro rendering.
///
/// `env` should be a snapshot of the current environment (e.g. from `std::env::vars()`).
/// All tokens resolve to a non-empty string where possible; missing data yields an empty string.
pub fn build_token_map(env: &HashMap<String, String>) -> HashMap<String, String> {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let mut map = HashMap::new();

    // --- Identity ---
    map.insert(
        "username".into(),
        env.get("USER").cloned().unwrap_or_default(),
    );
    map.insert(
        "hostname".into(),
        env.get("HOSTNAME").cloned().unwrap_or_default(),
    );
    map.insert(
        "shell".into(),
        env.get("SHELL")
            .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
            .unwrap_or_default(),
    );

    // --- OS ---
    let os = System::long_os_version().unwrap_or_default();
    map.insert("os".into(), os);

    // --- Time ---
    let now = Local::now();
    map.insert("datetime".into(), now.format("%Y-%m-%d %H:%M").to_string());
    map.insert("date".into(), now.format("%Y-%m-%d").to_string());
    map.insert("time".into(), now.format("%H:%M:%S").to_string());

    // --- Uptime ---
    let uptime_secs = System::uptime();
    map.insert("uptime".into(), format_uptime(uptime_secs));

    // --- CPU ---
    let cpus = sys.cpus();
    let cpu_model = cpus
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    map.insert("cpu_model".into(), cpu_model);
    map.insert("cpu_cores".into(), cpus.len().to_string());

    let usage = sys.global_cpu_usage();
    map.insert("cpu_usage".into(), format!("{usage:.1}%"));

    // --- Memory ---
    let used = sys.used_memory();
    let total = sys.total_memory();
    map.insert("mem_used".into(), format_bytes(used));
    map.insert("mem_total".into(), format_bytes(total));
    let pct = if total > 0 {
        (used as f64 / total as f64 * 100.0).round() as u64
    } else {
        0
    };
    map.insert("mem_pct".into(), format!("{pct}%"));

    // --- Lynx ---
    map.insert("lynx_version".into(), env!("CARGO_PKG_VERSION").to_string());

    map
}

fn format_uptime(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_bytes(bytes: u64) -> String {
    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else {
        format!("{:.0} MB", bytes as f64 / MIB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_env() -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("USER".into(), "proxikal".into());
        env.insert("HOSTNAME".into(), "macbook".into());
        env.insert("SHELL".into(), "/bin/zsh".into());
        env
    }

    #[test]
    fn identity_tokens_resolve_from_env() {
        let map = build_token_map(&mock_env());
        assert_eq!(map.get("username").map(String::as_str), Some("proxikal"));
        assert_eq!(map.get("hostname").map(String::as_str), Some("macbook"));
        assert_eq!(map.get("shell").map(String::as_str), Some("zsh"));
    }

    #[test]
    fn all_expected_tokens_present() {
        let map = build_token_map(&mock_env());
        let required = [
            "username",
            "hostname",
            "shell",
            "os",
            "datetime",
            "date",
            "time",
            "uptime",
            "cpu_model",
            "cpu_cores",
            "cpu_usage",
            "mem_used",
            "mem_total",
            "mem_pct",
            "lynx_version",
        ];
        for key in &required {
            assert!(map.contains_key(*key), "missing token: {}", key);
        }
    }

    #[test]
    fn numeric_tokens_are_non_empty() {
        let map = build_token_map(&mock_env());
        for key in &["cpu_cores", "cpu_usage", "mem_used", "mem_total", "mem_pct"] {
            let val = map.get(*key).expect("token missing");
            assert!(!val.is_empty(), "token {} is empty", key);
        }
    }

    #[test]
    fn shell_basename_extracted() {
        let mut env = mock_env();
        env.insert("SHELL".into(), "/usr/local/bin/fish".into());
        let map = build_token_map(&env);
        assert_eq!(map.get("shell").map(String::as_str), Some("fish"));
    }

    #[test]
    fn missing_env_vars_yield_empty_string() {
        let map = build_token_map(&HashMap::new());
        assert_eq!(map.get("username").map(String::as_str), Some(""));
        assert_eq!(map.get("hostname").map(String::as_str), Some(""));
    }

    #[test]
    fn lynx_version_matches_cargo_pkg() {
        let map = build_token_map(&mock_env());
        assert_eq!(
            map.get("lynx_version").map(String::as_str),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn format_uptime_hours_and_minutes() {
        assert_eq!(format_uptime(3661), "1h 1m");
        assert_eq!(format_uptime(7200), "2h 0m");
        assert_eq!(format_uptime(45), "0m");
        assert_eq!(format_uptime(90), "1m");
    }

    #[test]
    fn format_bytes_gib_and_mib() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(8 * 1024 * 1024 * 1024), "8.0 GB");
        assert_eq!(format_bytes(512 * 1024 * 1024), "512 MB");
    }
}
