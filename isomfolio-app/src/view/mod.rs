mod compare;
mod content;
mod context_menu;
mod loupe_image;
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
    active_chip_style, danger_btn_style, ghost_btn_style, icon_btn_style, ACCENT, BG_GRID,
    BG_MODAL, BG_PANEL, BG_PROGRESS_TRACK, BG_STATUSBAR, BORDER, ERR, FG, FG_DIM, FG_MUTED,
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

        let pick_count = self
            .files
            .iter()
            .filter(|f| f.flag == isomfolio_core::models::Flag::Pick)
            .count();

        let mut status_row = row![
            text(status).size(TEXT_MD).color(FG),
            Space::new().width(Length::Fill),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        if let Some(btn) = remove_btn {
            status_row = status_row.push(btn);
        }

        if self.reject_trash_pending {
            let n = self
                .files
                .iter()
                .filter(|f| f.flag == isomfolio_core::models::Flag::Reject && !f.is_orphaned)
                .count();
            status_row = status_row.push(
                row![
                    text(format!("Move {n} reject(s) to Trash?")).size(TEXT_MD).color(ERR),
                    button(text("Cancel").size(TEXT_MD))
                        .on_press(Msg::CancelMoveRejectsToTrash)
                        .style(ghost_btn_style),
                    button(text("Move").size(TEXT_MD))
                        .on_press(Msg::ConfirmMoveRejectsToTrash)
                        .style(danger_btn_style),
                ]
                .spacing(SPACE_1_5)
                .align_y(Alignment::Center),
            );
        }

        if pick_count > 0 {
            status_row = status_row.push(
                text(format!("✓ {pick_count}"))
                    .size(TEXT_MD)
                    .color(ACCENT),
            );
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
            ViewMode::Settings => self.view_settings_pane(),
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
        if self.has_any_bg_activity() || !self.bg_tasks.is_empty() {
            layers.push(self.view_task_panel());
        }
        if self.show_shortcut_help {
            layers.push(self.view_shortcut_help());
        }
        if self.add_folder_prompt.is_some() {
            layers.push(self.view_add_folder_prompt());
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

    /// Every long-running process, mapped to one uniform task shape. The task
    /// panel renders only this — no per-process special-casing.
    fn active_tasks(&self) -> Vec<TaskView> {
        let mut tasks = Vec::new();

        if self.faces.is_clustering {
            tasks.push(TaskView {
                title: "Finding people".into(),
                detail: self.faces.status.clone().unwrap_or_default(),
                progress: match self.faces.progress {
                    Some(r) => TaskProgress::Determinate(r),
                    None => TaskProgress::Indeterminate,
                },
                failed: false,
                dismiss: None,
            });
        }

        if self.is_syncing {
            let detail = if self.status.starts_with("Syncing") {
                self.status.clone()
            } else {
                "Scanning…".into()
            };
            tasks.push(TaskView {
                title: "Sync".into(),
                detail,
                progress: TaskProgress::Indeterminate,
                failed: false,
                dismiss: None,
            });
        }

        if self.thumb_ctx.total > 0 {
            let total = self.thumb_ctx.total;
            let done = total.saturating_sub(self.thumb_ctx.pending);
            let ratio = done as f32 / total.max(1) as f32;
            let eta = if done >= 3 {
                self.thumb_ctx
                    .start_at
                    .map(|s| {
                        let elapsed = s.elapsed().as_secs_f64();
                        let secs =
                            (self.thumb_ctx.pending as f64 / (done as f64 / elapsed)).ceil() as u64;
                        if secs < 60 {
                            format!(" · ~{secs}s")
                        } else {
                            format!(" · ~{}m{}s", secs / 60, secs % 60)
                        }
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            tasks.push(TaskView {
                title: "Thumbnails".into(),
                detail: format!("{done} / {total}{eta}"),
                progress: TaskProgress::Determinate(ratio),
                failed: false,
                dismiss: None,
            });
        }

        for task in &self.bg_tasks {
            let failed = task.failed.is_some();
            tasks.push(TaskView {
                title: task.label.clone(),
                detail: task.failed.clone().unwrap_or_default(),
                progress: match task.progress {
                    Some(r) => TaskProgress::Determinate(r),
                    None => TaskProgress::Indeterminate,
                },
                failed,
                dismiss: failed.then_some(task.id),
            });
        }

        tasks
    }

    fn view_task_panel(&self) -> Element<'_, Msg> {
        let open = self.task_panel_open;
        let tasks = self.active_tasks();

        // Collapsed pill — shows when panel is minimised.
        if !open {
            let n = tasks.len();
            let label = if n == 1 { "1 task".to_string() } else { format!("{n} tasks") };
            let pill = container(
                button(
                    row![
                        text("◌").size(TEXT_SM).color(FG_DIM),
                        text(label).size(TEXT_SM).color(FG_DIM),
                        Space::new().width(Length::Fill),
                        text("∨").size(TEXT_SM).color(FG_DIM),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .on_press(Msg::ToggleTaskPanel)
                .style(|_: &Theme, _| button::Style {
                    background: None,
                    text_color: FG_DIM,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                })
                .width(Length::Fill),
            )
            .width(200)
            .padding([SPACE_1, SPACE_2])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PANEL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow { color: OVERLAY_LIGHT, offset: Vector::new(0.0, 2.0), blur_radius: 8.0 },
                ..Default::default()
            });

            return container(pill)
                .width(Length::Fill).height(Length::Fill)
                .align_x(Alignment::End).align_y(Alignment::End)
                .padding(iced::Padding { top: 0.0, right: SPACE_3, bottom: 38.0, left: 0.0 })
                .into();
        }

        // Expanded panel — build rows for each active task.
        let mut col = column![].spacing(0);

        // Header
        col = col.push(
            row![
                text("Tasks").size(TEXT_SM).color(FG_DIM),
                Space::new().width(Length::Fill),
                button(text("∧").size(TEXT_SM).color(FG_DIM))
                    .on_press(Msg::ToggleTaskPanel)
                    .style(styles::icon_btn_style),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACE_1),
        );

        for task in tasks {
            col = col.push(Space::new().height(SPACE_2));
            col = col.push(task_row(task));
        }

        let panel = container(col.padding([SPACE_1_5, SPACE_2]))
            .width(260)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PANEL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow { color: OVERLAY_LIGHT, offset: Vector::new(0.0, 3.0), blur_radius: 10.0 },
                ..Default::default()
            });

        container(panel)
            .width(Length::Fill).height(Length::Fill)
            .align_x(Alignment::End).align_y(Alignment::End)
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
            Some(handle) => loupe_image::LoupeImage::new(
                handle,
                self.loupe.zoom,
                self.loupe.pan,
                |scale, pan| Msg::LoupeZoomChanged { scale, pan },
                |viewport, native| Msg::LoupeGeometry { viewport, native },
            )
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

        let zoomed = self.loupe.zoom > crate::app::LOUPE_ZOOM_MIN;
        let tip = |el, label: &'static str| styles::tip(el, label, styles::TipPos::Top);
        let zoom_cluster = row![
            tip(
                button(text("−").size(TEXT_LG)).on_press(Msg::LoupeZoomBy(0.8)).style(ghost_btn_style),
                "Zoom out",
            ),
            tip(
                button(text("+").size(TEXT_LG)).on_press(Msg::LoupeZoomBy(1.25)).style(ghost_btn_style),
                "Zoom in",
            ),
            tip(
                button(text("1:1").size(TEXT_MD)).on_press(Msg::LoupeZoomActual).style(ghost_btn_style),
                "Actual pixels (Z)",
            ),
            tip(
                button(text("Fit").size(TEXT_MD).color(if zoomed { FG } else { FG_DIM }))
                    .on_press(Msg::LoupeZoomReset).style(ghost_btn_style),
                "Fit to window",
            ),
            tip(
                button(text(if self.fullscreen { "⤢" } else { "⛶" }).size(TEXT_MD))
                    .on_press(Msg::ToggleFullscreen).style(ghost_btn_style),
                "Toggle fullscreen",
            ),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        let bottom_bar = container(
            row![
                Space::new().width(Length::Fill),
                button(text("‹ Prev").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: -1, dy: 0 })
                    .style(ghost_btn_style),
                button(text("Next ›").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: 1, dy: 0 })
                    .style(ghost_btn_style),
                Space::new().width(SPACE_3),
                zoom_cluster,
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

            let flag_btn = |glyph: &'static str, key: &'static str, f: F, active_color: Color, is_current: bool| -> Element<Msg> {
                button(
                    column![
                        text(glyph).size(TEXT_MD),
                        text(key).size(TEXT_SM).color(if is_current {
                            Color { a: 0.7, ..Color::WHITE }
                        } else {
                            Color { a: 0.35, ..Color::WHITE }
                        }),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(1.0),
                )
                .on_press(Msg::SetFlag(f))
                .style(move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(if is_current {
                        Color { r: active_color.r, g: active_color.g, b: active_color.b, a: 0.55 }
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
                flag_btn("✓", "P", F::Pick,      ACCENT, flag == F::Pick),
                flag_btn("○", "U", F::Unflagged, FG_DIM, flag == F::Unflagged),
                flag_btn("✕", "X", F::Reject,    ERR,    flag == F::Reject),
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

            let cur_label = self
                .files
                .get(idx)
                .and_then(|f| self.file_labels.get(&f.id))
                .cloned();
            let mut color_row = row![].spacing(SPACE_1).align_y(Alignment::Center);
            for name in styles::COLOR_LABELS {
                let active = cur_label.as_deref() == Some(name);
                let swatch = styles::color_label_swatch(name);
                color_row = color_row.push(styles::tip(
                    button(text("●").size(TEXT_LG).color(swatch))
                        .on_press(Msg::SetColorLabel(Some(name.to_string())))
                        .style(move |_: &Theme, _| button::Style {
                            background: if active { Some(Background::Color(Color { a: 0.25, ..swatch })) } else { None },
                            text_color: swatch,
                            border: Border { radius: 4.0.into(), ..Default::default() },
                            shadow: iced::Shadow::default(),
                            snap: false,
                        }),
                    format!("Label {name}"),
                    styles::TipPos::Top,
                ));
            }

            container(
                row![
                    flag_row,
                    Space::new().width(Length::Fill),
                    color_row,
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

        let main_col: Element<Msg> =
            column![top_bar, img_element, self.view_loupe_filmstrip(idx), bottom_bar].into();
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

    /// Horizontal thumbnail strip under the loupe image: a window of neighbours
    /// centred on the current photo, current one ringed, click to jump. Windowed
    /// (not the whole library) to keep the widget count bounded.
    fn view_loupe_filmstrip(&self, current: usize) -> Element<'_, Msg> {
        const THUMB: f32 = 56.0;
        const WINDOW: usize = 14;
        let total = self.files.len();
        let lo = current.saturating_sub(WINDOW);
        let hi = (current + WINDOW + 1).min(total);

        let mut strip = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for i in lo..hi {
            let file = &self.files[i];
            let is_cur = i == current;
            let thumb: Element<Msg> = match self.thumbnails.get(&file.id) {
                Some(isomfolio_core::models::ThumbnailState::Ready(p)) => {
                    image(image::Handle::from_path(p))
                        .width(THUMB)
                        .height(THUMB)
                        .content_fit(iced::ContentFit::Cover)
                        .into()
                }
                _ => Space::new().width(THUMB).height(THUMB).into(),
            };
            let ring = if is_cur { ACCENT } else { Color::TRANSPARENT };
            strip = strip.push(
                button(
                    container(thumb)
                        .style(move |_: &Theme| container::Style {
                            border: Border { color: ring, width: 2.0, radius: 3.0.into() },
                            ..Default::default()
                        })
                        .clip(true),
                )
                .padding(0)
                .on_press(Msg::LoupeJumpTo(i))
                .style(|_: &Theme, _| button::Style {
                    background: None,
                    text_color: FG,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }),
            );
        }

        container(strip)
            .width(Length::Fill)
            .height(Length::Fixed(THUMB + SPACE_2 * 2.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .clip(true)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_HEAVY)),
                ..Default::default()
            })
            .into()
    }

    fn view_settings_pane(&self) -> Element<'_, Msg> {
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
            "Auto-advance after flagging",
            "Move to the next photo automatically after pressing P, X, or U in loupe.",
            self.app_settings.auto_advance_on_flag,
            Msg::ToggleAutoAdvanceOnFlag,
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

    fn view_add_folder_prompt(&self) -> Element<'_, Msg> {
        let prompt = match &self.add_folder_prompt {
            Some(p) => p,
            None => return Space::new().into(),
        };
        let name = std::path::Path::new(&prompt.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(prompt.path.as_str());

        let glyph = if prompt.recursive { "☑" } else { "☐" };
        let subfolder_label = match prompt.subfolder_count {
            0 => "Include subfolders (none found)".to_string(),
            1 => "Include subfolders (1 found)".to_string(),
            n => format!("Include subfolders ({n} found)"),
        };
        let toggle = button(
            row![
                text(glyph).size(TEXT_BASE).color(FG),
                text(subfolder_label).size(TEXT_MD).color(FG),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center),
        )
        .on_press(Msg::AddFolderPromptToggleRecursive)
        .style(ghost_btn_style);

        let body = column![
            text(format!("Add \u{201C}{name}\u{201D} to the library"))
                .size(TEXT_TITLE).color(FG),
            Space::new().height(SPACE_2),
            text("Photos in this folder are indexed. With subfolders included, the whole tree is indexed and shown as a navigable hierarchy in the sidebar.")
                .size(TEXT_SM).color(FG_DIM),
            Space::new().height(SPACE_4),
            toggle,
            Space::new().height(SPACE_4),
            row![
                button(text("Cancel").size(TEXT_BASE))
                    .on_press(Msg::AddFolderCancel)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
                button(text("Add").size(TEXT_BASE))
                    .on_press(Msg::AddFolderConfirm)
                    .style(active_chip_style),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(0)
        .width(440);

        let modal = container(body)
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
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

/// One uniform progress entry — the only thing the task panel renders.
enum TaskProgress {
    /// Known fraction 0.0–1.0.
    Determinate(f32),
    /// In progress, amount unknown (engine starting, scanning, …).
    Indeterminate,
}

struct TaskView {
    title: String,
    detail: String,
    progress: TaskProgress,
    failed: bool,
    dismiss: Option<crate::app::BgTaskId>,
}

/// A 2px-high colored segment occupying `portion` of the track width.
fn bar_segment(portion: u16, color: Color) -> Element<'static, Msg> {
    container(Space::new())
        .width(Length::FillPortion(portion.max(1)))
        .height(2)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border { radius: 1.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

fn task_row(task: TaskView) -> Element<'static, Msg> {
    let label_color = if task.failed { ERR } else { FG };

    let mut header = row![
        text(task.title).size(TEXT_SM).color(label_color),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACE_1);

    if let Some(id) = task.dismiss {
        header = header.push(
            button(text("✕").size(TEXT_SM).color(FG_MUTED))
                .on_press(Msg::BgTaskDismissed(id))
                .style(icon_btn_style),
        );
    }

    let mut col = column![header].spacing(SPACE_0_5);

    if task.failed {
        if !task.detail.is_empty() {
            col = col.push(text(task.detail).size(TEXT_SM).color(ERR));
        }
        return col.into();
    }

    // Determinate fills proportionally; indeterminate floats a centered segment
    // so it reads as "working, amount unknown" rather than a partial fill.
    let bar = match task.progress {
        TaskProgress::Determinate(ratio) => {
            let filled = (ratio.clamp(0.0, 1.0) * 1000.0) as u16;
            row![
                bar_segment(filled, ACCENT),
                bar_segment(1000u16.saturating_sub(filled), BG_PROGRESS_TRACK),
            ]
        }
        TaskProgress::Indeterminate => row![
            bar_segment(35, BG_PROGRESS_TRACK),
            bar_segment(30, ACCENT),
            bar_segment(35, BG_PROGRESS_TRACK),
        ],
    }
    .width(Length::Fill);
    col = col.push(bar);

    if !task.detail.is_empty() {
        col = col.push(text(task.detail).size(TEXT_SM).color(FG_MUTED));
    }

    col.into()
}
