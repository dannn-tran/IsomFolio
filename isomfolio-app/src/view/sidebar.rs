use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, text, text_input, tooltip, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::folder_tree::FolderNode;
use isomfolio_core::models::AlbumKind;

use super::styles::{
    confirm_action_row, ghost_btn_style, icon_btn_style, sidebar_divider, ACCENT, ALBUM_HOVER,
    BG_SIDEBAR, BG_STATUSBAR, FG, FG_DIM, FG_MUTED,
    SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_3,
    TEXT_BASE, TEXT_MD, TEXT_SM,
};
use crate::app::{App, Msg, SidebarItem, ViewMode, ALBUM_ITEM_HEIGHT, FOLDER_ITEM_HEIGHT};

impl App {
    pub(super) fn view_sidebar(&self) -> Element<'_, Msg> {
        let drag_hover = self.drag.hover_album.clone();

        let catalog_name = std::path::Path::new(&self.catalog_dir)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Catalog");

        let catalog_header: Element<Msg> = row![text(catalog_name).size(TEXT_MD).color(FG_DIM),]
            .align_y(Alignment::Center)
            .into();

        let albums_header: Element<Msg> = row![
            text("Albums").size(TEXT_MD).color(FG_DIM),
            Space::new().width(Length::Fill),
            super::styles::tip(
                button(text("⚡").size(TEXT_MD))
                    .on_press(Msg::ToggleFilterPanel)
                    .style(icon_btn_style),
                "Filter / save Smart Album",
                super::styles::TipPos::Bottom,
            ),
            super::styles::tip(
                button(text("+").size(TEXT_BASE))
                    .on_press(Msg::StartCreateAlbum)
                    .style(icon_btn_style),
                "New album",
                super::styles::TipPos::Bottom,
            ),
        ]
        .spacing(SPACE_0_5)
        .align_y(Alignment::Center)
        .into();

        let is_sync_active = self.is_syncing || self.sync_pending;
        let mut folders_header_row = row![
            text("Folders").size(TEXT_MD).color(FG_DIM),
        ];
        if is_sync_active {
            folders_header_row = folders_header_row
                .push(Space::new().width(SPACE_1))
                .push(text("Syncing…").size(TEXT_MD).color(FG_DIM));
        }
        let folders_header: Element<Msg> = folders_header_row
            .push(Space::new().width(Length::Fill))
            .push(super::styles::tip(
                button(text("+").size(TEXT_BASE))
                    .on_press(if is_sync_active { Msg::NoOp } else { Msg::SyncPickFolder })
                    .style(icon_btn_style),
                "Add folder to library",
                super::styles::TipPos::Bottom,
            ))
            .align_y(Alignment::Center)
            .into();

        // Estimate max label chars that fit the current sidebar width.
        // Accounts for scroll padding (SPACE_3 × 2) + container padding (SPACE_1 × 2),
        // ~28px for the muted count suffix, and a small margin so the "…" reliably fits
        // before the clip boundary. Divisor 8.5 is conservative (system font ~7–8px/char).
        let available_px = self.sidebar_width - (SPACE_3 + SPACE_1) * 2.0 - 28.0;
        let max_chars = ((available_px / 8.5).floor() as usize).max(4);

        let mut content = column![
            catalog_header,
            Space::new().height(SPACE_1_5),
        ]
        .spacing(SPACE_0_5);

        content = content.push(folders_header);

        let mut folder_rows: Vec<Element<Msg>> = Vec::new();
        for node in &self.folder_tree {
            self.collect_folder_rows(node, 0, max_chars, &mut folder_rows);
        }
        for r in folder_rows {
            content = content.push(r);
        }

        content = content
            .push(Space::new().height(SPACE_1))
            .push(sidebar_divider())
            .push(Space::new().height(SPACE_1))
            .push(albums_header);

