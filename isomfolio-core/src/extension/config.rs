use std::path::{Path, PathBuf};

pub fn extension_config_path(extension_dir: &Path) -> PathBuf {
    extension_dir.join("config.json")
}

pub fn load_extension_config(extension_dir: &Path) -> serde_json::Value {
    let path = extension_config_path(extension_dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_extension_config(
    extension_dir: &Path,
    config: &serde_json::Value,
) -> Result<(), std::io::Error> {
    let path = extension_config_path(extension_dir);
    std::fs::write(path, serde_json::to_string_pretty(config).unwrap())
}
