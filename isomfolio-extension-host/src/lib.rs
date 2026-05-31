//! Host-side library for IsomFolio extensions: discovery, installation from
//! `.isfx` packages, and per-extension config.
//!
//! The crate is path-agnostic: callers pass in the extensions root and
//! per-launch data directory. No global filesystem state.

pub mod config;
pub mod install;
pub mod manifest;

pub use config::{load_extension_config, save_extension_config};
pub use install::{install_extension_package, uninstall_extension};
pub use manifest::{discover_extensions, ConfigField, ConfigFieldKind, ExtensionManifest};
