//! Smoke test against any real `.isfx` packages dropped into this crate's
//! `tests/fixtures/` directory.
//!
//! To test an extension end-to-end:
//! ```text
//! cp path/to/your-extension.isfx isomfolio-extension-host/tests/fixtures/
//! cargo test -p isomfolio-extension-host
//! ```
//!
//! For each `.isfx` found, installs it into a temp dir, launches the extension,
//! asserts handshake + ping, then uninstalls. If no packages are found, the
//! test prints a notice and passes (so `cargo test` works on a fresh checkout).
//! To force a failure when no packages are present, set `ISFX_REQUIRE_PACKAGE=1`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use isomfolio_extension_host::{install_extension_package, uninstall_extension, ExtensionProcess};
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Path to a real face image (reused from the C# test suite). Used to exercise
/// inference paths that need an actual decodable image. Returns `None` if the
/// asset is missing — capability tests that depend on it should skip.
fn sample_face_image() -> Option<PathBuf> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has parent");
    let path = workspace_root
        .join("extensions-cs")
        .join("Faces.Tests")
        .join("Assets")
        .join("test_face.jpg");
    path.exists().then_some(path)
}

fn find_isfx_packages() -> Vec<PathBuf> {
    let dir = fixtures_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("isfx"))
        .collect();
    found.sort();
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

        for capability in proc.manifest.capabilities.clone() {
            exercise_capability(&proc, &capability);
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

/// Drive a real inference call for the given capability. Skips with a notice if
/// no test image is available for capabilities that need one.
fn exercise_capability(proc: &ExtensionProcess, capability: &str) {
    match capability {
        "cluster_faces" => {
            let Some(image) = sample_face_image() else {
                eprintln!("notice: skipping cluster_faces inference — test image missing");
                return;
            };
            let params = serde_json::json!({
                "files": [{
                    "file_id": "smoke-test",
                    "image_path": image.to_string_lossy(),
                    "file_mtime": 0
                }],
                "force_full": true
            });
            let handle = proc.send("cluster_faces", params).expect("send cluster_faces");
            let result_opt = handle.result_rx.recv_timeout(Duration::from_secs(600));
            let result = match result_opt {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    let stderr = proc.last_stderr();
                    panic!(
                        "cluster_faces returned error: {e}. Last stderr:\n{}",
                        stderr.join("\n")
                    );
                }
                Err(_) => {
                    let stderr = proc.last_stderr();
                    panic!(
                        "cluster_faces produced no result within 10 minutes. Last stderr:\n{}",
                        stderr.join("\n")
                    );
                }
            };
            let clusters = result["clusters"].as_array().expect("missing clusters array");
            let noise = result["noise"].as_array().expect("missing noise array");
            let face_count: usize =
                clusters.iter().map(|c| c["members"].as_array().map(|m| m.len()).unwrap_or(0)).sum::<usize>()
                + noise.len();
            assert!(
                face_count >= 1,
                "expected to detect at least one face in test_face.jpg, got {face_count} (clusters={clusters:?}, noise={noise:?})"
            );
        }
        "classify" => {
            let image = sample_face_image()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let params = serde_json::json!({
                "file_id": "smoke-test",
                "thumbnail_path": image,
            });
            let result = proc.call("classify", params).expect("classify call failed");
            assert!(
                result.get("tags").is_some(),
                "classify response missing 'tags' field: {result:?}"
            );
        }
        other => {
            eprintln!("notice: no inference smoke test wired for capability '{other}'");
        }
    }
}
