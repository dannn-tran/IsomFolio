use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

use zip::ZipArchive;

use crate::app_paths::extensions_dir;

use super::manifest::{ExtensionManifest, discover_extensions};

/// Install an `.isfx` zip package into `extensions_dir()`.
///
/// Expected zip layout (flat, no subdirectory):
/// ```text
/// manifest.json
/// <extension-name>          <- executable (same name as the "name" field in manifest)
/// ```
pub fn install_extension_package(package_path: &Path) -> Result<ExtensionManifest, String> {
    let file = fs::File::open(package_path).map_err(|e| format!("open package: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;

    let manifest: ExtensionManifest = {
        let mut entry = archive
            .by_name("manifest.json")
            .map_err(|_| "package missing manifest.json".to_string())?;
        let mut s = String::new();
        entry.read_to_string(&mut s).map_err(|e| format!("read manifest: {e}"))?;
        serde_json::from_str(&s).map_err(|e| format!("parse manifest: {e}"))?
    };

    let extension_dir = extensions_dir().join(&manifest.name);
    fs::create_dir_all(&extension_dir).map_err(|e| format!("create extension dir: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry {i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        // Strip any path components from the entry name — prevents zip slip (path traversal).
        let entry_name = Path::new(entry.name())
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("zip entry '{}' has no valid filename", entry.name()))?
            .to_string();
        let dest = extension_dir.join(&entry_name);
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
    let installed = discover_extensions(&extensions_dir())
        .into_iter()
        .find(|m| m.name == manifest.name)
        .ok_or_else(|| {
            format!(
                "installed '{}' but executable not found — check the binary name matches the extension name",
                manifest.name
            )
        })?;

    if installed.needs_setup {
        run_setup(&installed)?;
    }

    Ok(installed)
}

fn run_setup(manifest: &ExtensionManifest) -> Result<(), String> {
    let mut child = Command::new(&manifest.executable)
        .arg("setup")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn setup for '{}': {e}", manifest.name))?;

    let stdout = child.stdout.take().unwrap();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        eprintln!("[{} setup] {line}", manifest.name);
    }

    let status = child.wait().map_err(|e| format!("wait for setup: {e}"))?;
    if !status.success() {
        return Err(format!(
            "'{}' setup exited with {}",
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

    fn make_manifest(exe: std::path::PathBuf) -> ExtensionManifest {
        ExtensionManifest {
            name: "test-extension".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec![],
            description: "test".to_string(),
            needs_setup: true,
            config_schema: vec![],
            executable: exe,
        }
    }

    #[test]
    fn setup_is_invoked() {
        let tmp = TempDir::new().unwrap();
        let sentinel = tmp.path().join("setup_ran");
        let exe = write_script(tmp.path(), "test-extension", &format!("touch {}", sentinel.display()));
        run_setup(&make_manifest(exe)).expect("setup failed");
        assert!(sentinel.exists(), "setup was not run");
    }

    #[test]
    fn setup_failure_returns_error() {
        let tmp = TempDir::new().unwrap();
        let exe = write_script(tmp.path(), "test-extension", "exit 1");
        let err = run_setup(&make_manifest(exe)).unwrap_err();
        assert!(err.contains("setup exited with 1"), "unexpected: {err}");
    }
}

/// Remove an installed extension by name, including its downloaded model weights.
pub fn uninstall_extension(name: &str) -> Result<(), String> {
    let extension_dir = extensions_dir().join(name);
    if extension_dir.exists() {
        fs::remove_dir_all(&extension_dir).map_err(|e| format!("remove extension dir: {e}"))?;
    }
    Ok(())
}
