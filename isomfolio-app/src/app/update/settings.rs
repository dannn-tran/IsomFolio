use std::sync::Arc;

use iced::Task;
use isomfolio_core::extension::{
    discover_extensions, install_extension_package, load_extension_config, save_extension_config,
    uninstall_extension, ConfigFieldKind, ExtensionProcess,
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
                            async move { ExtensionProcess::launch(manifest, None).map(Arc::new).ok() },
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
                self.settings.status = None;
                let task_id = self.bg_push("Installing extension…");
                self.settings.install_task_id = Some(task_id);
                Task::perform(
                    async move {
                        let path = std::path::PathBuf::from(path);
                        match install_extension_package(&path) {
                            Err(e) => InstallOutcome::Failed(e),
                            // The engine is launched on demand, not as an IEP process.
                            // Re-discover to get the executable-populated manifest.
                            Ok(m) if m.capabilities.iter().any(|c| c == "inference_engine") => {
                                match discover_extensions()
                                    .into_iter()
                                    .find(|d| d.capabilities.iter().any(|c| c == "inference_engine"))
                                {
                                    Some(engine) => InstallOutcome::Engine(engine),
                                    None => InstallOutcome::Failed(
                                        "engine installed but manifest not found".to_string(),
                                    ),
                                }
                            }
                            Ok(m) => match ExtensionProcess::launch(m, None).map(Arc::new) {
                                Ok(p) => InstallOutcome::Process(p),
                                Err(e) => InstallOutcome::Failed(e.to_string()),
                            },
                        }
                    },
                    |outcome| match outcome {
                        InstallOutcome::Process(p) => Msg::ExtensionInstalled(p),
                        InstallOutcome::Engine(m) => Msg::EngineInstalled(m),
                        InstallOutcome::Failed(e) => Msg::ExtensionInstallFailed(e),
                    },
                )
            }

            Msg::ExtensionInstalled(process) => {
                let name = process.manifest.name.clone();
                self.extensions.push(process);
                if let Some(id) = self.settings.install_task_id.take() {
                    self.bg_complete(id);
                }
                self.status = format!("'{name}' installed");
                Task::none()
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
                if let Some(idx) = self.extensions.iter().position(|a| a.manifest.name == name) {
                    self.extensions.remove(idx);
                }
                // The engine isn't in self.extensions — clear it explicitly and
                // drop any managed client so it stops.
                if self.inference_manifest.as_ref().is_some_and(|m| m.name == name) {
                    self.inference_manifest = None;
                    self.inference = None;
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
                let next = !self.app_settings.import_xmp_tags.unwrap_or(false);
                self.app_settings.import_xmp_tags = Some(next);
                isomfolio_core::app_paths::save_settings(&self.app_settings);
                Task::none()
            }

            Msg::ToggleImportAppleTags => {
                let next = !self.app_settings.import_apple_tags.unwrap_or(false);
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

/// Outcome of installing an `.isfx` package: a launched IEP process, the
/// inference engine (launched on demand), or a failure.
enum InstallOutcome {
    Process(Arc<ExtensionProcess>),
    Engine(isomfolio_core::extension::ExtensionManifest),
    Failed(String),
}