        if let Some(ref input_val) = self.create_album_input {
            content = content.push(
                container(
                    row![
                        text_input("Album name…", input_val)
                            .on_input(Msg::CreateAlbumInputChanged)
                            .on_submit(Msg::ConfirmCreateAlbum)
                            .padding([SPACE_1_5, SPACE_2])
                            .size(TEXT_BASE)
                            .width(Length::Fill),
                        button(text("✓").size(TEXT_SM).color(FG))
                            .on_press(Msg::ConfirmCreateAlbum)
                            .style(ghost_btn_style),
                        button(text("✕").size(TEXT_SM).color(FG_DIM))
                            .on_press(Msg::EscapePressed)
                            .style(ghost_btn_style),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .height(ALBUM_ITEM_HEIGHT)
                .align_y(Alignment::Center)
                .padding([0.0, SPACE_1]),
            );
        }

        for album in &self.albums {
            let sel = self.selected_item == SidebarItem::Album(album.id.clone());
            let hovered = drag_hover.as_deref() == Some(album.id.as_str());
            let count = self.album_counts.get(&album.id).copied().unwrap_or(0);
            let is_smart = matches!(album.kind, AlbumKind::Smart(_));

            if self.album_pending_delete.as_deref() == Some(album.id.as_str()) {
                content = content.push(confirm_action_row(
                    format!("Delete \"{}\"?", album.name),
                    Msg::DeleteAlbum(album.id.clone()),
                    Msg::CancelDeleteAlbum,
                ));
            } else if self.rename_album_id.as_deref() == Some(album.id.as_str()) {
                content = content.push(
                    container(
                        row![
                            text_input(&album.name, &self.rename_album_input)
                                .on_input(Msg::RenameAlbumInputChanged)
                                .on_submit(Msg::ConfirmRenameAlbum)
                                .padding([SPACE_1_5, SPACE_2])
                                .size(TEXT_BASE)
                                .width(Length::Fill),
                            button(text("✓").size(TEXT_SM).color(FG))
                                .on_press(Msg::ConfirmRenameAlbum)
                                .style(ghost_btn_style),
                            button(text("✕").size(TEXT_SM).color(FG_DIM))
                                .on_press(Msg::EscapePressed)
                                .style(ghost_btn_style),
                        ]
                        .spacing(SPACE_1)
                        .align_y(Alignment::Center),
                    )
                    .height(ALBUM_ITEM_HEIGHT)
                    .align_y(Alignment::Center)
                    .padding([0.0, SPACE_1]),
                );
            } else {
                let dirty = sel && is_smart && self.smart_album_dirty;
                let is_target = self.target_album.as_deref() == Some(album.id.as_str());
                content = content.push(album_sidebar_row(
                    album.name.clone(),
                    album.id.clone(),
                    count,
                    sel,
                    hovered,
                    is_smart,
                    dirty,
                    is_target,
                    max_chars,
                ));
            }
        }

        // People section
        if !self.faces.clusters.is_empty() || self.inference_manifest.is_some() {
            let count = self.faces.clusters.len();
            let count_label = if count > 0 { format!(" ({count})") } else { String::new() };
            let is_active = matches!(self.view_mode, ViewMode::People);
            let people_header: Element<Msg> = row![
                button(
                    text(format!("People{count_label}")).size(TEXT_MD).color(if is_active { FG } else { FG_DIM })
                )
                    .on_press(Msg::OpenPeopleView)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
                super::styles::tip(
                    button(text("⟳").size(TEXT_MD))
                        .on_press(Msg::RunFaceClustering { force_full: true })
                        .style(icon_btn_style),
                    "Re-cluster all faces",
                    super::styles::TipPos::Bottom,
                ),
            ]
            .spacing(SPACE_0_5)
            .align_y(Alignment::Center)
            .into();

            content = content
                .push(Space::new().height(SPACE_1))
                .push(sidebar_divider())
                .push(Space::new().height(SPACE_1))
                .push(people_header);
        }

        // Virtual "Deleted" folder — appears only when something is soft-deleted.
        if self.deleted_count > 0 {
            let sel = self.selected_item == SidebarItem::Deleted;
            content = content
                .push(Space::new().height(SPACE_1))
                .push(sidebar_divider())
                .push(Space::new().height(SPACE_1))
                .push(
                    button(
                        text(format!("Deleted ({})", self.deleted_count))
                            .size(TEXT_MD)
                            .color(if sel { FG } else { FG_DIM }),
                    )
                    .on_press(Msg::SidebarItemClicked(SidebarItem::Deleted))
                    .style(ghost_btn_style)
                    .width(Length::Fill),
                );
        }

        let sidebar_scroll = scrollable(content.spacing(SPACE_0_5).padding(SPACE_3))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(4).scroller_width(4),
            ))
            .on_scroll(|vp| Msg::SidebarScrolled(vp.absolute_offset().y))
            .height(Length::Fill);

