use iced::widget::scrollable::Direction;
use iced::{
    widget::{button, column, container, image, row, scrollable, stack, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::models::ThumbnailState;

use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_CRITERIA, BG_GRID,
    BG_TILE_LOADING, BORDER, ERR, FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1,
    SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, STAR_GOLD, TEXT_BASE,
    TEXT_MD, TEXT_SM, TEXT_STAR, TEXT_XS, TILE_CORNER,
};
use crate::app::{
    format_file_size, parse_date_str, sort_field_label, unix_to_date_str, App, Msg, BUFFER_ROWS,
    GRID_PADDING, TILE_GAP,
};

impl App {
    pub(super) fn view_grid(&self) -> Element<'_, Msg> {
        let show_criteria = self.criteria.show;
        let criteria_active = self.criteria_has_any();
        let sort_label = format!(
            "{} {}",
            sort_field_label(self.sort_by),
            if self.sort_asc { "▲" } else { "▼" }
        );

        let filter_toolbar = container(
            row![
                text_input("Search photos…", &self.search_text)
                    .on_input(Msg::SearchChanged)
                    .padding([SPACE_1_5, SPACE_2_5])
                    .size(TEXT_BASE)
                    .width(Length::Fill),
                button(
                    text(if criteria_active {
                        "Filters ●"
                    } else {
                        "Filters"
                    })
                    .size(TEXT_MD)
                )
                .on_press(Msg::ToggleCriteria)
                .style(move |t: &Theme, s| {
                    if show_criteria {
                        active_chip_style(t, s)
                    } else {
                        ghost_btn_style(t, s)
                    }
                }),
                button(text(sort_label).size(TEXT_MD))
                    .on_press(Msg::SortCycleAll)
                    .style(ghost_btn_style),
            ]
            .spacing(SPACE_2)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_1_5, SPACE_3])
        .width(Length::Fill);

        let empty_or_grid: Element<Msg> = if self.files.is_empty() {
            container(text("No photos in this view").size(TEXT_BASE).color(FG_DIM))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        } else {
            let cols = self.cols().max(1);
            let tile_px = self.tile_px;
            let step = tile_px + TILE_GAP;
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

            let mut row_elements: Vec<Element<Msg>> = Vec::new();
            for r in first_row..last_row {
                let start = r * cols;
                let end = (start + cols).min(total);
                let tiles: Vec<Element<Msg>> = (start..end).map(|i| self.view_tile(i)).collect();
                let padding = cols - tiles.len();
                let mut all_tiles = tiles;
                for _ in 0..padding {
                    all_tiles.push(Space::new().width(tile_px).into());
                }
                row_elements.push(row(all_tiles).spacing(TILE_GAP).into());
            }

            let grid_content = column![
                Space::new().height(top_space + GRID_PADDING),
                column(row_elements).spacing(TILE_GAP),
                Space::new().height(bottom_space + GRID_PADDING),
            ]
            .padding([0, GRID_PADDING as u16]);

            scrollable(grid_content)
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
        if self.criteria.show {
            grid_col = grid_col.push(self.view_criteria_panel());
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
        let dragging = self.dragging_ids.contains(&file.id);

        let thumb_state = self.thumbnails.get(&file.id);

        let tile_content: Element<Msg> = match thumb_state {
            Some(ThumbnailState::Ready(path)) => image(image::Handle::from_path(path))
                .width(self.tile_px)
                .height(self.tile_px)
                .content_fit(iced::ContentFit::Cover)
                .into(),
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

        let overlay_color = if dragging {
            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 }
        } else if selected {
            Color { a: 0.28, ..ACCENT }
        } else {
            Color::TRANSPARENT
        };

        let (ring_color, ring_width) = if selected && !dragging {
            (ACCENT, 3.0_f32)
        } else {
            (Color::TRANSPARENT, 0.0)
        };

        let tile_px = self.tile_px;
        stack(vec![
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
        ])
        .into()
    }

    pub(super) fn view_criteria_panel(&self) -> Element<'_, Msg> {
        let mut col = column![].spacing(SPACE_1_5).padding([SPACE_1_5, SPACE_3]);

        let mut tags_row = row![].spacing(SPACE_1_5).align_y(Alignment::Center);
        for tag in &self.criteria.tags {
            tags_row = tags_row.push(
                button(text(format!("{tag} ×")).size(TEXT_SM))
                    .on_press(Msg::RemoveCriteriaTag(tag.clone()))
                    .style(active_chip_style),
            );
        }
        tags_row = tags_row.push(
            text_input("+ tag", &self.criteria.tag_input)
                .on_input(Msg::CriteriaTagInputChanged)
                .on_submit(Msg::AddCriteriaTag)
                .padding([SPACE_1, SPACE_1_5])
                .size(TEXT_SM)
                .width(120),
        );
        col = col.push(tags_row);

        let from_err = !self.criteria.date_from.is_empty()
            && parse_date_str(&self.criteria.date_from).is_none();
        let to_err =
            !self.criteria.date_to.is_empty() && parse_date_str(&self.criteria.date_to).is_none();
        let mut date_row = row![].spacing(SPACE_1_5).align_y(Alignment::Center);
        date_row = date_row.push(text("From").size(TEXT_SM).color(FG_DIM));
        date_row = date_row.push(
            text_input("YYYY-MM-DD", &self.criteria.date_from)
                .on_input(Msg::CriteriaDateFromChanged)
                .padding([SPACE_1, SPACE_1_5])
                .size(TEXT_SM)
                .width(100),
        );
        if from_err {
            date_row = date_row.push(text("✕ bad date").size(TEXT_XS).color(ERR));
        }
        date_row = date_row.push(text("To").size(TEXT_SM).color(FG_DIM));
        date_row = date_row.push(
            text_input("YYYY-MM-DD", &self.criteria.date_to)
                .on_input(Msg::CriteriaDateToChanged)
                .padding([SPACE_1, SPACE_1_5])
                .size(TEXT_SM)
                .width(100),
        );
        if to_err {
            date_row = date_row.push(text("✕ bad date").size(TEXT_XS).color(ERR));
        }
        col = col.push(date_row);
        if from_err || to_err {
            col = col.push(
                text("Format: YYYY-MM-DD  e.g. 2024-06-15")
                    .size(TEXT_XS)
                    .color(ERR),
            );
        }

        let mut ext_row = row![text("Type").size(TEXT_SM).color(FG_DIM)]
            .spacing(SPACE_1)
            .align_y(Alignment::Center);
        for ext in ["jpg", "png", "webp", "gif"] {
            let active = self.criteria.exts.contains(ext);
            ext_row = ext_row.push(
                button(text(format!(".{}", ext.to_uppercase())).size(TEXT_SM))
                    .on_press(Msg::ToggleCriteriaExt(ext.to_string()))
                    .style(if active { active_chip_style } else { ghost_btn_style }),
            );
        }
        col = col.push(ext_row);

        if self.criteria_has_any() {
            let is_smart = self.current_album_is_smart();
            let mut action_row = row![
                button(text("Clear").size(TEXT_SM))
                    .on_press(Msg::ClearCriteria)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center);

            if is_smart {
                if self.smart_album_dirty {
                    action_row = action_row.push(
                        text("Unsaved changes").size(TEXT_SM).color(ERR),
                    );
                }
                action_row = action_row.push(
                    button(text("Update Smart Album").size(TEXT_SM))
                        .on_press(Msg::UpdateSmartAlbum)
                        .style(ghost_btn_style),
                );
            } else if let Some(ref name_input) = self.criteria.save_smart_input {
                action_row = action_row
                    .push(
                        text_input("Album name…", name_input)
                            .on_input(Msg::SmartAlbumNameChanged)
                            .on_submit(Msg::ConfirmSmartAlbum)
                            .padding([SPACE_1, SPACE_1_5])
                            .size(TEXT_SM)
                            .width(120),
                    )
                    .push(
                        button(text("Save").size(TEXT_SM))
                            .on_press(Msg::ConfirmSmartAlbum)
                            .style(ghost_btn_style),
                    );
            } else {
                action_row = action_row.push(
                    button(text("Save as Smart Album").size(TEXT_SM))
                        .on_press(Msg::SaveAsSmartAlbum)
                        .style(ghost_btn_style),
                );
            }
            col = col.push(action_row);
        }

        container(col)
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_CRITERIA)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    pub(super) fn view_detail(&self) -> Element<'_, Msg> {
        use super::styles::BG_SIDEBAR;
        use crate::app::SIDEBAR_WIDTH;

        let file = self.detail_file();

        let mut col = column![text("Info").size(TEXT_SM).color(FG_DIM)]
            .spacing(SPACE_2)
            .padding(SPACE_3);

        if let Some(file) = file {
            col = col.push(text(&file.name).size(TEXT_BASE));

            let size_str = format_file_size(file.size_bytes);
            let date_str = unix_to_date_str(file.mtime_unix);

            col = col
                .push(text(format!("Size  {size_str}")).size(TEXT_SM).color(FG_DIM))
                .push(text(format!("Date  {date_str}")).size(TEXT_SM).color(FG_DIM))
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

            col = col.push(Space::new().height(SPACE_1));
            col = col.push(text("Tags").size(TEXT_SM).color(FG_DIM));

            for tag in &self.detail.tags {
                col = col.push(
                    container(
                        row![
                            text(tag).size(TEXT_SM),
                            Space::new().width(Length::Fill),
                            button(text("×").size(TEXT_XS).color(FG_DIM))
                                .on_press(Msg::RemoveDetailTag(tag.clone()))
                                .style(|_: &Theme, _| button::Style {
                                    background: None,
                                    text_color: FG_DIM,
                                    border: Border::default(),
                                    shadow: iced::Shadow::default(),
                                    snap: false,
                                }),
                        ]
                        .align_y(Alignment::Center),
                    )
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

            col = col.push(
                text_input("Add tag…", &self.detail.tag_input)
                    .on_input(Msg::DetailTagInputChanged)
                    .on_submit(Msg::AddDetailTag)
                    .padding([SPACE_1, SPACE_1_5])
                    .size(TEXT_SM)
                    .width(Length::Fill),
            );

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
        } else {
            col = col.push(
                text(if self.grid_selected.is_empty() {
                    "Select a photo to see details"
                } else {
                    "Select a single photo"
                })
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
