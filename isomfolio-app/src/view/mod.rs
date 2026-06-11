mod compare;
mod content;
mod context_menu;
mod icons;
mod loupe;
mod loupe_image;
mod menu_bar;
mod modals;
mod people;
mod resolve;
mod settings;
mod sidebar;
pub mod styles;
mod task_panel;
mod tag_browser;
mod welcome;

use iced::{
    widget::{button, column, container, image, mouse_area, row, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Shadow, Theme, Vector,
};

use crate::app::{
    App, DragPayload, DropTarget, Msg, SidebarItem, ViewMode, SIDEBAR_HANDLE_WIDTH,
};
use styles::{
    active_chip_style, danger_btn_style, ghost_btn_style, ACCENT,
    BG_MODAL, BG_PANEL, BG_PROGRESS_TRACK, BG_STATUSBAR, BORDER, ERR, FG, FG_DIM, FG_MUTED,
    OVERLAY_HEAVY, OVERLAY_LIGHT, OVERLAY_MEDIUM, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2,
    SPACE_2_5, SPACE_3, SPACE_4, SPACE_6, STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM,
    TEXT_STAR, TEXT_TITLE, TEXT_XS, WARN,
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

        if matches!(self.view_mode, ViewMode::ResolveStacks) {
            return self.view_resolve();
        }

        let status = if let Some(pressed) = self.drag.dragging_group() {
            let name = self
                .groups
                .iter()
                .find(|g| &g.id == pressed)
                .map(|g| g.name.as_str())
                .unwrap_or("group");
            // Target a different group as the new parent; hovering itself is a no-op.
            let target = match &self.drag.hover {
                Some(DropTarget::Group(gid)) if gid != pressed => {
                    self.groups.iter().find(|g| &g.id == gid).map(|g| g.name.as_str())
                }
                _ => None,
            };
            match target {
                Some(into) => format!("Nesting \"{name}\" inside \"{into}\""),
                None => format!("Dragging \"{name}\" — drop on a group to nest, or release to keep"),
            }
        } else if let Some(pressed) = self.drag.dragging_album() {
            let count = if self.selected_albums.len() > 1 && self.selected_albums.contains(pressed) {
                self.selected_albums.len()
            } else {
                1
            };
            let target = match &self.drag.hover {
                Some(DropTarget::Group(sid)) => {
                    self.groups.iter().find(|s| &s.id == sid).map(|s| s.name.as_str())
                }
                _ => None,
            };
            match target {
                Some(name) => format!("Dragging {count} album(s) — drop on \"{name}\""),
                None => format!("Dragging {count} album(s) — drop on a group"),
            }
        } else if self.drag.dragging_photos() {
            let count = self.drag.photo_ids().map_or(0, |ids| ids.len());
            let target = match &self.drag.hover {
                Some(DropTarget::Album(id)) => {
                    self.albums.iter().find(|a| &a.id == id).map(|a| a.name.as_str())
                }
                _ => None,
            };
            match target {
                Some(name) => format!("Dragging {count} — drop on \"{name}\""),
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
        let reject_count = self
            .files
            .iter()
            .filter(|f| f.flag == isomfolio_core::models::Flag::Reject)
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

        if self.reject_delete_pending {
            let n = self
                .files
                .iter()
                .filter(|f| f.flag == isomfolio_core::models::Flag::Reject)
                .count();
            status_row = status_row.push(
                row![
                    text(format!("Move {n} rejected photo(s) to Deleted?")).size(TEXT_MD).color(ERR),
                    button(text("Cancel").size(TEXT_MD))
                        .on_press(Msg::CancelDeleteRejects)
                        .style(ghost_btn_style),
                    button(text("Delete").size(TEXT_MD))
                        .on_press(Msg::ConfirmDeleteRejects)
                        .style(danger_btn_style),
                ]
                .spacing(SPACE_1_5)
                .align_y(Alignment::Center),
            );
        }

        if self.selected_item == SidebarItem::Deleted && !self.files.is_empty() {
            status_row = status_row.push(
                button(text("Empty Deleted…").size(TEXT_MD).color(ERR))
                    .on_press(Msg::RequestPurgeAll)
                    .style(ghost_btn_style),
            );
        }

        if pick_count > 0 {
            status_row = status_row.push(
                text(format!("✓ {pick_count}"))
                    .size(TEXT_MD)
                    .color(ACCENT),
            );
        }
        if reject_count > 0 {
            status_row = status_row.push(
                text(format!("✕ {reject_count}"))
                    .size(TEXT_MD)
                    .color(ERR),
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
        if self.welcome.show_new_catalog_modal {
            layers.push(self.new_catalog_modal_overlay());
        }
        if self.purge_pending.is_some() {
            layers.push(self.purge_confirm_overlay());
        }
        // A small count pill that follows the cursor while a drag is in flight —
        // the visual "something is being dragged" signal (iced has no native drag
        // image), uniform across photo and album payloads.
        if let Some(ghost) = self.drag_ghost() {
            layers.push(ghost);
        }
        // Always wrap in a `stack`, even with a single layer. Adding/removing an
        // overlay must not change the *type* of the root widget — if it flipped
        // between `column` and `stack`, iced would rebuild the whole tree and the
        // grid scrollable would lose its offset (jumping to the top whenever a
        // context menu or other overlay opens).
        let root: Element<Msg> = stack(layers).into();

        if resizing {
            mouse_area(root)
                .interaction(iced::mouse::Interaction::ResizingHorizontally)
                .into()
        } else {
            root
        }
    }

    /// A count pill pinned just below-right of the cursor while a real drag is in
    /// flight. iced has no native drag image, so this is faked as a passive
    /// overlay layer positioned by padding (it has no `mouse_area`, so it never
    /// captures events). One shape for every payload — photos, albums, or a group.
    fn drag_ghost(&self) -> Option<Element<'_, Msg>> {
        if !self.drag.is_active() {
            return None;
        }
        let label = match &self.drag.current.as_ref()?.payload {
            DragPayload::Photos { .. } => {
                let n = self.drag.photo_ids().map_or(0, |ids| ids.len());
                format!("{n} photo{}", if n == 1 { "" } else { "s" })
            }
            DragPayload::Albums { pressed } => {
                let n = if self.selected_albums.len() > 1 && self.selected_albums.contains(pressed) {
                    self.selected_albums.len()
                } else {
                    1
                };
                format!("{n} album{}", if n == 1 { "" } else { "s" })
            }
            DragPayload::Group { pressed } => self
                .groups
                .iter()
                .find(|g| g.id == *pressed)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "group".to_string()),
        };
        let pill = container(text(label).size(TEXT_SM).color(Color::WHITE))
            .padding([SPACE_0_5, SPACE_1_5])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(ACCENT)),
                border: Border { radius: 10.0.into(), ..Default::default() },
                shadow: Shadow {
                    color: Color { a: 0.4, ..Color::BLACK },
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 4.0,
                },
                ..Default::default()
            });
        // Offset from the hotspot so the pill trails the pointer, not under it.
        let x = self.cursor.x + 14.0;
        let y = self.cursor.y + 10.0;
        Some(
            column![
                Space::new().height(Length::Fixed(y)),
                row![Space::new().width(Length::Fixed(x)), pill],
            ]
            .into(),
        )
    }
}


