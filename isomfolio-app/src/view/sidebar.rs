use iced::{
    widget::{button, column, container, mouse_area, row, scrollable, text, text_input, tooltip, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::folder_tree::FolderNode;
use isomfolio_core::models::{AlbumKind, Flag, RatingFilter, TagMatch};

use super::styles::{
    active_chip_style, color_label_swatch, confirm_action_row, danger_btn_style, ghost_btn_style,
    sidebar_divider, ACCENT, ALBUM_HOVER, BG_SIDEBAR, BG_STATUSBAR,
    COLOR_LABELS, ERR, FG, FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_3,
    STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM, TEXT_XS,
};
use super::icons::{Icon, ICON_SIZE};
use crate::app::{
    parse_date_str, unix_to_date_str, App, DatePreset, Msg, RatingCmp, SidebarItem, SidebarSection,
    ViewMode, ALBUM_ITEM_HEIGHT, FOLDER_ITEM_HEIGHT,
};

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

/// Width of the criteria-panel label column (px).
const FILTER_LABEL_W: f32 = 48.0;

/// One criteria row: `[label] [controls]`. The controls block fills the
/// remaining width so wrapping chip rows and full-width inputs both align to a
/// single left edge.
fn filter_field<'a>(label: &str, content: Element<'a, Msg>) -> Element<'a, Msg> {
    row![
        container(text(label.to_string()).size(TEXT_XS).color(FG_DIM)).width(FILTER_LABEL_W),
        container(content).width(Length::Fill),
    ]
    .spacing(SPACE_1_5)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// A uniform text chip (toggle/segment). Same size and padding everywhere so the
/// panel reads as one control family, not a grab-bag.
fn txt_chip<'a>(label: String, active: bool, msg: Msg) -> Element<'a, Msg> {
    button(text(label).size(TEXT_XS))
        .on_press(msg)
        .padding([SPACE_0_5, SPACE_1_5])
        .style(if active { active_chip_style } else { ghost_btn_style })
        .into()
}

/// A uniform glyph chip with a hover tooltip (the glyph carries the meaning, so
/// the tip is how it's discovered). Colour is fixed (a swatch / star / flag).
fn glyph_chip<'a>(glyph: &str, hint: String, active: bool, color: Color, msg: Msg) -> Element<'a, Msg> {
    super::styles::tip(
        button(text(glyph.to_string()).size(TEXT_SM).color(color))
            .on_press(msg)
            .padding([SPACE_0_5, SPACE_1])
            .style(if active { active_chip_style } else { ghost_btn_style }),
        hint,
        super::styles::TipPos::Right,
    )
}

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
                super::styles::tip(
                    super::styles::icon_btn_svg(Icon::Plus, Msg::StartCreateAlbum),
                    "New album",
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
                        super::styles::icon_btn("✓", Msg::ConfirmCreateAlbum),
                        super::styles::icon_btn("✕", Msg::EscapePressed),
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
            if albums_collapsed {
                break;
            }
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
                            super::styles::icon_btn("✓", Msg::ConfirmRenameAlbum),
                            super::styles::icon_btn("✕", Msg::EscapePressed),
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

    fn view_sidebar_filters(&self) -> Element<'_, Msg> {
        let cur = self.filters.rating;
        let cmp = self.filters.rating_cmp;

        // Uniform two-column grid: every row is `filter_field(label, controls)`,
        // so labels share a left column and controls a single left edge.
        let mut col = column![].spacing(SPACE_2).padding([SPACE_1, SPACE_0_5]);

        // Flags
        let flags = row![
            glyph_chip("✓", "Show picks".into(), self.filters.flags.allows(Flag::Pick), FG, Msg::ToggleFlagFilter(Flag::Pick)),
            glyph_chip("○", "Show unflagged".into(), self.filters.flags.allows(Flag::Unflagged), FG, Msg::ToggleFlagFilter(Flag::Unflagged)),
            glyph_chip("✕", "Show rejects".into(), self.filters.flags.allows(Flag::Reject), FG, Msg::ToggleFlagFilter(Flag::Reject)),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("Flags", flags.into()));

        // Rating — comparator, then 1–5, then unrated.
        let mut rating = row![].spacing(SPACE_1);
        for (c, hint) in [
            (RatingCmp::AtLeast, "At least"),
            (RatingCmp::Exactly, "Exactly"),
            (RatingCmp::AtMost, "At most"),
        ] {
            rating = rating.push(glyph_chip(c.symbol(), hint.to_string(), cmp == c, FG_DIM, Msg::SetRatingCmp(c)));
        }
        for n in 1..=5i32 {
            let active = matches!(cur,
                RatingFilter::AtLeast(v) | RatingFilter::Exactly(v) | RatingFilter::AtMost(v) if v == n);
            let msg = if active { RatingFilter::Any } else { cmp.apply(n) };
            rating = rating.push(glyph_chip(&n.to_string(), format!("{} {n}★", cmp.symbol()), active, STAR_GOLD, Msg::SetRatingFilter(msg)));
        }
        let unrated = matches!(cur, RatingFilter::Unrated);
        let unrated_msg = if unrated { RatingFilter::Any } else { RatingFilter::Unrated };
        rating = rating.push(glyph_chip("0", "Unrated only".into(), unrated, FG_DIM, Msg::SetRatingFilter(unrated_msg)));
        col = col.push(filter_field("Rating", rating.wrap().into()));

        // Colour
        let mut colour = row![].spacing(SPACE_1);
        for name in COLOR_LABELS {
            let active = self.filters.color.as_deref() == Some(name);
            let msg = if active { None } else { Some(name.to_string()) };
            colour = colour.push(glyph_chip("●", format!("Colour: {name}"), active, color_label_swatch(name), Msg::SetColorFilter(msg)));
        }
        col = col.push(filter_field("Colour", colour.wrap().into()));

        // Tags — match toggle, then a wrapped row of tag chips + the add input.
        let is_any = self.filters.tag_match == TagMatch::Any;
        let match_row = row![
            txt_chip("All".into(), !is_any, Msg::SetTagMatch(TagMatch::All)),
            txt_chip("Any".into(), is_any, Msg::SetTagMatch(TagMatch::Any)),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("Tags", match_row.into()));

        let tag_chip = |tag: &str, negated: bool| -> Element<'_, Msg> {
            let style: fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style =
                if negated { danger_btn_style } else { active_chip_style };
            let label = if negated { format!("−{tag}") } else { tag.to_string() };
            row![
                button(text(label).size(TEXT_XS))
                    .on_press(Msg::ToggleFilterTagNegate(tag.to_string()))
                    .padding([SPACE_0_5, SPACE_1])
                    .style(style),
                button(text("×").size(TEXT_XS))
                    .on_press(Msg::RemoveFilterTag(tag.to_string()))
                    .padding([SPACE_0_5, SPACE_1])
                    .style(style),
            ]
            .spacing(1.0)
            .into()
        };
        let mut tag_items = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for tag in &self.filters.tags {
            tag_items = tag_items.push(tag_chip(tag, false));
        }
        for tag in &self.filters.exclude_tags {
            tag_items = tag_items.push(tag_chip(tag, true));
        }
        tag_items = tag_items.push(
            text_input("+ tag", &self.filters.tag_input)
                .on_input(Msg::FilterTagInputChanged)
                .on_submit(Msg::AddFilterTag)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(90),
        );
        col = col.push(filter_field("", tag_items.wrap().into()));

        // Date range
        let from_err = !self.filters.date_from.is_empty()
            && parse_date_str(&self.filters.date_from).is_none();
        let to_err = !self.filters.date_to.is_empty()
            && parse_date_str(&self.filters.date_to).is_none();
        col = col.push(filter_field(
            "From",
            text_input("YYYY-MM-DD", &self.filters.date_from)
                .on_input(Msg::FilterDateFromChanged)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(Length::Fill)
                .into(),
        ));
        col = col.push(filter_field(
            "To",
            text_input("YYYY-MM-DD", &self.filters.date_to)
                .on_input(Msg::FilterDateToChanged)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(Length::Fill)
                .into(),
        ));
        if from_err || to_err {
            col = col.push(filter_field("", text("Format: YYYY-MM-DD").size(TEXT_XS).color(ERR).into()));
        }
        let mut presets = row![].spacing(SPACE_1);
        for (label, preset) in [
            ("7d", DatePreset::Last7),
            ("30d", DatePreset::Last30),
            ("Month", DatePreset::ThisMonth),
            ("Year", DatePreset::ThisYear),
        ] {
            presets = presets.push(txt_chip(label.into(), false, Msg::SetDatePreset(preset)));
        }
        col = col.push(filter_field("", presets.wrap().into()));

        // Type
        let mut types = row![].spacing(SPACE_1);
        for ext in ["jpg", "png", "webp", "gif"] {
            let active = self.filters.exts.contains(ext);
            types = types.push(txt_chip(format!(".{}", ext.to_uppercase()), active, Msg::ToggleFilterFileType(ext.to_string())));
        }
        col = col.push(filter_field("Type", types.wrap().into()));

        // GPS
        let gps = row![
            txt_chip("Any".into(), self.filters.has_location.is_none(), Msg::SetLocationFilter(None)),
            txt_chip("Yes".into(), self.filters.has_location == Some(true), Msg::SetLocationFilter(Some(true))),
            txt_chip("No".into(), self.filters.has_location == Some(false), Msg::SetLocationFilter(Some(false))),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("GPS", gps.into()));

        // Person (only when named clusters exist)
        let named: Vec<&isomfolio_core::models::FaceClusterSummary> = self
            .faces.clusters.iter()
            .filter(|c| c.name.is_some())
            .collect();
        if !named.is_empty() {
            let mut people = row![txt_chip("Any".into(), self.filters.person.is_none(), Msg::SetPersonFilter(None))]
                .spacing(SPACE_1);
            for c in named {
                let name = c.name.clone().unwrap_or_default();
                let active = self.filters.person.as_deref() == Some(c.cluster_id.as_str());
                people = people.push(txt_chip(name, active, Msg::SetPersonFilter(Some(c.cluster_id.clone()))));
            }
            col = col.push(filter_field("Person", people.wrap().into()));
        }

        // Added within
        let mut added = row![].spacing(SPACE_1);
        for (label, days) in [("Any", None), ("7d", Some(7)), ("30d", Some(30))] {
            added = added.push(txt_chip(label.into(), self.filters.added_within_days == days, Msg::SetAddedWithinFilter(days)));
        }
        col = col.push(filter_field("Added", added.into()));

        // Camera (only when cameras exist)
        if !self.cameras.is_empty() {
            let mut cams = row![txt_chip("Any".into(), self.filters.camera.is_none(), Msg::SetCameraFilter(None))]
                .spacing(SPACE_1);
            for cam in &self.cameras {
                let active = self.filters.camera.as_deref() == Some(cam.as_str());
                cams = cams.push(txt_chip(cam.clone(), active, Msg::SetCameraFilter(Some(cam.clone()))));
            }
            col = col.push(filter_field("Camera", cams.wrap().into()));
        }

        // Clear / Smart Album actions — a full-width row under the grid.
        if self.has_active_filters() {
            let is_smart = self.current_album_is_smart();
            let mut actions = row![
                button(text("Clear").size(TEXT_XS)).on_press(Msg::ClearFilters).style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_1)
            .align_y(Alignment::Center);

            if is_smart {
                if self.smart_album_dirty {
                    actions = actions.push(text("Unsaved").size(TEXT_XS).color(ERR));
                }
                actions = actions.push(
                    button(text("Update").size(TEXT_XS)).on_press(Msg::UpdateSmartAlbum).style(active_chip_style),
                );
            } else if let Some(ref name_input) = self.filters.save_smart_input {
                actions = actions
                    .push(
                        text_input("Album name…", name_input)
                            .on_input(Msg::SmartAlbumNameChanged)
                            .on_submit(Msg::ConfirmSmartAlbum)
                            .padding([SPACE_0_5, SPACE_1_5])
                            .size(TEXT_XS)
                            .width(Length::Fill),
                    )
                    .push(button(text("Save").size(TEXT_XS)).on_press(Msg::ConfirmSmartAlbum).style(active_chip_style));
            } else {
                actions = actions.push(
                    button(text("Save Smart…").size(TEXT_XS)).on_press(Msg::SaveAsSmartAlbum).style(ghost_btn_style),
                );
            }
            col = col.push(container(actions).padding([SPACE_1, 0.0]));
        }

        container(col).width(Length::Fill).into()
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

    let label = row![
        container(crumbs).width(Length::Fill).clip(true),
        // Eject glyph marks a root whose drive is unplugged.
        if offline {
            text("⏏").size(TEXT_SM).color(FG_MUTED)
        } else {
            text("").size(TEXT_SM).color(FG_MUTED)
        },
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
    .on_press(Msg::SidebarEntityPressed(SidebarItem::Album(
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
        .on_right_press(Msg::OpenSidebarEntityMenu(entity.clone()))
        .on_exit(Msg::HoverSidebarEntityEnd(entity));

    if was_truncated {
        tooltip(row_el, label_tooltip(label), tooltip::Position::Right).into()
    } else {
        row_el.into()
    }
}
