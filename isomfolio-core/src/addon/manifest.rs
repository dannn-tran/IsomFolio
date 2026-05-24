use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AddonManifest {
    pub name: String,
    pub protocol_version: u32,
    pub addon_api_version: u32,
    pub capabilities: Vec<String>,
    pub description: String,
    #[serde(skip)]
    pub executable: PathBuf,
}

pub fn discover_addons(dir: &Path) -> Vec<AddonManifest> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| load_manifest_from_dir(&e.path()))
        .collect()
}

fn load_manifest_from_dir(dir: &Path) -> Option<AddonManifest> {
    let text = std::fs::read_to_string(dir.join("isomfolio-addon.json")).ok()?;
    let mut manifest: AddonManifest = serde_json::from_str(&text).ok()?;
    let dir_name = dir.file_name()?.to_str()?;
    let exe = if cfg!(windows) {
        dir.join(format!("{}.exe", dir_name))
    } else {
        dir.join(dir_name)
    };
    if is_executable(&exe) {
        manifest.executable = exe;
        Some(manifest)
    } else {
        None
    }
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        path.extension().map(|e| e == "exe").unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    fn make_addon_dir(parent: &Path, name: &str, manifest_json: &str, make_exe: bool) {
        let dir = parent.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("isomfolio-addon.json"), manifest_json).unwrap();
        if make_exe {
            let exe = dir.join(name);
            fs::write(&exe, b"#!/bin/sh\n").unwrap();
            let mut perms = fs::metadata(&exe).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe, perms).unwrap();
        }
    }

    const VALID_MANIFEST: &str = r#"{
        "name": "test-addon",
        "protocol_version": 1,
        "addon_api_version": 1,
        "capabilities": ["classify"],
        "description": "Test addon"
    }"#;

    #[test]
    fn discovers_valid_addon() {
        let tmp = TempDir::new().unwrap();
        make_addon_dir(tmp.path(), "isomfolio-test", VALID_MANIFEST, true);
        let addons = discover_addons(tmp.path());
        assert_eq!(addons.len(), 1);
        assert_eq!(addons[0].name, "test-addon");
        assert_eq!(addons[0].capabilities, vec!["classify"]);
    }

    #[test]
    fn skips_missing_executable() {
        let tmp = TempDir::new().unwrap();
        make_addon_dir(tmp.path(), "isomfolio-test", VALID_MANIFEST, false);
        assert!(discover_addons(tmp.path()).is_empty());
    }

    #[test]
    fn skips_invalid_manifest() {
        let tmp = TempDir::new().unwrap();
        make_addon_dir(tmp.path(), "isomfolio-test", "not json", true);
        assert!(discover_addons(tmp.path()).is_empty());
    }

    #[test]
    fn skips_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("isomfolio-test")).unwrap();
        assert!(discover_addons(tmp.path()).is_empty());
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(discover_addons(tmp.path()).is_empty());
    }

    #[test]
    fn nonexistent_dir_returns_empty() {
        assert!(discover_addons(Path::new("/nonexistent/path/xyz")).is_empty());
    }
}
