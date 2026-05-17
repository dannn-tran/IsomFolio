use iced::widget::{button, container, Space};
use iced::Theme;
use iced::{Background, Border, Color, Element, Length};

use crate::app::Msg;

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
pub const SPACE_5: f32 = UNIT * 5.0;
pub const SPACE_6: f32 = UNIT * 6.0;

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
    use iced::widget::{button, container, row, text, Space};
    use iced::Alignment;

    container(
        row![
            text(prompt).size(TEXT_SM).color(ERR),
            Space::new().width(Length::Fill),
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
