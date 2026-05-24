mod content;
mod context_menu;
mod sidebar;
pub mod styles;
mod tag_browser;
mod welcome;

use iced::{
    widget::{button, column, container, image, mouse_area, row, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Shadow, Theme, Vector,
};

use crate::app::{App, Msg, SidebarItem, ViewMode, SIDEBAR_HANDLE_WIDTH};
use styles::{
    active_chip_style, danger_btn_style, ghost_btn_style, ACCENT, BG_STATUSBAR, BORDER, ERR, FG,
    FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, STAR_GOLD,
    TEXT_BASE, TEXT_LG, TEXT_MD, TEXT_SM,
};

impl App {
    pub fn view(&self) -> Element<'_, Msg> {
        if self.show_welcome {
            return self.view_welcome();
        }

        if matches!(self.view_mode, ViewMode::Loupe) {
            return self.view_loupe();
        }

        let dragging = self.drag.as_ref().map(|d| d.active).unwrap_or(false);
        let drag_hover = self.drag_hover_album.clone();
        let status = if dragging {
            let count = self.dragging_ids.len();
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
            self.status.clone()
        } else {
            match self.grid_selected.len() {
                0 => "Click to select".to_string(),
                1 => "Enter/Space for loupe · I for info · Drag to add to album".to_string(),
                n => format!("{n} photos selected · Drag to album"),
            }
        };

        let show_detail = self.detail.show;

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

        status_row = status_row
            .push(
                button(text("Info").size(TEXT_MD))
                    .on_press(Msg::ToggleDetail)
                    .style(move |t: &Theme, s| {
                        if show_detail {
                            active_chip_style(t, s)
                        } else {
                            ghost_btn_style(t, s)
                        }
                    }),
            )
            .push(
                button(text("−").size(TEXT_LG))
                    .on_press(Msg::TileSizeDown)
                    .style(ghost_btn_style),
            )
            .push(
                text(format!("{}px", self.tile_px as u32))
                    .size(TEXT_MD)
                    .color(FG_MUTED),
            )
            .push(
                button(text("+").size(TEXT_LG))
                    .on_press(Msg::TileSizeUp)
                    .style(ghost_btn_style),
            );

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
                        r: 0.28,
                        g: 0.28,
                        b: 0.34,
                        a: if resizing { 0.9 } else { 0.15 },
                    })),
                    ..Default::default()
                }),
        )
        .on_press(Msg::SidebarResizeStart)
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .into();

        let mut main_row = row![self.view_sidebar(), resize_handle, self.view_grid()]
            .height(Length::Fill);
        if self.detail.show {
            main_row = main_row.push(self.view_detail());
        }

        let base: Element<Msg> = column![main_row, status_bar].into();

        let mut layers: Vec<Element<Msg>> = vec![base];
        if let Some(cm) = self.view_context_menu() {
            layers.push(cm);
        }
        if let Some(tb) = self.view_tag_browser() {
            layers.push(tb);
        }
        if self.thumbnail_total > 0 {
            layers.push(self.view_thumbnail_progress_panel());
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
        let total = self.thumbnail_total;
        let done = total.saturating_sub(self.thumbnail_pending);
        let ratio = done as f32 / total.max(1) as f32;

        let eta_str = if done >= 3 {
            if let Some(start) = self.thumbnail_start_at {
                let elapsed = start.elapsed().as_secs_f64();
                let rate = done as f64 / elapsed;
                let secs_left = (self.thumbnail_pending as f64 / rate).ceil() as u64;
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
                    background: Some(Background::Color(Color { r: 0.25, g: 0.25, b: 0.28, a: 1.0 })),
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
                background: Some(Background::Color(Color { r: 0.13, g: 0.13, b: 0.16, a: 0.96 })),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow {
                    color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.4 },
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

    fn view_loupe(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe_idx.min(total.saturating_sub(1));

        let img_handle: Option<image::Handle> = if let Some((full_idx, handle)) = &self.loupe_full_res {
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
            background: Some(Background::Color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            })),
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
            background: Some(Background::Color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.7,
            })),
            ..Default::default()
        });

        let hud_bar = {
            let mut rating_row = row![].spacing(SPACE_1);
            if let Some(file) = self.files.get(idx) {
                let rating = self
                    .loupe_full_res
                    .as_ref()
                    .and_then(|_| self.detail.rating)
                    .or(self.detail.rating);
                for star in 1..=5i32 {
                    let filled = rating.map_or(false, |r| r >= star);
                    rating_row = rating_row.push(
                        text(if filled { "★" } else { "☆" })
                            .size(TEXT_BASE)
                            .color(if filled {
                                STAR_GOLD
                            } else {
                                Color { r: FG.r, g: FG.g, b: FG.b, a: 0.4 }
                            }),
                    );
                }
                let _ = file;
            }

            container(
                row![
                    Space::new().width(Length::Fill),
                    rating_row,
                    Space::new().width(Length::Fill),
                ]
                .align_y(Alignment::Center)
                .spacing(SPACE_2),
            )
            .padding([SPACE_1_5, SPACE_3])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                })),
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
}
