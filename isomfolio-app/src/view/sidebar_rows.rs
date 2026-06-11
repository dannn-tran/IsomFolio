use iced::{
    widget::{button, container, mouse_area, row, text, text_input, tooltip, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use isomfolio_core::folder_tree::FolderNode;

use super::styles::{
    ACCENT, ALBUM_HOVER, FG, FG_DIM, FG_MUTED, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, TEXT_BASE, TEXT_SM,
};
use super::icons::{Icon, ICON_SIZE};
use crate::app::{
    Msg, SidebarItem, ALBUM_ITEM_HEIGHT, FOLDER_ITEM_HEIGHT,
};

/// The inline "new album" name input row (name field + confirm / cancel), used
/// both at the top level (ungrouped) and nested under a target group.
pub(super) fn create_album_input_row<'a>(input_val: &str) -> Element<'a, Msg> {
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
pub(super) fn create_group_input_row<'a>(input_val: &str) -> Element<'a, Msg> {
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

pub(super) fn truncate_label(s: &str, max: usize) -> (String, bool) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        (s.to_string(), false)
    } else {
        (format!("{}…", chars[..max].iter().collect::<String>()), true)
    }
}

pub(super) fn label_tooltip<'a>(full_name: String) -> Element<'a, Msg> {
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
pub(super) fn nav_row<'a>(
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
pub(super) const CHEVRON_W: f32 = 20.0;

pub(super) fn folder_tree_row<'a>(
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
pub(super) fn album_sidebar_row<'a>(
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
