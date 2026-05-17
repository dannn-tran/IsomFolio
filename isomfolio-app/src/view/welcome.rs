use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{button, column, container, row, text, text_input, Space},
    Theme,
};

use crate::app::{App, Msg};
use super::styles::{
    BG_GRID, FG, FG_DIM, FG_MUTED, ERR,
    ghost_btn_style, active_chip_style,
};

impl App {
    pub(super) fn view_welcome(&self) -> Element<'_, Msg> {
        let location_display = match &self.new_catalog_dir {
            Some(dir) => std::path::Path::new(dir)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(dir.as_str())
                .to_string(),
            None => String::new(),
        };

        let location_row = row![
            text("Location").size(13).color(FG_DIM).width(110),
            if location_display.is_empty() {
                text("Not set").size(13).color(FG_MUTED)
            } else {
                text(location_display).size(13).color(FG)
            },
            Space::new().width(Length::Fill),
            button(text("Browse…").size(13))
                .on_press(Msg::PickNewCatalogDir)
                .style(ghost_btn_style),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let name_row = row![
            text("Catalog name").size(13).color(FG_DIM).width(110),
            text_input("My Photos", &self.new_catalog_name)
                .on_input(Msg::NewCatalogNameChanged)
                .on_submit(Msg::ConfirmNewCatalog)
                .size(13)
                .width(200),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let can_create =
            self.new_catalog_dir.is_some() && !self.new_catalog_name.trim().is_empty();

        let create_btn = {
            let b = button(text("Create Catalog").size(13)).style(if can_create {
                active_chip_style
            } else {
                |_: &Theme, _: button::Status| button::Style {
                    background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.04 })),
                    text_color: FG_MUTED,
                    border: Border { radius: 4.0.into(), ..Default::default() },
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            });
            if can_create { b.on_press(Msg::ConfirmNewCatalog) } else { b }
        };

        let new_catalog_form = container(
            column![
                text("New Catalog").size(11).color(FG_DIM),
                Space::new().height(10),
                location_row,
                Space::new().height(8),
                name_row,
                Space::new().height(14),
                create_btn,
            ]
            .align_x(Alignment::Start),
        )
        .padding(20)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.04 })),
            border: Border {
                color: Color { r: 0.28, g: 0.28, b: 0.34, a: 1.0 },
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

        let mut col = column![
            text("IsomFolio").size(36).color(FG),
            Space::new().height(4),
            text("Photo library manager").size(14).color(FG_DIM),
            Space::new().height(32),
            new_catalog_form,
            Space::new().height(16),
            button(text("Open Existing Catalog…").size(13))
                .on_press(Msg::PickOpenCatalog)
                .style(ghost_btn_style),
        ]
        .spacing(0)
        .align_x(Alignment::Center);

        if !self.status.is_empty() {
            col = col
                .push(Space::new().height(8))
                .push(text(&self.status).size(12).color(ERR));
        }

        if !self.recent_catalogs.is_empty() {
            col = col
                .push(Space::new().height(32))
                .push(text("Recent").size(11).color(FG_DIM))
                .push(Space::new().height(6));
            for path in &self.recent_catalogs {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str())
                    .to_string();
                let path_clone = path.clone();
                col = col.push(
                    button(text(name).size(13))
                        .on_press(Msg::OpenCatalog(path_clone))
                        .style(ghost_btn_style),
                );
            }
        }

        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }
}
