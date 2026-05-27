use std::path::{Path, PathBuf};

pub fn addon_config_path(addon_dir: &Path) -> PathBuf {
    addon_dir.join("config.json")
}

pub fn load_addon_config(addon_dir: &Path) -> serde_json::Value {
    let path = addon_config_path(addon_dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_addon_config(
    addon_dir: &Path,
    config: &serde_json::Value,
) -> Result<(), std::io::Error> {
    let path = addon_config_path(addon_dir);
    std::fs::write(path, serde_json::to_string_pretty(config).unwrap())
}
