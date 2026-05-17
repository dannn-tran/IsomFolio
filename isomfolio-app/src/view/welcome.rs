use iced::{
    widget::{button, column, container, row, scrollable, stack, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_GRID, BG_MODAL, BORDER, ERR, FG, FG_DIM,
    FG_MUTED, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, SPACE_4, SPACE_5, SPACE_6,
};
use crate::app::{App, Msg};

impl App {
    pub(super) fn view_welcome(&self) -> Element<'_, Msg> {
        let can_open_selected = self.selected_recent_catalog.is_some();
        let can_create = self.new_catalog_dir.is_some() && !self.new_catalog_name.trim().is_empty();

        let mut recent_list = column![].spacing(SPACE_2).align_x(Alignment::Start);
        if self.recent_catalogs.is_empty() {
            recent_list = recent_list.push(
                text("No recent catalogues yet. Create one or browse for an existing catalogue.")
                    .size(13)
                    .color(FG_MUTED),
            );
        } else {
            for path in &self.recent_catalogs {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str())
                    .to_string();
                let selected = self.selected_recent_catalog.as_deref() == Some(path.as_str());
                let path_clone = path.clone();

                let content = container(
                    column![
                        text(name).size(15).color(FG),
                        Space::new().height(SPACE_1),
                        text(path).size(12).width(Length::Fill).color(if selected {
                            FG
                        } else {
                            FG_DIM
                        }),
                    ]
                    .align_x(Alignment::Start),
                )
                .width(Length::Fill)
                .padding([SPACE_2, SPACE_3]);

                recent_list = recent_list.push(
                    button(content)
                        .on_press(Msg::SelectRecentCatalog(path_clone))
                        .width(Length::Fill)
                        .style(move |_: &Theme, status| recent_item_style(selected, status)),
                );
            }
        }

        let recent_section = column![
            text("Recents").size(11).color(FG_DIM),
            Space::new().height(SPACE_2),
            scrollable(recent_list)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .align_x(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill);

        let actions = row![
            {
                let btn = button(text("Open").size(14)).style(if can_open_selected {
                    active_chip_style
                } else {
                    quiet_btn_disabled_style
                });
                if can_open_selected {
                    btn.on_press(Msg::OpenSelectedRecentCatalog)
                } else {
                    btn
                }
            },
            button(text("New Catalog...").size(14))
                .on_press(Msg::ShowNewCatalogModal)
                .style(ghost_btn_style),
            button(text("Browse...").size(14))
                .on_press(Msg::PickOpenCatalog)
                .style(ghost_btn_style),
        ]
        .spacing(SPACE_2_5)
        .width(Length::Fill)
        .align_y(Alignment::Center);

        let mut base = column![
            text("IsomFolio").size(36).color(FG),
            Space::new().height(SPACE_1),
            text("Photo library manager").size(14).color(FG_DIM),
            Space::new().height(SPACE_6),
            recent_section,
            Space::new().height(SPACE_4),
            actions,
        ]
        .align_x(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill)
        .max_width(960);

        if !self.status.is_empty() {
            base = base
                .push(Space::new().height(SPACE_2_5))
                .push(text(&self.status).size(12).color(ERR));
        }

        let base_layer: Element<'_, Msg> = container(base)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([SPACE_5, SPACE_6])
            .align_x(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into();

        if !self.show_new_catalog_modal {
            return base_layer;
        }

        let location_display = self
            .new_catalog_dir
            .as_deref()
            .unwrap_or("Choose a folder for the new catalogue");

        let modal = container(
            column![
                text("New Catalog").size(20).color(FG),
                Space::new().height(SPACE_1_5),
                text("Create a catalogue in a chosen location.")
                    .size(13)
                    .color(FG_DIM),
                Space::new().height(SPACE_4),
                text("Catalog name").size(12).color(FG_DIM),
                Space::new().height(SPACE_1_5),
                text_input("My Photos", &self.new_catalog_name)
                    .on_input(Msg::NewCatalogNameChanged)
                    .on_submit(Msg::ConfirmNewCatalog)
                    .padding([SPACE_2, SPACE_2_5])
                    .size(13)
                    .width(Length::Fill),
                Space::new().height(SPACE_4),
                text("Location").size(12).color(FG_DIM),
                Space::new().height(SPACE_1_5),
                container(text(location_display).size(13).color(
                    if self.new_catalog_dir.is_some() { FG } else { FG_MUTED }
                ))
                .width(Length::Fill)
                .padding([SPACE_2_5, SPACE_3])
                .style(field_style),
                Space::new().height(SPACE_2_5),
                button(text("Choose Location…").size(13))
                    .on_press(Msg::PickNewCatalogDir)
                    .style(ghost_btn_style),
                Space::new().height(SPACE_4),
                row![
                    button(text("Cancel").size(13))
                        .on_press(Msg::HideNewCatalogModal)
                        .style(ghost_btn_style),
                    {
                        let btn = button(text("Create Catalog").size(13)).style(if can_create {
                            active_chip_style
                        } else {
                            |_: &Theme, _: button::Status| button::Style {
                                background: Some(Background::Color(Color {
                                    r: 1.0,
                                    g: 1.0,
                                    b: 1.0,
                                    a: 0.04,
                                })),
                                text_color: FG_MUTED,
                                border: Border {
                                    radius: 4.0.into(),
                                    ..Default::default()
                                },
                                shadow: iced::Shadow::default(),
                                snap: false,
                            }
                        });
                        if can_create {
                            btn.on_press(Msg::ConfirmNewCatalog)
                        } else {
                            btn
                        }
                    },
                ]
                .spacing(SPACE_2_5)
                .align_y(Alignment::Center),
            ]
            .width(420)
            .align_x(Alignment::Start),
        )
        .padding(SPACE_6)
        .style(modal_style);

        stack(vec![
            base_layer,
            container(
                container(modal)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.55,
                })),
                ..Default::default()
            })
            .into(),
        ])
        .into()
    }
}

fn field_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.03,
        })),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn modal_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_MODAL)),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn quiet_btn_disabled_style(_: &Theme, _: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.04,
        })),
        text_color: FG_MUTED,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn recent_item_style(selected: bool, status: button::Status) -> button::Style {
    let background = if selected {
        match status {
            button::Status::Hovered => Color { r: 0.25, g: 0.59, b: 1.0, a: 0.28 },
            button::Status::Pressed => Color { r: 0.18, g: 0.49, b: 0.88, a: 0.36 },
            _ => Color { r: ACCENT.r, g: ACCENT.g, b: ACCENT.b, a: 0.22 },
        }
    } else {
        match status {
            button::Status::Hovered => Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 },
            button::Status::Pressed => Color { r: 1.0, g: 1.0, b: 1.0, a: 0.16 },
            _ => Color { r: 1.0, g: 1.0, b: 1.0, a: 0.04 },
        }
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: FG,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}
