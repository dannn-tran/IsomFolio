use iced::widget::{button, container, text, tooltip, Space};
use iced::Theme;
use iced::{Background, Border, Color, Element, Length};

use crate::app::Msg;

pub use iced::widget::tooltip::Position as TipPos;

/// Wrap any control in a hover tooltip. Use on icon-only / glyph buttons so
/// their meaning is discoverable without a visible text label.
pub fn tip<'a>(
    content: impl Into<Element<'a, Msg>>,
    label: impl Into<String>,
    position: TipPos,
) -> Element<'a, Msg> {
    let label = label.into();
    tooltip(
        content,
        container(text(label).size(TEXT_SM).color(FG))
            .padding([SPACE_1, SPACE_1_5])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 4.0.into() },
                ..Default::default()
            }),
        position,
    )
    .gap(4)
    .into()
}

/// Standard colour-label names (Lightroom set), in key order (6–9 + Purple).
pub const COLOR_LABELS: [&str; 5] = ["Red", "Yellow", "Green", "Blue", "Purple"];

/// Swatch colour for a colour-label name. Unknown names fall back to `FG_DIM`.
pub fn color_label_swatch(name: &str) -> Color {
    match name {
        "Red" => Color { r: 0.90, g: 0.30, b: 0.24, a: 1.0 },
        "Yellow" => Color { r: 0.95, g: 0.77, b: 0.18, a: 1.0 },
        "Green" => Color { r: 0.40, g: 0.74, b: 0.39, a: 1.0 },
        "Blue" => Color { r: 0.30, g: 0.55, b: 0.92, a: 1.0 },
        "Purple" => Color { r: 0.65, g: 0.45, b: 0.86, a: 1.0 },
        _ => FG_DIM,
    }
}

pub const BG_SIDEBAR: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.16,
    a: 1.0,
};
pub const BG_GRID: Color = Color {
    r: 0.09,
    g: 0.09,
    b: 0.12,
    a: 1.0,
};
pub const BG_STATUSBAR: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.10,
    a: 1.0,
};
pub const BG_CRITERIA: Color = Color {
    r: 0.14,
    g: 0.14,
    b: 0.18,
    a: 1.0,
};
pub const FG: Color = Color {
    r: 0.93,
    g: 0.93,
    b: 0.95,
    a: 1.0,
};
pub const FG_DIM: Color = Color {
    r: 0.70,
    g: 0.70,
    b: 0.75,
    a: 1.0,
};
pub const FG_MUTED: Color = Color {
    r: 0.45,
    g: 0.45,
    b: 0.50,
    a: 1.0,
};
pub const ACCENT: Color = Color {
    r: 0.20,
    g: 0.55,
    b: 0.95,
    a: 1.0,
};
pub const WARN: Color = Color {
    r: 0.90,
    g: 0.55,
    b: 0.15,
    a: 1.0,
};
pub const ALBUM_HOVER: Color = Color {
    r: 0.10,
    g: 0.25,
    b: 0.50,
    a: 1.0,
};
pub const TILE_CORNER: f32 = 4.0;
pub const STAR_GOLD: Color = Color {
    r: 1.0,
    g: 0.82,
    b: 0.0,
    a: 1.0,
};
pub const ERR: Color = Color {
    r: 0.95,
    g: 0.35,
    b: 0.35,
    a: 1.0,
};
pub const BORDER: Color = Color {
    r: 0.28,
    g: 0.28,
    b: 0.34,
    a: 1.0,
};
pub const BG_MODAL: Color = Color {
    r: 0.11,
    g: 0.11,
    b: 0.14,
    a: 1.0,
};
pub const BG_TILE_LOADING: Color = Color {
    r: 0.20,
    g: 0.20,
    b: 0.25,
    a: 1.0,
};
pub const DANGER: Color = Color {
    r: 0.65,
    g: 0.12,
    b: 0.12,
    a: 1.0,
};
pub const ACCENT_HOVER: Color = Color {
    r: 0.28,
    g: 0.62,
    b: 1.0,
    a: 1.0,
};
pub const ACCENT_PRESSED: Color = Color {
    r: 0.15,
    g: 0.45,
    b: 0.82,
    a: 1.0,
};
pub const DANGER_HOVER: Color = Color {
    r: 0.75,
    g: 0.18,
    b: 0.18,
    a: 1.0,
};
pub const DANGER_PRESSED: Color = Color {
    r: 0.52,
    g: 0.08,
    b: 0.08,
    a: 1.0,
};

