use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::manifest::{discover_extensions, ExtensionManifest};

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

    discover_extensions(extensions_root)
        .into_iter()
        .find(|m| m.name == manifest.name)
        .ok_or_else(|| {
            format!(
                "installed '{}' but executable not found — check the binary name matches the extension name",
                manifest.name
            )
        })
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
    use super::safe_relative_path;
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
