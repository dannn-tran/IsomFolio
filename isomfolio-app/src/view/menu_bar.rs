use iced::{
    widget::{button, column, container, mouse_area, row, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use super::styles::{
    icon_btn_style, BG_MODAL, BG_STATUSBAR, BORDER, FG, FG_DIM, FG_MUTED, HINT_HOVER, HINT_SUBTLE,
    SPACE_1, SPACE_1_5, SPACE_2, TEXT_MD, TEXT_SM,
};
use crate::app::{App, Msg};

const MENU_ITEM_HEIGHT: f32 = 30.0;
const DROPDOWN_WIDTH: f32 = 220.0;
pub(super) const MENU_BAR_HEIGHT: f32 = 26.0;

struct MenuTab {
    label: &'static str,
    id: &'static str,
    tab_width: f32,
}

const MENU_TABS: &[MenuTab] = &[
    MenuTab { label: "Catalog", id: "catalog", tab_width: 72.0 },
    MenuTab { label: "Edit",    id: "edit",    tab_width: 52.0 },
    MenuTab { label: "View",    id: "view",    tab_width: 56.0 },
];

fn tab_left_edge(menu_id: &str) -> f32 {
    MENU_TABS
        .iter()
        .take_while(|tab| tab.id != menu_id)
        .map(|tab| tab.tab_width)
        .sum()
}

impl App {
    pub(super) fn view_menu_bar(&self) -> Element<'_, Msg> {
        let mut bar = row![].spacing(0).align_y(Alignment::Center);

        for tab in MENU_TABS {
            let is_open = self.open_menu.as_deref() == Some(tab.id);
            let id_owned = tab.id.to_string();
            let tab_button = button(
                text(tab.label)
                    .size(TEXT_MD)
                    .color(if is_open { FG } else { FG_DIM })
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
            )
            .on_press(Msg::OpenMenuDropdown(id_owned.clone()))
            .style(move |_: &Theme, status| {
                let bg = match (is_open, status) {
                    (true, _) => HINT_HOVER,
                    (_, iced::widget::button::Status::Hovered) => HINT_SUBTLE,
                    _ => Color::TRANSPARENT,
                };
                iced::widget::button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: FG,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            })
            .padding([SPACE_1, SPACE_1_5])
            .width(tab.tab_width)
            .height(MENU_BAR_HEIGHT);

            bar = bar.push(
                mouse_area(tab_button).on_enter(Msg::HoverMenuTab(id_owned)),
            );
        }

        bar = bar.push(Space::new().width(Length::Fill));
        bar = bar.push(
            button(text("?").size(TEXT_MD).color(FG_DIM))
                .on_press(Msg::ToggleShortcutHelp)
                .style(icon_btn_style)
                .padding([SPACE_1, SPACE_1_5])
                .height(MENU_BAR_HEIGHT),
        );
        bar = bar.push(
            button(text("⚙").size(TEXT_MD).color(FG_DIM))
                .on_press(Msg::OpenSettings)
                .style(icon_btn_style)
                .padding([SPACE_1, SPACE_1_5])
                .height(MENU_BAR_HEIGHT),
        );
        bar = bar.push(Space::new().width(SPACE_1));

        container(bar)
            .padding(0.0)
            .width(Length::Fill)
            .height(MENU_BAR_HEIGHT)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                border: Border {
                    color: BORDER,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    pub(super) fn view_menu_dropdown(&self) -> Option<Element<'_, Msg>> {
        let menu_id = self.open_menu.as_deref()?;
        let items = match menu_id {
            "catalog" => self.catalog_menu_items(),
            "edit" => self.edit_menu_items(),
            "view" => self.view_menu_items(),
            _ => return None,
        };

        let offset_x = tab_left_edge(menu_id);

        let mut col = column![].spacing(0).padding([SPACE_1, 0.0]);
        for item in items {
            match item {
                MenuItem::Action(label, shortcut, msg) => {
                    col = col.push(menu_action_row(label, shortcut, msg));
                }
                MenuItem::Separator => {
                    col = col.push(Space::new().height(SPACE_1));
                    col = col.push(
                        container(Space::new())
                            .width(Length::Fill)
                            .height(1.0)
                            .padding([0.0, SPACE_1_5])
                            .style(|_: &Theme| container::Style {
                                background: Some(Background::Color(BORDER)),
                                ..Default::default()
                            }),
                    );
                    col = col.push(Space::new().height(SPACE_1));
                }
            }
        }

        let dropdown = container(col)
            .width(DROPDOWN_WIDTH)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                ..Default::default()
            });

        let positioned_dropdown = container(dropdown).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: offset_x,
        });

        let below_bar = mouse_area(
            container(positioned_dropdown)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_: &Theme| container::Style::default()),
        )
        .on_press(Msg::CloseMenuDropdown);

        let overlay = column![
            Space::new().height(MENU_BAR_HEIGHT),
            below_bar,
        ];

        Some(overlay.into())
    }

    fn catalog_menu_items(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::Action("New Catalog…", "", Msg::ShowNewCatalogModal),
            MenuItem::Action("Open Catalog…", "", Msg::PickOpenCatalog),
        ]
    }

    fn view_menu_items(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::Action("Toggle Info Panel", "I", Msg::ToggleDetail),
            MenuItem::Action("Toggle Filters", "F", Msg::ToggleFilterPanel),
            MenuItem::Action("Preview", "E", Msg::TogglePreview),
            MenuItem::Action("Loupe", "Space", Msg::OpenLoupe),
            MenuItem::Action("People", "", Msg::OpenPeopleView),
            MenuItem::Separator,
            MenuItem::Action("Zoom In", "Cmd+=", Msg::TileSizeUp),
            MenuItem::Action("Zoom Out", "Cmd+−", Msg::TileSizeDown),
            MenuItem::Separator,
            MenuItem::Action("Hide Rejects", "\\", Msg::ToggleHideRejects),
        ]
    }

    fn edit_menu_items(&self) -> Vec<MenuItem> {
        vec![
            MenuItem::Action("Undo", "Cmd+Z", Msg::Undo),
            MenuItem::Action("Redo", "Cmd+Shift+Z", Msg::Redo),
            MenuItem::Separator,
            MenuItem::Action("Move Rejects to Trash…", "", Msg::RequestMoveRejectsToTrash),
        ]
    }
}

enum MenuItem {
    Action(&'static str, &'static str, Msg),
    Separator,
}

fn menu_action_row<'a>(label: &'a str, shortcut: &'a str, msg: Msg) -> Element<'a, Msg> {
    let mut r = row![
        text(label).size(TEXT_SM).color(FG),
        Space::new().width(Length::Fill),
    ]
    .spacing(SPACE_2)
    .align_y(Alignment::Center);

    if !shortcut.is_empty() {
        r = r.push(text(shortcut).size(TEXT_SM).color(FG_MUTED));
    }

    button(r.padding([0.0, SPACE_1_5]))
        .on_press(msg)
        .style(|_: &Theme, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed => {
                    HINT_HOVER
                }
                _ => Color::TRANSPARENT,
            };
            iced::widget::button::Style {
                background: Some(Background::Color(bg)),
                text_color: FG,
                border: Border { radius: 4.0.into(), ..Default::default() },
                shadow: iced::Shadow::default(),
                snap: false,
            }
        })
        .height(MENU_ITEM_HEIGHT)
        .width(Length::Fill)
        .into()
}