pub const BG_PANEL: Color = Color {
    r: 0.13,
    g: 0.13,
    b: 0.16,
    a: 0.96,
};
pub const BG_PROGRESS_TRACK: Color = Color {
    r: 0.25,
    g: 0.25,
    b: 0.28,
    a: 1.0,
};
pub const OVERLAY_LIGHT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.4,
};
pub const OVERLAY_MEDIUM: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.55,
};
pub const OVERLAY_HEAVY: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.7,
};

/// Subtle white-tint fill — input field backgrounds, tag chip backgrounds.
pub const HINT_SUBTLE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.05 };
/// Hover white-tint fill — context menu hover, menu button hover.
pub const HINT_HOVER: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 };

pub const TEXT_XS: f32 = 10.0;
pub const TEXT_SM: f32 = 11.0;
pub const TEXT_MD: f32 = 12.0;
pub const TEXT_BASE: f32 = 13.0;
pub const TEXT_LG: f32 = 14.0;
pub const TEXT_STAR: f32 = 18.0;
pub const TEXT_TITLE: f32 = 20.0;
pub const TEXT_DISPLAY: f32 = 36.0;

pub const UNIT: f32 = 4.0;
pub const SPACE_0_5: f32 = UNIT * 0.5;
pub const SPACE_1: f32 = UNIT;
pub const SPACE_1_5: f32 = UNIT * 1.5;
pub const SPACE_2: f32 = UNIT * 2.0;
pub const SPACE_2_5: f32 = UNIT * 2.5;
pub const SPACE_3: f32 = UNIT * 3.0;
pub const SPACE_4: f32 = UNIT * 4.0;
pub const SPACE_6: f32 = UNIT * 6.0;

/// Standard clickable square for icon-only buttons (px). One uniform footprint
/// app-wide so every bare-glyph control (chevrons, `+`, `×`, `⚙`, zoom, …) has
/// the same hit area. Sits at `FOLDER_ITEM_HEIGHT` — well above the 24 px
/// *Density floor* and a comfortable pointer/coarse target — without ballooning
/// the dense desktop layout the way a 44 px touch target would.
pub const ICON_BTN: f32 = 28.0;
/// Glyph size inside an `icon_btn` — the visible mark, sized independently of
/// the clickable square so the target can grow without the glyph distorting.
pub const ICON_GLYPH: f32 = 16.0;

/// An icon-only button: a glyph centred in a fixed `ICON_BTN` square, using
/// `icon_btn_style` (no box; tint brightens on hover). The single helper every
/// bare-glyph control should route through so hit areas stay uniform.
pub fn icon_btn<'a>(glyph: &str, msg: Msg) -> button::Button<'a, Msg> {
    icon_btn_styled(glyph, msg, icon_btn_style)
}

