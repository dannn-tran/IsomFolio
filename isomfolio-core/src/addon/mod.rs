pub mod config;
pub mod install;
pub mod manifest;
pub mod process;
pub mod protocol;

pub use config::{load_addon_config, save_addon_config};
pub use install::{install_addon_package, uninstall_addon};
pub use manifest::{discover_addons, AddonManifest, ConfigField, ConfigFieldKind};
pub use process::{AddonCallHandle, AddonProcess};
