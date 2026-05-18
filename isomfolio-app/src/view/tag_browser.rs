use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Space},
    Alignment, Background, Border, Color, Element, Length, Theme,
};

use super::styles::{
    danger_btn_style, ghost_btn_style, BG_MODAL, BORDER, ERR, FG, FG_DIM, SPACE_0_5, SPACE_1,
    SPACE_1_5, SPACE_2, SPACE_2_5, TEXT_BASE, TEXT_MD, TEXT_SM, TEXT_XS,
};
use crate::app::{App, Msg};

impl App {
    pub(super) fn view_tag_browser(&self) -> Option<Element<'_, Msg>> {
        let tb = self.tag_browser.as_ref()?;

        let filter_lower = tb.filter.to_lowercase();
        let filtered: Vec<&(String, usize)> = tb
            .tags
            .iter()
            .filter(|(tag, _)| {
                filter_lower.is_empty() || tag.to_lowercase().contains(&filter_lower)
            })
            .collect();

        let header = row![
            text("All Tags").size(TEXT_BASE).color(FG),
            Space::new().width(Length::Fill),
            button(text("✕").size(TEXT_MD).color(FG_DIM))
                .on_press(Msg::CloseTagBrowser)
                .style(ghost_btn_style),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACE_2);

        let filter_input = text_input("Filter tags…", &tb.filter)
            .on_input(Msg::TagBrowserFilterChanged)
            .padding([SPACE_1, SPACE_1_5])
            .size(TEXT_MD)
            .width(Length::Fill);

        let mut tag_list = column![].spacing(0);

        if filtered.is_empty() {
            tag_list = tag_list.push(
                container(
                    text(if tb.filter.is_empty() {
                        "No tags yet"
                    } else {
                        "No matches"
                    })
                    .size(TEXT_SM)
                    .color(FG_DIM),
                )
                .padding([SPACE_2, SPACE_1_5]),
            );
        }

        for (tag, count) in &filtered {
            let is_rename_target = tb
                .rename
                .as_ref()
                .map(|(orig, _)| orig.as_str() == tag.as_str())
                .unwrap_or(false);
            let is_delete_armed = tb.delete_armed.as_deref() == Some(tag.as_str());

            let row_el: Element<Msg> = if is_rename_target {
                let (_, input) = tb.rename.as_ref().unwrap();
                container(
                    row![
                        text_input("New name…", input)
                            .on_input(Msg::TagBrowserRenameChanged)
                            .on_submit(Msg::TagBrowserRenameConfirm)
                            .padding([SPACE_0_5, SPACE_1])
                            .size(TEXT_SM)
                            .width(Length::Fill),
                        button(text("✓").size(TEXT_SM).color(FG))
                            .on_press(Msg::TagBrowserRenameConfirm)
                            .style(ghost_btn_style),
                        button(text("✕").size(TEXT_SM).color(FG_DIM))
                            .on_press(Msg::TagBrowserRenameCancel)
                            .style(ghost_btn_style),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .padding([SPACE_0_5, SPACE_1_5])
                .width(Length::Fill)
                .into()
            } else if is_delete_armed {
                let sub_count = tb
                    .tags
                    .iter()
                    .filter(|(t, _)| t != tag && t.starts_with(&format!("{tag}/")))
                    .count();
                let label = if sub_count > 0 {
                    format!("Delete «{tag}» + {sub_count} sub-tag(s)?")
                } else {
                    format!("Delete «{tag}»?")
                };
                container(
                    row![
                        text(label).size(TEXT_SM).color(ERR),
                        Space::new().width(Length::Fill),
                        button(text("Confirm").size(TEXT_SM))
                            .on_press(Msg::TagBrowserDeleteConfirm)
                            .style(danger_btn_style),
                        button(text("Cancel").size(TEXT_SM))
                            .on_press(Msg::TagBrowserDeleteCancel)
                            .style(ghost_btn_style),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .padding([SPACE_0_5, SPACE_1_5])
                .width(Length::Fill)
                .into()
            } else {
                container(
                    row![
                        render_tag_name(tag.as_str()),
                        Space::new().width(Length::Fill),
                        text(format!("{count}")).size(TEXT_XS).color(FG_DIM),
                        button(text("Rename").size(TEXT_XS))
                            .on_press(Msg::TagBrowserRenameStart(tag.clone()))
                            .style(ghost_btn_style),
                        button(text("Delete").size(TEXT_XS).color(ERR))
                            .on_press(Msg::TagBrowserDeleteArm(tag.clone()))
                            .style(ghost_btn_style),
                    ]
                    .spacing(SPACE_1_5)
                    .align_y(Alignment::Center),
                )
                .padding([SPACE_0_5, SPACE_1_5])
                .width(Length::Fill)
                .into()
            };

            tag_list = tag_list.push(row_el);
        }

        let panel = container(
            column![
                container(header)
                    .padding([SPACE_2, SPACE_2_5])
                    .width(Length::Fill),
                container(filter_input)
                    .padding(iced::Padding {
                        top: 0.0,
                        right: SPACE_2_5,
                        bottom: SPACE_1_5,
                        left: SPACE_2_5,
                    })
                    .width(Length::Fill),
                scrollable(tag_list)
                    .height(Length::Fixed(420.0))
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new().width(4).scroller_width(4),
                    )),
            ]
        )
        .width(Length::Fixed(440.0))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(BG_MODAL)),
            border: Border {
                color: BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        });

        let overlay = container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.6,
                })),
                ..Default::default()
            });

        Some(overlay.into())
    }
}

fn render_tag_name<'a>(tag: &'a str) -> Element<'a, Msg> {
    let parts: Vec<&str> = tag.split('/').collect();
    let n = parts.len();
    if n == 1 {
        return text(tag).size(TEXT_SM).color(FG).into();
    }
    let mut r = row![].spacing(0);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            r = r.push(text("/").size(TEXT_SM).color(FG_DIM));
        }
        let color = if i == n - 1 { FG } else { FG_DIM };
        r = r.push(text(*part).size(TEXT_SM).color(color));
    }
    r.into()
}
