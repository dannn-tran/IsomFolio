use iced::{
    widget::{button, column, container, image, row, stack, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};
use isomfolio_core::models::Flag;

use crate::app::{App, Msg};
use super::styles::{
    active_chip_style, ghost_btn_style, ACCENT, BG_GRID, ERR, FG, FG_DIM, OVERLAY_HEAVY, SPACE_0_5,
    SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3, STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM,
};

/// A small segmented-control chip — `active_chip_style` when selected, ghost
/// otherwise. Used for the column-count and sort toggles in the Compare top bar.
fn chip<'a>(label: impl text::IntoFragment<'a>, msg: Msg, active: bool) -> Element<'a, Msg> {
    button(text(label).size(TEXT_MD))
        .padding([SPACE_0_5, SPACE_1_5])
        .on_press(msg)
        .style(move |t: &Theme, s| if active { active_chip_style(t, s) } else { ghost_btn_style(t, s) })
        .into()
}

impl App {
    pub(crate) fn view_compare(&self) -> Element<'_, Msg> {
        let n = self.compare.files.len();
        let cols = self.compare.grid_cols();
        let order = self.compare.display_order();

        // Column-count control: Auto (√n) plus explicit 1..=min(n,4). Lets the user
        // force a single row, a 2-up, etc. when the auto-square isn't what they want.
        // The bare "1 2 3" are opaque on their own, so each carries a tooltip.
        let tip = |el, label: &'static str| {
            super::styles::tip(el, label, super::styles::TipPos::Bottom)
        };
        let mut col_control = row![text("Columns").size(TEXT_MD).color(FG_DIM)]
            .spacing(SPACE_1)
            .align_y(Alignment::Center);
        col_control = col_control.push(tip(
            chip("Auto", Msg::CompareSetCols(None), self.compare.cols.is_none()),
            "Auto — a roughly square grid",
        ));
        for c in 1..=n.min(4) {
            let label: &'static str = match c {
                1 => "1 column (single row of rows)",
                2 => "2 columns",
                3 => "3 columns",
                _ => "4 columns",
            };
            col_control = col_control.push(tip(
                chip(c.to_string(), Msg::CompareSetCols(Some(c)), self.compare.cols == Some(c)),
                label,
            ));
        }

        let sort_chip = tip(
            chip("\u{25c9} Sharpest first", Msg::CompareToggleSort, self.compare.sort_sharp),
            "Reorder panes sharpest → softest",
        );

        let top_bar = container(
            row![
                super::styles::icon_btn("✕", Msg::EscapePressed),
                text(format!("Compare · {n}")).size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                col_control,
                sort_chip,
                Space::new().width(Length::Fill),
                text("←/→ focus · P pick · X reject · scroll zoom · Esc")
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

        // Wrap the display order into `cols`-wide rows; each row shares the height
        // and each cell the width, so panes stay as large and square as the count
        // allows instead of being squeezed into one horizontal strip.
        let mut grid = column![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
        for chunk in order.chunks(cols) {
            let mut r = row![].spacing(SPACE_2).width(Length::Fill).height(Length::Fill);
            for &slot in chunk {
                r = r.push(self.compare_panel(slot));
            }
            // Pad a short final row so its cells keep the same width as full rows
            // above (a Fill cell would otherwise stretch to fill the gap).
            for _ in chunk.len()..cols {
                r = r.push(Space::new().width(Length::Fill).height(Length::Fill));
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

        column![top_bar, body].into()
    }

    fn compare_panel(&self, slot: usize) -> Element<'_, Msg> {
        let file = self.compare.files.get(slot);
        let handle = self.compare.handles.get(slot).and_then(|h| h.as_ref());

        let img_el: Element<Msg> = match handle {
            // Full-res handle → a zoomable/pannable pane driven by the *shared*
            // compare zoom/pan, so panning/zooming one pane moves them all in lockstep.
            Some(h) => super::loupe_image::LoupeImage::new(
                h.clone(),
                self.compare.zoom,
                self.compare.pan,
                |scale, pan| Msg::CompareZoomChanged { scale, pan },
                |_, _| Msg::NoOp,
            )
            .into(),
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
