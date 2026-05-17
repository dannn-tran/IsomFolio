pub mod styles;
mod sidebar;
mod content;
mod welcome;

use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{button, column, container, image, row, text, Space},
    Theme,
};

use crate::app::{App, Msg, SidebarItem, ViewMode, sort_field_label};
use styles::{BG_STATUSBAR, FG, FG_DIM, FG_MUTED, ghost_btn_style, active_chip_style};

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
                    let name = self.albums.iter()
                        .find(|a| &a.id == id)
                        .map(|a| a.name.as_str())
                        .unwrap_or("?");
                    format!("Dragging {count} — drop on \"{name}\"")
                }
                None => format!("Dragging {count} photo(s)…"),
            }
        } else if !self.status.is_empty() {
            self.status.clone()
        } else if self.grid_selected.len() == 1 && !self.detail.show {
            "1 photo selected · Enter for loupe · I for info · Drag to album".to_string()
        } else {
            "Click to select · Cmd+click multi-select · Enter for loupe · Drag to album".to_string()
        };

        let sort_label = format!("{} {}", sort_field_label(self.sort_by), if self.sort_asc { "▲" } else { "▼" });

        let show_criteria = self.criteria.show;
        let show_detail = self.detail.show;

        let remove_btn: Option<Element<Msg>> =
            if matches!(self.selected_item, SidebarItem::Album(_)) && !self.grid_selected.is_empty() {
                let n = self.grid_selected.len();
                Some(
                    button(text(format!("Remove {n}")).size(12))
                        .on_press(Msg::RemoveFromAlbum)
                        .style(ghost_btn_style)
                        .into(),
                )
            } else {
                None
            };

        let mut status_row = row![
            text(status).size(12).color(FG),
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        if let Some(btn) = remove_btn {
            status_row = status_row.push(btn);
        }

        status_row = status_row
            .push({
                let criteria_active = self.criteria_has_any();
                let filter_label = if criteria_active {
                    format!("Filters ●")
                } else {
                    "Filters".to_string()
                };
                button(text(filter_label).size(12))
                    .on_press(Msg::ToggleCriteria)
                    .style(move |t: &Theme, s| {
                        if show_criteria { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                    })
            })
            .push(
                button(text("Info").size(12))
                    .on_press(Msg::ToggleDetail)
                    .style(move |t: &Theme, s| {
                        if show_detail { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                    }),
            )
            .push(
                button(text(sort_label).size(12))
                    .on_press(Msg::SortCycleAll)
                    .style(ghost_btn_style),
            )
            .push(
                button(text("−").size(14))
                    .on_press(Msg::TileSizeDown)
                    .style(ghost_btn_style),
            )
            .push(text(format!("{}px", self.tile_px as u32)).size(12).color(FG_MUTED))
            .push(
                button(text("+").size(14))
                    .on_press(Msg::TileSizeUp)
                    .style(ghost_btn_style),
            );

        let status_bar = container(status_row)
            .padding([4, 12])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                ..Default::default()
            });

        let mut main_row = row![self.view_sidebar(), self.view_grid()]
            .height(Length::Fill);
        if self.detail.show {
            main_row = main_row.push(self.view_detail());
        }

        column![main_row, status_bar].into()
    }

    fn view_loupe(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe_idx.min(total.saturating_sub(1));

        let img_element: Element<Msg> = if let Some(file) = self.files.get(idx) {
            image(image::Handle::from_path(&file.path))
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            Space::new().width(Length::Fill).height(Length::Fill).into()
        };

        let filename = self.files.get(idx)
            .map(|f| f.name.as_str())
            .unwrap_or("");
        let wrap_hint = if total > 1 && (idx == 0 || idx == total - 1) { " ↩" } else { "" };
        let counter = if total > 0 {
            format!("{} / {}{}", idx + 1, total, wrap_hint)
        } else {
            String::new()
        };

        let top_bar = container(
            row![
                button(text("✕").size(14).color(FG))
                    .on_press(Msg::OpenLoupe)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 })),
                        text_color: FG,
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                Space::new().width(Length::Fill),
                text(filename).size(13).color(FG),
                Space::new().width(Length::Fill),
                text(counter).size(12).color(FG_DIM),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 })),
            ..Default::default()
        });

        let bottom_bar = container(
            row![
                Space::new().width(Length::Fill),
                button(text("‹ Prev").size(13))
                    .on_press(Msg::Navigate { dx: -1, dy: 0 })
                    .style(ghost_btn_style),
                button(text("Next ›").size(13))
                    .on_press(Msg::Navigate { dx: 1, dy: 0 })
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 })),
            ..Default::default()
        });

        container(
            column![top_bar, img_element, bottom_bar]
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.03, g: 0.03, b: 0.03, a: 1.0 })),
            ..Default::default()
        })
        .into()
    }
}
