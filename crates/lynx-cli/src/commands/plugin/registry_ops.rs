// Registry operations: search, info, update, checksum, index-validate, and install-from-registry.
//
// All network/IO work is offloaded via spawn_blocking so the async runtime stays unblocked.

use anyhow::{Context, Result};
use lynx_config::snapshot::mutate_config_transaction;
use lynx_core::error::LynxError;
use lynx_registry::fetch::{
    check_for_update, checksum_file, checksum_plugin_dir, fetch_plugin, update_plugin, FetchOptions,
};
use lynx_registry::index::{
    get_index, load_index_from, load_lock, plugins_install_dir, validate_index,
};
use lynx_registry::lock::{LockFile, PluginSource};
use std::path::{Path, PathBuf};

pub(super) async fn cmd_add_from_registry(name: &str, force: bool) -> Result<()> {
    let install_dir = tokio::task::spawn_blocking({
        let name = name.to_string();
        move || {
            fetch_plugin(
                &name,
                &FetchOptions {
                    force,
                    refresh_index: true,
                    ..Default::default()
                },
            )
        }
    })
    .await??;

    mutate_config_transaction(&format!("plugin-add-{name}"), |cfg| {
        if !cfg.enabled_plugins.contains(&name.to_string()) {
            cfg.enabled_plugins.push(name.to_string());
        }
        Ok(())
    })?;
    println!("installed '{}' to {}", name, install_dir.display());
    Ok(())
}

pub(super) async fn cmd_reinstall(name: &str) -> Result<()> {
    cmd_add_from_registry(name, true).await
}

pub(super) async fn cmd_search(query: &str, refresh: bool) -> Result<()> {
    let idx = tokio::task::spawn_blocking(move || get_index(refresh, None)).await??;

    let results = idx.search(query);
    if results.is_empty() {
        println!("no results for '{query}'");
        return Ok(());
    }

    let lock = load_lock().unwrap_or_default();
    println!("{:<20} {:<10} DESCRIPTION", "NAME", "VERSION");
    println!("{}", "-".repeat(60));
    for entry in results {
        let installed = if lock.find(&entry.name).is_some() {
            "*"
        } else {
            " "
        };
        println!(
            "{installed}{:<19} {:<10} {}",
            entry.name, entry.latest_version, entry.description
        );
    }
    println!("\n* = installed   install: lx plugin add <name>");
    Ok(())
}

pub(super) async fn cmd_info(name: &str) -> Result<()> {
    let idx = tokio::task::spawn_blocking(|| get_index(false, None)).await??;
    let entry = idx.find(name).ok_or_else(|| {
        anyhow::Error::from(lynx_core::error::LynxError::NotFound {
            item_type: "Plugin".into(),
            name: name.to_string(),
            hint: "run `lx browse` to see available packages".into(),
        })
    })?;

    let lock = load_lock().unwrap_or_default();
    let installed = lock.find(name);

    println!("name:        {}", entry.name);
    println!("description: {}", entry.description);
    println!("author:      {}", entry.author);
    println!("latest:      {}", entry.latest_version);
    println!("versions:    {}", entry.versions.len());
    for v in &entry.versions {
        let min = v.min_lynx_version.as_deref().unwrap_or("any");
        println!("  {} — min_lynx: {min}", v.version);
    }
    if let Some(locked) = installed {
        println!("installed:   v{}", locked.version);
        if locked.version != entry.latest_version {
            println!("             (update available: {})", entry.latest_version);
        }
    } else {
        println!("installed:   no   (lx plugin add {name})");
    }
    Ok(())
}

pub(super) async fn cmd_update(name: Option<&str>, all: bool) -> Result<()> {
    if all {
        let lock = load_lock().unwrap_or_default();
        let registry_names: Vec<String> = lock
            .entries
            .iter()
            .filter(|e| e.source == PluginSource::Registry)
            .map(|e| e.name.clone())
            .collect();

        if registry_names.is_empty() {
            println!("no registry-installed plugins to update");
            return Ok(());
        }

        for plugin_name in &registry_names {
            match update_one(plugin_name).await {
                Ok(_) => {}
                Err(e) => println!("warning: failed to update '{plugin_name}': {e}"),
            }
        }
        return Ok(());
    }

    let name = name.ok_or_else(|| {
        anyhow::Error::from(lynx_core::error::LynxError::Plugin(
            "provide a plugin name or use --all".into(),
        ))
    })?;
    update_one(name).await
}

async fn update_one(name: &str) -> Result<()> {
    let update_available = tokio::task::spawn_blocking({
        let name = name.to_string();
        move || check_for_update(&name, true, None)
    })
    .await??;

    match update_available {
        None => println!("'{name}' is already up to date (or not registry-installed)"),
        Some((current, latest)) => {
            println!("updating '{name}': {current} → {latest}");
            tokio::task::spawn_blocking({
                let name = name.to_string();
                move || update_plugin(&name, None)
            })
            .await??;
            println!("updated '{name}' to {latest}");
        }
    }
    Ok(())
}

pub(super) fn cmd_checksum(target: &str) -> Result<()> {
    let path = PathBuf::from(target);
    if path.exists() && path.is_file() {
        let digest = checksum_file(&path)?;
        println!("{digest}");
        return Ok(());
    }

    let name = target;
    let lock = load_lock()?;
    let (expected, actual, plugin_dir) = verify_installed_plugin_checksum(name, &lock)?;

    if actual == expected {
        println!(
            "checksum OK for '{}'\nexpected: {}\nactual:   {}\npath:     {}",
            name,
            expected,
            actual,
            plugin_dir.display()
        );
        Ok(())
    } else {
        Err(LynxError::Registry(format!(
            "checksum mismatch for '{}'\nexpected: {}\nactual:   {}\npath:     {}",
            name,
            expected,
            actual,
            plugin_dir.display()
        ))
        .into())
    }
}

