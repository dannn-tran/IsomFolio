use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, text, text_input, tooltip, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::folder_tree::FolderNode;
use isomfolio_core::models::{Album, AlbumKind, Group};

use super::styles::{
    confirm_action_row, ghost_btn_style,
    sidebar_divider, ACCENT, ALBUM_HOVER, BG_SIDEBAR, BG_STATUSBAR, FG, FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_3, TEXT_BASE, TEXT_MD, TEXT_SM, TEXT_XS,
};
use super::icons::{Icon, ICON_SIZE};
use crate::app::{
    unix_to_date_str, App, DropTarget, Msg, SidebarItem,
    SidebarSection, ViewMode, ALBUM_ITEM_HEIGHT, FOLDER_ITEM_HEIGHT,
};
use isomfolio_core::models::AlbumId;

/// How many recent import batches the sidebar shows before "Show all".
const IMPORTS_COLLAPSED: usize = 10;

/// A section-collapse chevron, shown at the *trailing* (right) edge of a section
/// header — the disclosure convention (▾ expanded / ▸ collapsed). Kept off the
/// leading edge so every section's icon shares one column with the nav-row icons.
/// Separate control: toggling collapse never changes selection.
fn section_chevron<'a>(collapsed: bool, section: SidebarSection) -> Element<'a, Msg> {
    super::styles::icon_btn_svg(
        if collapsed { Icon::ChevronRight } else { Icon::ChevronDown },
        Msg::ToggleSidebarSection(section),
    )
    .into()
}

/// Build a section header that aligns with the nav rows: `[icon] Title …
/// [actions] [collapse-chevron]`, wrapped in the same horizontal padding the nav
/// rows use so all leading icons share one column.
fn section_header<'a>(
    icon: Icon,
    title: &'a str,
    collapsed: bool,
    section: SidebarSection,
    trailing: Vec<Element<'a, Msg>>,
) -> Element<'a, Msg> {
    // The whole header band (icon · label · gap) toggles collapse — the obvious
    // target is the section name, not only the far-right chevron (Fitts). Action
    // glyphs and the chevron sit *outside* this hit area as their own controls,
    // so clicking `+` adds rather than collapsing. The chevron stays as the
    // glanceable open/closed indicator (redundant control, not chrome).
    let label_area: Element<Msg> = mouse_area(
        row![
            super::icons::icon(icon, FG_DIM),
            text(title).size(TEXT_MD).color(FG_DIM),
            Space::new().width(Length::Fill),
        ]
        .spacing(SPACE_1_5)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .on_press(Msg::ToggleSidebarSection(section))
    .into();

    let mut r = row![label_area].spacing(SPACE_1_5).align_y(Alignment::Center);
    for el in trailing {
        r = r.push(el);
    }
    container(r.push(section_chevron(collapsed, section)))
        .padding([0.0, SPACE_1])
        .into()
}

// Criteria-panel layout primitives. Every filter row is a fixed-width label
// column + a controls block, so labels and controls line up in two columns
// instead of each row finding its own shape.

impl App {
    pub(super) fn view_sidebar(&self) -> Element<'_, Msg> {
        // The album under the cursor as a drop target — only while a photo drag is
        // live (album rows light up for photo drops, groups for album drops).
        let drag_hover: Option<AlbumId> = match &self.drag.hover {
            Some(DropTarget::Album(id)) if self.drag.dragging_photos() => Some(id.clone()),
            _ => None,
        };

        let catalog_name = std::path::Path::new(&self.catalog_dir)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Catalog");

        let catalog_header: Element<Msg> = row![text(catalog_name).size(TEXT_MD).color(FG_DIM),]
            .align_y(Alignment::Center)
            .into();

        let filters_collapsed = self.collapsed_sections.contains(&SidebarSection::Filters);
        let folders_collapsed = self.collapsed_sections.contains(&SidebarSection::Folders);
        let albums_collapsed = self.collapsed_sections.contains(&SidebarSection::Albums);
        let imports_collapsed = self.collapsed_sections.contains(&SidebarSection::Imports);

        let filters_active = self.has_active_filters();
        let filter_title = if filters_active { "Filters ●" } else { "Filters" };
        let filters_header: Element<Msg> = section_header(
            Icon::Filters,
            filter_title,
            filters_collapsed,
            SidebarSection::Filters,
            vec![],
        );