/// `icon_btn` with a fixed glyph colour (no hover-brighten) — for glyphs whose
/// colour carries meaning (a colour-label swatch, an `ERR`-tinted ×).
pub fn icon_btn_color<'a>(glyph: &str, msg: Msg, color: Color) -> button::Button<'a, Msg> {
    button(
        container(text(glyph.to_string()).size(ICON_GLYPH).color(color))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(ICON_BTN)
    .height(ICON_BTN)
    .padding(0)
    .on_press(msg)
    .style(icon_btn_style)
}

/// `icon_btn` with a caller-supplied style fn — for icon-only *toggles* that
/// show an active state (e.g. the Grid/List layout switch), where
/// `active_chip_style` replaces `icon_btn_style` when on.
pub fn icon_btn_styled<'a>(
    glyph: &str,
    msg: Msg,
    style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> button::Button<'a, Msg> {
    button(
        container(text(glyph.to_string()).size(ICON_GLYPH))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(ICON_BTN)
    .height(ICON_BTN)
    .padding(0)
    .on_press(msg)
    .style(style)
}

/// An icon-only button whose mark is a tinted **SVG** glyph (Lucide), centred in
/// the same `ICON_BTN` square as `icon_btn`. Used for disclosure chevrons and the
/// `+` add actions so they sit at the weight of the leading section icons instead
/// of reading as heavier unicode. An SVG's tint can't follow button hover state
/// the way text colour can, so feedback is a faint background on hover (rather
/// than `icon_btn`'s tint-brighten). Returns a `Button` so callers can size it to
/// a host slot (e.g. the folder-tree chevron column).
pub fn icon_btn_svg<'a>(kind: super::icons::Icon, msg: Msg) -> button::Button<'a, Msg> {
    icon_btn_svg_color(kind, msg, FG_DIM)
}

/// `icon_btn_svg` with a caller-chosen tint — for SVG icon buttons that sit on a
/// dark compositing layer (the loupe overlay) where `FG_DIM` would read too faint.
pub fn icon_btn_svg_color<'a>(
    kind: super::icons::Icon,
    msg: Msg,
    color: Color,
) -> button::Button<'a, Msg> {
    button(
        container(super::icons::icon(kind, color))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(ICON_BTN)
    .height(ICON_BTN)
    .padding(0)
    .on_press(msg)
    .style(icon_svg_btn_style)
}

fn icon_svg_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    let alpha = match status {
        button::Status::Hovered => 0.10,
        button::Status::Pressed => 0.16,
        _ => 0.0,
    };
    button::Style {
        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: alpha })),
        text_color: FG_DIM,
        border: Border { radius: 4.0.into(), ..Default::default() },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn icon_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (text_color, alpha) = match status {
        button::Status::Hovered => (Color::WHITE, 0.0),
        button::Status::Pressed => (FG, 0.0),
        _ => (FG_DIM, 0.0),
    };
    button::Style {
        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: alpha })),
        text_color,
        border: Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn ghost_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    let alpha = match status {
        button::Status::Hovered => 0.13,
        button::Status::Pressed => 0.22,
        _ => 0.06,
    };
    button::Style {
        background: Some(Background::Color(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: alpha,
        })),
        text_color: FG,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn solid_btn(base: Color, hover: Color, pressed: Color, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => hover,
        button::Status::Pressed => pressed,
        _ => base,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

pub fn active_chip_style(_theme: &Theme, status: button::Status) -> button::Style {
    solid_btn(ACCENT, ACCENT_HOVER, ACCENT_PRESSED, status)
}

pub fn danger_btn_style(_theme: &Theme, status: button::Status) -> button::Style {
    solid_btn(DANGER, DANGER_HOVER, DANGER_PRESSED, status)
}

pub fn sidebar_divider<'a>() -> Element<'a, Msg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1.0)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BORDER)),
            ..Default::default()
        })
        .into()
}

pub fn confirm_action_row<'a>(
    prompt: String,
    confirm_msg: Msg,
    cancel_msg: Msg,
) -> Element<'a, Msg> {
    use crate::app::ALBUM_ITEM_HEIGHT;
    use iced::widget::{button, container, row, text};
    use iced::Alignment;

    container(
        row![
            // The prompt takes the *remaining* width and clips — the Cancel/Confirm
            // buttons stay pinned and on-screen even in the narrow sidebar. (A bare
            // `text` + `Space::Fill` let a long prompt push Confirm off the right
            // edge, where it couldn't be clicked — e.g. group deletion.)
            container(
                text(prompt)
                    .size(TEXT_SM)
                    .color(ERR)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            button(text("Cancel").size(TEXT_SM))
                .on_press(cancel_msg)
                .style(ghost_btn_style),
            button(text("Confirm").size(TEXT_SM))
                .on_press(confirm_msg)
                .style(danger_btn_style),
        ]
        .spacing(SPACE_1)
        .align_y(Alignment::Center),
    )
    .height(ALBUM_ITEM_HEIGHT)
    .align_y(Alignment::Center)
    .padding([0.0, SPACE_1])
    .into()
}
