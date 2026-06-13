use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Space},
    Alignment, Background, Element, Length, Theme,
};

use super::styles;
use crate::app::{App, Msg, SettingsTab};
use styles::{
    active_chip_style, danger_btn_style, ghost_btn_style, ACCENT, BG_GRID, BORDER, ERR, FG, FG_DIM,
    FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, SPACE_6, TEXT_BASE,
    TEXT_LG, TEXT_MD, TEXT_SM, TEXT_TITLE,
};

impl App {
    pub(super) fn view_settings_pane(&self) -> Element<'_, Msg> {
        let header = row![
            text("Settings").size(TEXT_TITLE).color(FG),
            Space::new().width(Length::Fill),
            self.settings_tab_chip("General", SettingsTab::General),
            self.settings_tab_chip("Extensions", SettingsTab::Extensions),
            styles::icon_btn("✕", Msg::CloseSettings),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_1_5)
        .width(Length::Fill);

        let content: Element<'_, Msg> = match self.settings.tab {
            SettingsTab::General => self.settings_general_pane(),
            SettingsTab::Extensions => self.settings_extensions_pane(),
        };

        let scroll_area = scrollable(
            container(content)
                .padding([SPACE_1, SPACE_1])
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(6).scroller_width(6),
        ));

        let footer_status = match (
            self.settings.install_error.as_deref(),
            self.settings.status.as_deref(),
        ) {
            (Some(err), _) => Some((err.to_string(), ERR)),
            (_, Some(s)) => Some((s.to_string(), FG_DIM)),
            _ => None,
        };

        let footer = row![
            {
                if let Some((msg, color)) = footer_status {
                    text(msg).size(TEXT_SM).color(color)
                } else {
                    text("").size(TEXT_SM)
                }
            },
            Space::new().width(Length::Fill),
            button(text("Save").size(TEXT_BASE))
                .on_press(Msg::SaveSettings)
                .style(active_chip_style),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        let body = column![
            header,
            Space::new().height(SPACE_3),
            container(Space::new())
                .width(Length::Fill)
                .height(1.0)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(BORDER)),
                    ..Default::default()
                }),
            Space::new().height(SPACE_2),
            scroll_area,
            Space::new().height(SPACE_3),
            container(Space::new())
                .width(Length::Fill)
                .height(1.0)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(BORDER)),
                    ..Default::default()
                }),
            Space::new().height(SPACE_2_5),
            footer,
        ]
        .spacing(0)
        .width(Length::Fill);

        container(body)
            .padding(SPACE_6)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }

    fn settings_tab_chip(&self, label: &str, tab: SettingsTab) -> Element<'_, Msg> {
        let selected = self.settings.tab == tab;
        button(text(label.to_string()).size(TEXT_MD))
            .on_press(Msg::SwitchSettingsTab(tab))
            .style(move |t: &Theme, s| {
                if selected { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
            })
            .into()
    }

    fn settings_general_pane(&self) -> Element<'_, Msg> {
        let mut col = column![].spacing(SPACE_3).width(Length::Fill);

        col = col.push(self.toggle_row(
            "Auto-advance after culling",
            "Move to the next photo automatically after a flag (P/X/U), rating (1–5), or colour label (6–9) in loupe.",
            self.app_settings.auto_advance_on_cull,
            Msg::ToggleAutoAdvanceOnCull,
        ));
        col = col.push(self.toggle_row(
            "Auto-detect people",
            "Run after each sync that finds new photos.",
            self.app_settings.auto_face_cluster,
            Msg::ToggleAutoFaceCluster,
        ));
        col = col.push(self.toggle_row(
            "Import XMP keywords",
            "Copy dc:subject keywords into new photos as tags. Applies going forward — turning this off never removes tags already imported.",
            self.app_settings.import_xmp_tags.unwrap_or(true),
            Msg::ToggleImportXmpTags,
        ));
        if cfg!(target_os = "macos") {
            col = col.push(self.toggle_row(
                "Import Apple Finder tags",
                "Copy macOS Finder tags (kMDItemUserTags) into new photos as tags. Applies going forward — turning this off never removes tags already imported.",
                self.app_settings.import_apple_tags.unwrap_or(true),
                Msg::ToggleImportAppleTags,
            ));
        }

        col = col.push(Space::new().height(SPACE_3));
        col = col.push(self.inference_engine_section());

        col.into()
    }

    /// Inference-engine settings: Auto (managed local) vs a custom URL, the
    /// managed port, and the people-clustering knobs (eps / min faces).
    fn inference_engine_section(&self) -> Element<'_, Msg> {
        let custom = self.app_settings.inference_custom_url.is_some();

        let header = column![
            text("Face inference engine").size(TEXT_BASE).color(FG),
            text("Where face detection runs. Auto manages a local engine; Custom URL points at a self-hosted one.")
                .size(TEXT_SM)
                .color(FG_DIM),
        ]
        .spacing(SPACE_0_5);

        let mode_chips = row![
            button(text("Auto").size(TEXT_MD))
                .on_press_maybe(custom.then_some(Msg::ToggleInferenceCustom))
                .style(if custom { ghost_btn_style } else { active_chip_style }),
            button(text("Custom URL").size(TEXT_MD))
                .on_press_maybe((!custom).then_some(Msg::ToggleInferenceCustom))
                .style(if custom { active_chip_style } else { ghost_btn_style }),
        ]
        .spacing(SPACE_1_5);

        let mut col = column![header, mode_chips].spacing(SPACE_2).width(Length::Fill);

        if custom {
            let url = self.app_settings.inference_custom_url.clone().unwrap_or_default();
            col = col.push(
                text_input("http://127.0.0.1:45876", &url)
                    .on_input(Msg::InferenceUrlChanged)
                    .size(TEXT_MD)
                    .padding(SPACE_1_5),
            );
        } else {
            col = col.push(self.labeled_input(
                "Port",
                &self.app_settings.inference_port.to_string(),
                Msg::InferencePortChanged,
            ));
        }

        col = col.push(self.labeled_input(
            "Sensitivity (lower = stricter, 0.05–2.0)",
            &format!("{:.2}", self.app_settings.face_eps),
            Msg::FaceEpsChanged,
        ));
        col = col.push(self.labeled_input(
            "Min faces per person",
            &self.app_settings.face_min_pts.to_string(),
            Msg::FaceMinPtsChanged,
        ));

        col.into()
    }

    fn labeled_input<'a>(
        &self,
        label: &str,
        value: &str,
        on_input: impl Fn(String) -> Msg + 'a,
    ) -> Element<'a, Msg> {
        row![
            text(label.to_string()).size(TEXT_SM).color(FG_DIM).width(Length::FillPortion(2)),
            text_input("", value)
                .on_input(on_input)
                .size(TEXT_MD)
                .padding(SPACE_1)
                .width(Length::FillPortion(1)),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_2)
        .into()
    }

    fn toggle_row<'a>(
        &self,
        label: &str,
        help: &str,
        on: bool,
        msg: Msg,
    ) -> Element<'a, Msg> {
        let glyph = if on { "●" } else { "○" };
        let tint = if on { ACCENT } else { FG_DIM };
        row![
            mouse_area(
                row![
                    text(glyph.to_string()).size(TEXT_LG).color(tint),
                    column![
                        text(label.to_string()).size(TEXT_BASE).color(FG),
                        text(help.to_string()).size(TEXT_SM).color(FG_DIM),
                    ]
                    .spacing(SPACE_0_5),
                ]
                .spacing(SPACE_2)
                .align_y(Alignment::Center),
            )
            .on_press(msg)
            .interaction(iced::mouse::Interaction::Pointer),
        ]
        .into()
    }

    fn settings_extensions_pane(&self) -> Element<'_, Msg> {
        let install_btn = row![
            button(text("Install from file…").size(TEXT_BASE))
                .on_press(Msg::InstallExtensionPickFile)
                .style(ghost_btn_style),
            Space::new().width(Length::Fill),
            text(format!(
                "{} installed",
                self.inference_manifest.is_some() as usize
            ))
            .size(TEXT_SM)
            .color(FG_DIM),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_2);

        let mut body = column![
            install_btn,
            Space::new().height(SPACE_3),
        ]
        .spacing(0)
        .width(Length::Fill);

        if self.inference_manifest.is_none() {
            body = body.push(
                container(
                    text("No extensions installed. Click \"Install from file…\" above to add one.")
                        .size(TEXT_MD)
                        .color(FG_MUTED),
                )
                .padding(SPACE_3),
            );
        }

        // The inference engine isn't an IEP process (not in self.extensions), so
        // render it from its manifest: identity, model_variant config, Remove.
        if let Some(engine) = &self.inference_manifest {
            let name = engine.name.clone();
            body = body.push(
                row![
                    column![
                        row![
                            text(name.clone()).size(TEXT_BASE).color(FG),
                            text("inference engine").size(TEXT_SM).color(ACCENT),
                        ]
                        .spacing(SPACE_1_5),
                        text(engine.description.clone()).size(TEXT_SM).color(FG_DIM),
                    ]
                    .spacing(SPACE_0_5),
                    Space::new().width(Length::Fill),
                    button(text("Remove").size(TEXT_SM))
                        .on_press(Msg::UninstallExtension(name.clone()))
                        .style(danger_btn_style),
                ]
                .align_y(Alignment::Center)
                .spacing(SPACE_2),
            );

            let empty_map = std::collections::HashMap::new();
            let field_values = self.settings.extension_configs.get(&name).unwrap_or(&empty_map);
            for field in &engine.config_schema {
                use isomfolio_core::extension::ConfigFieldKind;
                if !matches!(field.kind, ConfigFieldKind::Select) {
                    continue;
                }
                let current = field_values.get(&field.key).cloned()
                    .or_else(|| field.default.clone())
                    .unwrap_or_default();
                body = body.push(Space::new().height(SPACE_2));
                body = body.push(text(&field.label).size(TEXT_MD).color(FG_DIM));
                body = body.push(Space::new().height(SPACE_1_5));
                let mut option_row = row![].spacing(SPACE_1_5);
                for opt in &field.options {
                    let selected = current == *opt;
                    let opt_val = opt.clone();
                    let k = field.key.clone();
                    let an = name.clone();
                    option_row = option_row.push(
                        button(text(opt.as_str()).size(TEXT_MD))
                            .on_press(Msg::SettingsConfigChanged {
                                extension_name: an,
                                key: k,
                                value: opt_val,
                            })
                            .style(move |t: &Theme, st| {
                                if selected { active_chip_style(t, st) } else { ghost_btn_style(t, st) }
                            }),
                    );
                }
                body = body.push(option_row);
            }
            body = body.push(Space::new().height(SPACE_3));
        }

        body.into()
    }
}
