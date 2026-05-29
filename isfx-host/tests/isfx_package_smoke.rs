//! Smoke test against any real `.isfx` packages found in the workspace.
//!
//! Discovery: walks workspace-rooted candidate dirs (`dist/`, `extensions-cs/dist/`,
//! plus workspace root one level deep) for `*.isfx` files. For each one found,
//! installs it into a temp dir, launches the extension, asserts handshake +
//! ping, then uninstalls.
//!
//! If no packages are found, the test prints a notice and passes (CI-friendly
//! when developers haven't built any packages). To force a failure when no
//! packages are present, set `ISFX_REQUIRE_PACKAGE=1`.

use std::path::{Path, PathBuf};

use isfx_host::{install_extension_package, uninstall_extension, ExtensionProcess};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has parent")
        .to_path_buf()
}

fn find_isfx_packages() -> Vec<PathBuf> {
    let root = workspace_root();
    let candidate_dirs = [
        root.join("dist"),
        root.join("extensions-cs").join("dist"),
        root.clone(),
    ];

    let mut found = Vec::new();
    for dir in candidate_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("isfx") {
                found.push(path);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn each_discovered_isfx_passes_smoke_test() {
    let packages = find_isfx_packages();

    if packages.is_empty() {
        let msg = "no .isfx packages found in workspace dist/ — skipping real-package smoke test";
        if std::env::var("ISFX_REQUIRE_PACKAGE").is_ok() {
            panic!("{msg}");
        }
        eprintln!("notice: {msg}");
        return;
    }

    for pkg in packages {
        eprintln!("smoke testing: {}", pkg.display());

        let install_root = TempDir::new().expect("tempdir");
        let data_dir = TempDir::new().expect("tempdir");

        let manifest = install_extension_package(&pkg, install_root.path())
            .unwrap_or_else(|e| panic!("install {} failed: {e}", pkg.display()));

        let extension_name = manifest.name.clone();
        let proc = ExtensionProcess::launch(manifest, Some(data_dir.path().to_path_buf()))
            .unwrap_or_else(|e| panic!("launch {extension_name} failed: {e}"));

        assert_eq!(
            proc.manifest.name, extension_name,
            "launched manifest name should match installed manifest"
        );
        assert!(
            !proc.manifest.capabilities.is_empty(),
            "{extension_name} declared no capabilities — extension probably broken"
        );

        match proc.call("ping", serde_json::json!({})) {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("unknown") || msg.contains("ping"),
                    "{extension_name} ping failed unexpectedly: {msg}"
                );
            }
        }

        drop(proc);

        uninstall_extension(install_root.path(), &extension_name)
            .unwrap_or_else(|e| panic!("uninstall {extension_name} failed: {e}"));

        assert!(
            !install_root.path().join(&extension_name).exists(),
            "uninstall left {extension_name} on disk"
        );
    }
}
