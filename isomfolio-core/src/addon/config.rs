use std::path::PathBuf;

use crate::app_paths::app_data_root;

pub fn addon_config_path(addon_name: &str) -> PathBuf {
    app_data_root()
        .join("addon-settings")
        .join(format!("{}.json", addon_name))
}

pub fn load_addon_config(addon_name: &str) -> serde_json::Value {
    let path = addon_config_path(addon_name);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_addon_config(
    addon_name: &str,
    config: &serde_json::Value,
) -> Result<(), std::io::Error> {
    let path = addon_config_path(addon_name);
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_string_pretty(config).unwrap())
}
