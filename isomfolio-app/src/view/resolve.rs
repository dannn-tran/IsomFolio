use iced::{
    widget::{button, column, container, image, mouse_area, row, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};
use isomfolio_core::models::{AssetFile, ThumbnailState};

use crate::app::{App, Msg};
use super::styles::{
    active_chip_style, ghost_btn_style, icon_btn, ACCENT, BG_GRID, ERR, FG, FG_DIM,
    OVERLAY_HEAVY, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, TEXT_BASE, TEXT_MD,
    TEXT_SM, TILE_CORNER,
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
                text(if self.resolve.scenes { "Review Scenes" } else { "Review Stacks" })
                    .size(TEXT_BASE)
                    .color(FG),
                Space::new().width(Length::Fill),
                text(format!(
                    "{} {} of {total}",
                    if self.resolve.scenes { "Scene" } else { "Stack" },
                    self.resolve.idx + 1
                ))
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

        // Wrap frames into an adaptive grid sized to the window instead of one
        // horizontal strip — a row of landscape frames would otherwise force a
        // horizontal scroll and shrink each to a sliver.
        let cols = resolve_cols(frames.len());
        let indexed: Vec<(usize, &AssetFile)> = frames.iter().enumerate().collect();
        let mut grid = column![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
        for chunk in indexed.chunks(cols) {
            let mut r = row![]
                .spacing(SPACE_2)
                .width(Length::Fill)
                .height(Length::FillPortion(1));
            for (i, f) in chunk {
                r = r.push(self.resolve_frame(*i, f, f.id == stack_review.rep_id));
            }
            // Pad a short final row so its cells keep the same width as full rows.
            for _ in chunk.len()..cols {
                r = r.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(r);
        }
        let body = container(grid)
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
        // Re-arm the auto-pick (sharpest) after manual toggling. Only offered when
        // the current keepers differ from the lone auto choice.
        let on_auto = self.resolve.keepers.len() == 1
            && self.resolve.keepers.contains(&stack_review.rep_id);
        if !on_auto {
            left = left.push(
                button(text("↺ Reset to auto").size(TEXT_SM))
                    .on_press(Msg::ResolveResetAuto)
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
    /// gets an `ACCENT` ring + "✓ Keep" badge; a reject is dimmed. The auto-picked
    /// (sharpest) frame carries a persistent marker so its suggestion is always
    /// legible — even after the user overrides it. The filename sits in a caption
    /// *below* the image, never overlaying it.
    fn resolve_frame<'a>(
        &'a self,
        frame_idx: usize,
        f: &AssetFile,
        is_sharpest: bool,
    ) -> Element<'a, Msg> {
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

        let chip = |label: &str, color: Color| {
            container(text(label.to_string()).size(TEXT_SM).color(color))
                .padding([SPACE_1, SPACE_1_5])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(OVERLAY_HEAVY)),
                    border: Border { radius: 3.0.into(), ..Default::default() },
                    ..Default::default()
                })
        };

        let (badge_label, badge_color) = if keep { ("✓ Keep", ACCENT) } else { ("✕ Reject", ERR) };
        let mut top = row![chip(badge_label, badge_color)].spacing(SPACE_1);
        // The auto-pick marker persists regardless of keep state, so the user can
        // always see (and trust, or override) what the app chose and why.
        if is_sharpest {
            top = top.push(chip("★ sharpest", ACCENT));
        }

        let overlay: Element<Msg> = container(
            container(top).width(Length::Fill).align_x(Alignment::Start),
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

        let picture = container(stack![img_el, scrim, overlay])
            .width(Length::Fill)
            .height(Length::Fill);

        // Filename rides in a caption strip *below* the image, never on top of it.
        let caption = container(text(f.name.clone()).size(TEXT_SM).color(FG_DIM))
            .padding([SPACE_1, SPACE_0_5])
            .width(Length::Fill)
            .align_x(Alignment::Center);

        let cell = column![picture, caption]
            .spacing(SPACE_0_5)
            .width(Length::FillPortion(1))
            .height(Length::Fill);

        mouse_area(cell)
            .on_press(Msg::ToggleResolveKeeper(f.id.clone()))
            .into()
    }
}

/// Columns for the adaptive review grid, chosen from frame count so a burst fits
/// the window without horizontal scrolling (the old single-row layout crushed
/// landscape frames). Wide windows favour rows over a long strip.
fn resolve_cols(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        2 => 2,
        3 | 4 => 2,
        5 | 6 => 3,
        7..=9 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_cols;

    #[test]
    fn grid_wraps_instead_of_one_long_row() {
        // Every count past a pair wraps into <= the count's columns, so frames
        // never spill into a horizontal scroll.
        for n in 1..=20 {
            let cols = resolve_cols(n);
            assert!(cols >= 1 && cols <= n.max(1));
            let rows = n.div_ceil(cols);
            // No single row may hold the whole burst once it exceeds 2 frames.
            if n > 2 {
                assert!(rows >= 2, "n={n} cols={cols} rows={rows}");
            }
        }
    }
}