        let bottom_strip = column![
            sidebar_divider(),
            container(
                button(text("Open Catalog…").size(TEXT_MD))
                    .on_press(Msg::PickOpenCatalog)
                    .style(ghost_btn_style)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([SPACE_1_5, SPACE_3])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                ..Default::default()
            }),
        ];

        container(column![sidebar_scroll, bottom_strip])
            .width(self.sidebar_width)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_SIDEBAR)),
                ..Default::default()
            })
            .into()
    }

    /// Append `node` and (if expanded) its descendants to `out` as indented rows.
    fn collect_folder_rows<'a>(
        &'a self,
        node: &'a FolderNode,
        depth: usize,
        max_chars: usize,
        out: &mut Vec<Element<'a, Msg>>,
    ) {
        let path = node.path.as_str();
        if self.folder_pending_remove.as_deref() == Some(path) {
            out.push(confirm_action_row(
                "Remove folder? Indexed data deleted.".to_string(),
                Msg::RemoveFolder(node.path.clone()),
                Msg::CancelRemoveFolder,
            ));
        } else if self.remove_missing_folder.as_deref() == Some(path) {
            let n = self.files.iter().filter(|f| f.is_orphaned
                && (f.folder == node.path || f.folder.starts_with(&format!("{path}/")))).count();
            out.push(confirm_action_row(
                format!("Remove {n} missing from catalog?"),
                Msg::ConfirmRemoveMissing,
                Msg::CancelRemoveMissing,
            ));
        } else {
            let sel = self.selected_item == SidebarItem::Folder(node.path.clone());
            let expanded = self.expanded_folders.contains(&node.path);
            let dirty = self.folder_is_dirty(path);
            out.push(folder_tree_row(node, depth, sel, expanded, dirty, max_chars));
        }

        if !node.children.is_empty() && self.expanded_folders.contains(&node.path) {
            for child in &node.children {
                self.collect_folder_rows(child, depth + 1, max_chars, out);
            }
        }
    }

    /// A folder is dirty if the watcher flagged on-disk changes in it or any
    /// descendant since the last sync.
    fn folder_is_dirty(&self, path: &str) -> bool {
        let prefix = format!("{path}{}", std::path::MAIN_SEPARATOR);
        self.dirty_folders
            .iter()
            .any(|d| d == path || d.starts_with(&prefix))
    }
}

fn truncate_label(s: &str, max: usize) -> (String, bool) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        (s.to_string(), false)
    } else {
        (format!("{}…", chars[..max].iter().collect::<String>()), true)
    }
}

fn label_tooltip<'a>(full_name: String) -> Element<'a, Msg> {
    container(text(full_name).size(super::styles::TEXT_SM).color(super::styles::FG))
        .padding([super::styles::SPACE_1, super::styles::SPACE_1_5])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.12, g: 0.12, b: 0.15, a: 0.97 })),
            border: Border { color: super::styles::BORDER, width: 1.0, radius: 4.0.into() },
            ..Default::default()
        })
        .into()
}

/// Width of the chevron / its placeholder, so parent and leaf labels align.
const CHEVRON_W: f32 = 16.0;

