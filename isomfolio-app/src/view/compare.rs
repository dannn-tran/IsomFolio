use iced::{
    widget::{column, container, image, row, stack, text, Space},
    Alignment, Background, Element, Length, Theme,
};
use isomfolio_core::models::Flag;

use crate::app::{App, Msg};
use super::styles::{
    ACCENT, BG_GRID, ERR, FG, FG_DIM, OVERLAY_HEAVY, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5,
    SPACE_3, STAR_GOLD, TEXT_BASE, TEXT_MD, TEXT_SM,
};

impl App {
    pub(crate) fn view_compare(&self) -> Element<'_, Msg> {
        let panels: Vec<Element<Msg>> = (0..2)
            .map(|slot| self.compare_panel(slot))
            .collect();

        let top_bar = container(
            row![
                super::styles::icon_btn("✕", Msg::EscapePressed),
                Space::new().width(Length::Fill),
                text("Compare").size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                text("Esc to exit").size(TEXT_MD).color(FG_DIM),
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

        let images_row = row(panels)
            .spacing(SPACE_2)
            .padding([SPACE_2, SPACE_3])
            .width(Length::Fill)
            .height(Length::Fill);

        let body = container(images_row)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            });

        column![top_bar, body].into()
    }

    fn compare_panel(&self, slot: usize) -> Element<'_, Msg> {
        let file = self.compare.files[slot].as_ref();
        let handle = self.compare.handles[slot].as_ref();

        let img_el: Element<Msg> = match handle {
            Some(h) => image(h.clone())
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
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

        stack![img_el, overlay].width(Length::Fill).height(Length::Fill).into()
    }
}
