//! Monochrome line icons (Lucide, ISC) embedded as SVG and tinted at render
//! time. They are a dedicated *icon* resource, distinct from text — the
//! design-system "no decorative fonts" rule governs typography, not iconography.
//! Each icon is rendered single-colour via `svg::Style.color`, so it picks up
//! the row's state colour (quiet `FG_DIM`, brighter when selected).

use iced::widget::svg;
use iced::{Color, Element, Theme};

/// Sidebar icon edge length (px). These are quiet *illustrative* marks (the
/// leading folder/album/people glyphs), not interactive buttons — they sit just
/// under `TEXT_BASE` so they read as a peer of the label, never as a control.
/// (Interactive icon *buttons* — chevrons, `+` — are sized by `ICON_BTN`.)
pub const ICON_SIZE: f32 = 15.0;

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    AllPhotos,
    Filters,
    Folders,
    Albums,
    People,
    Imports,
    Deleted,
    // Control glyphs (disclosure + add) — rendered in `icon_btn_svg`, sized to
    // match the leading section icons so a header's chevron/`+` read as peers of
    // its icon rather than heavier unicode marks.
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Plus,
    ZoomIn,
    ZoomOut,
    /// Leading glyph for a group row (books on a group).
    Group,
}

fn bytes(icon: Icon) -> &'static [u8] {
    match icon {
        Icon::AllPhotos => include_bytes!("../../assets/icons/images.svg"),
        Icon::Filters => include_bytes!("../../assets/icons/sliders-horizontal.svg"),
        Icon::Folders => include_bytes!("../../assets/icons/folder.svg"),
        Icon::Albums => include_bytes!("../../assets/icons/book-image.svg"),
        Icon::People => include_bytes!("../../assets/icons/users.svg"),
        Icon::Imports => include_bytes!("../../assets/icons/import.svg"),
        Icon::Deleted => include_bytes!("../../assets/icons/trash-2.svg"),
        Icon::ChevronDown => include_bytes!("../../assets/icons/chevron-down.svg"),
        Icon::ChevronRight => include_bytes!("../../assets/icons/chevron-right.svg"),
        Icon::ChevronUp => include_bytes!("../../assets/icons/chevron-up.svg"),
        Icon::Plus => include_bytes!("../../assets/icons/plus.svg"),
        Icon::ZoomIn => include_bytes!("../../assets/icons/zoom-in.svg"),
        Icon::ZoomOut => include_bytes!("../../assets/icons/zoom-out.svg"),
        Icon::Group => include_bytes!("../../assets/icons/library.svg"),
    }
}

/// A tinted `ICON_SIZE` square of the given icon.
pub fn icon<'a, M: 'a>(kind: Icon, color: Color) -> Element<'a, M> {
    svg(svg::Handle::from_memory(bytes(kind)))
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .style(move |_theme: &Theme, _status| svg::Style { color: Some(color) })
        .into()
}
