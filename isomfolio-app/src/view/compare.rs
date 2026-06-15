use iced::{
    widget::{button, column, container, image, row, scrollable, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};
use isomfolio_core::models::Flag;

use crate::app::{App, Msg, ReviewLayout};
use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_GRID, ERR, FG, FG_DIM, OVERLAY_HEAVY, SPACE_0_5,
    SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM,
};

/// A small segmented-control chip — `active_chip_style` when selected, ghost
/// otherwise. Used for the layout, column-count, and sort toggles in the top bar.
fn chip<'a>(label: impl text::IntoFragment<'a>, msg: Msg, active: bool) -> Element<'a, Msg> {
    button(text(label).size(TEXT_MD))
        .padding([SPACE_0_5, SPACE_1_5])
        .on_press(msg)
        .style(move |t: &Theme, s| if active { active_chip_style(t, s) } else { ghost_btn_style(t, s) })
        .into()
}

fn tip<'a>(el: impl Into<Element<'a, Msg>>, label: &'static str) -> Element<'a, Msg> {
    super::styles::tip(el, label, super::styles::TipPos::Bottom)
}

impl App {
    pub(crate) fn view_compare(&self) -> Element<'_, Msg> {
        let n = self.compare.files.len();
        let layout = self.compare.layout;

        // The review surface is one place at two presentations — a layout switch,
        // not two features. Survey = all at once (synced zoom); One-up = focused
        // frame + filmstrip (pixel-peep / blink-compare). Space flips them in place.
        let layout_switch = row![
            tip(
                chip("▦ Survey", Msg::CompareSetLayout(ReviewLayout::Survey), layout == ReviewLayout::Survey),
                "All frames at once · synced zoom",
            ),
            tip(
                chip("⛶ One-up", Msg::CompareSetLayout(ReviewLayout::OneUp), layout == ReviewLayout::OneUp),
                "Focused frame + filmstrip · pixel-peep",
            ),
        ]
        .spacing(SPACE_1)
        .align_y(Alignment::Center);

        // Layout-specific control: Row/Grid arrangement in Survey, zoom-lock in One-up.
        let mode_control: Element<Msg> = match layout {
            ReviewLayout::Survey => self.compare_arrange_control(),
            ReviewLayout::OneUp => tip(
                chip("⊞ Lock zoom", Msg::CompareToggleZoomLock, self.compare.lock_zoom),
                "Hold zoom across frames (blink-compare)",
            ),
        };

        let sort_chip = tip(
            chip("\u{25c9} Sharpest first", Msg::CompareToggleSort, self.compare.sort_sharp),
            "Reorder frames sharpest → softest",
        );

        let hint = match layout {
            ReviewLayout::Survey => "←/→ focus · Space one-up · P pick · X reject · R remove · Esc",
            ReviewLayout::OneUp => "←/→ frame · Space survey · scroll zoom · P pick · X reject · R remove · Esc",
        };

        let top_bar = container(
            row![
                super::styles::icon_btn("✕", Msg::EscapePressed),
                text(format!("Compare · {n}")).size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                layout_switch,
                mode_control,
                sort_chip,
                Space::new().width(Length::Fill),
                text(hint).size(TEXT_MD).color(FG_DIM),
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

        let body = match layout {
            ReviewLayout::Survey => self.compare_survey_body(),
            ReviewLayout::OneUp => self.compare_oneup_body(),
        };

        column![top_bar, body].into()
    }

    /// Column-count control: Auto (√n) plus explicit 1..=min(n,4). The bare "1 2 3"
    /// Survey arrangement toggle: a single horizontal **Row**, or a **Grid**. Both
    /// fit the window with no scroll — Row shrinks cells, Grid wraps them into rows.
    fn compare_arrange_control(&self) -> Element<'_, Msg> {
        let grid = self.compare.survey_grid;
        row![
            tip(
                chip("▭ Row", Msg::CompareSetArrange(false), !grid),
                "All in one horizontal row (fit to window)",
            ),
            tip(
                chip("▦ Grid", Msg::CompareSetArrange(true), grid),
                "Wrap into a roughly square grid (fit to window)",
            ),
        ]
        .spacing(SPACE_1)
        .align_y(Alignment::Center)
        .into()
    }

