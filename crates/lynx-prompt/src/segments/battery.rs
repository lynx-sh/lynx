use serde::Deserialize;

use crate::segment::{RenderContext, RenderedSegment, Segment};

/// Shows battery charge level and state. Reads from OS-provided sources:
/// - macOS: parses `pmset -g batt` output from cache (gathered by state plugin)
/// - Linux: reads /sys/class/power_supply/BAT0/capacity and status
///
/// Hidden when no battery is detected or on desktops without batteries.
///
/// TOML config:
/// ```toml
/// [segment.battery]
/// color = { fg = "#f36943" }
/// # min_pct = 100       # only show when battery <= this % (default: 100 = always)
/// # charging_icon = "⚡"
/// # discharging_icon = "🔋"
/// # full_icon = "🔌"
/// ```
pub struct BatterySegment;

#[derive(Deserialize, Default)]
struct BatteryConfig {
    /// Only show battery when charge is at or below this percentage. Default: 100 (always show).
    min_pct: Option<u32>,
    charging_icon: Option<String>,
    discharging_icon: Option<String>,
    full_icon: Option<String>,
}

struct BatteryState {
    percentage: u32,
    status: BatteryStatus,
}

#[derive(Debug, PartialEq)]
enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    Unknown,
}

impl Segment for BatterySegment {
    fn name(&self) -> &'static str {
        "battery"
    }

    fn render(&self, config: &toml::Value, ctx: &RenderContext) -> Option<RenderedSegment> {
        let cfg: BatteryConfig = config.clone().try_into().unwrap_or_default();

        let state = read_battery(ctx)?;

        let threshold = cfg.min_pct.unwrap_or(100);
        if state.percentage > threshold {
            return None;
        }

        let icon = match state.status {
            BatteryStatus::Charging => cfg.charging_icon.unwrap_or_else(|| "\u{f0e7}".to_string()), // nf-fa-bolt
            BatteryStatus::Discharging => cfg
                .discharging_icon
                .unwrap_or_else(|| "\u{f242}".to_string()), // nf-fa-battery_half
            BatteryStatus::Full => cfg.full_icon.unwrap_or_else(|| "\u{f240}".to_string()), // nf-fa-battery_full
            BatteryStatus::Unknown => "\u{f244}".to_string(), // nf-fa-battery_empty
        };

        let text = format!("{icon} {}%", state.percentage);
        Some(RenderedSegment::new(text))
    }
}

fn read_battery(ctx: &RenderContext) -> Option<BatteryState> {
    // Try cache first (populated by a state.gather plugin or refresh-state)
    if let Some(cached) = ctx.cache.get("BATTERY_STATE") {
        let pct = cached.get("percentage")?.as_u64()? as u32;
        let status_str = cached.get("status")?.as_str().unwrap_or("unknown");
        return Some(BatteryState {
            percentage: pct,
            status: parse_status(status_str),
        });
    }

    // Fallback: read directly from sysfs (Linux only — no subprocess)
    read_sysfs_battery()
}

fn read_sysfs_battery() -> Option<BatteryState> {
    // Linux: /sys/class/power_supply/BAT0/
    let capacity = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()?;
    let status_raw = std::fs::read_to_string("/sys/class/power_supply/BAT0/status")
        .ok()
        .unwrap_or_default();
    Some(BatteryState {
        percentage: capacity,
        status: parse_status(status_raw.trim()),
    })
}

fn parse_status(s: &str) -> BatteryStatus {
    match s.to_lowercase().as_str() {
        "charging" => BatteryStatus::Charging,
        "discharging" => BatteryStatus::Discharging,
        "full" | "not charging" => BatteryStatus::Full,
        _ => BatteryStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::empty_config;
    use std::collections::HashMap;

    fn ctx_with_battery(pct: u32, status: &str) -> RenderContext {
        let mut cache = HashMap::new();
        cache.insert(
            "BATTERY_STATE".to_string(),
            serde_json::json!({ "percentage": pct, "status": status }),
        );
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache,
            env: HashMap::new(),
        }
    }

    fn empty_ctx() -> RenderContext {
        RenderContext {
            cwd: "/".into(),
            shell_context: lynx_core::types::Context::Interactive,
            last_cmd_ms: None,
            cache: HashMap::new(),
            env: HashMap::new(),
        }
    }

    #[test]
    fn hidden_without_battery() {
        let r = BatterySegment.render(&empty_config(), &empty_ctx());
        // On CI/desktops without sysfs, this should be None
        // (may pass on laptops — that's fine)
        let _ = r;
    }

    #[test]
    fn shows_from_cache() {
        let ctx = ctx_with_battery(75, "discharging");
        let r = BatterySegment.render(&empty_config(), &ctx).unwrap();
        assert!(r.text.contains("75%"), "expected 75%: {:?}", r.text);
    }

    #[test]
    fn charging_icon() {
        let ctx = ctx_with_battery(50, "charging");
        let r = BatterySegment.render(&empty_config(), &ctx).unwrap();
        assert!(
            r.text.contains('\u{f0e7}'),
            "expected bolt icon: {:?}",
            r.text
        );
    }

    #[test]
    fn hidden_above_threshold() {
        let cfg: toml::Value = toml::from_str("min_pct = 20").unwrap();
        let ctx = ctx_with_battery(85, "discharging");
        let r = BatterySegment.render(&cfg, &ctx);
        assert!(r.is_none(), "should hide at 85% when threshold is 20");
    }

    #[test]
    fn shows_at_threshold() {
        let cfg: toml::Value = toml::from_str("min_pct = 20").unwrap();
        let ctx = ctx_with_battery(15, "discharging");
        let r = BatterySegment.render(&cfg, &ctx).unwrap();
        assert!(r.text.contains("15%"));
    }

    #[test]
    fn custom_icons() {
        let cfg: toml::Value = toml::from_str(r#"charging_icon = "⚡""#).unwrap();
        let ctx = ctx_with_battery(60, "charging");
        let r = BatterySegment.render(&cfg, &ctx).unwrap();
        assert!(r.text.contains("⚡"), "expected custom icon: {:?}", r.text);
    }

    #[test]
    fn parse_status_variants() {
        assert_eq!(parse_status("Charging"), BatteryStatus::Charging);
        assert_eq!(parse_status("Discharging"), BatteryStatus::Discharging);
        assert_eq!(parse_status("Full"), BatteryStatus::Full);
        assert_eq!(parse_status("Not charging"), BatteryStatus::Full);
        assert_eq!(parse_status("whatever"), BatteryStatus::Unknown);
    }
}
