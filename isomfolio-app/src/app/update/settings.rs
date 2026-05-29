use std::sync::Arc;

use iced::Task;
use isomfolio_core::extension::{
    install_extension_package, load_extension_config, save_extension_config, uninstall_extension,
    ConfigFieldKind, ExtensionProcess,
};

use super::super::{App, Msg, SettingsState};

impl App {
    pub(super) fn handle_settings(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenSettings => {
                let mut extension_configs = std::collections::HashMap::new();
                for ext in &self.extensions {
                    if ext.manifest.config_schema.is_empty() {
                        continue;
                    }
                    let ext_dir =
                        ext.manifest.executable.parent().unwrap_or(std::path::Path::new("."));
                    let stored = load_extension_config(ext_dir);
                    let mut fields = std::collections::HashMap::new();
                    for field in &ext.manifest.config_schema {
                        let val = stored
                            .get(&field.key)
                            .and_then(|v| v.as_str())
                            .unwrap_or(field.default.as_deref().unwrap_or(""))
                            .to_string();
                        fields.insert(field.key.clone(), val);
                    }
                    extension_configs.insert(ext.manifest.name.clone(), fields);
                }
                self.settings =
                    SettingsState { show: true, extension_configs, install_error: None, status: None };
                Task::none()
            }

            Msg::CloseSettings => {
                self.settings.show = false;
                Task::none()
            }

            Msg::SettingsConfigChanged { extension_name, key, value } => {
                let kind = self
                    .extensions
                    .iter()
                    .find(|a| a.manifest.name == extension_name)
                    .and_then(|a| a.manifest.config_schema.iter().find(|f| f.key == key))
                    .map(|f| &f.kind);
                let valid = match kind {
                    Some(ConfigFieldKind::Number) => {
                        value.is_empty()
                            || value == "."
                            || value == "-"
                            || value.parse::<f64>().is_ok()
                    }
                    Some(ConfigFieldKind::Integer) => {
                        value.is_empty() || value == "-" || value.parse::<i64>().is_ok()
                    }
                    _ => true,
                };
                if valid {
                    self.settings
                        .extension_configs
                        .entry(extension_name)
                        .or_default()
                        .insert(key, value);
                }
                Task::none()
            }

            Msg::SaveSettings => {
                self.settings.show = false;
                let mut restart_tasks = Vec::new();
                for (extension_name, fields) in &self.settings.extension_configs {
                    let schema = self
                        .extensions
                        .iter()
                        .find(|a| &a.manifest.name == extension_name)
                        .map(|a| &a.manifest.config_schema);
                    let config: serde_json::Value = fields
                        .iter()
                        .map(|(k, v)| {
                            let kind = schema
                                .and_then(|s| s.iter().find(|f| &f.key == k))
                                .map(|f| &f.kind);
                            let val = match kind {
                                Some(ConfigFieldKind::Number) => v
                                    .parse::<f64>()
                                    .map(serde_json::Value::from)
                                    .unwrap_or_else(|_| serde_json::Value::String(v.clone())),
                                Some(ConfigFieldKind::Integer) => v
                                    .parse::<i64>()
                                    .map(serde_json::Value::from)
                                    .unwrap_or_else(|_| serde_json::Value::String(v.clone())),
                                _ => serde_json::Value::String(v.clone()),
                            };
                            (k.clone(), val)
                        })
                        .collect::<serde_json::Map<_, _>>()
                        .into();
                    let ext_dir = self
                        .extensions
                        .iter()
                        .find(|a| &a.manifest.name == extension_name)
                        .and_then(|a| a.manifest.executable.parent().map(|p| p.to_path_buf()))
                        .unwrap_or_default();
                    if let Err(e) = save_extension_config(&ext_dir, &config) {
                        self.status = format!("Settings save failed: {e}");
                        return Task::none();
                    }
                    let idx = self
                        .extensions
                        .iter()
                        .position(|a| &a.manifest.name == extension_name);
                    if let Some(idx) = idx {
                        let manifest = self.extensions[idx].manifest.clone();
                        restart_tasks.push(Task::perform(
                            async move { ExtensionProcess::launch(manifest).map(Arc::new).ok() },
                            move |p| Msg::ExtensionRestarted { idx, process: p },
                        ));
                    }
                }
                if restart_tasks.is_empty() {
                    Task::none()
                } else {
                    self.settings.status = Some("Saving & restarting extensions…".to_string());
                    Task::batch(restart_tasks)
                }
            }

            Msg::InstallExtensionPickFile => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("IsomFolio Extension", &["isfx"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_string_lossy().into_owned())
                },
                Msg::ExtensionPackagePicked,
            ),

            Msg::ExtensionPackagePicked(None) => Task::none(),

            Msg::ExtensionPackagePicked(Some(path)) => {
                self.settings.install_error = None;
                self.settings.status = Some("Installing extension…".to_string());
                Task::perform(
                    async move {
                        let path = std::path::PathBuf::from(path);
                        install_extension_package(&path).and_then(|m| {
                            ExtensionProcess::launch(m).map(Arc::new).map_err(|e| e.to_string())
                        })
                    },
                    |result| match result {
                        Ok(p) => Msg::ExtensionInstalled(p),
                        Err(e) => Msg::ExtensionInstallFailed(e),
                    },
                )
            }

            Msg::ExtensionInstalled(process) => {
                self.settings.status = Some(format!("'{}' installed", process.manifest.name));
                self.settings.install_error = None;
                self.extensions.push(process);
                Task::none()
            }

            Msg::ExtensionInstallFailed(e) => {
                self.settings.install_error = Some(e);
                Task::none()
            }

            Msg::UninstallExtension(name) => {
                if let Some(idx) = self.extensions.iter().position(|a| a.manifest.name == name) {
                    self.extensions.remove(idx);
                }
                self.app_settings.preferred_extension.retain(|_, v| v != &name);
                if let Err(e) = uninstall_extension(&name) {
                    self.settings.status = Some(format!("Uninstall failed: {e}"));
                } else {
                    self.settings.status = Some(format!("'{name}' removed"));
                }
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::SetPreferredExtension { capability, extension_name } => {
                self.app_settings.preferred_extension.insert(capability, extension_name);
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleAutoFaceCluster => {
                self.app_settings.auto_face_cluster = !self.app_settings.auto_face_cluster;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleImportXmpTags => {
                self.app_settings.import_xmp_tags = !self.app_settings.import_xmp_tags;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            _ => Task::none(),
        }
    }
}
