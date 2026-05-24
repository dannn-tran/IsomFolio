mod content;
mod context_menu;
mod sidebar;
pub mod styles;
mod tag_browser;
mod welcome;

use iced::{
    widget::{button, column, container, image, row, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use crate::app::{App, Msg, SidebarItem, ViewMode};
use styles::{
    active_chip_style, danger_btn_style, ghost_btn_style, BG_STATUSBAR, ERR, FG, FG_DIM, FG_MUTED,
    SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, STAR_GOLD, TEXT_BASE, TEXT_LG, TEXT_MD,
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
                1 => "Enter for loupe · I for info · Drag to add to album".to_string(),
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

        let pending = self.thumbnail_pending;
        let status_left: Element<Msg> = if pending > 0 {
            row![
                text(status).size(TEXT_MD).color(FG),
                text(format!("Generating {pending} thumbnails…"))
                    .size(TEXT_MD)
                    .color(FG_DIM),
            ]
            .spacing(SPACE_2)
            .align_y(Alignment::Center)
            .into()
        } else {
            text(status).size(TEXT_MD).color(FG).into()
        };

        let mut status_row = row![
            status_left,
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

        let mut main_row = row![self.view_sidebar(), self.view_grid()].height(Length::Fill);
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
        if layers.len() == 1 {
            layers.remove(0)
        } else {
            stack(layers).into()
        }
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
