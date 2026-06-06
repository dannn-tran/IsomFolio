use iced::widget::scrollable::Direction;
use iced::{
    widget::{button, column, container, image, mouse_area, pick_list, row, scrollable, slider, stack, text, text_input, tooltip, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::models::{Flag, SortField, ThumbnailState};

/// Sort fields in the order they appear in the toolbar dropdown.
const SORT_FIELDS: [SortField; 4] =
    [SortField::Name, SortField::Date, SortField::Size, SortField::Ext];

/// `pick_list` wrapper so a `SortField` renders with its human label.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SortChoice(SortField);

impl std::fmt::Display for SortChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(sort_field_label(self.0))
    }
}

use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_CRITERIA, BG_GRID,
    BG_TILE_LOADING, BORDER, ERR, FG, FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2,
    SPACE_3, STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM, TEXT_STAR, TEXT_XS, TILE_CORNER,
    WARN,
};
use crate::app::{
    format_file_size, sort_field_label, unix_to_date_str, App, GridLayout, ListCol, Msg,
    DetailField, BUFFER_ROWS, GRID_PADDING, LIST_HEADER_HEIGHT, LIST_ROW_HEIGHT, TILE_GAP,
    TILE_SIZE_MAX, TILE_SIZE_MIN,
};

// Fixed (non-resizable) glyph columns for the List layout. The resizable
// columns (Name/Stars/Date/Size/Type) live in `App::list_col`. Header and rows
// read the same widths so they stay aligned; trailing slack absorbs the rest.
const LIST_THUMB_W: f32 = 32.0;
const LIST_COL_FLAG: f32 = 20.0;
const LIST_COL_COLOR: f32 = 16.0;
/// Width of a column-resize grab handle at the right edge of a header cell.
const LIST_HANDLE_W: f32 = 6.0;

impl App {
    pub(super) fn view_grid(&self) -> Element<'_, Msg> {
        let is_list = matches!(self.grid_layout, GridLayout::List);

        let layout_toggle = row![
            super::styles::tip(
                super::styles::icon_btn_styled("▦", Msg::SetGridLayout(GridLayout::Grid),
                    move |t: &Theme, s| if is_list { ghost_btn_style(t, s) } else { active_chip_style(t, s) }),
                "Grid view",
                super::styles::TipPos::Bottom,
            ),
            super::styles::tip(
                super::styles::icon_btn_styled("≡", Msg::SetGridLayout(GridLayout::List),
                    move |t: &Theme, s| if is_list { active_chip_style(t, s) } else { ghost_btn_style(t, s) }),
                "List view",
                super::styles::TipPos::Bottom,
            ),
        ]
        .spacing(SPACE_0_5);

        // Thumbnail-size control (Grid only — List rows are fixed height). A
        // continuous slider reads as "size" directly (Lightroom/Photos/Finder
        // convention) where a `+`/`−` pair read ambiguously as zoom; the small→
        // large glyphs flanking it name the axis, and ⌘−/⌘+ still step it.
        let size_control: Element<Msg> = if is_list {
            Space::new().width(0.0).into()
        } else {
            super::styles::tip(
                row![
                    text("▪").size(TEXT_XS).color(FG_DIM),
                    slider(TILE_SIZE_MIN..=TILE_SIZE_MAX, self.tile_px, Msg::SetTileSize)
                        .step(2.0)
                        .width(96),
                    text("▰").size(TEXT_MD).color(FG_DIM),
                ]
                .spacing(SPACE_1)
                .align_y(Alignment::Center),
                "Thumbnail size (⌘− / ⌘+)",
                super::styles::TipPos::Bottom,
            )
        };

        let sort_choices: Vec<SortChoice> = SORT_FIELDS.iter().copied().map(SortChoice).collect();
        let sort_picker = pick_list(
            sort_choices,
            Some(SortChoice(self.sort_by)),
            |c| Msg::SetSortField(c.0),
        )
        .text_size(TEXT_MD)
        .padding([SPACE_1, SPACE_1_5]);
        let sort_dir =
            super::styles::icon_btn(if self.sort_asc { "▲" } else { "▼" }, Msg::SortDirToggle);

