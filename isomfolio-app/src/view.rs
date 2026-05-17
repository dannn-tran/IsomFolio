use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding,
    widget::{
        column, container, image, mouse_area, row, scrollable, text, text_input, Space,
        button,
    },
    Theme,
};
use iced::widget::scrollable::Direction;

use isomfolio_core::models::ThumbnailState;

use crate::app::{
    App, Msg, SidebarItem, ViewMode, SIDEBAR_WIDTH, GRID_PADDING, TILE_GAP,
    ALBUM_ITEM_HEIGHT, BUFFER_ROWS, sort_field_label, unix_to_date_str, format_file_size,
    parse_date_str,
};

const BG_SIDEBAR: Color   = Color { r: 0.11, g: 0.11, b: 0.14, a: 1.0 };
const BG_GRID: Color      = Color { r: 0.08, g: 0.08, b: 0.10, a: 1.0 };
const BG_STATUSBAR: Color = Color { r: 0.07, g: 0.07, b: 0.09, a: 1.0 };
const BG_CRITERIA: Color  = Color { r: 0.10, g: 0.10, b: 0.13, a: 1.0 };
const FG: Color           = Color { r: 0.90, g: 0.90, b: 0.92, a: 1.0 };
const FG_DIM: Color       = Color { r: 0.55, g: 0.55, b: 0.60, a: 1.0 };
const ACCENT: Color       = Color { r: 0.20, g: 0.55, b: 0.95, a: 1.0 };
const ALBUM_HOVER: Color  = Color { r: 0.10, g: 0.25, b: 0.50, a: 1.0 };
const SEL_RING: Color     = Color::WHITE;
const TILE_CORNER: f32    = 4.0;
const STAR_GOLD: Color    = Color { r: 1.0, g: 0.82, b: 0.0, a: 1.0 };
const ERR: Color          = Color { r: 0.95, g: 0.35, b: 0.35, a: 1.0 };

impl App {
    pub fn view(&self) -> Element<'_, Msg> {
        if matches!(self.view_mode, ViewMode::Loupe) {
            return self.view_loupe();
        }

        let dragging = self.drag.as_ref().map(|d| d.active).unwrap_or(false);
        let drag_hover = self.drag_hover_album.clone();
        let status = if dragging {
            let count = self.dragging_ids.len();
            match &drag_hover {
                Some(id) => {
                    let name = self.albums.iter()
                        .find(|a| &a.id == id)
                        .map(|a| a.name.as_str())
                        .unwrap_or("?");
                    format!("Dragging {count} — drop on \"{name}\"")
                }
                None => format!("Dragging {count} photo(s)…"),
            }
        } else if !self.status.is_empty() {
            self.status.clone()
        } else {
            "Click to select · Cmd+click multi-select · Drag to album".to_string()
        };

        let scan_label = if self.is_scanning { "Scanning…" } else { "Scan Folder" };
        let scan_msg = if self.is_scanning { Msg::NoOp } else { Msg::ScanPickFolder };

        let sort_label = format!("{} {}", sort_field_label(self.sort_by), if self.sort_asc { "▲" } else { "▼" });

        let show_criteria = self.show_criteria;
        let show_detail = self.show_detail;

        let remove_btn: Option<Element<Msg>> =
            if matches!(self.selected_item, SidebarItem::Album(_)) && !self.grid_selected.is_empty() {
                let n = self.grid_selected.len();
                Some(
                    button(text(format!("Remove {n}")).size(12))
                        .on_press(Msg::RemoveFromAlbum)
                        .style(ghost_btn_style)
                        .into(),
                )
            } else {
                None
            };

