mod compare;
mod content;
mod context_menu;
mod menu_bar;
mod people;
mod sidebar;
pub mod styles;
mod tag_browser;
mod welcome;

use iced::{
    widget::{button, column, container, image, mouse_area, row, scrollable, stack, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Shadow, Theme, Vector,
};

use crate::app::{App, Msg, SettingsTab, SidebarItem, ViewMode, SIDEBAR_HANDLE_WIDTH};
use styles::{
    active_chip_style, danger_btn_style, ghost_btn_style, ACCENT, BG_MODAL,
    BG_PANEL, BG_PROGRESS_TRACK, BG_STATUSBAR, BORDER, ERR, FG, FG_DIM, FG_MUTED,
    OVERLAY_HEAVY, OVERLAY_LIGHT, OVERLAY_MEDIUM, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2,
    SPACE_2_5, SPACE_3, SPACE_4, SPACE_6, STAR_GOLD, TEXT_BASE, TEXT_LG, TEXT_MD, TEXT_SM,
    TEXT_STAR, TEXT_TITLE,
};

impl App {
    pub fn view(&self) -> Element<'_, Msg> {
        if self.welcome.show {
            return self.view_welcome();
        }

        if matches!(self.view_mode, ViewMode::Loupe) {
            return self.view_loupe();
        }

        if matches!(self.view_mode, ViewMode::Compare) {
            return self.view_compare();
        }

        let dragging = self.drag.state.as_ref().map(|d| d.active).unwrap_or(false);
        let drag_hover = self.drag.hover_album.clone();
        let status = if dragging {
            let count = self.drag.ids.len();
            match &drag_hover {
                Some(id) => {
                    let name = self
                        .albums
                        .iter()
                        .find(|a| &a.id == id)
                        .map(|a| a.name.as_str())
                        .unwrap_or("?");
                    format!("Dragging {count} — drop on \"{name}\"")
                }
                None => format!("Dragging {count} photo(s)…"),
            }
        } else if !self.status.is_empty() {
            let missing = self.missing_count();
            if missing > 0 {
                format!("{} · {} missing", self.status, missing)
            } else {
                self.status.clone()
            }
        } else {
            let base = match self.grid_selected.len() {
                0 => "Click to select".to_string(),
                1 => "Space for loupe · I for info · ? for shortcuts".to_string(),
                2 => "2 photos selected · c to compare · drag to album".to_string(),
                n => format!("{n} photos selected · drag to album"),
            };
            let missing = self.missing_count();
            if missing > 0 {
                format!("{base} · {} missing", missing)
            } else {
                base
            }
        };

        let remove_btn: Option<Element<Msg>> = if matches!(
            self.selected_item,
            SidebarItem::Album(_)
        ) && !self.grid_selected.is_empty()
        {
            let n = self.grid_selected.len();
            if self.remove_from_album_pending {
                Some(
                    row![
                        text(format!("Remove {n} from album?"))
                            .size(TEXT_MD)
                            .color(ERR),
                        button(text("Cancel").size(TEXT_MD))
                            .on_press(Msg::CancelRemoveFromAlbum)
                            .style(ghost_btn_style),
                        button(text("Remove").size(TEXT_MD))
                            .on_press(Msg::ConfirmRemoveFromAlbum)
                            .style(danger_btn_style),
                    ]
                    .spacing(SPACE_1_5)
                    .align_y(Alignment::Center)
                    .into(),
                )
            } else {
                Some(
                    button(text(format!("Remove {n}")).size(TEXT_MD))
                        .on_press(Msg::RemoveFromAlbum)
                        .style(ghost_btn_style)
                        .into(),
                )
            }
        } else {
            None
        };

        let mut status_row = row![
            text(status).size(TEXT_MD).color(FG),
            Space::new().width(Length::Fill),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        if let Some(btn) = remove_btn {
            status_row = status_row.push(btn);
        }

        let status_bar = container(status_row)
            .padding([SPACE_1, SPACE_3])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                ..Default::default()
            });

        let resizing = self.sidebar_resizing;
        let resize_handle: Element<Msg> = mouse_area(
            container(Space::new())
                .width(SIDEBAR_HANDLE_WIDTH)
                .height(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: if resizing { 0.9 } else { 0.15 },
                        ..BORDER
                    })),
                    ..Default::default()
                }),
        )
        .on_press(Msg::SidebarResizeStart)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into();

        let content_area: Element<Msg> = match self.view_mode {
            ViewMode::People => self.view_people_grid(),
            ViewMode::Preview => self.view_preview(),
            ViewMode::Compare => self.view_compare(),
            _ => self.view_grid(),
        };
        let mut main_row = row![self.view_sidebar(), resize_handle, content_area]
            .height(Length::Fill);
        if self.detail.show && matches!(self.view_mode, ViewMode::Browse | ViewMode::Preview) {
            main_row = main_row.push(self.view_detail());
        }

        let base: Element<Msg> = column![self.view_menu_bar(), main_row, status_bar].into();

        let mut layers: Vec<Element<Msg>> = vec![base];
        if let Some(dd) = self.view_menu_dropdown() {
            layers.push(dd);
        }
        if let Some(cm) = self.view_context_menu() {
            layers.push(cm);
        }
        if let Some(tb) = self.view_tag_browser() {
            layers.push(tb);
        }
        if self.thumb_ctx.total > 0 {
            layers.push(self.view_thumbnail_progress_panel());
        }
        if self.settings.show {
            layers.push(self.view_settings_modal());
        }
        if self.show_shortcut_help {
            layers.push(self.view_shortcut_help());
        }
        if self.metadata_import_prompt.is_some() {
            layers.push(self.view_metadata_import_prompt());
        }
        let root: Element<Msg> = if layers.len() == 1 {
            layers.remove(0)
        } else {
            stack(layers).into()
        };

        if resizing {
            mouse_area(root)
                .interaction(iced::mouse::Interaction::ResizingHorizontally)
                .into()
        } else {
            root
        }
    }

    fn view_thumbnail_progress_panel(&self) -> Element<'_, Msg> {
        let total = self.thumb_ctx.total;
        let done = total.saturating_sub(self.thumb_ctx.pending);
        let ratio = done as f32 / total.max(1) as f32;

        let eta_str = if done >= 3 {
            if let Some(start) = self.thumb_ctx.start_at {
                let elapsed = start.elapsed().as_secs_f64();
                let rate = done as f64 / elapsed;
                let secs_left = (self.thumb_ctx.pending as f64 / rate).ceil() as u64;
                if secs_left < 60 {
                    format!("~{secs_left}s")
                } else {
                    format!("~{}m{}s", secs_left / 60, secs_left % 60)
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let filled = ((ratio * 1000.0) as u16).max(1);
        let empty = (1000u16).saturating_sub(filled).max(1);
        let progress_track = row![
            container(Space::new())
                .width(Length::FillPortion(filled))
                .height(3)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(ACCENT)),
                    border: Border { radius: 1.5.into(), ..Default::default() },
                    ..Default::default()
                }),
            container(Space::new())
                .width(Length::FillPortion(empty))
                .height(3)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(BG_PROGRESS_TRACK)),
                    border: Border { radius: 1.5.into(), ..Default::default() },
                    ..Default::default()
                }),
        ]
        .width(Length::Fill);

        let count_row = row![
            text("Thumbnails").size(TEXT_SM).color(FG_DIM),
            Space::new().width(Length::Fill),
            text(format!("{done}/{total}")).size(TEXT_SM).color(FG_DIM),
        ]
        .align_y(Alignment::Center);

        let mut col = column![count_row, Space::new().height(SPACE_0_5), progress_track]
            .spacing(0);

        if !eta_str.is_empty() {
            col = col.push(
                container(text(eta_str).size(TEXT_SM).color(FG_DIM))
                    .align_x(Alignment::End)
                    .width(Length::Fill),
            );
        }

        let panel = container(col.spacing(SPACE_0_5))
            .width(220)
            .padding([SPACE_1_5, SPACE_2])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PANEL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow {
                    color: OVERLAY_LIGHT,
                    offset: Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            });

        container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .padding(iced::Padding { top: 0.0, right: SPACE_3, bottom: 38.0, left: 0.0 })
            .into()
    }

    fn view_preview(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe.idx.min(total.saturating_sub(1));

        let img_handle: Option<image::Handle> = self.loupe.full_res.as_ref()
            .and_then(|(full_idx, handle)| if *full_idx == idx { Some(handle.clone()) } else { None });

        let preview_img: Element<Msg> = match img_handle {
            Some(handle) => image(handle)
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => {
                let thumb = self.files.get(idx)
                    .and_then(|f| self.thumb_ctx.handles.get(&f.id).cloned());
                match thumb {
                    Some(handle) => image(handle)
                        .content_fit(iced::ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    None => container(text("Loading…").size(TEXT_MD).color(FG_DIM))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .into(),
                }
            }
        };

        let preview_area = container(preview_img)
            .width(Length::Fill)
            .height(Length::FillPortion(3))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 })),
                ..Default::default()
            });

        let grid_strip = container(self.view_grid())
            .height(Length::FillPortion(2));

        column![preview_area, grid_strip]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_loupe(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe.idx.min(total.saturating_sub(1));

        let img_handle: Option<image::Handle> = if let Some((full_idx, handle)) = &self.loupe.full_res {
            if *full_idx == idx {
                Some(handle.clone())
            } else {
                None
            }
        } else {
            None
        };

        let img_element: Element<Msg> = match img_handle {
            Some(handle) => image(handle)
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => {
                if let Some(file) = self.files.get(idx) {
                    let thumb = self
                        .thumbnails
                        .get(&file.id)
                        .and_then(|s| {
                            if let isomfolio_core::models::ThumbnailState::Ready(p) = s {
                                Some(p.clone())
                            } else {
                                None
                            }
                        });
                    match thumb {
                        Some(path) => image(image::Handle::from_path(path))
                            .content_fit(iced::ContentFit::Contain)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .into(),
                        None => Space::new().width(Length::Fill).height(Length::Fill).into(),
                    }
                } else {
                    Space::new().width(Length::Fill).height(Length::Fill).into()
                }
            }
        };

        let filename = self.files.get(idx).map(|f| f.name.as_str()).unwrap_or("");
        let wrap_hint = if total > 1 && (idx == 0 || idx == total - 1) {
            " ↩"
        } else {
            ""
        };
        let counter = if total > 0 {
            format!("{} / {}{}", idx + 1, total, wrap_hint)
        } else {
            String::new()
        };

        let top_bar = container(
            row![
                button(text("✕").size(TEXT_LG).color(FG))
                    .on_press(Msg::OpenLoupe)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 0.1
                        })),
                        text_color: FG,
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                Space::new().width(Length::Fill),
                text(filename).size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                text(counter).size(TEXT_MD).color(FG_DIM),
            ]
            .spacing(SPACE_2_5)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_1_5, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        let bottom_bar = container(
            row![
                Space::new().width(Length::Fill),
                button(text("‹ Prev").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: -1, dy: 0 })
                    .style(ghost_btn_style),
                button(text("Next ›").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: 1, dy: 0 })
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_3)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_2, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        let hud_bar = {
            use isomfolio_core::models::Flag as F;
            let flag = self.files.get(idx).map(|f| f.flag).unwrap_or(F::Unflagged);
            let rating = self.file_ratings.get(
                self.files.get(idx).map(|f| f.id.as_str()).unwrap_or("")
            ).copied().unwrap_or(0);

            let flag_btn = |label: &'static str, f: F, active_color: Color, is_current: bool| -> Element<Msg> {
                button(text(label).size(TEXT_MD))
                    .on_press(Msg::SetFlag(f))
                    .style(move |_: &Theme, _| button::Style {
                        background: Some(Background::Color(if is_current {
                            Color { r: active_color.r, g: active_color.g, b: active_color.b, a: 0.35 }
                        } else {
                            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 }
                        })),
                        text_color: if is_current { Color::WHITE } else { FG_DIM },
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    })
                    .into()
            };

            let flag_row = row![
                flag_btn("✓", F::Pick,      ACCENT, flag == F::Pick),
                flag_btn("○", F::Unflagged, FG_DIM, flag == F::Unflagged),
                flag_btn("✕", F::Reject,    ERR,    flag == F::Reject),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center);

            let mut rating_row = row![].spacing(SPACE_0_5);
            for star in 1..=5i32 {
                let filled = rating >= star;
                rating_row = rating_row.push(
                    button(
                        text(if filled { "★" } else { "☆" })
                            .size(TEXT_STAR)
                            .color(if filled { STAR_GOLD } else { Color { r: FG.r, g: FG.g, b: FG.b, a: 0.4 } }),
                    )
                    .on_press(Msg::SetRating(if rating == star { None } else { Some(star) }))
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: FG,
                        border: Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                );
            }

            container(
                row![
                    flag_row,
                    Space::new().width(Length::Fill),
                    rating_row,
                ]
                .align_y(Alignment::Center)
                .spacing(SPACE_2),
            )
            .padding([SPACE_1_5, SPACE_3])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_MEDIUM)),
                ..Default::default()
            })
        };

        let main_col: Element<Msg> = column![top_bar, img_element, bottom_bar].into();
        let hud_overlay: Element<Msg> = container(
            column![Space::new().height(Length::Fill), hud_bar],
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        container(stack([main_col, hud_overlay]))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.03,
                    g: 0.03,
                    b: 0.03,
                    a: 1.0,
                })),
                ..Default::default()
            })
            .into()
    }

    fn view_settings_modal(&self) -> Element<'_, Msg> {
        let header = row![
            text("Settings").size(TEXT_TITLE).color(FG),
            Space::new().width(Length::Fill),
            self.settings_tab_chip("General", SettingsTab::General),
            self.settings_tab_chip("Extensions", SettingsTab::Extensions),
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
        .height(Length::Fixed(480.0))
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
            button(text("Cancel").size(TEXT_BASE))
                .on_press(Msg::CloseSettings)
                .style(ghost_btn_style),
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
        .width(560);

        let modal = container(body)
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
                shadow: Shadow {
                    color: OVERLAY_LIGHT,
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            });

        modal_with_backdrop(modal).into()
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
            "Auto-advance after flagging",
            "Move to the next photo automatically after pressing P, X, or U in loupe.",
            self.app_settings.auto_advance_on_flag,
            Msg::ToggleAutoAdvanceOnFlag,
        ));
        col = col.push(self.toggle_row(
            "Auto face clustering",
            "Run after each sync that finds new photos.",
            self.app_settings.auto_face_cluster,
            Msg::ToggleAutoFaceCluster,
        ));
        col = col.push(self.toggle_row(
            "Import XMP keywords",
            "Add dc:subject keywords as tags on first sync of a new photo.",
            self.app_settings.import_xmp_tags.unwrap_or(false),
            Msg::ToggleImportXmpTags,
        ));
        if cfg!(target_os = "macos") {
            col = col.push(self.toggle_row(
                "Import Apple Finder tags",
                "Add macOS Finder tags (kMDItemUserTags) as tags on first sync of a new photo.",
                self.app_settings.import_apple_tags.unwrap_or(false),
                Msg::ToggleImportAppleTags,
            ));
        }

        col.into()
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
        use isomfolio_core::extension::ConfigFieldKind;

        let install_btn = row![
            button(text("Install from file…").size(TEXT_BASE))
                .on_press(Msg::InstallExtensionPickFile)
                .style(ghost_btn_style),
            Space::new().width(Length::Fill),
            text(format!(
                "{} installed",
                self.extensions.len()
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

        if self.extensions.is_empty() {
            body = body.push(
                container(
                    text("No extensions installed. Click \"Install from file…\" above to add one.")
                        .size(TEXT_MD)
                        .color(FG_MUTED),
                )
                .padding(SPACE_3),
            );
        }

        for ext in &self.extensions {
            let name = ext.manifest.name.clone();
            let desc = ext.manifest.description.clone();

            // Header row: name + uninstall button
            body = body.push(
                row![
                    column![
                        text(name.clone()).size(TEXT_BASE).color(FG),
                        text(desc).size(TEXT_SM).color(FG_DIM),
                    ]
                    .spacing(SPACE_0_5),
                    Space::new().width(Length::Fill),
                    button(text("Remove").size(TEXT_SM))
                        .on_press(Msg::UninstallExtension(name.clone()))
                        .style(|_: &Theme, s| {
                            let alpha = match s {
                                iced::widget::button::Status::Hovered => 0.13,
                                _ => 0.0,
                            };
                            iced::widget::button::Style {
                                background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: alpha })),
                                text_color: styles::ERR,
                                border: Border { radius: 4.0.into(), ..Default::default() },
                                shadow: iced::Shadow::default(),
                                snap: false,
                            }
                        }),
                ]
                .align_y(Alignment::Center)
                .spacing(SPACE_2),
            );

            // Config fields
            let empty_map = std::collections::HashMap::new();
            let field_values = self.settings.extension_configs.get(&name).unwrap_or(&empty_map);

            for field in &ext.manifest.config_schema {
                let current = field_values.get(&field.key).cloned().unwrap_or_default();
                let key = field.key.clone();
                let extension_name = name.clone();

                body = body.push(Space::new().height(SPACE_2));
                body = body.push(text(&field.label).size(TEXT_MD).color(FG_DIM));
                body = body.push(Space::new().height(SPACE_1_5));

                match field.kind {
                    ConfigFieldKind::Select => {
                        let mut option_row = row![].spacing(SPACE_1_5);
                        for opt in &field.options {
                            let selected = current == *opt;
                            let opt_val = opt.clone();
                            let k = key.clone();
                            let an = extension_name.clone();
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
                    ConfigFieldKind::Secret => {
                        body = body.push(
                            text_input(field.default.as_deref().unwrap_or(""), &current)
                                .on_input(move |v| Msg::SettingsConfigChanged {
                                    extension_name: extension_name.clone(),
                                    key: key.clone(),
                                    value: v,
                                })
                                .secure(true)
                                .padding([SPACE_2, SPACE_2_5])
                                .size(TEXT_BASE)
                                .width(Length::Fill),
                        );
                    }
                    ConfigFieldKind::Text => {
                        body = body.push(
                            text_input(field.default.as_deref().unwrap_or(""), &current)
                                .on_input(move |v| Msg::SettingsConfigChanged {
                                    extension_name: extension_name.clone(),
                                    key: key.clone(),
                                    value: v,
                                })
                                .padding([SPACE_2, SPACE_2_5])
                                .size(TEXT_BASE)
                                .width(Length::Fill),
                        );
                    }
                    ConfigFieldKind::Number | ConfigFieldKind::Integer => {
                        let placeholder = field.default.as_deref().unwrap_or("0");
                        body = body.push(
                            text_input(placeholder, &current)
                                .on_input(move |v| Msg::SettingsConfigChanged {
                                    extension_name: extension_name.clone(),
                                    key: key.clone(),
                                    value: v,
                                })
                                .padding([SPACE_2, SPACE_2_5])
                                .size(TEXT_BASE)
                                .width(120),
                        );
                    }
                }
            }

            body = body.push(Space::new().height(SPACE_3));
        }

        // Capability defaults — only shown when 2+ extensions share a capability
        let mut cap_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for ext in &self.extensions {
            for cap in &ext.manifest.capabilities {
                cap_map.entry(cap.clone()).or_default().push(ext.manifest.name.clone());
            }
        }
        let mut contested: Vec<(String, Vec<String>)> = cap_map
            .into_iter()
            .filter(|(_, names)| names.len() > 1)
            .collect();
        contested.sort_by(|a, b| a.0.cmp(&b.0));
        if !contested.is_empty() {
            body = body.push(Space::new().height(SPACE_4));
            body = body.push(text("Defaults").size(TEXT_SM).color(FG_DIM));
            body = body.push(Space::new().height(SPACE_2));
            for (cap, names) in contested {
                let current = self.app_settings.preferred_extension.get(&cap).cloned()
                    .unwrap_or_else(|| names[0].clone());
                body = body.push(text(format!("Auto-{cap}:")).size(TEXT_MD).color(FG));
                body = body.push(Space::new().height(SPACE_1_5));
                let mut chip_row = row![].spacing(SPACE_1_5);
                for n in names {
                    let selected = current == n;
                    let cap2 = cap.clone();
                    let n2 = n.clone();
                    let n_label = n.clone();
                    chip_row = chip_row.push(
                        button(text(n_label).size(TEXT_MD))
                            .on_press(Msg::SetPreferredExtension {
                                capability: cap2,
                                extension_name: n2,
                            })
                            .style(move |t: &Theme, s| {
                                if selected { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                            }),
                    );
                }
                body = body.push(chip_row);
                body = body.push(Space::new().height(SPACE_2));
            }
        }

        body.into()
    }

    fn view_metadata_import_prompt(&self) -> Element<'_, Msg> {
        let prompt = match &self.metadata_import_prompt {
            Some(p) => p,
            None => return Space::new().into(),
        };
        let apple_available = cfg!(target_os = "macos");
        let all_on = prompt.import_xmp && (!apple_available || prompt.import_apple);

        let checkbox = |checked: bool, label: &str, msg: Msg| -> Element<'_, Msg> {
            let glyph = if checked { "☑" } else { "☐" };
            button(
                row![
                    text(glyph).size(TEXT_BASE).color(FG),
                    text(label.to_string()).size(TEXT_MD).color(FG),
                ]
                .spacing(SPACE_1_5)
                .align_y(Alignment::Center),
            )
            .on_press(msg)
            .style(ghost_btn_style)
            .into()
        };

        let mut body = column![
            text("Import keywords from photo metadata?")
                .size(TEXT_TITLE).color(FG),
            Space::new().height(SPACE_2),
            text("When IsomFolio first discovers a photo, it can copy keywords from external metadata into the catalog as searchable tags.")
                .size(TEXT_SM).color(FG_DIM),
            Space::new().height(SPACE_4),
            checkbox(all_on, "Import all metadata", Msg::MetadataImportPromptToggleAll),
            Space::new().height(SPACE_1),
        ]
        .spacing(0)
        .width(440);

        body = body.push(
            row![
                Space::new().width(SPACE_3),
                checkbox(prompt.import_xmp, "XMP keywords (dc:subject)", Msg::MetadataImportPromptToggleXmp),
            ]
            .align_y(Alignment::Center),
        );

        if apple_available {
            body = body.push(
                row![
                    Space::new().width(SPACE_3),
                    checkbox(prompt.import_apple, "Apple Finder tags", Msg::MetadataImportPromptToggleApple),
                ]
                .align_y(Alignment::Center),
            );
        }

        body = body
            .push(Space::new().height(SPACE_4))
            .push(
                text("You can change this later in Settings → Behaviour.")
                    .size(TEXT_SM).color(FG_MUTED),
            )
            .push(Space::new().height(SPACE_4))
            .push(
                row![
                    button(text("Cancel").size(TEXT_BASE))
                        .on_press(Msg::MetadataImportPromptCancel)
                        .style(ghost_btn_style),
                    Space::new().width(Length::Fill),
                    button(text("Continue").size(TEXT_BASE))
                        .on_press(Msg::MetadataImportPromptContinue)
                        .style(active_chip_style),
                ]
                .align_y(Alignment::Center),
            );

        let modal = container(body)
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
                shadow: Shadow {
                    color: OVERLAY_LIGHT,
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 20.0,
                },
                ..Default::default()
            });

        modal_with_backdrop(modal).into()
    }

    fn view_shortcut_help(&self) -> Element<'_, Msg> {
        use crate::app::keybinds::{self, Category};

        let bindings = keybinds::bindings();
        let categories = [
            (Category::Navigation, "Navigation"),
            (Category::View, "View"),
            (Category::Culling, "Culling"),
            (Category::Tagging, "Tagging"),
        ];

        let mut col = column![
            row![
                text("Keyboard Shortcuts").size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                button(text("✕").size(TEXT_MD).color(FG_DIM))
                    .on_press(Msg::ToggleShortcutHelp)
                    .style(ghost_btn_style),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACE_2),
        ]
        .spacing(SPACE_2)
        .padding(SPACE_3);

        for (cat, cat_label) in &categories {
            let cat_bindings: Vec<_> = bindings.iter().filter(|b| &b.category == cat).collect();
            if cat_bindings.is_empty() {
                continue;
            }
            col = col.push(text(*cat_label).size(TEXT_SM).color(FG_DIM));
            for bind in cat_bindings {
                let key_str = keybinds::format_key(bind);
                col = col.push(
                    row![
                        container(text(key_str).size(TEXT_SM).color(ACCENT))
                            .width(Length::Fixed(100.0)),
                        text(bind.label).size(TEXT_SM).color(FG),
                    ]
                    .spacing(SPACE_2)
                    .align_y(Alignment::Center),
                );
            }
        }

        let panel = container(col)
            .width(Length::Fixed(340.0))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

        modal_with_backdrop(panel).into()
    }
}

/// Wrap a modal panel with a backdrop that blocks all mouse events from reaching
/// the layers below. The backdrop is darkened (OVERLAY_MEDIUM) and the modal is
/// centered on top.
fn modal_with_backdrop<'a, E>(modal: E) -> Element<'a, Msg>
where
    E: Into<Element<'a, Msg>>,
{
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_MEDIUM)),
                ..Default::default()
            }),
    )
    .on_press(Msg::NoOp)
    .on_release(Msg::NoOp)
    .on_right_press(Msg::NoOp)
    .on_right_release(Msg::NoOp)
    .on_double_click(Msg::NoOp);

    let centered = container(modal.into())
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    iced::widget::stack(vec![backdrop.into(), centered.into()]).into()
}