pub(super) fn cmd_index_validate(path: &str) -> Result<()> {
    validate_registry_index_path(Path::new(path))?;
    println!("index is valid: {path}");
    Ok(())
}

fn verify_installed_plugin_checksum(
    name: &str,
    lock: &LockFile,
) -> Result<(String, String, PathBuf)> {
    let locked = lock.find(name).with_context(|| {
        format!("plugin '{name}' not found in lynx.lock (install it from registry first)")
    })?;
    let expected = locked
        .installed_checksum_sha256
        .as_ref()
        .or(Some(&locked.checksum_sha256))
        .ok_or_else(|| {
            anyhow::Error::from(lynx_core::error::LynxError::Plugin(format!(
                "no checksum recorded for '{name}'"
            )))
        })?
        .to_string();

    let plugin_dir = plugins_install_dir().join(name);
    if !plugin_dir.exists() {
        return Err(LynxError::NotFound {
            item_type: "Plugin directory".into(),
            name: name.to_string(),
            hint: format!("expected at {}", plugin_dir.display()),
        }
        .into());
    }
    let actual = checksum_plugin_dir(&plugin_dir)?;
    Ok((expected, actual, plugin_dir))
}

fn validate_registry_index_path(path: &Path) -> Result<()> {
    let idx = load_index_from(path)?;
    validate_index(&idx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lynx_test_utils::{env_lock, temp_home};

    struct EnvGuard {
        vars: Vec<(String, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&str]) -> Self {
            let vars = keys
                .iter()
                .map(|k| (k.to_string(), std::env::var_os(k)))
                .collect();
            Self { vars }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.vars {
                match v {
                    Some(val) => std::env::set_var(k, val),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn verify_installed_plugin_checksum_matches_expected() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        let lynx_dir = home.path().join(".config/lynx");
        std::env::set_var("LYNX_DIR", &lynx_dir);

        let install_root = lynx_dir.join("plugins/git");
        std::fs::create_dir_all(install_root.join("shell")).expect("create plugin dir");
        std::fs::write(install_root.join(lynx_core::brand::PLUGIN_MANIFEST), "x")
            .expect("write plugin.toml");
        std::fs::write(install_root.join("shell/init.zsh"), "y").expect("write init");
        let checksum = checksum_plugin_dir(&install_root).expect("checksum");

        let lock = LockFile {
            entries: vec![lynx_registry::lock::LockEntry {
                name: "git".into(),
                version: "1.0.0".into(),
                checksum_sha256: "archive-hash".into(),
                installed_checksum_sha256: Some(checksum.clone()),
                url: "https://example.com/git.tar.gz".into(),
                source: PluginSource::Registry,
            }],
        };

        let (expected, actual, _path) =
            verify_installed_plugin_checksum("git", &lock).expect("verify");
        assert_eq!(expected, checksum);
        assert_eq!(actual, checksum);
    }

    #[test]
    fn verify_installed_plugin_checksum_detects_mismatch() {
        let _lock = env_lock().lock().expect("lock");
        let _guard = EnvGuard::new(&["HOME", "LYNX_DIR"]);
        let home = temp_home();
        std::env::set_var("HOME", home.path());
        let lynx_dir = home.path().join(".config/lynx");
        std::env::set_var("LYNX_DIR", &lynx_dir);

        let install_root = lynx_dir.join("plugins/git");
        std::fs::create_dir_all(install_root.join("shell")).expect("create plugin dir");
        std::fs::write(install_root.join(lynx_core::brand::PLUGIN_MANIFEST), "x")
            .expect("write plugin.toml");
        std::fs::write(install_root.join("shell/init.zsh"), "y").expect("write init");

        let lock = LockFile {
            entries: vec![lynx_registry::lock::LockEntry {
                name: "git".into(),
                version: "1.0.0".into(),
                checksum_sha256: "archive-hash".into(),
                installed_checksum_sha256: Some("not-the-real-hash".into()),
                url: "https://example.com/git.tar.gz".into(),
                source: PluginSource::Registry,
            }],
        };

        let (expected, actual, _path) =
            verify_installed_plugin_checksum("git", &lock).expect("verify");
        assert_ne!(expected, actual);
    }

    #[test]
    fn checksum_file_target_computes_sha256_hex() {
        let home = temp_home();
        let file = home.path().join("artifact.tar.gz");
        std::fs::write(&file, b"abc").expect("write file");
        let digest = checksum_file(&file).expect("checksum file");
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn validate_registry_index_path_accepts_valid_index() {
        let home = temp_home();
        let index = home.path().join("valid-index.toml");
        std::fs::write(
            &index,
            r#"
[[plugin]]
name = "git"
description = "Git integration"
author = "proxikal"
latest_version = "1.0.0"

[[plugin.versions]]
version = "1.0.0"
url = "https://example.com/git-1.0.0.tar.gz"
checksum_sha256 = "abc123"
"#,
        )
        .expect("write valid index");

        assert!(validate_registry_index_path(&index).is_ok());
    }

    #[test]
    fn validate_registry_index_path_rejects_invalid_index() {
        let home = temp_home();
        let index = home.path().join("invalid-index.toml");
        std::fs::write(
            &index,
            r#"
[[plugin]]
name = "git"
description = "Git integration"
author = "proxikal"
latest_version = "1.0.0"

[[plugin.versions]]
version = "1.0.0"
url = "https://example.com/git-1.0.0.tar.gz"
checksum_sha256 = ""
"#,
        )
        .expect("write invalid index");

        let err = validate_registry_index_path(&index).expect_err("expected invalid index");
        assert!(err.to_string().contains("empty checksum_sha256"));
    }
}