        let albums_header: Element<Msg> = section_header(
            Icon::Albums,
            "Albums",
            albums_collapsed,
            SidebarSection::Albums,
            vec![
                // One add affordance → a small menu (New Album / New Group). Two
                // near-identical plus glyphs read as ambiguous; a single labelled
                // menu is unambiguous and the standard collections pattern.
                super::styles::tip(
                    super::styles::icon_btn_svg(Icon::Plus, Msg::OpenAlbumsAddMenu),
                    "Add album or group",
                    super::styles::TipPos::Bottom,
                ),
            ],
        );

        let is_sync_active = self.is_syncing || self.sync_pending;
        let mut folders_trailing: Vec<Element<Msg>> = Vec::new();
        if is_sync_active {
            folders_trailing.push(text("Syncing…").size(TEXT_SM).color(FG_DIM).into());
        }
        folders_trailing.push(super::styles::tip(
            super::styles::icon_btn_svg(
                Icon::Plus,
                if is_sync_active { Msg::NoOp } else { Msg::SyncPickFolder },
            ),
            "Add folder to library",
            super::styles::TipPos::Bottom,
        ));
        let folders_header: Element<Msg> = section_header(
            Icon::Folders,
            "Folders",
            folders_collapsed,
            SidebarSection::Folders,
            folders_trailing,
        );

        // Estimate max label chars that fit the current sidebar width.
        // Accounts for scroll padding (SPACE_3 × 2) + container padding (SPACE_1 × 2),
        // ~28px for the muted count suffix, and a small margin so the "…" reliably fits
        // before the clip boundary. Divisor 8.5 is conservative (system font ~7–8px/char).
        let available_px = self.sidebar_width - (SPACE_3 + SPACE_1) * 2.0 - 28.0;
        let max_chars = ((available_px / 8.5).floor() as usize).max(4);

        // "All Photos" — the catalog-level home (default selection). Without a
        // visible row there is no control to return to the whole catalog after
        // picking a folder/album. Count = every indexed non-deleted file, i.e.
        // the sum of the folder-tree roots.
        let total_files: usize = self.folder_tree.iter().map(|n| n.total_count).sum();
        let all_sel = self.selected_item == SidebarItem::AllFiles;

        // Navigation only — "where to look". Filters moved out to a panel pinned
        // at the sidebar bottom (built below), so the criteria controls never
        // bury the folder/album the user is actually navigating to.
        let mut content = column![
            catalog_header,
            Space::new().height(SPACE_1),
        ]
        .spacing(SPACE_0_5);

        content = content.push(nav_row(
            Some(Icon::AllPhotos),
            "All Photos".to_string(),
            Some(total_files),
            all_sel,
            Msg::SidebarItemClicked(SidebarItem::AllFiles),
        ));
        content = content.push(Space::new().height(SPACE_1_5));

        content = content.push(folders_header);

        if !folders_collapsed {
            let mut folder_rows: Vec<Element<Msg>> = Vec::new();
            for node in &self.folder_tree {
                self.collect_folder_rows(node, 0, max_chars, &mut folder_rows);
            }
            for r in folder_rows {
                content = content.push(r);
            }
        }

        // Content sections (Folders, Albums, People) are separated by spacing
        // alone — the chevron'd header already signals a new group, so a rule
        // between each would be decorative chrome (design-system "quiet by
        // default"). One divider, below, fences off the system collections.
        content = content
            .push(Space::new().height(SPACE_2))
            .push(albums_header);

        // The new-album input renders at the top only for an ungrouped album;
        // when a group is the target it appears nested under that group (below).
        if self.pending_album_group.is_none() {
            if let Some(ref input_val) = self.create_album_input {
                content = content.push(create_album_input_row(input_val));
            }
        }

        // The new-group input renders at the top only for a top-level group; when
        // a parent is the target it appears nested under that group (in
        // `render_group_block`, mirroring nested album creation).
        if self.pending_group_parent.is_none() {
            if let Some(ref input_val) = self.create_group_input {
                content = content.push(create_group_input_row(input_val));
            }
        }