        let mut status_row = row![
            text(status).size(12).color(FG_DIM),
            Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        if let Some(btn) = remove_btn {
            status_row = status_row.push(btn);
        }

        status_row = status_row
            .push(
                button(text(scan_label).size(12))
                    .on_press(scan_msg)
                    .style(ghost_btn_style),
            )
            .push(
                button(text("Filters").size(12))
                    .on_press(Msg::ToggleCriteria)
                    .style(move |t: &Theme, s| {
                        if show_criteria { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                    }),
            )
            .push(
                button(text("Info").size(12))
                    .on_press(Msg::ToggleDetail)
                    .style(move |t: &Theme, s| {
                        if show_detail { active_chip_style(t, s) } else { ghost_btn_style(t, s) }
                    }),
            )
            .push(
                button(text(sort_label).size(12))
                    .on_press(Msg::SortFieldCycle)
                    .style(ghost_btn_style),
            )
            .push(
                button(text(if self.sort_asc { "▲" } else { "▼" }).size(12))
                    .on_press(Msg::SortDirToggle)
                    .style(ghost_btn_style),
            )
            .push(
                button(text("−").size(14))
                    .on_press(Msg::TileSizeDown)
                    .style(ghost_btn_style),
            )
            .push(text(format!("{}px", self.tile_px as u32)).size(12).color(FG_DIM))
            .push(
                button(text("+").size(14))
                    .on_press(Msg::TileSizeUp)
                    .style(ghost_btn_style),
            );

        let status_bar = container(status_row)
            .padding([4, 12])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                ..Default::default()
            });

        let mut main_row = row![self.view_sidebar(), self.view_grid()]
            .height(Length::Fill);
        if self.show_detail {
            main_row = main_row.push(self.view_detail());
        }

        column![main_row, status_bar].into()
    }

    fn view_loupe(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe_idx.min(total.saturating_sub(1));

        let img_element: Element<Msg> = if let Some(file) = self.files.get(idx) {
            image(image::Handle::from_path(&file.path))
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            Space::new().width(Length::Fill).height(Length::Fill).into()
        };

        let filename = self.files.get(idx)
            .map(|f| f.name.as_str())
            .unwrap_or("");
        let counter = if total > 0 {
            format!("{} / {}", idx + 1, total)
        } else {
            String::new()
        };

        let top_bar = container(
            row![
                button(text("✕").size(14).color(FG))
                    .on_press(Msg::OpenLoupe)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 })),
                        text_color: FG,
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                Space::new().width(Length::Fill),
                text(filename).size(13).color(FG),
                Space::new().width(Length::Fill),
                text(counter).size(12).color(FG_DIM),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 })),
            ..Default::default()
        });

        let bottom_bar = container(
            row![
                Space::new().width(Length::Fill),
                button(text("‹ Prev").size(13))
                    .on_press(Msg::Navigate { dx: -1, dy: 0 })
                    .style(ghost_btn_style),
                button(text("Next ›").size(13))
                    .on_press(Msg::Navigate { dx: 1, dy: 0 })
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.7 })),
            ..Default::default()
        });

        container(
            column![top_bar, img_element, bottom_bar]
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(Color { r: 0.03, g: 0.03, b: 0.03, a: 1.0 })),
            ..Default::default()
        })
        .into()
    }
}

impl App {
    fn view_sidebar(&self) -> Element<'_, Msg> {
        let drag_hover = self.drag_hover_album.clone();

        let all_sel = self.selected_item == SidebarItem::AllFiles;
        let all_row = sidebar_row_button(
            "All Photos".to_string(),
            all_sel,
            false,
            Msg::SidebarItemClicked(SidebarItem::AllFiles),
        );

