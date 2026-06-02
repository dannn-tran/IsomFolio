use iced::{
    widget::{button, column, container, row, text, Space},
    Alignment, Background, Border, Color, Element, Length, Padding, Theme,
};

use isomfolio_core::models::AlbumKind;

use super::styles::{BG_MODAL, BORDER, ERR, FG, FG_DIM, HINT_HOVER, SPACE_1, SPACE_1_5, SPACE_2, TEXT_MD};
use crate::app::{App, ContextMenuTarget, ExportMode, Msg};

const MENU_WIDTH: f32 = 180.0;
const SUBMENU_WIDTH: f32 = 160.0;
const ITEM_HEIGHT: f32 = 32.0;
const SEPARATOR_HEIGHT: f32 = 9.0;

#[cfg(target_os = "macos")]
const REVEAL_LABEL: &str = "Show in Finder";
#[cfg(target_os = "windows")]
const REVEAL_LABEL: &str = "Show in Explorer";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const REVEAL_LABEL: &str = "Open Containing Folder";

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
            let submenu = match &cm.target {
                ContextMenuTarget::FaceCluster(source_id) => {
                    let others: Vec<_> = self.faces.clusters.iter()
                        .filter(|c| &c.cluster_id != source_id)
                        .collect();
                    if others.is_empty() {
                        column![text("No other people").size(TEXT_MD).color(FG_DIM)]
                            .padding([SPACE_1, SPACE_1_5])
                    } else {
                        let mut col = column![].spacing(0);
                        for cluster in &others {
                            let target = cluster.cluster_id.clone();
                            let src = source_id.clone();
                            let label = cluster
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("Unnamed ({})", cluster.file_count));
                            col = col.push(
                                button(text(label).size(TEXT_MD).color(FG))
                                    .on_press(Msg::MergeFaceClusters(target, src))
                                    .style(menu_item_style)
                                    .height(ITEM_HEIGHT)
                                    .width(Length::Fill),
                            );
                        }
                        col.padding([SPACE_1, 0.0])
                    }
                }
                _ => {
                    let manual_albums: Vec<_> = self
                        .albums
                        .iter()
                        .filter(|a| matches!(a.kind, AlbumKind::Manual))
                        .collect();
                    if manual_albums.is_empty() {
                        column![text("No albums yet").size(TEXT_MD).color(FG_DIM)]
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
                    }
                }
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
            ContextMenuTarget::Folder(path) => {
                let has_missing = self.files.iter().any(|f| f.is_orphaned
                    && (f.folder == *path || f.folder.starts_with(&format!("{path}/"))));
                let mut items = vec![
                    Some(("Sync Folder".into(), Msg::SyncFolder(path.clone()), false)),
                    None,
                    Some(("Remove from Library…".into(), Msg::RequestRemoveFolder(path.clone()), true)),
                ];
                if has_missing {
                    items.insert(1, Some(("Remove Missing Files…".into(), Msg::RequestRemoveMissing(path.clone()), true)));
                }
                items
            }
            ContextMenuTarget::ManualAlbum(id) => {
                let target_label = if self.target_album.as_deref() == Some(id.as_str()) {
                    "Clear Target Album"
                } else {
                    "Set as Target Album"
                };
                vec![
                    Some(("Rename".into(), Msg::StartRenameAlbum(id.clone()), false)),
                    Some(("Duplicate".into(), Msg::DuplicateAlbum(id.clone()), false)),
                    Some((target_label.into(), Msg::SetTargetAlbum(id.clone()), false)),
                    None,
                    Some(("Delete…".into(), Msg::RequestDeleteAlbum(id.clone()), true)),
                ]
            }
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
            ContextMenuTarget::FaceCluster(cluster_id) => {
                let id = cluster_id.clone();
                let mut items: Vec<Option<(String, Msg, bool)>> = vec![
                    Some(("Rename".into(), Msg::RenameFaceCluster(id.clone()), false)),
                ];
                let has_others = self.faces.clusters.iter().any(|c| &c.cluster_id != cluster_id);
                if has_others {
                    items.push(Some(("Merge into ▶".into(), Msg::ToggleAddToAlbumSubmenu, false)));
                }
                items
            }
            ContextMenuTarget::GridTiles => {
                // In the Deleted view: restore, or permanently delete from disk.
                if self.selected_item == crate::app::SidebarItem::Deleted {
                    return vec![
                        Some(("Restore".into(), Msg::RestoreSelection, false)),
                        None,
                        Some(("Delete Permanently…".into(), Msg::RequestPurgeSelected, true)),
                    ];
                }
                let n = self.grid_selected.len();
                let mut items: Vec<Option<(String, Msg, bool)>> = Vec::new();
                if n == 1 {
                    items.push(Some(("Open in Loupe".into(), Msg::OpenLoupe, false)));
                }
                items.push(Some(("Add to Album ▶".into(), Msg::ToggleAddToAlbumSubmenu, false)));
                items.push(Some(("Delete".into(), Msg::DeleteSelection, true)));

                items.push(None);
                items.push(Some(("Import XMP metadata".into(), Msg::SyncXmpForSelection, false)));
                if cfg!(target_os = "macos") {
                    items.push(Some(("Import Apple Finder tags".into(), Msg::SyncAppleTagsForSelection, false)));
                }

                let has_non_orphaned = self
                    .grid_selected
                    .iter()
                    .filter_map(|id| self.files.iter().find(|f| &f.id == id))
                    .any(|f| !f.is_orphaned);

                if n == 1 {
                    let selected_file = self
                        .grid_selected
                        .iter()
                        .next()
                        .and_then(|id| self.files.iter().find(|f| &f.id == id));
                    if let Some(f) = selected_file {
                        if f.is_orphaned {
                            items.push(Some(("Locate…".into(), Msg::LocateFile(f.id.clone()), false)));
                        } else {
                            items.push(Some((REVEAL_LABEL.into(), Msg::ShowInFinder(vec![f.path.clone()]), false)));
                        }
                    }
                } else if has_non_orphaned {
                    let paths: Vec<String> = self
                        .grid_selected
                        .iter()
                        .filter_map(|id| self.files.iter().find(|f| &f.id == id))
                        .filter(|f| !f.is_orphaned)
                        .map(|f| f.path.clone())
                        .collect();
                    items.push(Some((REVEAL_LABEL.into(), Msg::ShowInFinder(paths), false)));
                }

                if has_non_orphaned {
                    items.push(Some(("Copy to Folder…".into(), Msg::ExportSelectionToDialog(ExportMode::Copy), false)));
                    items.push(Some(("Move to Folder…".into(), Msg::ExportSelectionToDialog(ExportMode::Move), false)));
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
        button::Status::Hovered | button::Status::Pressed => HINT_HOVER,
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