        // Active-filter indicator: when a filter narrows the current collection,
        // say so right above the grid with a one-click Clear. Without it, landing
        // in a filtered view (e.g. importing into an active filter) silently shows
        // fewer photos than the collection holds, with no visible cause.
        let filter_indicator: Element<Msg> = if self.has_active_filters() {
            super::styles::tip(
                button(
                    row![
                        text("Filtered").size(TEXT_MD),
                        text("✕").size(TEXT_MD),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .on_press(Msg::ClearFilters)
                .style(active_chip_style),
                "Filters are narrowing this view — click to clear",
                super::styles::TipPos::Bottom,
            )
        } else {
            Space::new().width(0.0).into()
        };

        let toolbar_row = row![
            super::styles::tip(
                button(text("⧉ Stack").size(TEXT_MD))
                    .on_press(Msg::ToggleCollapseBursts)
                    .style({
                        let on = self.collapse_bursts;
                        move |t: &Theme, s| if on { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                    }),
                "Collapse bursts to one tile",
                super::styles::TipPos::Bottom,
            ),
            filter_indicator,
            Space::new().width(Length::Fill),
            size_control,
            layout_toggle,
            text("Sort").size(TEXT_MD).color(FG_DIM),
            sort_picker,
            sort_dir,
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        let filter_toolbar = container(toolbar_row)
            .padding([SPACE_1_5, SPACE_3])
            .width(Length::Fill);

        let empty_or_grid: Element<Msg> = if self.files.is_empty() {
            // No library at all → onboarding call-to-action; otherwise just an
            // empty view (e.g. a filter or album with no matches).
            let inner: Element<Msg> = if self.folders.is_empty() {
                column![
                    text("No photos yet").size(TEXT_MD).color(FG),
                    text("Add a folder to start your catalog.").size(TEXT_SM).color(FG_DIM),
                    Space::new().height(SPACE_2),
                    button(text("Add Folder…").size(TEXT_BASE))
                        .on_press(Msg::SyncPickFolder)
                        .style(active_chip_style),
                ]
                .spacing(SPACE_1)
                .align_x(Alignment::Center)
                .into()
            } else {
                text("No photos in this view").size(TEXT_BASE).color(FG_DIM).into()
            };
            container(inner)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        } else {
            let cols = self.cols().max(1);
            let tile_px = self.tile_px;
            let step = self.row_step();
            let total = self.files.len();
            let total_rows = (total + cols - 1) / cols;

            let first_row = {
                let r = ((self.scroll_y - GRID_PADDING) / step) as usize;
                r.saturating_sub(BUFFER_ROWS)
            };
            let visible_rows = (self.viewport_height / step) as usize + 1 + BUFFER_ROWS * 2;
            let last_row = (first_row + visible_rows).min(total_rows);

            let top_space = first_row as f32 * step;
            let bottom_space = ((total_rows - last_row) as f32 * step).max(0.0);

            let row_elements: Vec<Element<Msg>> = (first_row..last_row)
                .map(|r| {
                    if is_list {
                        // cols == 1 in List, so the row index *is* the file index.
                        self.view_list_row(r)
                    } else {
                        let start = r * cols;
                        let end = (start + cols).min(total);
                        let tiles = (start..end).map(|i| self.view_tile(i));
                        let padding = cols - (end - start);
                        let pad_iter =
                            std::iter::repeat_with(|| Space::new().width(tile_px).into())
                                .take(padding);
                        let all_tiles: Vec<Element<Msg>> = tiles.chain(pad_iter).collect();
                        row(all_tiles).spacing(TILE_GAP).into()
                    }
                })
                .collect();

            let row_spacing = if is_list { 0.0 } else { TILE_GAP };
            let grid_content = column![
                Space::new().height(top_space + GRID_PADDING),
                column(row_elements).spacing(row_spacing),
                Space::new().height(bottom_space + GRID_PADDING),
            ]
            .padding([0, GRID_PADDING as u16]);

            scrollable(grid_content)
                .id(crate::app::GRID_SCROLL_ID.clone())
                .direction(Direction::Vertical(
                    scrollable::Scrollbar::new().width(6).scroller_width(6),
                ))
                .on_scroll(|vp| Msg::Scrolled {
                    y: vp.absolute_offset().y,
                    height: vp.bounds().height,
                    width: vp.bounds().width,
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        let mut grid_col = column![filter_toolbar];
        if is_list && !self.files.is_empty() {
            grid_col = grid_col.push(self.view_list_header());
        }
        grid_col = grid_col.push(empty_or_grid);

        container(grid_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }

    pub(super) fn view_tile(&self, idx: usize) -> Element<'_, Msg> {
        let file = &self.files[idx];
        let selected = self.grid_selected.contains(&file.id);
        let dragging = self.drag.ids.contains(&file.id);

        let thumb_state = self.thumbnails.get(&file.id);

        let tile_content: Element<Msg> = match thumb_state {
            // The renderer decodes the cached JPEG on its own worker thread and
            // evicts off-screen textures — so we hand it the path and hold no
            // decoded pixels ourselves (bounded RAM regardless of library size).
            Some(ThumbnailState::Ready(path)) => {
                image(image::Handle::from_path(path))
                    .width(self.tile_px)
                    .height(self.tile_px)
                    .content_fit(iced::ContentFit::Cover)
                    .into()
            }
            _ => {
                container(Space::new())
                    .width(self.tile_px)
                    .height(self.tile_px)
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(BG_TILE_LOADING)),
                        border: Border { radius: TILE_CORNER.into(), ..Default::default() },
                        ..Default::default()
                    })
                    .into()
            }
        };

        let flag = file.flag;
        // Dim rejected photos in place (don't remove them) so the grid keeps its
        // continuity during a cull and rejects stay one click from un-rejecting.
        // Exception: when the view is filtered to rejects *only*, show them
        // normally — you're reviewing them deliberately.
        let rejects_isolated = self.filters.flags.reject
            && !self.filters.flags.pick
            && !self.filters.flags.unflagged;
        let in_deleted_view = self.selected_item == crate::app::SidebarItem::Deleted;
        let dimmed = flag == Flag::Reject && !rejects_isolated && !in_deleted_view && !selected && !dragging;

        let overlay_color = if dragging {
            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 }
        } else if selected {
            Color { a: 0.28, ..ACCENT }
        } else if dimmed {
            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.55 }
        } else {
            Color::TRANSPARENT
        };

        let (ring_color, ring_width) = if selected && !dragging {
            (ACCENT, 3.0_f32)
        } else {
            (Color::TRANSPARENT, 0.0)
        };

        let rating = self.file_ratings.get(&file.id).copied().unwrap_or(0);
        let color = self.file_labels.get(&file.id).cloned();
        let burst = self.file_burst_sizes.get(&file.id).copied();
        let tile_px = self.tile_px;

        let mut layers: Vec<Element<Msg>> = vec![
            container(tile_content)
                .width(tile_px)
                .height(tile_px)
                .style(|_: &Theme| container::Style {
                    border: Border { radius: TILE_CORNER.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),
            container(Space::new())
                .width(tile_px)
                .height(tile_px)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(overlay_color)),
                    border: Border {
                        color: ring_color,
                        width: ring_width,
                        radius: TILE_CORNER.into(),
                    },
                    ..Default::default()
                })
                .into(),
        ];

        if flag != Flag::Unflagged || rating > 0 || color.is_some() || burst.is_some() {
            let (flag_label, flag_color) = match flag {
                Flag::Pick => ("✓", ACCENT),
                Flag::Reject => ("✕", ERR),
                Flag::Unflagged => ("", Color::TRANSPARENT),
            };
            let scrim = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.55 };
            let badge_style = move |_: &Theme| container::Style {
                background: Some(Background::Color(scrim)),
                border: Border { radius: 3.0.into(), ..Default::default() },
                ..Default::default()
            };
            let mut badge_col: Vec<Element<Msg>> = Vec::new();
            if flag != Flag::Unflagged {
                badge_col.push(
                    container(text(flag_label).size(TEXT_SM).color(flag_color))
                        .padding([2.0, 4.0])
                        .style(badge_style)
                        .into(),
                );
            }
            if let Some(ref name) = color {
                badge_col.push(
                    container(text("●").size(TEXT_SM).color(super::styles::color_label_swatch(name)))
                        .padding([2.0, 4.0])
                        .style(badge_style)
                        .into(),
                );
            }
            if let Some(n) = burst {
                badge_col.push(
                    container(text(format!("⧉ {n}")).size(TEXT_SM).color(FG))
                        .padding([2.0, 4.0])
                        .style(badge_style)
                        .into(),
                );
            }
            badge_col.push(Space::new().height(Length::Fill).into());
            if rating > 0 {
                badge_col.push(
                    container(text("★".repeat(rating as usize)).size(TEXT_SM).color(STAR_GOLD))
                        .padding([2.0, 4.0])
                        .style(badge_style)
                        .into(),
                );
            }
            let badge_layer: Element<Msg> = container(column(badge_col))
                .width(tile_px)
                .height(tile_px)
                .padding([4.0, 5.0])
                .align_x(Alignment::Start)
                .into();
            layers.push(badge_layer);
        }

