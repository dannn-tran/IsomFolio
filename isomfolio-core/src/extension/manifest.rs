use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigFieldKind {
    #[default]
    Text,
    Secret,
    Select,
    Number,
    Integer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub kind: ConfigFieldKind,
    pub default: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionManifest {
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub description: String,
    #[serde(default)]
    pub needs_setup: bool,
    #[serde(default)]
    pub config_schema: Vec<ConfigField>,
    #[serde(skip)]
    pub executable: PathBuf,
}

pub fn discover_extensions(dir: &Path) -> Vec<ExtensionManifest> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| load_manifest_from_dir(&e.path()))
        .collect()
}

fn load_manifest_from_dir(dir: &Path) -> Option<ExtensionManifest> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let mut manifest: ExtensionManifest = serde_json::from_str(&text).ok()?;
    let exe = if cfg!(windows) {
        dir.join(format!("{}.exe", manifest.name))
    } else {
        dir.join(&manifest.name)
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

    fn make_extension_dir(parent: &Path, dir_name: &str, manifest_name: &str, manifest_json: &str, make_exe: bool) {
        let dir = parent.join(dir_name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.json"), manifest_json).unwrap();
        if make_exe {
            let exe = dir.join(manifest_name);
            fs::write(&exe, b"#!/bin/sh\n").unwrap();
            let mut perms = fs::metadata(&exe).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exe, perms).unwrap();
        }
    }

    const VALID_MANIFEST: &str = r#"{
        "name": "test-extension",
        "version": "1.0.0",
        "capabilities": ["classify"],
        "description": "Test extension"
    }"#;

    #[test]
    fn discovers_valid_extension() {
        let tmp = TempDir::new().unwrap();
        make_extension_dir(tmp.path(), "test-extension", "test-extension", VALID_MANIFEST, true);
        let extensions = discover_extensions(tmp.path());
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "test-extension");
        assert_eq!(extensions[0].version, "1.0.0");
        assert_eq!(extensions[0].capabilities, vec!["classify"]);
    }

    #[test]
    fn skips_missing_executable() {
        let tmp = TempDir::new().unwrap();
        make_extension_dir(tmp.path(), "test-extension", "test-extension", VALID_MANIFEST, false);
        assert!(discover_extensions(tmp.path()).is_empty());
    }

    #[test]
    fn skips_invalid_manifest() {
        let tmp = TempDir::new().unwrap();
        make_extension_dir(tmp.path(), "test-extension", "test-extension", "not json", true);
        assert!(discover_extensions(tmp.path()).is_empty());
    }

    #[test]
    fn skips_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("test-extension")).unwrap();
        assert!(discover_extensions(tmp.path()).is_empty());
    }

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(discover_extensions(tmp.path()).is_empty());
    }

    #[test]
    fn nonexistent_dir_returns_empty() {
        assert!(discover_extensions(Path::new("/nonexistent/path/xyz")).is_empty());
    }
}