        let folder_section: Vec<Element<Msg>> = self
            .folders
            .iter()
            .map(|(path, count)| {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str());
                let sel = self.selected_item == SidebarItem::Folder(path.clone());
                folder_sidebar_row(name.to_string(), path.clone(), *count, sel)
            })
            .collect();

        let albums_header: Element<Msg> = row![
            text("Albums").size(11).color(FG_DIM),
            Space::new().width(Length::Fill),
            button(text("+").size(13))
                .on_press(Msg::StartCreateAlbum)
                .style(ghost_btn_style),
        ]
        .align_y(Alignment::Center)
        .into();

        let mut content = column![
            text("Library").size(11).color(FG_DIM),
            all_row,
        ]
        .spacing(2);

        if !self.folders.is_empty() {
            content = content
                .push(Space::new().height(8))
                .push(text("Folders").size(11).color(FG_DIM));
            for row in folder_section {
                content = content.push(row);
            }
        }

        content = content
            .push(Space::new().height(8))
            .push(albums_header);

        for album in &self.albums {
            let sel = self.selected_item == SidebarItem::Album(album.id.clone());
            let hovered = drag_hover.as_deref() == Some(album.id.as_str());
            let count = self.album_counts.get(&album.id).copied().unwrap_or(0);
            let is_smart = matches!(album.kind, isomfolio_core::models::AlbumKind::Smart(_));

            if self.rename_album_id.as_deref() == Some(album.id.as_str()) {
                content = content.push(
                    container(
                        text_input(&album.name, &self.rename_album_input)
                            .on_input(Msg::RenameAlbumInputChanged)
                            .on_submit(Msg::ConfirmRenameAlbum)
                            .padding([6, 8])
                            .size(13),
                    )
                    .height(ALBUM_ITEM_HEIGHT)
                    .align_y(Alignment::Center)
                    .padding([0, 4]),
                );
            } else {
                let album_id = album.id.clone();
                content = content.push(
                    mouse_area(album_sidebar_row(
                        album.name.clone(),
                        album.id.clone(),
                        count,
                        sel,
                        hovered,
                        is_smart,
                    ))
                    .on_enter(Msg::DragHoverAlbum(Some(album_id)))
                    .on_exit(Msg::DragHoverAlbum(None)),
                );
            }
        }

        if let Some(ref input_val) = self.create_album_input {
            content = content.push(
                text_input("Album name…", input_val)
                    .on_input(Msg::CreateAlbumInputChanged)
                    .on_submit(Msg::ConfirmCreateAlbum)
                    .padding([6, 8])
                    .size(13),
            );
        }

        container(scrollable(content.spacing(2).padding(12)))
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_SIDEBAR)),
                ..Default::default()
            })
            .into()
    }
}

fn sidebar_row_button<'a>(
    label: String,
    selected: bool,
    drop_hover: bool,
    msg: Msg,
) -> Element<'a, Msg> {
    let bg = if drop_hover {
        ALBUM_HOVER
    } else if selected {
        Color { r: ACCENT.r * 0.6, g: ACCENT.g * 0.6, b: ACCENT.b * 0.6, a: 0.4 }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if drop_hover || selected { ACCENT } else { Color::TRANSPARENT };

    container(
        button(
            text(label).size(13).color(if selected || drop_hover { Color::WHITE } else { FG }),
        )
        .on_press(msg)
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: FG,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        })
        .width(Length::Fill),
    )
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0, 4])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border_color,
            width: if drop_hover { 2.0 } else { 0.0 },
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn folder_sidebar_row<'a>(
    name: String,
    path: String,
    count: usize,
    selected: bool,
) -> Element<'a, Msg> {
    let bg = if selected {
        Color { r: ACCENT.r * 0.6, g: ACCENT.g * 0.6, b: ACCENT.b * 0.6, a: 0.4 }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if selected { ACCENT } else { Color::TRANSPARENT };
    let text_color = if selected { Color::WHITE } else { FG };

    let count_str = if count > 0 { format!("  {count}") } else { String::new() };
    let label_btn = button(
        row![
            text(format!("{name}{count_str}")).size(13).color(text_color),
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

    let remove_btn = button(text("×").size(11).color(FG_DIM))
        .on_press(Msg::RemoveFolder(path))
        .style(|_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: FG_DIM,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        });

    container(row![label_btn, remove_btn].align_y(Alignment::Center))
        .height(ALBUM_ITEM_HEIGHT)
        .align_y(Alignment::Center)
        .padding([0, 4])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 0.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn album_sidebar_row<'a>(
    label: String,
    album_id: String,
    count: usize,
    selected: bool,
    drop_hover: bool,
    is_smart: bool,
) -> Element<'a, Msg> {
    let text_color = if selected || drop_hover { Color::WHITE } else { FG };
    let bg = if drop_hover {
        ALBUM_HOVER
    } else if selected {
        Color { r: ACCENT.r * 0.6, g: ACCENT.g * 0.6, b: ACCENT.b * 0.6, a: 0.4 }
    } else {
        Color::TRANSPARENT
    };
    let border_color = if drop_hover || selected { ACCENT } else { Color::TRANSPARENT };

    let name_press = if selected {
        Msg::StartRenameAlbum(album_id.clone())
    } else {
        Msg::SidebarItemClicked(SidebarItem::Album(album_id.clone()))
    };

    let smart_indicator = if is_smart { "⚡ " } else { "" };
    let count_str = if count > 0 { format!("  {count}") } else { String::new() };
    let name_btn = button(
        row![
            text(format!("{smart_indicator}{label}")).size(13).color(text_color),
            text(count_str).size(11).color(FG_DIM),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(name_press)
    .width(Length::Fill)
    .style(|_: &Theme, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: FG,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    });

    let delete_btn = button(text("×").size(11).color(FG_DIM))
        .on_press(Msg::DeleteAlbum(album_id))
        .style(|_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: FG_DIM,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        });

    container(
        row![name_btn, delete_btn].align_y(Alignment::Center),
    )
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0, 4])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border_color,
            width: if drop_hover { 2.0 } else { 0.0 },
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .into()
}

