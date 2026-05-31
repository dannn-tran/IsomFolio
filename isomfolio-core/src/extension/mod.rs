//! Thin wrapper pinning `isomfolio-extension-host` to IsomFolio's app-specific
//! extensions root (`extensions_dir()`).
//!
//! Only discovery, installation, and config are used: the one shipped extension
//! (the face inference engine) runs as a managed HTTP process launched by the
//! app, not over an IEP stdin/stdout protocol.

use std::path::Path;

use crate::app_paths::extensions_dir;

pub use isomfolio_extension_host::{
    load_extension_config, save_extension_config, ConfigField, ConfigFieldKind, ExtensionManifest,
};

/// Discover installed extensions under the app's extensions root.
pub fn discover_extensions() -> Vec<ExtensionManifest> {
    isomfolio_extension_host::discover_extensions(&extensions_dir())
}

/// Install an `.isfx` package into the app's extensions root.
pub fn install_extension_package(package_path: &Path) -> Result<ExtensionManifest, String> {
    isomfolio_extension_host::install_extension_package(package_path, &extensions_dir())
}

/// Uninstall an extension by name from the app's extensions root.
pub fn uninstall_extension(name: &str) -> Result<(), String> {
    isomfolio_extension_host::uninstall_extension(&extensions_dir(), name)
}
