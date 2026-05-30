use std::fs;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zip::ZipArchive;

use crate::manifest::{discover_extensions, ExtensionManifest};
use crate::process::BoundedLines;

/// Install an `.isfx` zip package into `extensions_root`.
///
/// Expected zip layout:
/// ```text
/// manifest.json
/// <extension-name>          <- executable (same name as the "name" field in manifest)
/// [other files/dirs]        <- native libs, models, config — directory structure preserved
/// ```
pub fn install_extension_package(
    package_path: &Path,
    extensions_root: &Path,
) -> Result<ExtensionManifest, String> {
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

    let extension_dir = extensions_root.join(&manifest.name);
    fs::create_dir_all(&extension_dir).map_err(|e| format!("create extension dir: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry {i}: {e}"))?;
        let raw_name = entry.name().to_string();
        let rel_path = safe_relative_path(&raw_name)
            .ok_or_else(|| format!("zip entry '{raw_name}' has unsafe or invalid path"))?;
        let dest = extension_dir.join(&rel_path);
        if entry.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| format!("create dir {rel_path:?}: {e}"))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create parent of {rel_path:?}: {e}"))?;
        }
        let mut out = fs::File::create(&dest)
            .map_err(|e| format!("create {rel_path:?}: {e}"))?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("extract {rel_path:?}: {e}"))?;

        #[cfg(unix)]
        if rel_path == Path::new(&manifest.name) {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("chmod {rel_path:?}: {e}"))?;
        }
    }

    let installed = discover_extensions(extensions_root)
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

/// Resolve a zip entry's path to a safe relative path under the extension dir.
/// Rejects absolute paths, `..` components, drive letters, and other escapes.
fn safe_relative_path(raw: &str) -> Option<PathBuf> {
    use std::path::Component;
    let p = Path::new(raw);
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    Some(out)
}

fn run_setup(manifest: &ExtensionManifest) -> Result<(), String> {
    let mut child = Command::new(&manifest.executable)
        .arg("setup")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn setup for '{}': {e}", manifest.name))?;

    let stdout = child.stdout.take().expect("stdout piped on spawn");
    for line in BoundedLines(BufReader::new(stdout)) {
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

/// Remove an installed extension by name from `extensions_root`, including its files.
pub fn uninstall_extension(extensions_root: &Path, name: &str) -> Result<(), String> {
    let extension_dir = extensions_root.join(name);
    if extension_dir.exists() {
        fs::remove_dir_all(&extension_dir).map_err(|e| format!("remove extension dir: {e}"))?;
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

    mod safe_relative_path_tests {
        use super::super::safe_relative_path;
        use std::path::PathBuf;

        #[test]
        fn accepts_flat_filename() {
            assert_eq!(safe_relative_path("faces"), Some(PathBuf::from("faces")));
        }

        #[test]
        fn accepts_nested_path() {
            assert_eq!(
                safe_relative_path("runtimes/osx-arm64/native/libonnxruntime.dylib"),
                Some(PathBuf::from("runtimes/osx-arm64/native/libonnxruntime.dylib"))
            );
        }

        #[test]
        fn rejects_parent_dir_escape() {
            assert_eq!(safe_relative_path("../etc/passwd"), None);
            assert_eq!(safe_relative_path("a/../../b"), None);
        }

        #[test]
        fn rejects_absolute_paths() {
            assert_eq!(safe_relative_path("/etc/passwd"), None);
        }

        #[test]
        fn strips_curdir() {
            assert_eq!(safe_relative_path("./faces"), Some(PathBuf::from("faces")));
        }

        #[test]
        fn rejects_empty() {
            assert_eq!(safe_relative_path(""), None);
            assert_eq!(safe_relative_path("./"), None);
        }
    }
}
