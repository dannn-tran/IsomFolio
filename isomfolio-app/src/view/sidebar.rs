use iced::{
    Alignment, Background, Border, Color, Element, Length,
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Theme,
};

use isomfolio_core::models::AlbumKind;

use crate::app::{
    App, Msg, SidebarItem, ALBUM_ITEM_HEIGHT, SIDEBAR_WIDTH,
};
use super::styles::{
    BG_SIDEBAR, BG_STATUSBAR, FG, FG_DIM, FG_MUTED, ACCENT, ALBUM_HOVER,
    ghost_btn_style, sidebar_divider, confirm_action_row,
};

impl App {
    pub(super) fn view_sidebar(&self) -> Element<'_, Msg> {
        let drag_hover = self.drag_hover_album.clone();

        let catalog_name = std::path::Path::new(&self.catalog_dir)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Catalog");

        let catalog_header: Element<Msg> = row![
            text(catalog_name).size(11).color(FG_DIM),
        ]
        .align_y(Alignment::Center)
        .into();

        let all_sel = self.selected_item == SidebarItem::AllFiles;
        let all_row = sidebar_row_button(
            "All Photos".to_string(),
            all_sel,
            false,
            Msg::SidebarItemClicked(SidebarItem::AllFiles),
        );

        let albums_header: Element<Msg> = row![
            text("Albums").size(11).color(FG_DIM),
            Space::new().width(Length::Fill),
            button(text("+").size(13))
                .on_press(Msg::StartCreateAlbum)
                .style(ghost_btn_style),
        ]
        .align_y(Alignment::Center)
        .into();

        let is_scan_active = self.is_scanning || self.scan_pending;
        let scan_btn_label = if is_scan_active { "Scanning…" } else { "Scan" };
        let folders_header: Element<Msg> = row![
            text("Folders").size(11).color(FG_DIM),
            Space::new().width(Length::Fill),
            button(text(scan_btn_label).size(11))
                .on_press(if is_scan_active { Msg::NoOp } else { Msg::ScanPickFolder })
                .style(ghost_btn_style),
        ]
        .align_y(Alignment::Center)
        .into();

        let mut content = column![
            catalog_header,
            Space::new().height(6),
            text("Library").size(11).color(FG_DIM),
            all_row,
            Space::new().height(4),
            sidebar_divider(),
            Space::new().height(4),
            folders_header,
        ]
        .spacing(2);

        for (path, count) in &self.folders {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path.as_str())
                .to_string();
            let sel = self.selected_item == SidebarItem::Folder(path.clone());
            if self.folder_pending_remove.as_deref() == Some(path.as_str()) {
                content = content.push(confirm_action_row(
                    "Remove folder? Indexed data deleted.".to_string(),
                    Msg::RemoveFolder(path.clone()),
                    Msg::CancelRemoveFolder,
                ));
            } else {
                content = content.push(folder_sidebar_row(name, path.clone(), *count, sel));
            }
        }

        content = content
            .push(Space::new().height(4))
            .push(sidebar_divider())
            .push(Space::new().height(4))
            .push(albums_header);

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
                content = content.push(album_sidebar_row(
                    album.name.clone(),
                    album.id.clone(),
                    count,
                    sel,
                    hovered,
                    is_smart,
                ));
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

        let sidebar_scroll = scrollable(content.spacing(2).padding(12))
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(4).scroller_width(4),
            ))
            .on_scroll(|vp| Msg::SidebarScrolled(vp.absolute_offset().y))
            .height(Length::Fill);

        let bottom_strip = column![
            sidebar_divider(),
            container(
                button(text("Open Catalog…").size(12))
                    .on_press(Msg::PickOpenCatalog)
                    .style(ghost_btn_style)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([6, 12])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_STATUSBAR)),
                ..Default::default()
            }),
        ];

        container(column![sidebar_scroll, bottom_strip])
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
        .on_press(Msg::RequestRemoveFolder(path))
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

    let smart_indicator = if is_smart { "⚡ " } else { "" };
    let count_str = if count > 0 { format!("  {count}") } else { String::new() };
    let name_btn = button(
        row![
            text(format!("{smart_indicator}{label}")).size(13).color(text_color),
            text(count_str).size(11).color(FG_MUTED),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(Msg::SidebarItemClicked(SidebarItem::Album(album_id.clone())))
    .width(Length::Fill)
    .style(|_: &Theme, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: FG,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    });

    let rename_btn = if selected {
        Some(
            button(text("✎").size(11).color(FG_DIM))
                .on_press(Msg::StartRenameAlbum(album_id.clone()))
                .style(|_: &Theme, _| button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: FG_DIM,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }),
        )
    } else {
        None
    };

    let delete_btn = button(text("×").size(11).color(FG_DIM))
        .on_press(Msg::RequestDeleteAlbum(album_id))
        .style(|_: &Theme, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: FG_DIM,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        });

    let mut row_content = row![name_btn].align_y(Alignment::Center);
    if let Some(btn) = rename_btn {
        row_content = row_content.push(btn);
    }
    row_content = row_content.push(delete_btn);

    container(row_content)
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