    /// Survey: every frame at once, wrapped into `cols`-wide rows so panes stay as
    /// large and square as the count allows instead of one squeezed strip.
    fn compare_survey_body(&self) -> Element<'_, Msg> {
        let cols = self.compare.grid_cols();
        let order = self.compare.display_order();
        let mut grid = column![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
        for chunk in order.chunks(cols) {
            let mut r = row![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
            for &slot in chunk {
                r = r.push(self.compare_panel(slot));
            }
            // Pad a short final row so its cells keep the same width as full rows.
            for _ in chunk.len()..cols {
                r = r.push(Space::new().width(Length::Fill).height(Length::Fill));
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

    /// One-up: the focused frame big (shared zoom/pan, blink-compare with lock) over
    /// a filmstrip of the whole review set.
    fn compare_oneup_body(&self) -> Element<'_, Msg> {
        let big = container(self.compare_panel(self.compare.focus))
            .padding([SPACE_2, SPACE_3])
            .width(Length::Fill)
            .height(Length::Fill);
        container(column![big, self.compare_filmstrip()].width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }

    /// Filmstrip of the review set (display order), the focused frame ringed; click a
    /// thumb to focus it. Horizontally scrollable for large sets.
    fn compare_filmstrip(&self) -> Element<'_, Msg> {
        const THUMB: f32 = 64.0;
        let mut strip = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for slot in self.compare.display_order() {
            let Some(f) = self.compare.files.get(slot) else { continue };
            let is_cur = slot == self.compare.focus;
            let thumb: Element<Msg> = match self.thumbnails.get(&f.id) {
                Some(isomfolio_core::models::ThumbnailState::Ready(p)) => {
                    image(image::Handle::from_path(p))
                        .content_fit(iced::ContentFit::Contain)
                        .width(THUMB)
                        .height(THUMB)
                        .into()
                }
                _ => Space::new().width(THUMB).height(THUMB).into(),
            };
            let cell = container(thumb).width(THUMB).height(THUMB).style(move |_: &Theme| {
                container::Style {
                    border: Border {
                        color: if is_cur { ACCENT } else { Color::TRANSPARENT },
                        width: 2.0,
                        radius: 3.0.into(),
                    },
                    ..Default::default()
                }
            });
            strip = strip.push(
                button(cell)
                    .padding(0)
                    .on_press(Msg::CompareSetFocus(slot))
                    .style(|_: &Theme, _| button::Style::default()),
            );
        }
        container(
            scrollable(strip).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new().width(4).scroller_width(4),
            )),
        )
        .padding([SPACE_1, SPACE_2])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        })
        .into()
    }

    fn compare_panel(&self, slot: usize) -> Element<'_, Msg> {
        let file = self.compare.files.get(slot);
        let handle = self.compare.handles.get(slot).and_then(|h| h.as_ref());

        let img_el: Element<Msg> = match handle {
            // Full-res handle → a zoomable/pannable pane driven by the *shared*
            // compare zoom/pan, so panning/zooming one pane moves them all in lockstep.
            Some(h) => {
                let img = super::loupe_image::LoupeImage::new(
                    h.clone(),
                    self.compare.zoom,
                    self.compare.pan,
                    |scale, pan| Msg::CompareZoomChanged { scale, pan },
                    |_, _| Msg::NoOp,
                );
                // Survey: click *focuses* the frame (zoom is on scroll), so picking
                // which frame to flag is a click — not an accidental synced zoom.
                // One-up: keep click-to-zoom for pixel-peeking the single big frame.
                match self.compare.layout {
                    ReviewLayout::Survey => img.on_click(move || Msg::CompareSetFocus(slot)).into(),
                    ReviewLayout::OneUp => img.into(),
                }
            }
            None => {
                if let Some(f) = file {
                    let thumb = self.thumbnails.get(&f.id).and_then(|s| {
                        if let isomfolio_core::models::ThumbnailState::Ready(p) = s {
                            Some(p.clone())
                        } else {
                            None
                        }
                    });
                    match thumb {
                        Some(path) => image(image::Handle::from_path(path))
                            .content_fit(iced::ContentFit::Contain)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .into(),
                        None => Space::new().width(Length::Fill).height(Length::Fill).into(),
                    }
                } else {
                    Space::new().width(Length::Fill).height(Length::Fill).into()
                }
            }
        };

        let overlay: Element<Msg> = if let Some(f) = file {
            let rating = self.file_ratings.get(&f.id).copied().unwrap_or(0);
            let (flag_label, flag_color) = match f.flag {
                Flag::Pick => ("✓ Pick", ACCENT),
                Flag::Reject => ("✕ Reject", ERR),
                Flag::Unflagged => ("", FG_DIM),
            };

            let mut meta_row = row![].spacing(SPACE_1_5).align_y(Alignment::Center);
            if f.flag != Flag::Unflagged {
                meta_row = meta_row.push(text(flag_label).size(TEXT_SM).color(flag_color));
            }
            if rating > 0 {
                meta_row = meta_row
                    .push(text("★".repeat(rating as usize)).size(TEXT_SM).color(STAR_GOLD));
            }
            // Sharpness ordering: every scored pane shows its place (`#2 / 5`), so the
            // full ranking is visible at a glance; the clear winner additionally gets
            // an accented ◉ Sharpest. Always relative among these frames — never an
            // absolute focus verdict.
            if let Some((rank, total)) = self.compare.sharpness_rank(slot) {
                if self.compare.sharpest_slot() == Some(slot) {
                    meta_row = meta_row.push(text("\u{25c9} Sharpest").size(TEXT_SM).color(ACCENT));
                } else {
                    meta_row = meta_row
                        .push(text(format!("Sharp #{rank}/{total}")).size(TEXT_SM).color(FG_DIM));
                }
            }

            container(
                column![
                    Space::new().height(Length::Fill),
                    container(
                        column![
                            text(f.name.as_str()).size(TEXT_MD).color(FG),
                            meta_row,
                        ]
                        .spacing(SPACE_1),
                    )
                    .padding([SPACE_1_5, SPACE_2])
                    .width(Length::Fill)
                    .style(|_: &Theme| container::Style {
                        background: Some(Background::Color(OVERLAY_HEAVY)),
                        ..Default::default()
                    }),
                ]
                .width(Length::Fill)
                .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            Space::new().width(Length::Fill).height(Length::Fill).into()
        };

        // Ring the focused pane so it's clear which frame P/X/U and ratings act on.
        let focused = slot == self.compare.focus;
        container(stack![img_el, overlay].width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                border: Border {
                    color: if focused { ACCENT } else { Color::TRANSPARENT },
                    width: 2.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}
