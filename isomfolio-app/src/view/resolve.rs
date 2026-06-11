use iced::{
    widget::{button, column, container, image, mouse_area, row, scrollable, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};
use isomfolio_core::models::{AssetFile, ThumbnailState};

use crate::app::{App, Msg, StackReview, SurfaceLayout};
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
                text(if self.resolve.scenes { "Sift · Scenes" } else { "Sift · Bursts" })
                    .size(TEXT_BASE)
                    .color(FG),
                Space::new().width(Length::Fill),
                self.sift_layout_btn("▦ Grid", SurfaceLayout::Grid),
                self.sift_layout_btn("▭ Strip", SurfaceLayout::Strip),
                Space::new().width(SPACE_2),
                text(format!("Group {} of {total}", self.resolve.idx + 1))
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

        let body = match self.resolve.layout {
            SurfaceLayout::Grid => self.sift_grid_body(stack_review),
            // Sift offers Grid/Strip; Full is a Browse-only (loupe) layout.
            SurfaceLayout::Strip | SurfaceLayout::Full => self.sift_strip_body(stack_review),
        };

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
    /// gets an `ACCENT` ring + "✓ Keep" badge; a reject is dimmed. Every frame shows
    /// its sharpness `rank` (1 = sharpest, the auto-pick), so overriding the default
    /// keeper is informed — you can see which of the rest is next-sharpest. The
    /// filename sits in a caption *below* the image, never overlaying it.
    fn resolve_frame<'a>(
        &'a self,
        frame_idx: usize,
        f: &AssetFile,
        rank: usize,
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

        let (badge_label, badge_color) = if keep { ("✓ Keep", ACCENT) } else { ("✕ Reject", ERR) };
        // Sharpness rank rides on every frame regardless of keep state: #1 (the
        // auto-pick) is starred, the rest numbered, so the user can always see what
        // the app chose *and* the next-best alternative if they override it.
        let (rank_label, rank_color) = if rank == 1 {
            ("★ sharpest".to_string(), ACCENT)
        } else {
            (format!("#{rank} sharp"), FG)
        };
        let top = row![sift_chip(badge_label, badge_color), sift_chip(&rank_label, rank_color)]
            .spacing(SPACE_1);

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

    /// Header toggle for one layout; the active layout reads as a filled chip.
    fn sift_layout_btn(&self, label: &'static str, layout: SurfaceLayout) -> Element<'_, Msg> {
        let active = self.resolve.layout == layout;
        button(text(label).size(TEXT_SM))
            .on_press(Msg::SiftSetLayout(layout))
            .style(move |t: &Theme, s| if active { active_chip_style(t, s) } else { ghost_btn_style(t, s) })
            .into()
    }

    /// Survey layout: every frame at once in an adaptive, window-filling grid.
    fn sift_grid_body(&self, stack: &StackReview) -> Element<'_, Msg> {
        let cols = resolve_cols(stack.frames.len());
        let indexed: Vec<(usize, &AssetFile)> = stack.frames.iter().enumerate().collect();
        let mut grid = column![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
        for chunk in indexed.chunks(cols) {
            let mut r = row![]
                .spacing(SPACE_2)
                .width(Length::Fill)
                .height(Length::FillPortion(1));
            for (i, f) in chunk {
                r = r.push(self.resolve_frame(*i, f, stack.sharpness_rank(*i)));
            }
            for _ in chunk.len()..cols {
                r = r.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(r);
        }
        container(grid)
            .padding([SPACE_2, SPACE_3])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }

    /// One-up layout: a large preview of the focused frame over a thumbnail
    /// filmstrip — keeps the focused frame big when a group has many frames.
    fn sift_strip_body(&self, stack: &StackReview) -> Element<'_, Msg> {
        column![
            container(self.sift_preview(stack))
                .width(Length::Fill)
                .height(Length::FillPortion(5)),
            container(self.sift_filmstrip(stack))
                .width(Length::Fill)
                .height(Length::FillPortion(1))
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(OVERLAY_HEAVY)),
                    ..Default::default()
                }),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn sift_preview(&self, stack: &StackReview) -> Element<'_, Msg> {
        let focus = self.resolve.focus.min(stack.frames.len().saturating_sub(1));
        let f = &stack.frames[focus];
        let keep = self.resolve.keepers.contains(&f.id);
        let rank = stack.sharpness_rank(focus);

        // Prefer the decoded full-res handle (already loaded for every frame), fall
        // back to the thumbnail while it lands.
        let img_el: Element<Msg> = match self.resolve.handles.get(&focus) {
            Some(h) => image(h.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => match self.thumbnails.get(&f.id) {
                Some(ThumbnailState::Ready(p)) => image(image::Handle::from_path(p))
                    .content_fit(iced::ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
                _ => container(text("Loading…").size(TEXT_MD).color(FG_DIM))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
            },
        };

        let (badge_label, badge_color) = if keep { ("✓ Keep", ACCENT) } else { ("✕ Reject", ERR) };
        let (rank_label, rank_color) = if rank == 1 {
            ("★ sharpest".to_string(), ACCENT)
        } else {
            (format!("#{rank} sharp"), FG)
        };
        let chips = row![sift_chip(badge_label, badge_color), sift_chip(&rank_label, rank_color)]
            .spacing(SPACE_1);
        let overlay = container(container(chips).width(Length::Fill).align_x(Alignment::Start))
            .padding(SPACE_1_5)
            .width(Length::Fill)
            .height(Length::Fill);

        let ring = if keep { ACCENT } else { Color::TRANSPARENT };
        let framed = container(stack![img_el, overlay])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 })),
                border: Border { color: ring, width: if keep { 3.0 } else { 0.0 }, radius: 0.0.into() },
                ..Default::default()
            });

        // Filename below the preview, never overlaying it.
        let caption = container(
            text(format!("{}  ·  frame {} of {}", f.name, focus + 1, stack.frames.len()))
                .size(TEXT_SM)
                .color(FG_DIM),
        )
        .padding([SPACE_1, SPACE_2])
        .width(Length::Fill)
        .align_x(Alignment::Center);

        column![
            mouse_area(framed).on_press(Msg::ToggleResolveKeeper(f.id.clone())),
            caption,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn sift_filmstrip(&self, stack: &StackReview) -> Element<'_, Msg> {
        const THUMB: f32 = 84.0;
        let mut strip = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for (i, f) in stack.frames.iter().enumerate() {
            let focused = i == self.resolve.focus;
            let keep = self.resolve.keepers.contains(&f.id);
            let thumb: Element<Msg> = match self.thumbnails.get(&f.id) {
                Some(ThumbnailState::Ready(p)) => image(image::Handle::from_path(p))
                    .width(THUMB)
                    .height(THUMB)
                    .content_fit(iced::ContentFit::Cover)
                    .into(),
                _ => Space::new().width(THUMB).height(THUMB).into(),
            };
            // Rejects dim; the focused frame gets a bright ring.
            let scrim_color = if keep { Color::TRANSPARENT } else { Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 } };
            let ring = if focused { ACCENT } else { Color::TRANSPARENT };
            let scrim = container(Space::new())
                .width(THUMB)
                .height(THUMB)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(scrim_color)),
                    border: Border { color: ring, width: 2.5, radius: 3.0.into() },
                    ..Default::default()
                });
            let rank = stack.sharpness_rank(i);
            let tag = container(
                text(if rank == 1 { "★".to_string() } else { format!("#{rank}") })
                    .size(TEXT_SM)
                    .color(FG),
            )
            .padding([0.0, SPACE_0_5])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_HEAVY)),
                border: Border { radius: 2.0.into(), ..Default::default() },
                ..Default::default()
            });
            let cell = stack![
                thumb,
                scrim,
                container(tag).width(THUMB).height(THUMB).align_x(Alignment::Start).align_y(Alignment::End),
            ];
            strip = strip.push(
                button(container(cell).clip(true))
                    .padding(0)
                    .on_press(Msg::SiftFocusFrame(i))
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: FG,
                        border: Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
            );
        }
        container(
            scrollable(strip)
                .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new()))
                .width(Length::Fill),
        )
        .padding([SPACE_1, SPACE_2])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// A small status chip (keep/reject, sharpness rank) on a heavy overlay — shared by
/// the survey tiles and the one-up preview.
fn sift_chip<'a>(label: &str, color: Color) -> Element<'a, Msg> {
    container(text(label.to_string()).size(TEXT_SM).color(color))
        .padding([SPACE_1, SPACE_1_5])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            border: Border { radius: 3.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
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
