use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding,
    widget::{
        column, container, image, row, scrollable, text, Space,
        button,
    },
    Theme,
};
use iced::widget::scrollable::Direction;

use isomfolio_core::models::ThumbnailState;

use crate::app::{
    App, Msg, SidebarItem, SIDEBAR_WIDTH, GRID_PADDING, TILE_GAP,
    ALBUM_ITEM_HEIGHT, BUFFER_ROWS,
};

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const BG_SIDEBAR: Color = Color { r: 0.11, g: 0.11, b: 0.14, a: 1.0 };
const BG_GRID: Color    = Color { r: 0.08, g: 0.08, b: 0.10, a: 1.0 };
const BG_STATUSBAR: Color = Color { r: 0.07, g: 0.07, b: 0.09, a: 1.0 };
const FG: Color         = Color { r: 0.90, g: 0.90, b: 0.92, a: 1.0 };
const FG_DIM: Color     = Color { r: 0.55, g: 0.55, b: 0.60, a: 1.0 };
const ACCENT: Color     = Color { r: 0.20, g: 0.55, b: 0.95, a: 1.0 };
const ALBUM_HOVER: Color = Color { r: 0.10, g: 0.25, b: 0.50, a: 1.0 };
const SEL_RING: Color   = Color::WHITE;
const DRAG_ALPHA: f32   = 0.35;
const TILE_CORNER: f32  = 4.0;

// ---------------------------------------------------------------------------
// Top-level view
// ---------------------------------------------------------------------------

impl App {
    pub fn view(&self) -> Element<'_, Msg> {
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

        let status_bar = container(
            row![
                text(status).size(12).color(FG_DIM),
                Space::new().width(Length::Fill),
                button(text("−").size(14))
                    .on_press(Msg::TileSizeDown)
                    .style(ghost_btn_style),
                text(format!("{}px", self.tile_px as u32)).size(12).color(FG_DIM),
                button(text("+").size(14))
                    .on_press(Msg::TileSizeUp)
                    .style(ghost_btn_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 12])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_STATUSBAR)),
            ..Default::default()
        });

        column![
            row![
                self.view_sidebar(),
                self.view_grid(),
            ]
            .height(Length::Fill),
            status_bar,
        ]
        .into()
    }
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

impl App {
    fn view_sidebar(&self) -> Element<'_, Msg> {
        let drag_hover = self.drag_hover_album.clone();

        // "All Photos" row
        let all_sel = self.selected_item == SidebarItem::AllFiles;
        let all_row = sidebar_row_button(
            "All Photos".to_string(),
            all_sel,
            false,
            Msg::SidebarItemClicked(SidebarItem::AllFiles),
        );

        // Folder rows
        let folder_section: Vec<Element<Msg>> = self
            .folders
            .iter()
            .map(|(path, count)| {
                let name = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str());
                let sel = self.selected_item == SidebarItem::Folder(path.clone());
                sidebar_row_button(
                    format!("{name}  {count}"),
                    sel,
                    false,
                    Msg::SidebarItemClicked(SidebarItem::Folder(path.clone())),
                )
            })
            .collect();

        // Album rows
        let album_section: Vec<Element<Msg>> = self
            .albums
            .iter()
            .map(|album| {
                let sel = self.selected_item == SidebarItem::Album(album.id.clone());
                let hovered = drag_hover.as_deref() == Some(album.id.as_str());
                sidebar_row_button(
                    album.name.clone(),
                    sel,
                    hovered,
                    Msg::SidebarItemClicked(SidebarItem::Album(album.id.clone())),
                )
            })
            .collect();

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

        if !self.albums.is_empty() {
            content = content
                .push(Space::new().height(8))
                .push(text("Albums").size(11).color(FG_DIM));
            for row in album_section {
                content = content.push(row);
            }
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

// ---------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------

impl App {
    pub fn view_grid(&self) -> Element<'_, Msg> {
        if self.files.is_empty() {
            let placeholder = container(
                text("No photos in this view").size(16).color(FG_DIM),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            });
            return placeholder.into();
        }

        let cols = self.cols().max(1);
        let tile_px = self.tile_px;
        let step = tile_px + TILE_GAP;
        let total = self.files.len();
        let total_rows = (total + cols - 1) / cols;

        // Windowed render bounds
        let first_row = {
            let r = ((self.scroll_y - GRID_PADDING) / step) as usize;
            r.saturating_sub(BUFFER_ROWS)
        };
        let visible_rows = (self.viewport_height / step) as usize + 1 + BUFFER_ROWS * 2;
        let last_row = (first_row + visible_rows).min(total_rows);

        let top_space = first_row as f32 * step;
        let bottom_space = ((total_rows - last_row) as f32 * step).max(0.0);

        // Build visible rows
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

        container(
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
                .height(Length::Fill),
        )
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
                // Placeholder — grey box with file name
                let bg = if dragging {
                    Color { r: 0.3, g: 0.3, b: 0.35, a: 0.5 }
                } else {
                    Color { r: 0.20, g: 0.20, b: 0.25, a: 1.0 }
                };
                container(
                    text(&file.name).size(10).color(FG_DIM),
                )
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

        let (border_color, border_width, _alpha) = if dragging {
            (Color::TRANSPARENT, 0.0_f32, DRAG_ALPHA)
        } else if selected {
            (SEL_RING, 2.5, 1.0)
        } else {
            (Color::TRANSPARENT, 0.0, 1.0)
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

// ---------------------------------------------------------------------------
// Ghost button style
// ---------------------------------------------------------------------------

fn ghost_btn_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 })),
        text_color: FG_DIM,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}
