use iced::{
    widget::{button, column, container, image, row, text, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use crate::app::{App, Msg};
use super::styles::{
    BG_GRID, FG, FG_DIM, OVERLAY_HEAVY, SPACE_1, SPACE_1_5, SPACE_2, SPACE_2_5, SPACE_3,
    TEXT_BASE, TEXT_MD,
};

impl App {
    pub(crate) fn view_compare(&self) -> Element<'_, Msg> {
        let panels: Vec<Element<Msg>> = (0..2)
            .map(|slot| self.compare_panel(slot))
            .collect();

        let top_bar = container(
            row![
                button(text("✕").size(TEXT_BASE).color(FG))
                    .on_press(Msg::EscapePressed)
                    .style(|_: &Theme, _| button::Style {
                        background: Some(Background::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 })),
                        text_color: FG,
                        border: Border { radius: 4.0.into(), ..Default::default() },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
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

        let images_row = row(panels).spacing(SPACE_2).width(Length::Fill).height(Length::Fill);

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

        let name = file.map(|f| f.name.as_str()).unwrap_or("");
        let label = container(
            text(name).size(TEXT_MD).color(FG_DIM),
        )
        .padding([SPACE_1, SPACE_1_5])
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        column![img_el, label].spacing(SPACE_2).width(Length::Fill).height(Length::Fill).into()
    }
}