fn folder_tree_row<'a>(
    node: &'a FolderNode,
    depth: usize,
    selected: bool,
    expanded: bool,
    dirty: bool,
    max_chars: usize,
) -> Element<'a, Msg> {
    let path = node.path.clone();
    let name = node.name.clone();
    let count = node.total_count;
    let has_children = !node.children.is_empty();

    let bg = if selected {
        Color {
            r: ACCENT.r * 0.6,
            g: ACCENT.g * 0.6,
            b: ACCENT.b * 0.6,
            a: 0.4,
        }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { ACCENT } else { Color::TRANSPARENT };
    let text_color = if selected { Color::WHITE } else { FG };

    // Indent deepens with tree depth; truncation budget shrinks to match.
    let indent = depth as f32 * SPACE_3;
    let effective_max = max_chars.saturating_sub(depth * 2 + 2).max(4);
    let (display_name, was_truncated) = truncate_label(&name, effective_max);

    let chevron: Element<Msg> = if has_children {
        button(text(if expanded { "▾" } else { "▸" }).size(TEXT_SM).color(FG_DIM))
            .on_press(Msg::ToggleFolderExpanded(path.clone()))
            .width(Length::Fixed(CHEVRON_W))
            .style(icon_btn_style)
            .into()
    } else {
        Space::new().width(CHEVRON_W).into()
    };

    let label_btn = button(
        row![
            container(
                text(display_name)
                    .size(TEXT_BASE)
                    .color(text_color)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            if dirty {
                text("●").size(TEXT_SM).color(ACCENT)
            } else {
                text("").size(TEXT_SM).color(FG_MUTED)
            },
            if count > 0 {
                text(format!(" {count}")).size(TEXT_SM).color(FG_MUTED)
            } else {
                text("").size(TEXT_SM).color(FG_MUTED)
            },
        ]
        .align_y(Alignment::Center),
    )
    .on_press(Msg::SidebarItemClicked(SidebarItem::Folder(path.clone())))
    .width(Length::Fill)
    .style(|_: &Theme, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: FG,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    });

    let inner = container(
        row![Space::new().width(indent), chevron, label_btn]
            .align_y(Alignment::Center),
    )
    .height(FOLDER_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0.0, SPACE_1])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border { color: border_color, width: 0.0, radius: 4.0.into() },
        ..Default::default()
    });

    let entity = SidebarItem::Folder(path.clone());
    let row_el = mouse_area(inner)
        .on_enter(Msg::HoverSidebarEntityStart(entity.clone()))
        .on_exit(Msg::HoverSidebarEntityEnd(entity));

    if was_truncated {
        tooltip(row_el, label_tooltip(name), tooltip::Position::Right).into()
    } else {
        row_el.into()
    }
}

fn album_sidebar_row<'a>(
    label: String,
    album_id: String,
    count: usize,
    selected: bool,
    drop_hover: bool,
    is_smart: bool,
    dirty: bool,
    is_target: bool,
    max_chars: usize,
) -> Element<'a, Msg> {
    let text_color = if selected || drop_hover {
        Color::WHITE
    } else {
        FG
    };
    let bg = if drop_hover {
        ALBUM_HOVER
    } else if selected {
        Color {
            r: ACCENT.r * 0.6,
            g: ACCENT.g * 0.6,
            b: ACCENT.b * 0.6,
            a: 0.4,
        }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if drop_hover || selected {
        ACCENT
    } else {
        Color::TRANSPARENT
    };

    let smart_indicator = if is_smart { "⚡ " } else { "" };
    let target_indicator = if is_target { "◎ " } else { "" };
    let (display_label, was_truncated) = truncate_label(&label, max_chars);
    let dirty_dot = if dirty { " ●" } else { "" };
    let count_str = if count > 0 { format!("{dirty_dot} {count}") } else { dirty_dot.to_string() };
    let name_btn = button(
        row![
            container(
                text(format!("{target_indicator}{smart_indicator}{display_label}"))
                    .size(TEXT_BASE)
                    .color(text_color)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            text(count_str).size(TEXT_SM).color(FG_MUTED),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(Msg::SidebarItemClicked(SidebarItem::Album(
        album_id.clone(),
    )))
    .width(Length::Fill)
    .style(|_: &Theme, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: FG,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    });

    let inner = container(name_btn)
        .height(ALBUM_ITEM_HEIGHT)
        .align_y(Alignment::Center)
        .padding([0.0, SPACE_1])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: if drop_hover { 2.0 } else { 0.0 },
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let entity = SidebarItem::Album(album_id.clone());
    let row_el = mouse_area(inner)
        .on_enter(Msg::HoverSidebarEntityStart(entity.clone()))
        .on_exit(Msg::HoverSidebarEntityEnd(entity));

    if was_truncated {
        tooltip(row_el, label_tooltip(label), tooltip::Position::Right).into()
    } else {
        row_el.into()
    }
}
