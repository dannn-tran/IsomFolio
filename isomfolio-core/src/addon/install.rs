use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

use zip::ZipArchive;

use crate::app_paths::{addons_dir, models_dir};

use super::manifest::{AddonManifest, discover_addons};

/// Install an `.isfx` zip package into `addons_dir()`.
///
/// Expected zip layout (flat, no subdirectory):
/// ```text
/// manifest.json
/// <addon-name>          <- executable (same name as the "name" field in manifest)
/// ```
pub fn install_addon_package(package_path: &Path) -> Result<AddonManifest, String> {
    let file = fs::File::open(package_path).map_err(|e| format!("open package: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;

    let manifest: AddonManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|_| "package missing manifest.json".to_string())?;
        let mut s = String::new();
        entry.read_to_string(&mut s).map_err(|e| format!("read manifest: {e}"))?;
        serde_json::from_str(&s).map_err(|e| format!("parse manifest: {e}"))?
    };

    let addon_dir = addons_dir().join(&manifest.name);
    fs::create_dir_all(&addon_dir).map_err(|e| format!("create addon dir: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry {i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry.name().to_string();
        let dest = addon_dir.join(&entry_name);
        let mut out = fs::File::create(&dest).map_err(|e| format!("create {entry_name}: {e}"))?;
        io::copy(&mut entry, &mut out).map_err(|e| format!("extract {entry_name}: {e}"))?;

        #[cfg(unix)]
        if entry_name == manifest.name {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("chmod {entry_name}: {e}"))?;
        }
    }

    // Re-discover to get the executable path resolved properly
    let installed = discover_addons(&addons_dir())
        .into_iter()
        .find(|m| m.name == manifest.name)
        .ok_or_else(|| {
            format!(
                "installed '{}' but executable not found — check the binary name matches the addon name",
                manifest.name
            )
        })?;

    if installed.has_install_step {
        run_install_step(&installed)?;
    }

    Ok(installed)
}

fn run_install_step(manifest: &AddonManifest) -> Result<(), String> {
    let mut child = Command::new(&manifest.executable)
        .arg("install")
        .arg("--data-dir")
        .arg(models_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn installer for '{}': {e}", manifest.name))?;

    let stdout = child.stdout.take().unwrap();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        eprintln!("[{} install] {line}", manifest.name);
    }

    let status = child.wait().map_err(|e| format!("wait for installer: {e}"))?;
    if !status.success() {
        return Err(format!(
            "'{}' installer exited with {}",
            manifest.name,
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string())
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn write_script(dir: &Path, name: &str, script: &str) -> std::path::PathBuf {
        let exe = dir.join(name);
        fs::write(&exe, format!("#!/bin/sh\n{script}")).unwrap();
        let mut perms = fs::metadata(&exe).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&exe, perms).unwrap();
        exe
    }

    fn make_manifest(exe: std::path::PathBuf) -> AddonManifest {
        AddonManifest {
            name: "test-addon".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![],
            description: "test".to_string(),
            has_install_step: true,
            config_schema: vec![],
            executable: exe,
        }
    }

    #[test]
    fn install_step_is_invoked() {
        let tmp = TempDir::new().unwrap();
        let sentinel = tmp.path().join("install_ran");
        let exe = write_script(tmp.path(), "test-addon", &format!("touch {}", sentinel.display()));
        run_install_step(&make_manifest(exe)).expect("install step failed");
        assert!(sentinel.exists(), "install step was not run");
    }

    #[test]
    fn install_step_failure_returns_error() {
        let tmp = TempDir::new().unwrap();
        let exe = write_script(tmp.path(), "test-addon", "exit 1");
        let err = run_install_step(&make_manifest(exe)).unwrap_err();
        assert!(err.contains("installer exited with 1"), "unexpected: {err}");
    }
}

/// Remove an installed addon by name. Leaves model weights untouched.
pub fn uninstall_addon(name: &str) -> Result<(), String> {
    let addon_dir = addons_dir().join(name);
    if addon_dir.exists() {
        fs::remove_dir_all(&addon_dir).map_err(|e| format!("remove addon dir: {e}"))?;
    }
    Ok(())
}
