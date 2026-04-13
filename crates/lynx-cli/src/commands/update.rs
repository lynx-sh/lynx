use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Args;
use lynx_core::error::LynxError;
use sha2::{Digest, Sha256};

use lynx_core::brand;

const CHECK_CACHE_TTL_SECS: u64 = 3600; // 1 hour

#[derive(Args)]
pub struct UpdateArgs {
    /// Only check for updates, don't install
    #[arg(long)]
    pub check: bool,
    /// Install without prompting
    #[arg(long)]
    pub yes: bool,
}

pub fn run(args: UpdateArgs) -> Result<()> {
    // Rate-limit GitHub API calls.
    if let Some(cached) = read_cached_version() {
        if cached.is_fresh() {
            if args.check {
                println!("Latest: {} (current: {})", cached.latest, brand::VERSION);
            }
            if !args.check && is_newer(&cached.latest, brand::VERSION) {
                return do_update(&cached.latest, args.yes);
            } else if !args.check {
                println!("lx is up to date ({})", brand::VERSION);
            }
            return Ok(());
        }
    }

    println!("Checking for updates...");
    let latest = fetch_latest_version()?;
    cache_version(&latest);

    if args.check {
        println!("Latest: {latest} (current: {})", brand::VERSION);
        return Ok(());
    }

    if is_newer(&latest, brand::VERSION) {
        do_update(&latest, args.yes)
    } else {
        println!("lx is up to date ({})", brand::VERSION);
        Ok(())
    }
}

fn do_update(version: &str, yes: bool) -> Result<()> {
    if !yes {
        print!("Update lx to {version}? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let platform = detect_platform();
    let url = format!("{}/releases/download/{version}/lx-{platform}", brand::REPO);

    println!("Downloading {version} for {platform}...");
    let bytes = download(&url)?;

    // Verify checksum (placeholder — real impl fetches .sha256 and compares).
    verify_checksum(&bytes, version)?;

    // Atomic replacement: write to temp, then rename.
    let current_bin = std::env::current_exe()?;
    let tmp = current_bin.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }

    std::fs::rename(&tmp, &current_bin).map_err(|e| {
        anyhow::Error::from(lynx_core::error::LynxError::Io {
            message: format!("failed to replace binary: {e}"),
            path: current_bin.clone(),
            fix: "check file permissions or try running with sudo".into(),
        })
    })?;

    println!("Updated to {version}. Restart your shell or run: exec lx");
    Ok(())
}

fn fetch_latest_version() -> Result<String> {
    let api_url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        brand::REPO.trim_start_matches("https://github.com/")
    );
    let response = ureq::get(&api_url)
        .set("User-Agent", &format!("lx/{}", brand::VERSION))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| {
            LynxError::Shell(format!(
                "failed to reach GitHub releases API: {e} — check your internet connection"
            ))
        })?;
    let body: serde_json::Value = serde_json::from_reader(response.into_reader())
        .context("invalid JSON from GitHub releases API")?;
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            LynxError::Shell(
                "unexpected GitHub API response: missing tag_name — try again later".into(),
            )
        })?;
    Ok(tag.to_string())
}

fn download(url: &str) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .set("User-Agent", &format!("lx/{}", brand::VERSION))
        .call()
        .map_err(|e| {
            LynxError::Shell(format!(
                "download failed: {e} — check your internet connection or download manually from {}",
                brand::REPO
            ))
        })?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes)
        .context("failed to read download body")?;
    Ok(bytes)
}

fn verify_checksum(bytes: &[u8], version: &str) -> Result<()> {
    let platform = detect_platform();
    let checksum_url = format!(
        "{}/releases/download/{version}/lx-{platform}.sha256",
        brand::REPO
    );
    let response = ureq::get(&checksum_url)
        .set("User-Agent", &format!("lx/{}", brand::VERSION))
        .call()
        .map_err(|e| {
            LynxError::Shell(format!(
                "failed to fetch checksum file: {e} — cannot verify download integrity"
            ))
        })?;
    let checksum_text = response
        .into_string()
        .context("failed to read checksum response")?;
    // Checksum files are "<hex>  filename" or just "<hex>"
    let expected = checksum_text
        .split_whitespace()
        .next()
        .ok_or_else(|| LynxError::Shell("checksum file was empty or malformed".into()))?
        .to_lowercase();

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let actual = hex::encode(hasher.finalize());

    if actual != expected {
        return Err(LynxError::Shell(format!(
            "checksum mismatch — download may be corrupt\n  expected: {expected}\n  got:      {actual}\nRun `lx update` again or download manually from {}",
            brand::REPO
        ))
        .into());
    }
    Ok(())
}

fn detect_platform() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    format!("{os}-{arch}")
}

fn is_newer(latest: &str, current: &str) -> bool {
    latest != current && semver_gt(latest, current)
}

fn semver_gt(a: &str, b: &str) -> bool {
    fn parse(s: &str) -> (u64, u64, u64) {
        let s = s.trim_start_matches('v');
        let mut p = s.splitn(3, '.');
        let ma: u64 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let mi: u64 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        let pa: u64 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        (ma, mi, pa)
    }
    parse(a) > parse(b)
}

// ── Cache ──────────────────────────────────────────────────────────────────

struct CachedVersion {
    latest: String,
    checked_at: u64,
}

impl CachedVersion {
    fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now.saturating_sub(self.checked_at) < CHECK_CACHE_TTL_SECS
    }
}

fn cache_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(brand::CONFIG_DIR).join(".update-check")
}

fn read_cached_version() -> Option<CachedVersion> {
    let content = std::fs::read_to_string(cache_path()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    Some(CachedVersion {
        latest: v.get("latest")?.as_str()?.to_string(),
        checked_at: v.get("checked_at")?.as_u64()?,
    })
}

fn cache_version(version: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let json = serde_json::json!({ "latest": version, "checked_at": now });
    let path = cache_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!("failed to create update cache dir: {e}");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, json.to_string()) {
        tracing::warn!("failed to write update cache: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn platform_string_nonempty() {
        assert!(!detect_platform().is_empty());
    }

    #[test]
    fn checksum_hex_is_consistent_for_same_input() {
        let bytes = b"test payload";
        let digest1 = {
            let mut h = Sha256::new();
            h.update(bytes);
            hex::encode(h.finalize())
        };
        let digest2 = {
            let mut h = Sha256::new();
            h.update(bytes);
            hex::encode(h.finalize())
        };
        assert_eq!(digest1, digest2);
        assert_eq!(digest1.len(), 64);
    }

    #[test]
    fn checksum_hex_differs_for_different_inputs() {
        let mut h1 = Sha256::new();
        h1.update(b"payload-a");
        let d1 = hex::encode(h1.finalize());

        let mut h2 = Sha256::new();
        h2.update(b"payload-b");
        let d2 = hex::encode(h2.finalize());

        assert_ne!(d1, d2);
    }
}
