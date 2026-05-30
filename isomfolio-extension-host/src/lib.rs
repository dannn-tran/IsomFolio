//! Host-side library for embedding IsomFolio extensions.
//!
//! An IsomFolio extension is an external process that speaks newline-delimited JSON
//! over stdin/stdout. This crate handles discovery, installation from `.isfx`
//! packages, lifecycle management (handshake + ready), and request/response dispatch.
//!
//! The crate is path-agnostic: callers pass in the extensions root and per-launch
//! data directory. No global filesystem state.

pub mod config;
mod error;
pub mod install;
pub mod manifest;
pub mod process;
pub mod protocol;

pub use config::{load_extension_config, save_extension_config};
pub use error::Error;
pub use install::{install_extension_package, uninstall_extension};
pub use manifest::{discover_extensions, ConfigField, ConfigFieldKind, ExtensionManifest};
pub use process::{format_log_line, BatchHandle, ExtensionCallHandle, ExtensionProcess};