impl App {
    fn view_criteria_panel(&self) -> Element<'_, Msg> {
        let mut col = column![].spacing(6).padding([6, 12]);

        // Tags row: active tag chips + input
        let mut tags_row = row![].spacing(6).align_y(Alignment::Center);
        for tag in &self.criteria_tags {
            tags_row = tags_row.push(
                button(text(format!("{tag} ×")).size(11))
                    .on_press(Msg::RemoveCriteriaTag(tag.clone()))
                    .style(active_chip_style),
            );
        }
        tags_row = tags_row.push(
            text_input("+ tag", &self.criteria_tag_input)
                .on_input(Msg::CriteriaTagInputChanged)
                .on_submit(Msg::AddCriteriaTag)
                .padding([3, 6])
                .size(11)
                .width(80),
        );
        col = col.push(tags_row);

        // Date range row
        let from_err = !self.criteria_date_from.is_empty()
            && parse_date_str(&self.criteria_date_from).is_none();
        let to_err = !self.criteria_date_to.is_empty()
            && parse_date_str(&self.criteria_date_to).is_none();
        let mut date_row = row![]
            .spacing(6)
            .align_y(Alignment::Center);
        date_row = date_row.push(text("From").size(11).color(FG_DIM));
        date_row = date_row.push(
            text_input("YYYY-MM-DD", &self.criteria_date_from)
                .on_input(Msg::CriteriaDateFromChanged)
                .padding([3, 6])
                .size(11)
                .width(100),
        );
        if from_err {
            date_row = date_row.push(text("✕").size(10).color(ERR));
        }
        date_row = date_row.push(text("To").size(11).color(FG_DIM));
        date_row = date_row.push(
            text_input("YYYY-MM-DD", &self.criteria_date_to)
                .on_input(Msg::CriteriaDateToChanged)
                .padding([3, 6])
                .size(11)
                .width(100),
        );
        if to_err {
            date_row = date_row.push(text("✕").size(10).color(ERR));
        }
        col = col.push(date_row);

        // Extension toggles
        let mut ext_row = row![text("Type").size(11).color(FG_DIM)]
            .spacing(4)
            .align_y(Alignment::Center);
        for ext in ["jpg", "png", "webp", "gif"] {
            let active = self.criteria_exts.contains(ext);
            ext_row = ext_row.push(
                button(text(ext).size(11))
                    .on_press(Msg::ToggleCriteriaExt(ext.to_string()))
                    .style(if active { active_chip_style } else { inactive_chip_style }),
            );
        }
        col = col.push(ext_row);

