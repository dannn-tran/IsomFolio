use std::fs;
use std::io::{self, Read};
use std::path::Path;

use zip::ZipArchive;

use crate::app_paths::addons_dir;

use super::manifest::{AddonManifest, discover_addons};

/// Install an `.isfx` zip package into `addons_dir()`.
///
/// Expected zip layout (flat, no subdirectory):
/// ```text
/// isomfolio-addon.json
/// <addon-name>          <- executable (same name as the "name" field in manifest)
/// ```
pub fn install_addon_package(package_path: &Path) -> Result<AddonManifest, String> {
    let file = fs::File::open(package_path).map_err(|e| format!("open package: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("read zip: {e}"))?;

    let manifest: AddonManifest = {
        let mut entry = archive
            .by_name("isomfolio-addon.json")
            .map_err(|_| "package missing isomfolio-addon.json".to_string())?;
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
    discover_addons(&addons_dir())
        .into_iter()
        .find(|m| m.name == manifest.name)
        .ok_or_else(|| {
            format!(
                "installed '{}' but executable not found — check the binary name matches the addon name",
                manifest.name
            )
        })
}

/// Remove an installed addon by name. Leaves model weights untouched.
pub fn uninstall_addon(name: &str) -> Result<(), String> {
    let addon_dir = addons_dir().join(name);
    if addon_dir.exists() {
        fs::remove_dir_all(&addon_dir).map_err(|e| format!("remove addon dir: {e}"))?;
    }
    Ok(())
}