        // "Missing" = file gone but its drive is present; "Offline" = the whole
        // library root (drive) is currently unplugged. Offline is recoverable by
        // reconnecting, so it reads as a state, not a loss.
        let offline = self.is_offline_path(&file.folder);
        if file.is_orphaned || offline {
            let label = if offline { "Offline" } else { "Missing" };
            let scrim = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.65 };
            let banner: Element<Msg> = container(
                container(text(label).size(TEXT_XS).color(WARN))
                    .padding([2.0, 6.0])
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(scrim)),
                        ..Default::default()
                    }),
            )
            .width(tile_px)
            .height(tile_px)
            .align_y(Alignment::End)
            .into();
            layers.push(banner);
        }


        stack(layers).into()
    }

    /// Clickable, resizable column-header strip for the List layout. The four
    /// real `SortField`s (Name/Date/Size/Type) sort on click — clicking the
    /// active column toggles direction. Each resizable column ends in a drag
    /// handle that sets its width (`App::list_col`); Rating is resizable but not
    /// sortable; flag/colour are fixed glyph columns. Widths mirror
    /// `view_list_row`, with trailing slack absorbing the remainder.
    pub(super) fn view_list_header(&self) -> Element<'_, Msg> {
        let w = &self.list_col;

        // The grab handle: a thin zone at a column's right edge with a 1px
        // separator line. Discovered via the horizontal-resize cursor on hover.
        let handle = |col: ListCol| -> Element<Msg> {
            mouse_area(
                container(
                    container(Space::new())
                        .width(1.0)
                        .height(Length::Fill)
                        .style(|_: &Theme| container::Style {
                            background: Some(Background::Color(BORDER)),
                            ..Default::default()
                        }),
                )
                .width(LIST_HANDLE_W)
                .height(Length::Fill)
                .align_x(Alignment::Center),
            )
            .interaction(iced::mouse::Interaction::ResizingHorizontally)
            .on_press(Msg::ListColResizeStart(col))
            .into()
        };
        let sortable_col = |label: &str, field: SortField, col: ListCol, width: f32| -> Element<Msg> {
            let active = self.sort_by == field;
            let arrow = if active {
                if self.sort_asc { " ▲" } else { " ▼" }
            } else {
                ""
            };
            let msg = if active { Msg::SortDirToggle } else { Msg::SetSortField(field) };
            container(
                row![
                    button(
                        text(format!("{label}{arrow}"))
                            .size(TEXT_SM)
                            .color(if active { FG } else { FG_DIM }),
                    )
                    .on_press(msg)
                    .padding([0.0, SPACE_1])
                    .width(Length::Fill)
                    .style(ghost_btn_style),
                    handle(col),
                ]
                .align_y(Alignment::Center),
            )
            .width(width)
            .into()
        };
        let plain_col = |label: &str, col: ListCol, width: f32| -> Element<Msg> {
            container(
                row![
                    container(text(label.to_string()).size(TEXT_SM).color(FG_DIM))
                        .padding([0.0, SPACE_1])
                        .width(Length::Fill),
                    handle(col),
                ]
                .align_y(Alignment::Center),
            )
            .width(width)
            .into()
        };
        let fixed = |label: &str, width: f32| -> Element<Msg> {
            container(text(label.to_string()).size(TEXT_SM).color(FG_DIM))
                .padding([0.0, SPACE_1])
                .width(width)
                .into()
        };

        let header = row![
            Space::new().width(LIST_THUMB_W),
            sortable_col("Name", SortField::Name, ListCol::Name, w.name),
            fixed("Flag", LIST_COL_FLAG),
            plain_col("Rating", ListCol::Stars, w.stars),
            fixed("Col", LIST_COL_COLOR),
            sortable_col("Date Shot", SortField::Date, ListCol::Date, w.date),
            sortable_col("Size", SortField::Size, ListCol::Size, w.size),
            sortable_col("Type", SortField::Ext, ListCol::Type, w.type_),
            Space::new().width(Length::Fill),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        container(header)
            .width(Length::Fill)
            .height(Length::Fixed(LIST_HEADER_HEIGHT))
            .padding([0, GRID_PADDING as u16])
            .align_y(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_CRITERIA)),
                border: Border { color: BORDER, width: 0.0, radius: 0.0.into() },
                ..Default::default()
            })
            .into()
    }

    /// One file rendered as a compact line (List layout). Pure presentation —
    /// clicks/drag/right-click flow through the global mouse handler + `tile_index_at`
    /// exactly like grid tiles, so the selection model is shared.
    pub(super) fn view_list_row(&self, idx: usize) -> Element<'_, Msg> {
        let file = &self.files[idx];
        let selected = self.grid_selected.contains(&file.id);
        let dragging = self.drag.ids.contains(&file.id);
        let rejected = file.flag == Flag::Reject;

        let thumb_px = LIST_ROW_HEIGHT - 8.0;
        let placeholder = || {
            container(Space::new())
                .width(thumb_px)
                .height(thumb_px)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(BG_TILE_LOADING)),
                    border: Border { radius: 2.0.into(), ..Default::default() },
                    ..Default::default()
                })
        };
        let thumb: Element<Msg> = match self.thumbnails.get(&file.id) {
            Some(ThumbnailState::Ready(path)) => image(image::Handle::from_path(path))
                .width(thumb_px)
                .height(thumb_px)
                .content_fit(iced::ContentFit::Cover)
                .into(),
            _ => placeholder().into(),
        };

        let name_color = if selected {
            Color::WHITE
        } else if rejected {
            FG_MUTED
        } else {
            FG
        };
        let flag_cell: Element<Msg> = match file.flag {
            Flag::Pick => text("✓").size(TEXT_SM).color(ACCENT).into(),
            Flag::Reject => text("✕").size(TEXT_SM).color(ERR).into(),
            Flag::Unflagged => text("").size(TEXT_SM).into(),
        };
        let rating = self.file_ratings.get(&file.id).copied().unwrap_or(0);
        let stars = if rating > 0 { "★".repeat(rating as usize) } else { String::new() };
        let color_cell: Element<Msg> = match self.file_labels.get(&file.id) {
            Some(name) => text("●")
                .size(TEXT_SM)
                .color(super::styles::color_label_swatch(name))
                .into(),
            None => text("").size(TEXT_SM).into(),
        };
        let date_unix = file.exif_date_unix.unwrap_or(file.mtime_unix);
        let w = &self.list_col;

        // Name is a fixed (resizable) width; estimate its char budget to decide
        // whether the clipped filename needs a hover tooltip to stay readable
        // (same trick the sidebar uses for folder/album labels).
        let name_budget = ((w.name / 7.5).floor() as usize).max(4);
        let name_clipped = file.name.chars().count() > name_budget;

        let name_cell = container(
            text(&file.name)
                .size(TEXT_BASE)
                .color(name_color)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(w.name)
        .padding([0.0, SPACE_1])
        .clip(true);
        let name_el: Element<Msg> = if name_clipped {
            tooltip(
                name_cell,
                container(text(&file.name).size(TEXT_SM).color(FG))
                    .padding([SPACE_1, SPACE_1_5])
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(Color {
                            r: 0.12,
                            g: 0.12,
                            b: 0.15,
                            a: 0.97,
                        })),
                        border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
                        ..Default::default()
                    }),
                tooltip::Position::Bottom,
            )
            .into()
        } else {
            name_cell.into()
        };

        let line = row![
            container(thumb).width(LIST_THUMB_W).align_x(Alignment::Center),
            name_el,
            container(flag_cell).width(LIST_COL_FLAG).padding([0.0, SPACE_1]),
            container(text(stars).size(TEXT_SM).color(STAR_GOLD))
                .width(w.stars)
                .padding([0.0, SPACE_1])
                .clip(true),
            container(color_cell).width(LIST_COL_COLOR).padding([0.0, SPACE_1]),
            container(text(unix_to_date_str(date_unix)).size(TEXT_SM).color(FG_DIM))
                .width(w.date)
                .padding([0.0, SPACE_1]),
            container(text(format_file_size(file.size_bytes)).size(TEXT_SM).color(FG_DIM))
                .width(w.size)
                .padding([0.0, SPACE_1]),
            container(
                text(format!(".{}", file.ext.to_uppercase()))
                    .size(TEXT_SM)
                    .color(FG_DIM),
            )
            .width(w.type_)
            .padding([0.0, SPACE_1]),
            // Trailing slack so the fixed columns stay left-packed and the row's
            // right edge tracks a column-resize handle (matches the header).
            Space::new().width(Length::Fill),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        // Hover feedback: the global mouse model has no per-widget hover, so
        // derive it from the tracked cursor via the shared hit-test.
        let hovered = !selected && !dragging && self.tile_index_at(self.cursor) == Some(idx);
        let bg = if selected || dragging {
            Color { a: 0.28, ..ACCENT }
        } else if hovered {
            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 }
        } else {
            Color::TRANSPARENT
        };

        // No horizontal padding here: the enclosing grid_content column already
        // insets by GRID_PADDING (matching the header strip), so adding it again
        // would misalign rows against the header.
        container(line)
            .width(Length::Fill)
            .height(Length::Fixed(LIST_ROW_HEIGHT))
            .align_y(Alignment::Center)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(bg)),
                ..Default::default()
            })
            .into()
    }


    pub(super) fn view_detail(&self) -> Element<'_, Msg> {
        use super::styles::BG_SIDEBAR;
        use crate::app::SIDEBAR_WIDTH;

        let file = self.detail_file();
        let is_batch = !self.detail.batch_file_ids.is_empty();
        let has_tags = file.is_some() || is_batch;

        let mut col = column![text("Info").size(TEXT_SM).color(FG_DIM)]
            .spacing(SPACE_2)
            .padding(SPACE_3);

        if let Some(file) = file {
            col = col.push(text(&file.name).size(TEXT_BASE));

            let size_str = format_file_size(file.size_bytes);
            let date_unix = file.exif_date_unix.unwrap_or(file.mtime_unix);
            let date_str = unix_to_date_str(date_unix);
            let date_label = if file.exif_date_unix.is_some() { "Taken" } else { "Modified" };

            col = col
                .push(text(format!("Size  {size_str}")).size(TEXT_SM).color(FG_DIM))
                .push(text(format!("{date_label}  {date_str}")).size(TEXT_SM).color(FG_DIM))
                .push(
                    text(format!("Type  .{}", file.ext.to_uppercase()))
                        .size(TEXT_SM)
                        .color(FG_DIM),
                );

            col = col.push(Space::new().height(SPACE_1));
            let mut stars_row = row![].spacing(SPACE_1);
            for star in 1..=5i32 {
                let filled = self.detail.rating.map(|r| r >= star).unwrap_or(false);
                stars_row = stars_row.push(
                    button(
                        text(if filled { "★" } else { "☆" })
                            .size(TEXT_STAR)
                            .color(if filled { STAR_GOLD } else { FG_DIM }),
                    )
                    .on_press(Msg::SetDetailRating(star))
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: FG_DIM,
                        border: Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                );
            }
            col = col.push(stars_row);
        } else if is_batch {
            let n = self.detail.batch_file_ids.len();
            col = col.push(
                text(format!("{n} photos selected")).size(TEXT_BASE),
            );
        }

        // Editable descriptive metadata (caption/creator/rights) — the core of
        // cataloging for research/archival. Enter saves; in batch it applies to
        // all selected.
        if has_tags {
            col = col.push(Space::new().height(SPACE_1));
            col = col.push(text("Description").size(TEXT_SM).color(FG_DIM));
            let field = |label: &'static str, value: &str, f: DetailField| -> Element<'_, Msg> {
                column![
                    text(label).size(TEXT_XS).color(FG_MUTED),
                    text_input("", value)
                        .on_input(move |s| Msg::DetailFieldChanged(f, s))
                        .on_submit(Msg::SaveDetailField(f))
                        .padding([SPACE_1, SPACE_1_5])
                        .size(TEXT_SM),
                ]
                .spacing(2.0)
                .into()
            };
            col = col
                .push(field("Title", &self.detail.title_input, DetailField::Title))
                .push(field("Caption", &self.detail.caption_input, DetailField::Caption))
                .push(field("Creator", &self.detail.creator_input, DetailField::Creator))
                .push(field("Copyright", &self.detail.rights_input, DetailField::Rights));
        }

        if has_tags {
            col = col.push(Space::new().height(SPACE_1));
            let tag_label = if is_batch { "Shared Tags" } else { "Tags" };
            col = col.push(
                row![
                    text(tag_label).size(TEXT_SM).color(FG_DIM),
                    Space::new().width(Length::Fill),
                    button(text("Browse").size(TEXT_XS).color(FG_DIM))
                        .on_press(Msg::OpenTagBrowser)
                        .style(ghost_btn_style),
                ]
                .align_y(Alignment::Center),
            );

            for tag in &self.detail.tags {
                let mut tag_row = row![render_tag_name(tag.as_str())].align_y(Alignment::Center);
                tag_row = tag_row
                    .push(Space::new().width(Length::Fill))
                    .push(
                        super::styles::icon_btn("×", Msg::RemoveDetailTag(tag.clone())),
                    );
                col = col.push(
                    container(tag_row)
                    .padding([SPACE_0_5, SPACE_1])
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 0.05,
                        })),
                        border: Border { radius: 3.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                );
            }

            if !self.detail.tag_input.is_empty() {
                let input_lower = self.detail.tag_input.to_lowercase();
                let mut scored: Vec<(&String, u8)> = self
                    .detail
                    .all_tags
                    .iter()
                    .filter(|t| !self.detail.tags.contains(t))
                    .filter_map(|t| {
                        let tl = t.to_lowercase();
                        if tl.starts_with(&input_lower) {
                            Some((t, 0u8))
                        } else if tl.contains(&input_lower) {
                            Some((t, 1))
                        } else {
                            let leaf = tl.rsplit('/').next().unwrap_or(&tl);
                            if leaf.starts_with(&input_lower) {
                                Some((t, 0))
                            } else {
                                None
                            }
                        }
                    })
                    .collect();
                scored.sort_by_key(|&(_, rank)| rank);
                let suggestions: Vec<&String> = scored.into_iter().map(|(t, _)| t).take(5).collect();
                if !suggestions.is_empty() {
                    let chips: Vec<Element<Msg>> = suggestions
                        .into_iter()
                        .map(|tag| {
                            button(text(tag.as_str()).size(TEXT_XS))
                                .on_press(Msg::AddDetailTagDirect(tag.clone()))
                                .style(ghost_btn_style)
                                .into()
                        })
                        .collect();
                    col = col.push(row(chips).spacing(SPACE_1).wrap());
                }
            }

            let recent: Vec<&String> = self
                .detail
                .recent_tags
                .iter()
                .filter(|t| !self.detail.tags.contains(t))
                .take(5)
                .collect();
            if !recent.is_empty() {
                let mut recent_row = row![text("Recent").size(TEXT_XS).color(FG_DIM)]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center);
                for tag in recent {
                    recent_row = recent_row.push(
                        button(text(tag.as_str()).size(TEXT_XS))
                            .on_press(Msg::AddDetailTagDirect(tag.clone()))
                            .style(ghost_btn_style),
                    );
                }
                col = col.push(recent_row.wrap());
            }

            col = col.push(
                text_input("Add tag…", &self.detail.tag_input)
                    .on_input(Msg::DetailTagInputChanged)
                    .on_submit(Msg::AddDetailTag)
                    .padding([SPACE_1, SPACE_1_5])
                    .size(TEXT_SM)
                    .width(Length::Fill),
            );

            if self.grid_selected.len() > 1 {
                col = col.push(
                    text(format!("Applies to {} photos", self.grid_selected.len()))
                        .size(TEXT_XS)
                        .color(FG_MUTED),
                );
            }
        }

        if self.detail_file().is_some() {
            if let Some(title) = &self.detail.title {
                col = col.push(Space::new().height(SPACE_1));
                col = col.push(
                    row![
                        text("Title").size(TEXT_SM).color(FG_DIM),
                        Space::new().width(Length::Fill),
                        text("read-only").size(TEXT_XS).color(FG_MUTED),
                    ]
                    .align_y(Alignment::Center),
                );
                col = col.push(text(title).size(TEXT_MD));
            }

            if let Some(label) = &self.detail.label {
                col = col.push(
                    row![
                        text(format!("Label  {label}")).size(TEXT_SM).color(FG_DIM),
                        Space::new().width(Length::Fill),
                        text("read-only").size(TEXT_XS).color(FG_MUTED),
                    ]
                    .align_y(Alignment::Center),
                );
            }

            if let Some(tech) = &self.detail.exif_tech {
                col = col.push(Space::new().height(SPACE_2));
                col = col.push(text("Camera").size(TEXT_SM).color(FG_DIM));

                let camera = match (&tech.camera_make, &tech.camera_model) {
                    (Some(make), Some(model)) => Some(format!("{make} {model}")),
                    (None, Some(model)) => Some(model.clone()),
                    (Some(make), None) => Some(make.clone()),
                    _ => None,
                };
                if let Some(cam) = camera {
                    col = col.push(text(cam).size(TEXT_SM).color(FG_MUTED));
                }
                if let Some(lens) = &tech.lens_model {
                    col = col.push(text(lens).size(TEXT_SM).color(FG_MUTED));
                }

                let tech_str = [
                    tech.focal_length_mm.map(|fl| format!("{:.0}mm", fl)),
                    tech.aperture.map(|ap| format!("f/{:.1}", ap)),
                    tech.shutter_speed.as_ref().map(|ss| format!("{}s", ss)),
                    tech.iso.map(|iso| format!("ISO {iso}")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("  ");
                if !tech_str.is_empty() {
                    col = col.push(text(tech_str).size(TEXT_SM).color(FG_MUTED));
                }

                if let Some(flash) = tech.flash {
                    col = col.push(
                        text(if flash { "Flash fired" } else { "No flash" })
                            .size(TEXT_SM)
                            .color(FG_MUTED),
                    );
                }
            }

            if let Some(file) = self.detail_file() {
                if let (Some(lat), Some(lon)) = (file.gps_lat, file.gps_lon) {
                    let lat_str = if lat >= 0.0 { format!("{:.4}° N", lat) } else { format!("{:.4}° S", -lat) };
                    let lon_str = if lon >= 0.0 { format!("{:.4}° E", lon) } else { format!("{:.4}° W", -lon) };
                    col = col.push(Space::new().height(SPACE_2));
                    col = col.push(text("Location").size(TEXT_SM).color(FG_DIM));
                    col = col.push(text(format!("{lat_str}  {lon_str}")).size(TEXT_SM).color(FG_MUTED));
                }
            }
        }

        if !has_tags {
            col = col.push(
                text("Select a photo to see details")
                    .size(TEXT_MD)
                    .color(FG_DIM),
            );
        }

        container(
            scrollable(col)
                .height(Length::Fill)
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(4).scroller_width(4),
                )),
        )
        .width(SIDEBAR_WIDTH)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_SIDEBAR)),
            ..Default::default()
        })
        .into()
    }
}

fn render_tag_name<'a>(tag: &'a str) -> Element<'a, Msg> {
    let parts: Vec<&str> = tag.split('/').collect();
    let n = parts.len();
    if n == 1 {
        return text(tag).size(TEXT_SM).color(FG).into();
    }
    let mut r = row![].spacing(0);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            r = r.push(text("/").size(TEXT_SM).color(FG_DIM));
        }
        let color = if i == n - 1 { FG } else { FG_DIM };
        r = r.push(text(*part).size(TEXT_SM).color(color));
    }
    r.into()
}
