use iced::{
    widget::{button, column, container, image, mouse_area, row, scrollable, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};
use isomfolio_core::models::{AssetFile, ThumbnailState};

use crate::app::{App, Msg};
use super::styles::{
    active_chip_style, ghost_btn_style, icon_btn, ACCENT, BG_GRID, ERR, FG, FG_DIM,
    OVERLAY_HEAVY, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, TEXT_BASE, TEXT_MD, TEXT_SM,
    TILE_CORNER,
};

impl App {
    pub(crate) fn view_resolve(&self) -> Element<'_, Msg> {
        let total = self.resolve.stacks.len();
        let Some(stack_review) = self.resolve.stacks.get(self.resolve.idx) else {
            // Should not happen — the mode is only entered with a non-empty queue.
            return container(text("No stacks to review").size(TEXT_BASE).color(FG_DIM))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };
        let frames = &stack_review.frames;
        let kept = frames.iter().filter(|f| self.resolve.keepers.contains(&f.id)).count();
        let rejected = frames.len() - kept;

        let top_bar = container(
            row![
                icon_btn("✕", Msg::EscapePressed),
                Space::new().width(Length::Fill),
                text("Review Stacks").size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                text(format!("Stack {} of {total}", self.resolve.idx + 1))
                    .size(TEXT_MD)
                    .color(FG_DIM),
            ]
            .spacing(SPACE_2_5)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_1, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        let panels: Vec<Element<Msg>> = frames
            .iter()
            .enumerate()
            .map(|(i, f)| self.resolve_frame(i, f))
            .collect();
        let frames_row = row(panels)
            .spacing(SPACE_2)
            .height(Length::Fill);
        let body = container(
            scrollable(frames_row)
                .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new()))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding([SPACE_2, SPACE_3])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_GRID)),
            ..Default::default()
        });

        let mut left = row![].spacing(SPACE_2).align_y(Alignment::Center);
        if self.resolve.idx > 0 {
            left = left.push(
                button(text("‹ Previous").size(TEXT_SM))
                    .on_press(Msg::ResolvePrevStack)
                    .style(ghost_btn_style),
            );
        }

        let footer = container(
            row![
                left,
                Space::new().width(Length::Fill),
                text(format!("Keeping {kept} · rejecting {rejected}"))
                    .size(TEXT_SM)
                    .color(FG_DIM),
                Space::new().width(Length::Fill),
                button(text("Skip").size(TEXT_SM))
                    .on_press(Msg::ResolveSkipStack)
                    .style(ghost_btn_style),
                button(text("Keep selected & Next ›").size(TEXT_SM))
                    .on_press(Msg::ResolveApplyAndNext)
                    .style(active_chip_style),
            ]
            .spacing(SPACE_2)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_1_5, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        column![top_bar, body, footer].into()
    }

    /// One reviewable frame: large image, click to toggle keeper. A kept frame
    /// gets an `ACCENT` ring + "✓ Keep" badge; a reject is dimmed.
    fn resolve_frame<'a>(&'a self, frame_idx: usize, f: &AssetFile) -> Element<'a, Msg> {
        let keep = self.resolve.keepers.contains(&f.id);

        let img_el: Element<Msg> = match self.resolve.handles.get(&frame_idx) {
            Some(h) => image(h.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => {
                let thumb = self.thumbnails.get(&f.id).and_then(|s| match s {
                    ThumbnailState::Ready(p) => Some(p.clone()),
                    _ => None,
                });
                match thumb {
                    Some(path) => image(image::Handle::from_path(path))
                        .content_fit(iced::ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    None => Space::new().width(Length::Fill).height(Length::Fill).into(),
                }
            }
        };

        // Reject frames get a dark scrim; the choice reads at a glance.
        let scrim_color = if keep {
            Color::TRANSPARENT
        } else {
            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.45 }
        };
        let (ring_color, ring_width) = if keep { (ACCENT, 3.0_f32) } else { (Color::TRANSPARENT, 0.0) };

        let (badge_label, badge_color) = if keep { ("✓ Keep", ACCENT) } else { ("✕ Reject", ERR) };
        let badge = container(text(badge_label).size(TEXT_SM).color(badge_color))
            .padding([SPACE_1, SPACE_1_5])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_HEAVY)),
                border: Border { radius: 3.0.into(), ..Default::default() },
                ..Default::default()
            });

        let overlay: Element<Msg> = container(
            column![
                container(badge).width(Length::Fill).align_x(Alignment::Start),
                Space::new().height(Length::Fill),
                container(text(f.name.clone()).size(TEXT_MD).color(FG))
                    .padding([SPACE_1, SPACE_1_5])
                    .width(Length::Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(OVERLAY_HEAVY)),
                        ..Default::default()
                    }),
            ]
            .spacing(SPACE_1)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .padding(SPACE_1_5)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        let scrim = container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(scrim_color)),
                border: Border {
                    color: ring_color,
                    width: ring_width,
                    radius: TILE_CORNER.into(),
                },
                ..Default::default()
            });

        // Each panel takes an equal share of the row but never narrower than a
        // legible width — wider stacks scroll horizontally rather than crushing.
        let panel = container(stack![img_el, scrim, overlay])
            .width(Length::FillPortion(1))
            .height(Length::Fill);

        mouse_area(panel)
            .on_press(Msg::ToggleResolveKeeper(f.id.clone()))
            .into()
    }
}
