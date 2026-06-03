use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, stack, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_GRID, BG_MODAL, BORDER, ERR, FG, FG_DIM,
    FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, SPACE_4, SPACE_6,
    TEXT_BASE, TEXT_DISPLAY, TEXT_LG, TEXT_MD, TEXT_SM, TEXT_TITLE,
};
use crate::app::{App, Msg};

impl App {
    pub(super) fn view_welcome(&self) -> Element<'_, Msg> {
        let can_open_selected = self.welcome.selected_recent_catalog.is_some();

        let mut recent_list = column![].spacing(SPACE_2).align_x(Alignment::Start);
        if self.welcome.recent_catalogs.is_empty() {
            recent_list = recent_list.push(
                text("No recent catalogues yet. Create one or browse for an existing catalogue.")
                    .size(TEXT_BASE)
                    .color(FG_MUTED),
            );
        } else {
            for path in &self.welcome.recent_catalogs {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str())
                    .to_string();
                let selected = self.welcome.selected_recent_catalog.as_deref() == Some(path.as_str());
                let path_clone = path.clone();

                let content = container(
                    column![
                        text(name).size(TEXT_LG).color(FG),
                        Space::new().height(SPACE_1),
                        text(path).size(TEXT_MD).width(Length::Fill).color(if selected {
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
            text("Recents").size(TEXT_SM).color(FG_DIM),
            Space::new().height(SPACE_2),
            scrollable(recent_list)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .align_x(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill);

        let actions = row![
            button(text("New Catalog...").size(TEXT_LG))
                .on_press(Msg::ShowNewCatalogModal)
                .style(ghost_btn_style),
            button(text("Browse...").size(TEXT_LG))
                .on_press(Msg::PickOpenCatalog)
                .style(ghost_btn_style),
            Space::new().width(Length::Fill),
            {
                let btn = button(text("Open").size(TEXT_LG)).style(if can_open_selected {
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
        ]
        .spacing(SPACE_2_5)
        .width(Length::Fill)
        .align_y(Alignment::Center);

        let mut base = column![
            text("IsomFolio").size(TEXT_DISPLAY).color(FG),
            Space::new().height(SPACE_0_5),
            text("Photo library manager").size(TEXT_LG).color(FG_DIM),
            Space::new().height(SPACE_3),
            recent_section,
            Space::new().height(SPACE_2),
            actions,
        ]
        .align_x(Alignment::Start)
        .width(Length::Fill)
        .height(Length::Fill)
        .max_width(960);

        if !self.status.is_empty() {
            base = base
                .push(Space::new().height(SPACE_2_5))
                .push(text(&self.status).size(TEXT_MD).color(ERR));
        }

        let base_layer: Element<'_, Msg> = container(base)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([SPACE_3, SPACE_4])
            .align_x(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into();

        if !self.welcome.show_new_catalog_modal {
            return base_layer;
        }
        stack(vec![base_layer, self.new_catalog_modal_overlay()]).into()
    }

    /// The New Catalog modal as a self-contained overlay (backdrop + centered
    /// panel). Layered over whatever view is active — the welcome screen on
    /// first launch, or the current catalogue when invoked from the menu.
    pub(super) fn new_catalog_modal_overlay(&self) -> Element<'_, Msg> {
        let can_create = self.welcome.new_catalog_dir.is_some()
            && !self.welcome.new_catalog_name.trim().is_empty();

        let location_display = self
            .welcome.new_catalog_dir
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("Choose a folder for the new catalogue");

        let modal = container(
            column![
                text("New Catalog").size(TEXT_TITLE).color(FG),
                Space::new().height(SPACE_1_5),
                text("Create a catalogue in a chosen location.")
                    .size(TEXT_BASE)
                    .color(FG_DIM),
                Space::new().height(SPACE_4),
                text("Catalog name").size(TEXT_MD).color(FG_DIM),
                Space::new().height(SPACE_1_5),
                text_input("My Photos", &self.welcome.new_catalog_name)
                    .on_input(Msg::NewCatalogNameChanged)
                    .on_submit(Msg::ConfirmNewCatalog)
                    .padding([SPACE_2, SPACE_2_5])
                    .size(TEXT_BASE)
                    .width(Length::Fill),
                Space::new().height(SPACE_4),
                text("Location").size(TEXT_MD).color(FG_DIM),
                Space::new().height(SPACE_1_5),
                mouse_area(
                    container(text(location_display).size(TEXT_BASE).color(
                        if self.welcome.new_catalog_dir.is_some() { FG } else { FG_MUTED }
                    ))
                    .width(Length::Fill)
                    .padding([SPACE_2_5, SPACE_3])
                    .style(field_style),
                )
                .on_press(Msg::PickNewCatalogDir)
                .interaction(iced::mouse::Interaction::Pointer),
                Space::new().height(SPACE_4),
                row![
                    button(text("Cancel").size(TEXT_BASE))
                        .on_press(Msg::HideNewCatalogModal)
                        .style(ghost_btn_style),
                    {
                        let btn = button(text("Create Catalog").size(TEXT_BASE)).style(if can_create {
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

        let backdrop = mouse_area(
            container(Space::new())
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
                }),
        )
        .on_press(Msg::NoOp)
        .on_release(Msg::NoOp)
        .on_right_press(Msg::NoOp)
        .on_right_release(Msg::NoOp)
        .on_double_click(Msg::NoOp);

        let centered = container(modal)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        stack(vec![backdrop.into(), centered.into()]).into()
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
            _ => Color { a: 0.22, ..ACCENT },
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
