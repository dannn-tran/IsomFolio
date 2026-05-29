//! Thin wrapper that pins `isfx-host` to IsomFolio's app-specific path conventions
//! (`extensions_dir()` and `models_dir()`) and adapts errors to `AppError`.

use std::path::Path;
use std::sync::Arc;

use crate::app_paths::{extensions_dir, models_dir};
use crate::models::AppError;

pub use isfx_host::{
    load_extension_config, save_extension_config, BatchHandle, ConfigField, ConfigFieldKind,
    ExtensionCallHandle, ExtensionManifest,
};

/// Discover installed extensions under the app's extensions root.
pub fn discover_extensions() -> Vec<ExtensionManifest> {
    isfx_host::discover_extensions(&extensions_dir())
}

/// Install an `.isfx` package into the app's extensions root.
pub fn install_extension_package(package_path: &Path) -> Result<ExtensionManifest, String> {
    isfx_host::install_extension_package(package_path, &extensions_dir())
}

/// Uninstall an extension by name from the app's extensions root.
pub fn uninstall_extension(name: &str) -> Result<(), String> {
    isfx_host::uninstall_extension(&extensions_dir(), name)
}

/// App-side handle to a launched extension. Wraps `isfx_host::ExtensionProcess`
/// so launch defaults to the app's `models_dir` and errors map to `AppError`.
#[derive(Debug)]
pub struct ExtensionProcess(Arc<isfx_host::ExtensionProcess>);

impl ExtensionProcess {
    pub fn launch(manifest: ExtensionManifest) -> Result<Self, AppError> {
        isfx_host::ExtensionProcess::launch(manifest, Some(models_dir()))
            .map(|p| ExtensionProcess(Arc::new(p)))
            .map_err(map_err)
    }

    pub fn manifest(&self) -> &ExtensionManifest {
        &self.0.manifest
    }

    pub fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.0.call(method, params).map_err(map_err)
    }

    pub fn call_long(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        self.0.call_long(method, params).map_err(map_err)
    }

    pub fn send(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<ExtensionCallHandle, AppError> {
        self.0.send(method, params).map_err(map_err)
    }

    pub fn send_many(
        &self,
        requests: &[(&str, serde_json::Value)],
    ) -> Result<BatchHandle, AppError> {
        self.0.send_many(requests).map_err(map_err)
    }

    pub fn last_stderr(&self) -> Vec<String> {
        self.0.last_stderr()
    }
}

impl std::ops::Deref for ExtensionProcess {
    type Target = isfx_host::ExtensionProcess;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn map_err(e: isfx_host::Error) -> AppError {
    match e {
        isfx_host::Error::Extension(s) => AppError::Extension(s),
        isfx_host::Error::Install(s) => AppError::Extension(s),
    }
}