        if !albums_collapsed {
            // Top-level groups first (each recursively rendering its child groups
            // and albums), then the ungrouped albums at the very top level.
            for group in self.groups.iter().filter(|g| g.parent_id.is_none()) {
                content = content.push(self.render_group_block(group, drag_hover.as_deref(), max_chars, 0));
            }
            for album in self.albums.iter().filter(|a| a.group_id.is_none()) {
                content = content.push(self.render_album_row(album, drag_hover.as_deref(), max_chars, 0));
            }
        }

        // People — a single nav destination (Class-B nav row), not a list-of-
        // children section, so no chevron and no inline action glyph. The
        // re-cluster-all action lives in the Photo menu ("Re-cluster All Faces").
        if !self.faces.clusters.is_empty() || self.inference_manifest.is_some() {
            let count = self.faces.clusters.len();
            let unnamed = self.faces.clusters.iter().filter(|c| c.name.is_none()).count();
            let is_active = matches!(self.view_mode, ViewMode::People);
            content = content.push(Space::new().height(SPACE_2)).push(nav_row(
                Some(Icon::People),
                "People".to_string(),
                Some(count),
                is_active,
                Msg::OpenPeopleView,
            ));
            if unnamed > 0 {
                content = content.push(
                    container(
                        text(format!("{unnamed} unnamed"))
                            .size(TEXT_XS)
                            .color(FG_MUTED),
                    )
                    .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: ICON_SIZE + SPACE_1_5 + SPACE_1 }),
                );
            }
        }

        // System collections — generated by the app, not created by the user
        // (recent import batches, then the virtual Deleted bin). Grouped behind
        // a single divider that fences them off from the content sections above.
        let has_imports = !self.import_batches.is_empty();
        if has_imports || self.deleted_count > 0 {
            content = content
                .push(Space::new().height(SPACE_1_5))
                .push(sidebar_divider())
                .push(Space::new().height(SPACE_1));
        }

        // Imports — recent sync batches as discrete, stable views.
        if has_imports {
            let imports_header = section_header(
                Icon::Imports,
                "Imports",
                imports_collapsed,
                SidebarSection::Imports,
                Vec::new(),
            );
            content = content.push(imports_header);

            let shown = if imports_collapsed {
                0
            } else if self.show_all_imports {
                self.import_batches.len()
            } else {
                IMPORTS_COLLAPSED.min(self.import_batches.len())
            };
            for batch in self.import_batches.iter().take(shown) {
                let sel = self.selected_item == SidebarItem::Import(batch.id);
                content = content.push(nav_row(
                    None,
                    unix_to_date_str(batch.created_at_unix),
                    Some(batch.count),
                    sel,
                    Msg::SidebarItemClicked(SidebarItem::Import(batch.id)),
                ));
            }
            if !imports_collapsed && self.import_batches.len() > IMPORTS_COLLAPSED {
                let more_label = if self.show_all_imports {
                    "Show less".to_string()
                } else {
                    format!("Show all ({})", self.import_batches.len())
                };
                content = content.push(
                    button(text(more_label).size(TEXT_SM).color(FG_MUTED))
                        .on_press(Msg::ToggleShowAllImports)
                        .style(ghost_btn_style)
                        .width(Length::Fill),
                );
            }
        }

        // Virtual "Deleted" bin — appears only when something is soft-deleted.
        if self.deleted_count > 0 {
            let sel = self.selected_item == SidebarItem::Deleted;
            if has_imports {
                content = content.push(Space::new().height(SPACE_1));
            }
            content = content.push(nav_row(
                Some(Icon::Deleted),
                "Deleted".to_string(),
                Some(self.deleted_count),
                sel,
                Msg::SidebarItemClicked(SidebarItem::Deleted),
            ));
        }

        let search_bar = container(
            text_input("Search…", &self.search_text)
                .on_input(Msg::SearchChanged)
                .padding([SPACE_1, SPACE_1_5])
                .size(TEXT_MD)
                .width(Length::Fill),
        )
        .padding([SPACE_1_5, SPACE_1_5]);

        let sidebar_scroll = scrollable(content.spacing(SPACE_0_5).padding(SPACE_3))
            .id(crate::app::SIDEBAR_SCROLL_ID.clone())
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(4).scroller_width(4),
            ))
            .on_scroll(|vp| Msg::SidebarScrolled(vp.absolute_offset().y))
            .height(Length::FillPortion(2));

        // Filters panel pinned to the sidebar bottom — the lower of two stacked
        // panels (navigation above, filtering here). Always visible as a header
        // (with its `●` active marker), collapsed by default. Expanded it takes a
        // bounded share (FillPortion 1 vs the nav's 2) and scrolls internally, so
        // opening it squeezes — never hides — the navigation list above.
        let mut filters_footer = column![
            sidebar_divider(),
            container(filters_header).padding([0.0, SPACE_3]),
        ]
        .spacing(SPACE_0_5);
        if !filters_collapsed {
            filters_footer = filters_footer.push(
                scrollable(container(self.view_sidebar_filters()).padding([0.0, SPACE_3]))
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new().width(4).scroller_width(4),
                    ))
                    .height(Length::FillPortion(1)),
            );
        }

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

        container(column![search_bar, sidebar_divider(), sidebar_scroll, filters_footer, bottom_strip])
            .width(self.sidebar_width)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_SIDEBAR)),
                ..Default::default()
            })
            .into()
    }

    /// One album row in the Albums list, handling its inline delete-confirm and
    /// One group and (when expanded) its nested child groups and albums, rendered
    /// recursively. `depth` is the nesting level (0 = top); each level indents by
    /// one disclosure column so the tree reads like the folder tree. Child groups
    /// list before this group's own albums (folder-tree order).
    fn render_group_block<'a>(
        &'a self,
        group: &'a Group,
        drag_hover: Option<&str>,
        max_chars: usize,
        depth: usize,
    ) -> Element<'a, Msg> {
        let header: Element<Msg> = if depth > 0 {
            row![
                Space::new().width(CHEVRON_W * depth as f32),
                self.render_group_header(group, max_chars),
            ]
            .into()
        } else {
            self.render_group_header(group, max_chars)
        };

        let mut block = column![header];
        if !self.collapsed_groups.contains(&group.id) {
            for child in self
                .groups
                .iter()
                .filter(|g| g.parent_id.as_deref() == Some(group.id.as_str()))
            {
                block = block.push(self.render_group_block(child, drag_hover, max_chars, depth + 1));
            }
            // A new group being created inside this one: its inline input nests
            // among the child groups, indented one level deeper.
            if self.pending_group_parent.as_deref() == Some(group.id.as_str()) {
                if let Some(ref input_val) = self.create_group_input {
                    block = block.push(row![
                        Space::new().width(CHEVRON_W * (depth as f32 + 1.0)),
                        create_group_input_row(input_val),
                    ]);
                }
            }
            for album in self
                .albums
                .iter()
                .filter(|a| a.group_id.as_deref() == Some(group.id.as_str()))
            {
                block = block.push(self.render_album_row(album, drag_hover, max_chars, depth + 1));
            }
            // A new album being created under this group: its inline input sits
            // where the album will land (indented like its siblings).
            if self.pending_album_group.as_deref() == Some(group.id.as_str()) {
                if let Some(ref input_val) = self.create_album_input {
                    block = block.push(row![
                        Space::new().width(CHEVRON_W * (depth as f32 + 1.0)),
                        create_album_input_row(input_val),
                    ]);
                }
            }
        }

        // While a drag that targets a group is live (album being filed, or group
        // being nested), the whole expanded block is the drop zone — header and
        // nested rows — so a release anywhere over it lands here. Nested child
        // blocks mount their own zones inside this one; the deepest under the
        // cursor wins `drag.hover` (the innermost `on_enter` fires last).
        if self.drag.dragging_onto_group() {
            mouse_area(block)
                .on_enter(Msg::HoverDrop(Some(DropTarget::Group(group.id.clone()))))
                .on_exit(Msg::HoverDrop(None))
                .into()
        } else {
            block.into()
        }
    }

    /// rename states. `depth` is the album's nesting level (0 = ungrouped top
    /// level); each level indents by one disclosure column so the label sits
    /// under its group's glyph.
    fn render_album_row<'a>(
        &'a self,
        album: &'a Album,
        drag_hover: Option<&str>,
        max_chars: usize,
        depth: usize,
    ) -> Element<'a, Msg> {
        let sel = self.selected_item == SidebarItem::Album(album.id.clone());
        let hovered = drag_hover == Some(album.id.as_str());
        let count = self.album_counts.get(&album.id).copied().unwrap_or(0);
        let is_smart = matches!(album.kind, AlbumKind::Smart(_));

        let el: Element<Msg> = if self.album_pending_delete.as_deref() == Some(album.id.as_str()) {
            confirm_action_row(
                format!("Delete \"{}\"?", album.name),
                Msg::DeleteAlbum(album.id.clone()),
                Msg::CancelDeleteAlbum,
            )
        } else if self.rename_album_id.as_deref() == Some(album.id.as_str()) {
            container(
                row![
                    text_input(&album.name, &self.rename_album_input)
                        .id(crate::app::input_ids::rename_album())
                        .on_input(Msg::RenameAlbumInputChanged)
                        .on_submit(Msg::ConfirmRenameAlbum)
                        .padding([SPACE_1_5, SPACE_2])
                        .size(TEXT_BASE)
                        .width(Length::Fill),
                    super::styles::icon_btn("✓", Msg::ConfirmRenameAlbum),
                    super::styles::icon_btn("✕", Msg::EscapePressed),
                ]
                .spacing(SPACE_1)
                .align_y(Alignment::Center),
            )
            .height(ALBUM_ITEM_HEIGHT)
            .align_y(Alignment::Center)
            .padding([0.0, SPACE_1])
            .into()
        } else {
            let dirty = sel && is_smart && self.smart_album_dirty;
            let is_target = self.target_album.as_deref() == Some(album.id.as_str());
            let multi = self.selected_albums.contains(&album.id);
            album_sidebar_row(
                album.name.clone(),
                album.id.clone(),
                count,
                sel,
                hovered,
                multi,
                is_smart,
                dirty,
                is_target,
                max_chars,
            )
        };

        let row_el: Element<Msg> = if depth > 0 {
            // One disclosure column (CHEVRON_W) per nesting level puts the album
            // label under its group's glyph — the chevron-column nesting the
            // folder tree uses.
            row![Space::new().width(CHEVRON_W * depth as f32), el].into()
        } else {
            el
        };

        // Mount the photo drop-zone only while a photo drag is live and this is a
        // manual album (smart albums are criteria-defined, never drop targets).
        // Gating by payload means there's no cross-talk with album→group drags,
        // which mount group drop-zones instead.
        if self.drag.dragging_photos() && !is_smart {
            mouse_area(row_el)
                .on_enter(Msg::HoverDrop(Some(DropTarget::Album(album.id.clone()))))
                .on_exit(Msg::HoverDrop(None))
                .into()
        } else {
            row_el
        }
    }

    /// A group header row in the Albums list: a collapse chevron, the group glyph,
    /// its name, and the count of albums it holds. Right-click / Ctrl+Click opens
    /// its context menu (rename / delete). Renders inline rename / delete states.
    fn render_group_header<'a>(&'a self, group: &'a Group, max_chars: usize) -> Element<'a, Msg> {
        if self.group_pending_delete.as_deref() == Some(group.id.as_str()) {
            // Albums are kept (rehomed to Ungrouped) — `delete_group` sets their
            // group_id NULL, it doesn't cascade. Prompt stays short so it fits the
            // narrow sidebar alongside the buttons.
            return confirm_action_row(
                format!("Delete group \u{201C}{}\u{201D}?", group.name),
                Msg::DeleteGroup(group.id.clone()),
                Msg::CancelDeleteGroup,
            );
        }
        if self.rename_group_id.as_deref() == Some(group.id.as_str()) {
            return container(
                row![
                    text_input(&group.name, &self.rename_group_input)
                        .id(crate::app::input_ids::rename_group())
                        .on_input(Msg::RenameGroupInputChanged)
                        .on_submit(Msg::ConfirmRenameGroup)
                        .padding([SPACE_1_5, SPACE_2])
                        .size(TEXT_BASE)
                        .width(Length::Fill),
                    super::styles::icon_btn("✓", Msg::ConfirmRenameGroup),
                    super::styles::icon_btn("✕", Msg::EscapePressed),
                ]
                .spacing(SPACE_1)
                .align_y(Alignment::Center),
            )
            .height(ALBUM_ITEM_HEIGHT)
            .align_y(Alignment::Center)
            .padding([0.0, SPACE_1])
            .into();
        }

        let collapsed = self.collapsed_groups.contains(&group.id);
        let album_count = self.albums.iter().filter(|a| a.group_id.as_deref() == Some(group.id.as_str())).count();
        let (display_label, was_truncated) = truncate_label(&group.name, max_chars);

        // Same fixed chevron slot as the folder tree (CHEVRON_W) so a group header
        // and a folder row share one disclosure column — narrower than ICON_BTN,
        // which kept the group glyph from lining up with its nested albums.
        let chevron = super::styles::icon_btn_svg(
            if collapsed { Icon::ChevronRight } else { Icon::ChevronDown },
            Msg::ToggleGroupCollapsed(group.id.clone()),
        )
        .width(Length::Fixed(CHEVRON_W))
        .height(Length::Fixed(ALBUM_ITEM_HEIGHT));

        // Highlight as a drop target while an album or group is being dragged over
        // it. A group dragged onto itself is excluded — that's a no-op, not a nest.
        let drop_target = self.drag.dragging_onto_group()
            && self.drag.dragging_group() != Some(&group.id)
            && matches!(&self.drag.hover, Some(DropTarget::Group(id)) if id.as_str() == group.id.as_str());

        let label_area = mouse_area(
            row![
                super::icons::icon(Icon::Group, if drop_target { ACCENT } else { FG_DIM }),
                container(
                    text(display_label)
                        .size(TEXT_BASE)
                        .color(if drop_target { Color::WHITE } else { FG })
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
                text(if album_count > 0 { album_count.to_string() } else { String::new() })
                    .size(TEXT_SM)
                    .color(FG_MUTED),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .on_press(Msg::GroupHeaderPressed(group.id.clone()))
        .on_right_press(Msg::OpenGroupMenu(group.id.clone()));
        // Drop-target hover (HoverDrop) is owned by the whole expanded group block
        // during an album drag (see the Albums-section loop), not the header alone
        // — so a release anywhere over the group files the album.

        let inner = container(
            row![chevron, label_area].align_y(Alignment::Center),
        )
        .height(ALBUM_ITEM_HEIGHT)
        .align_y(Alignment::Center)
        .padding([0.0, SPACE_1])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(if drop_target { ALBUM_HOVER } else { Color::TRANSPARENT })),
            border: Border {
                color: if drop_target { ACCENT } else { Color::TRANSPARENT },
                width: if drop_target { 2.0 } else { 0.0 },
                radius: 6.0.into(),
            },
            ..Default::default()
        });

        if was_truncated {
            tooltip(inner, label_tooltip(group.name.clone()), tooltip::Position::Right).into()
        } else {
            inner.into()
        }
    }

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
            let selected_path = match &self.selected_item {
                SidebarItem::Folder(p) => Some(p.as_str()),
                _ => None,
            };
            let expanded = self.expanded_folders.contains(&node.path);
            let dirty = self.folder_is_dirty(path);
            let offline = self.is_offline_path(&node.path);
            out.push(folder_tree_row(node, depth, selected_path, expanded, dirty, offline, max_chars));
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

