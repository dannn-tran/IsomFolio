use iced::{
    widget::{button, column, container, row, text, Space},
    Alignment, Background, Border, Color, Element, Length, Padding, Theme,
};

use isomfolio_core::models::AlbumKind;

use super::styles::{BG_MODAL, BORDER, ERR, FG, FG_DIM, SPACE_1, SPACE_1_5, SPACE_2, TEXT_MD};
use crate::app::{App, ContextMenuTarget, Msg};

const MENU_WIDTH: f32 = 180.0;
const SUBMENU_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 32.0;
const SEPARATOR_HEIGHT: f32 = 9.0;

impl App {
    pub(super) fn view_context_menu(&self) -> Option<Element<'_, Msg>> {
        let cm = self.context_menu.as_ref()?;
        let pos = cm.position;

        let items = self.context_menu_items(&cm.target);
        let menu_h: f32 = items.iter().fold(0.0, |acc, item| {
            acc + if item.is_none() { SEPARATOR_HEIGHT } else { ITEM_HEIGHT }
        }) + SPACE_2 * 2.0;

        let menu_col = self.build_menu_column(items);

        let mut menu_layers: Vec<Element<Msg>> = vec![menu_panel(menu_col, MENU_WIDTH)];

        if cm.submenu_open {
            let manual_albums: Vec<_> = self
                .albums
                .iter()
                .filter(|a| matches!(a.kind, AlbumKind::Manual))
                .collect();
            let submenu = if manual_albums.is_empty() {
                column![
                    text("No albums yet").size(TEXT_MD).color(FG_DIM)
                ]
                .padding([SPACE_1, SPACE_1_5])
            } else {
                let mut col = column![].spacing(0);
                for album in &manual_albums {
                    let id = album.id.clone();
                    col = col.push(
                        button(text(&album.name).size(TEXT_MD).color(FG))
                            .on_press(Msg::AddSelectionToAlbum(id))
                            .style(menu_item_style)
                            .height(ITEM_HEIGHT)
                            .width(Length::Fill),
                    );
                }
                col.padding([SPACE_1, 0.0])
            };
            menu_layers.push(
                container(submenu)
                    .width(SUBMENU_WIDTH)
                    .style(menu_panel_style)
                    .into(),
            );
        }

        let menu_with_submenu: Element<Msg> = if menu_layers.len() > 1 {
            row(menu_layers).spacing(SPACE_1).into()
        } else {
            menu_layers.remove(0)
        };

        let x = pos.x.min(self.viewport_width + self.sidebar_width - MENU_WIDTH - 8.0).max(4.0);
        let y = pos.y.min(self.viewport_height - menu_h - 4.0).max(4.0);

        let overlay = container(
            container(menu_with_submenu)
                .padding(Padding { top: y, right: 0.0, bottom: 0.0, left: x }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style::default());

        Some(overlay.into())
    }

    fn context_menu_items(&self, target: &ContextMenuTarget) -> Vec<Option<(String, Msg, bool)>> {
        match target {
            ContextMenuTarget::Folder(path) => vec![
                Some(("Rescan".into(), Msg::RescanFolder(path.clone()), false)),
                None,
                Some(("Remove from Library…".into(), Msg::RequestRemoveFolder(path.clone()), true)),
            ],
            ContextMenuTarget::ManualAlbum(id) => vec![
                Some(("Rename".into(), Msg::StartRenameAlbum(id.clone()), false)),
                Some(("Duplicate".into(), Msg::DuplicateAlbum(id.clone()), false)),
                None,
                Some(("Delete…".into(), Msg::RequestDeleteAlbum(id.clone()), true)),
            ],
            ContextMenuTarget::SmartAlbum(id) => vec![
                Some(("Rename".into(), Msg::StartRenameAlbum(id.clone()), false)),
                Some(("Duplicate".into(), Msg::DuplicateAlbum(id.clone()), false)),
                Some((
                    "Edit Criteria".into(),
                    Msg::SidebarItemClicked(crate::app::SidebarItem::Album(id.clone())),
                    false,
                )),
                None,
                Some(("Delete…".into(), Msg::RequestDeleteAlbum(id.clone()), true)),
            ],
            ContextMenuTarget::GridTiles => {
                let n = self.grid_selected.len();
                let mut items: Vec<Option<(String, Msg, bool)>> = Vec::new();
                if n == 1 {
                    items.push(Some(("Open in Loupe".into(), Msg::OpenLoupe, false)));
                }
                items.push(Some(("Add to Album ▶".into(), Msg::ToggleAddToAlbumSubmenu, false)));

                let classify_addons: Vec<(usize, &str)> = self
                    .addons
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.manifest.capabilities.iter().any(|c| c == "classify"))
                    .map(|(i, a)| (i, a.manifest.name.as_str()))
                    .collect();

                if !classify_addons.is_empty() {
                    items.push(None);
                    let file_ids: Vec<String> = self.grid_selected.iter().cloned().collect();
                    let multiple = classify_addons.len() > 1;
                    for (idx, name) in classify_addons {
                        let label = if multiple {
                            format!("Auto-tag with {name}")
                        } else {
                            "Auto-tag".into()
                        };
                        items.push(Some((
                            label,
                            Msg::RunAddon {
                                addon_idx: idx,
                                method: "classify".to_string(),
                                file_ids: file_ids.clone(),
                            },
                            false,
                        )));
                    }
                }

                if n == 1 {
                    let path = self
                        .grid_selected
                        .iter()
                        .next()
                        .and_then(|id| self.files.iter().find(|f| &f.id == id))
                        .map(|f| f.path.clone())
                        .unwrap_or_default();
                    items.push(None);
                    items.push(Some(("Show in Finder".into(), Msg::ShowInFinder(path), false)));
                }
                items
            }
        }
    }

    fn build_menu_column(&self, items: Vec<Option<(String, Msg, bool)>>) -> Element<'_, Msg> {
        let mut col = column![].spacing(0).padding([SPACE_1, 0.0]);
        for item in items {
            match item {
                None => {
                    col = col.push(
                        container(Space::new())
                            .width(Length::Fill)
                            .height(1.0)
                            .padding([SPACE_1, SPACE_1_5])
                            .style(|_: &Theme| container::Style {
                                background: Some(Background::Color(BORDER)),
                                ..Default::default()
                            }),
                    );
                    col = col.push(Space::new().height(SPACE_1));
                }
                Some((label, msg, is_destructive)) => {
                    let color = if is_destructive { ERR } else { FG };
                    col = col.push(
                        button(
                            row![text(label).size(TEXT_MD).color(color)]
                                .padding([0.0, SPACE_1_5])
                                .align_y(Alignment::Center),
                        )
                        .on_press(msg.clone())
                        .style(menu_item_style)
                        .height(ITEM_HEIGHT)
                        .width(Length::Fill),
                    );
                }
            }
        }
        col.into()
    }
}

fn menu_panel<'a>(content: Element<'a, Msg>, width: f32) -> Element<'a, Msg> {
    container(content).width(width).style(menu_panel_style).into()
}

fn menu_panel_style(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_MODAL)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn menu_item_style(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 }
        }
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: FG,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}
