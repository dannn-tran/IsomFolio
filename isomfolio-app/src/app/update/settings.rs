use iced::Task;
use isomfolio_core::extension::{
    discover_extensions, install_extension_package, load_extension_config, save_extension_config,
    uninstall_extension, ConfigFieldKind,
};

use super::super::{App, Msg, SettingsState, ViewMode};

impl App {
    pub(super) fn handle_settings(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::OpenSettings => {
                self.open_menu = None;
                if matches!(self.view_mode, ViewMode::Settings) {
                    self.view_mode = ViewMode::Browse;
                    return Task::none();
                }
                let mut extension_configs = std::collections::HashMap::new();
                if let Some(manifest) = self.inference_manifest.as_ref() {
                    if !manifest.config_schema.is_empty() {
                        let ext_dir =
                            manifest.executable.parent().unwrap_or(std::path::Path::new("."));
                        let stored = load_extension_config(ext_dir);
                        let mut fields = std::collections::HashMap::new();
                        for field in &manifest.config_schema {
                            let val = stored
                                .get(&field.key)
                                .and_then(|v| v.as_str())
                                .unwrap_or(field.default.as_deref().unwrap_or(""))
                                .to_string();
                            fields.insert(field.key.clone(), val);
                        }
                        extension_configs.insert(manifest.name.clone(), fields);
                    }
                }
                self.settings = SettingsState {
                    tab: self.settings.tab,
                    extension_configs,
                    install_error: None,
                    status: None,
                    install_task_id: None,
                };
                self.view_mode = ViewMode::Settings;
                Task::none()
            }

            Msg::SwitchSettingsTab(tab) => {
                self.settings.tab = tab;
                Task::none()
            }

            Msg::CloseSettings => {
                self.view_mode = ViewMode::Browse;
                Task::none()
            }

            Msg::SettingsConfigChanged { extension_name, key, value } => {
                let kind = self
                    .inference_manifest
                    .as_ref()
                    .filter(|m| m.name == extension_name)
                    .and_then(|m| m.config_schema.iter().find(|f| f.key == key))
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
                // The inference engine is the only configurable extension.
                let Some(manifest) = self.inference_manifest.clone() else {
                    return Task::none();
                };
                let Some(fields) = self.settings.extension_configs.get(&manifest.name) else {
                    return Task::none();
                };
                let config: serde_json::Value = fields
                    .iter()
                    .map(|(k, v)| {
                        let kind = manifest.config_schema.iter().find(|f| &f.key == k).map(|f| &f.kind);
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
                let ext_dir = manifest
                    .executable
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                if let Err(e) = save_extension_config(&ext_dir, &config) {
                    self.status = format!("Settings save failed: {e}");
                    return Task::none();
                }
                // New config (e.g. model size) takes effect on the next run.
                self.inference = None;
                self.settings.status = Some("Settings saved".to_string());
                Task::none()
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
                self.settings.status = None;
                let task_id = self.bg_push("Installing extension…");
                self.settings.install_task_id = Some(task_id);
                Task::perform(
                    async move {
                        let path = std::path::PathBuf::from(path);
                        match install_extension_package(&path) {
                            Err(e) => Err(e),
                            // The engine is launched on demand (HTTP). Re-discover
                            // to get the executable-populated manifest.
                            Ok(_) => discover_extensions()
                                .into_iter()
                                .find(|d| d.capabilities.iter().any(|c| c == "inference_engine"))
                                .ok_or_else(|| "installed package is not an inference engine".to_string()),
                        }
                    },
                    |outcome| match outcome {
                        Ok(m) => Msg::EngineInstalled(m),
                        Err(e) => Msg::ExtensionInstallFailed(e),
                    },
                )
            }

            Msg::EngineInstalled(manifest) => {
                let name = manifest.name.clone();
                self.inference_manifest = Some(manifest);
                // A fresh engine binary may differ from a running one; drop any
                // managed client so the next run spawns the new version.
                self.inference = None;
                if let Some(id) = self.settings.install_task_id.take() {
                    self.bg_complete(id);
                }
                self.status = format!("'{name}' installed");
                Task::none()
            }

            Msg::ExtensionInstallFailed(e) => {
                if let Some(id) = self.settings.install_task_id.take() {
                    self.bg_fail(id, e.clone());
                }
                self.settings.install_error = Some(e);
                Task::none()
            }

            Msg::UninstallExtension(name) => {
                // The engine is the only installable extension; drop its manifest
                // and any managed client so it stops.
                if self.inference_manifest.as_ref().is_some_and(|m| m.name == name) {
                    self.inference_manifest = None;
                    self.inference = None;
                }
                if let Err(e) = uninstall_extension(&name) {
                    self.settings.status = Some(format!("Uninstall failed: {e}"));
                } else {
                    self.settings.status = Some(format!("'{name}' removed"));
                }
                Task::none()
            }

            Msg::ToggleAutoFaceCluster => {
                self.app_settings.auto_face_cluster = !self.app_settings.auto_face_cluster;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleImportXmpTags => {
                let next = !self.app_settings.import_xmp_tags.unwrap_or(true);
                self.app_settings.import_xmp_tags = Some(next);
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleImportAppleTags => {
                let next = !self.app_settings.import_apple_tags.unwrap_or(true);
                self.app_settings.import_apple_tags = Some(next);
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleAutoAdvanceOnFlag => {
                self.app_settings.auto_advance_on_flag = !self.app_settings.auto_advance_on_flag;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleInferenceCustom => {
                self.app_settings.inference_custom_url =
                    match self.app_settings.inference_custom_url {
                        Some(_) => None,
                        None => Some(String::new()),
                    };
                // A mode change invalidates any running managed/remote client.
                self.inference = None;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::InferenceUrlChanged(url) => {
                self.app_settings.inference_custom_url = Some(url);
                self.inference = None;
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::InferencePortChanged(s) => {
                if let Ok(port) = s.trim().parse::<u16>() {
                    self.app_settings.inference_port = port;
                    self.inference = None;
                    isomfolio_core::app_paths::save_settings(&self.app_settings);
                }
                Task::none()
            }

            Msg::FaceEpsChanged(s) => {
                if let Ok(eps) = s.trim().parse::<f32>() {
                    if (0.05..=2.0).contains(&eps) {
                        self.app_settings.face_eps = eps;
                        isomfolio_core::app_paths::save_settings(&self.app_settings);
                    }
                }
                Task::none()
            }

            Msg::FaceMinPtsChanged(s) => {
                if let Ok(n) = s.trim().parse::<u32>() {
                    if n >= 1 {
                        self.app_settings.face_min_pts = n;
                        isomfolio_core::app_paths::save_settings(&self.app_settings);
                    }
                }
                Task::none()
            }

            _ => Task::none(),
        }
    }
}