/// The inline "new album" name input row (name field + confirm / cancel), used
/// both at the top level (ungrouped) and nested under a target group.
fn create_album_input_row<'a>(input_val: &str) -> Element<'a, Msg> {
    container(
        row![
            text_input("Album name…", input_val)
                .id(crate::app::input_ids::create_album())
                .on_input(Msg::CreateAlbumInputChanged)
                .on_submit(Msg::ConfirmCreateAlbum)
                .padding([SPACE_1_5, SPACE_2])
                .size(TEXT_BASE)
                .width(Length::Fill),
            super::styles::icon_btn("✓", Msg::ConfirmCreateAlbum),
            super::styles::icon_btn("✕", Msg::EscapePressed),
        ]
        .spacing(SPACE_1)
        .align_y(Alignment::Center),
    )
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0.0, SPACE_1])
    .into()
}

/// The inline "new group" name input row (group glyph + name field + confirm /
/// cancel), used both at the top level and nested under a parent group.
fn create_group_input_row<'a>(input_val: &str) -> Element<'a, Msg> {
    container(
        row![
            super::icons::icon(Icon::Group, FG_DIM),
            text_input("Group name…", input_val)
                .id(crate::app::input_ids::create_group())
                .on_input(Msg::CreateGroupInputChanged)
                .on_submit(Msg::ConfirmCreateGroup)
                .padding([SPACE_1_5, SPACE_2])
                .size(TEXT_BASE)
                .width(Length::Fill),
            super::styles::icon_btn("✓", Msg::ConfirmCreateGroup),
            super::styles::icon_btn("✕", Msg::EscapePressed),
        ]
        .spacing(SPACE_1)
        .align_y(Alignment::Center),
    )
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0.0, SPACE_1])
    .into()
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