        // Action row (only when any criteria active)
        if self.criteria_has_any() {
            let is_smart = self.current_album_is_smart();
            let mut action_row = row![
                button(text("Clear").size(11))
                    .on_press(Msg::ClearCriteria)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(6)
            .align_y(Alignment::Center);

            if is_smart {
                action_row = action_row.push(
                    button(text("Update Smart Album").size(11))
                        .on_press(Msg::UpdateSmartAlbum)
                        .style(ghost_btn_style),
                );
            } else if let Some(ref name_input) = self.save_smart_input {
                action_row = action_row
                    .push(
                        text_input("Album name…", name_input)
                            .on_input(Msg::SmartAlbumNameChanged)
                            .on_submit(Msg::ConfirmSmartAlbum)
                            .padding([3, 6])
                            .size(11)
                            .width(120),
                    )
                    .push(
                        button(text("Save").size(11))
                            .on_press(Msg::ConfirmSmartAlbum)
                            .style(ghost_btn_style),
                    );
            } else {
                action_row = action_row.push(
                    button(text("Save as Smart Album").size(11))
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
                    color: Color { r: 0.20, g: 0.20, b: 0.26, a: 1.0 },
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_detail(&self) -> Element<'_, Msg> {
        let file = self.detail_file();

        let mut col = column![text("Info").size(11).color(FG_DIM)]
            .spacing(8)
            .padding(12);

        if let Some(file) = file {
            col = col.push(text(&file.name).size(13));

            let size_str = format_file_size(file.size_bytes);
            let date_str = unix_to_date_str(file.mtime_unix);

            col = col
                .push(text(format!("Size  {size_str}")).size(11).color(FG_DIM))
                .push(text(format!("Date  {date_str}")).size(11).color(FG_DIM))
                .push(text(format!("Type  .{}", file.ext.to_uppercase())).size(11).color(FG_DIM));

            // Star rating
            col = col.push(Space::new().height(4));
            let mut stars_row = row![].spacing(2);
            for star in 1..=5i32 {
                let filled = self.detail_rating.map(|r| r >= star).unwrap_or(false);
                stars_row = stars_row.push(
                    button(
                        text(if filled { "★" } else { "☆" })
                            .size(18)
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

            // Tags section
            col = col.push(Space::new().height(4));
            col = col.push(text("Tags").size(11).color(FG_DIM));

            for tag in &self.detail_tags {
                col = col.push(
                    container(
                        row![
                            text(tag).size(11),
                            Space::new().width(Length::Fill),
                            button(text("×").size(10).color(FG_DIM))
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
                    .padding([2, 4])
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 })),
                        border: Border { radius: 3.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                );
            }

            col = col.push(
                text_input("Add tag…", &self.detail_tag_input)
                    .on_input(Msg::DetailTagInputChanged)
                    .on_submit(Msg::AddDetailTag)
                    .padding([4, 6])
                    .size(11)
                    .width(Length::Fill),
            );

            if let Some(title) = &self.detail_title {
                col = col.push(Space::new().height(4));
                col = col.push(text("Title").size(11).color(FG_DIM));
                col = col.push(text(title).size(12));
            }

            if let Some(label) = &self.detail_label {
                col = col.push(
                    text(format!("Label  {label}")).size(11).color(FG_DIM),
                );
            }
        } else {
            col = col.push(
                text(if self.grid_selected.is_empty() {
                    "Select a photo to see details"
                } else {
                    "Select a single photo"
                })
                .size(12)
                .color(FG_DIM),
            );
        }

        container(col)
            .width(SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_SIDEBAR)),
                ..Default::default()
            })
            .into()
    }

    pub fn view_grid(&self) -> Element<'_, Msg> {
        let search_bar = container(
            text_input("Search photos…", &self.search_text)
                .on_input(Msg::SearchChanged)
                .padding([6, 10])
                .size(13)
                .width(Length::Fill),
        )
        .padding([5, 12])
        .width(Length::Fill);

        let empty_or_grid: Element<Msg> = if self.files.is_empty() {
            container(
                text("No photos in this view").size(16).color(FG_DIM),
            )
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
                let tiles: Vec<Element<Msg>> = (start..end)
                    .map(|i| self.view_tile(i))
                    .collect();
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

        let mut grid_col = column![search_bar];
        if self.show_criteria {
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

    fn view_tile(&self, idx: usize) -> Element<'_, Msg> {
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
                let bg = if dragging {
                    Color { r: 0.3, g: 0.3, b: 0.35, a: 0.5 }
                } else {
                    Color { r: 0.20, g: 0.20, b: 0.25, a: 1.0 }
                };
                container(text(&file.name).size(10).color(FG_DIM))
                    .width(self.tile_px)
                    .height(self.tile_px)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::End)
                    .padding(Padding { top: 0.0, right: 4.0, bottom: 6.0, left: 4.0 })
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(bg)),
                        border: Border { radius: TILE_CORNER.into(), ..Default::default() },
                        ..Default::default()
                    })
                    .into()
            }
        };

        let (border_color, border_width) = if selected && !dragging {
            (SEL_RING, 2.5_f32)
        } else {
            (Color::TRANSPARENT, 0.0)
        };

        container(tile_content)
            .width(self.tile_px)
            .height(self.tile_px)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    color: border_color,
                    width: border_width,
                    radius: TILE_CORNER.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn ghost_btn_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 })),
        text_color: FG_DIM,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn active_chip_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(ACCENT)),
        text_color: Color::WHITE,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn inactive_chip_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 })),
        text_color: FG_DIM,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}