/// A Class-B nav row: a single catalog-level destination (All Photos, People,
/// Deleted, an import batch). Leading icon (quiet `FG_DIM`, brighter when
/// selected) · label · right-aligned count · accent background fill when
/// selected (selection is never colour-only — see design-system "Density
/// floor"). No chevron, no inline action buttons. `icon: None` (import batches)
/// reserves the icon column so labels still align.
fn nav_row<'a>(
    icon: Option<Icon>,
    label: String,
    count: Option<usize>,
    selected: bool,
    on_press: Msg,
) -> Element<'a, Msg> {
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
    let text_color = if selected { Color::WHITE } else { FG };
    let count_str = match count {
        Some(n) if n > 0 => format!("{n}"),
        _ => String::new(),
    };
    let lead: Element<Msg> = match icon {
        Some(kind) => {
            super::icons::icon(kind, if selected { Color::WHITE } else { FG_DIM })
        }
        None => Space::new().width(ICON_SIZE).into(),
    };

    let btn = button(
        row![
            lead,
            container(
                text(label)
                    .size(TEXT_BASE)
                    .color(text_color)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            text(count_str).size(TEXT_SM).color(FG_MUTED),
        ]
        .spacing(SPACE_1_5)
        .align_y(Alignment::Center),
    )
    .on_press(on_press)
    .width(Length::Fill)
    .style(|_: &Theme, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: FG,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    });

    container(btn)
        .height(ALBUM_ITEM_HEIGHT)
        .align_y(Alignment::Center)
        .padding([0.0, SPACE_1])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border { radius: 6.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// Width of the chevron / its placeholder, so parent and leaf labels align.
const CHEVRON_W: f32 = 20.0;

fn folder_tree_row<'a>(
    node: &'a FolderNode,
    depth: usize,
    selected_path: Option<&str>,
    expanded: bool,
    dirty: bool,
    offline: bool,
    max_chars: usize,
) -> Element<'a, Msg> {
    let path = node.path.clone();
    let count = node.total_count;
    let has_children = !node.children.is_empty();
    // The row highlights if *any* of its breadcrumb segments is the current
    // selection (a compacted chain occupies one row but spans several folders).
    let row_selected = node.chain.iter().any(|s| Some(s.path.as_str()) == selected_path);

    let bg = if row_selected {
        Color {
            r: ACCENT.r * 0.6,
            g: ACCENT.g * 0.6,
            b: ACCENT.b * 0.6,
            a: 0.4,
        }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if row_selected { ACCENT } else { Color::TRANSPARENT };

    // Indent deepens with tree depth. Kept tight (SPACE_2/level) for compactness.
    let indent = depth as f32 * SPACE_2;
    // Full breadcrumb text, for the overflow tooltip + truncation heuristic.
    let full_label = node
        .chain
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join("/");
    let effective_max = max_chars.saturating_sub(depth * 2 + 2).max(4);
    let was_truncated = full_label.chars().count() > effective_max;

    let chevron: Element<Msg> = if has_children {
        super::styles::icon_btn_svg(
            if expanded { Icon::ChevronDown } else { Icon::ChevronRight },
            Msg::ToggleFolderExpanded(path.clone()),
        )
        .width(Length::Fixed(CHEVRON_W))
        .height(Length::Fixed(FOLDER_ITEM_HEIGHT))
        .into()
    } else {
        Space::new().width(CHEVRON_W).into()
    };

    // Breadcrumb: each segment a tight, separately-clickable button, joined by
    // muted "/" separators (VS Code compact-folders style). A plain folder has a
    // single segment, so it reads exactly like an ordinary row.
    let mut crumbs = row![].spacing(SPACE_0_5).align_y(Alignment::Center);
    for (i, seg) in node.chain.iter().enumerate() {
        if i > 0 {
            crumbs = crumbs.push(text("/").size(TEXT_BASE).color(FG_MUTED));
        }
        let seg_selected = Some(seg.path.as_str()) == selected_path;
        let seg_color = if seg_selected || row_selected {
            Color::WHITE
        } else if offline {
            FG_MUTED
        } else {
            FG
        };
        crumbs = crumbs.push(
            button(
                text(seg.name.clone())
                    .size(TEXT_BASE)
                    .color(seg_color)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .on_press(Msg::SidebarItemClicked(SidebarItem::Folder(seg.path.clone())))
            .padding(0)
            .style(|_: &Theme, _| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: FG,
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            }),
        );
    }

    // The dirty `●` doubles as a one-click sync trigger for this folder (the
    // watcher only flags changes; this resolves them without hunting for the
    // right-click "Sync Folder"). It is a status dot first, action second — see
    // design-system "No action buttons on entity rows" for why this is the one
    // sanctioned inline trigger.
    let dirty_el: Element<Msg> = if dirty {
        let dot: Element<Msg> = mouse_area(text("●").size(TEXT_SM).color(ACCENT))
            .on_press(Msg::SyncFolder(path.clone()))
            .interaction(iced::mouse::Interaction::Pointer)
            .into();
        tooltip(dot, label_tooltip("Click to sync new files".to_string()), tooltip::Position::Left).into()
    } else {
        text("").size(TEXT_SM).color(FG_MUTED).into()
    };

    let label = row![
        container(crumbs).width(Length::Fill).clip(true),
        // Eject glyph marks a root whose drive is unplugged.
        if offline {
            text("⏏").size(TEXT_SM).color(FG_MUTED)
        } else {
            text("").size(TEXT_SM).color(FG_MUTED)
        },
        dirty_el,
        if count > 0 {
            text(format!(" {count}")).size(TEXT_SM).color(FG_MUTED)
        } else {
            text("").size(TEXT_SM).color(FG_MUTED)
        },
    ]
    .align_y(Alignment::Center);

    let inner = container(
        row![Space::new().width(indent), chevron, label]
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

    // Right-click targets the deepest folder of the row.
    let entity = SidebarItem::Folder(path.clone());
    let row_el = mouse_area(inner)
        .on_enter(Msg::HoverSidebarEntityStart(entity.clone()))
        .on_right_press(Msg::OpenSidebarEntityMenu(entity.clone()))
        .on_exit(Msg::HoverSidebarEntityEnd(entity));

    if was_truncated {
        tooltip(row_el, label_tooltip(full_label.replace('/', " / ")), tooltip::Position::Right).into()
    } else {
        row_el.into()
    }
}

#[allow(clippy::too_many_arguments)]
fn album_sidebar_row<'a>(
    label: String,
    album_id: String,
    count: usize,
    selected: bool,
    drop_hover: bool,
    multi_selected: bool,
    is_smart: bool,
    dirty: bool,
    is_target: bool,
    max_chars: usize,
) -> Element<'a, Msg> {
    let text_color = if selected || drop_hover || multi_selected {
        Color::WHITE
    } else {
        FG
    };
    let bg = if drop_hover {
        ALBUM_HOVER
    } else if selected || multi_selected {
        Color {
            r: ACCENT.r * 0.6,
            g: ACCENT.g * 0.6,
            b: ACCENT.b * 0.6,
            a: 0.4,
        }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if drop_hover || selected || multi_selected {
        ACCENT
    } else {
        Color::TRANSPARENT
    };

    let smart_indicator = if is_smart { "⚡ " } else { "" };
    let target_indicator = if is_target { "◎ " } else { "" };
    let (display_label, was_truncated) = truncate_label(&label, max_chars);
    let dirty_dot = if dirty { " ●" } else { "" };
    let count_str = if count > 0 { format!("{dirty_dot} {count}") } else { dirty_dot.to_string() };

    // The whole row is the press target (no inner button) so its `mouse_area` can
    // capture press-down — needed to start an album→group drag. Click vs drag is
    // resolved on release in the update loop (see `Msg::AlbumPressed`).
    let inner = container(
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
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0.0, SPACE_1])
    .width(Length::Fill)
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
        .on_press(Msg::AlbumPressed(album_id))
        .on_enter(Msg::HoverSidebarEntityStart(entity.clone()))
        .on_right_press(Msg::OpenSidebarEntityMenu(entity.clone()))
        .on_exit(Msg::HoverSidebarEntityEnd(entity));

    if was_truncated {
        tooltip(row_el, label_tooltip(label), tooltip::Position::Right).into()
    } else {
        row_el.into()
    }
}
