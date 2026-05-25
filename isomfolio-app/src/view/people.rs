use iced::{
    widget::{button, column, container, image, row, scrollable, text, text_input, Space},
    Alignment, Background, Border, Element, Length, Theme,
};

use isomfolio_core::models::FaceClusterSummary;

use super::styles::{
    ghost_btn_style, icon_btn_style, BG_GRID, BG_TILE_LOADING, FG, FG_DIM, FG_MUTED,
    SPACE_1, SPACE_1_5, SPACE_2, SPACE_3, SPACE_4, TEXT_BASE, TEXT_MD, TEXT_SM,
};
use crate::app::{App, Msg, SidebarItem, TILE_GAP};

const PERSON_CARD_SIZE: f32 = 96.0;
const CARD_TOTAL: f32 = PERSON_CARD_SIZE + 28.0;

impl App {
    pub(super) fn view_people(&self) -> Element<'_, Msg> {
        let header = row![
            button(text("← Back").size(TEXT_MD))
                .on_press(Msg::SidebarItemClicked(SidebarItem::AllFiles))
                .style(ghost_btn_style),
            text("People").size(TEXT_BASE).color(FG),
            Space::new().width(Length::Fill),
            button(text("⟳").size(TEXT_MD))
                .on_press(Msg::RunFaceClustering)
                .style(icon_btn_style),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center)
        .padding([SPACE_2, SPACE_3]);

        let grid_width = self.viewport_width;
        let cols = ((grid_width - SPACE_3 * 2.0) / (CARD_TOTAL + TILE_GAP)).max(1.0) as usize;

        let named: Vec<_> = self.faces.clusters.iter()
            .filter(|c| c.name.is_some() && c.cluster_id != "face-unknown")
            .collect();
        let unnamed: Vec<_> = self.faces.clusters.iter()
            .filter(|c| c.name.is_none() && c.cluster_id != "face-unknown")
            .collect();
        let unknown: Vec<_> = self.faces.clusters.iter()
            .filter(|c| c.cluster_id == "face-unknown")
            .collect();

        let mut content = column![].spacing(SPACE_3).padding([0.0, SPACE_3]);

        if !named.is_empty() {
            content = content.push(self.people_grid("Named", &named, cols));
        }
        if !unnamed.is_empty() {
            content = content.push(self.people_grid("Unnamed", &unnamed, cols));
        }
        if !unknown.is_empty() {
            content = content.push(self.people_grid("Unknown", &unknown, cols));
        }

        if self.faces.clusters.is_empty() {
            content = content.push(
                container(
                    column![
                        text("No people found yet").size(TEXT_BASE).color(FG_DIM),
                        text("Click ⟳ to run face detection").size(TEXT_SM).color(FG_MUTED),
                    ]
                    .spacing(SPACE_1)
                    .align_x(Alignment::Center),
                )
                .width(Length::Fill)
                .padding(SPACE_4)
                .align_x(Alignment::Center),
            );
        }

        let scrollable_content = scrollable(content)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::new().width(6).scroller_width(6),
            ))
            .height(Length::Fill);

        container(column![header, scrollable_content])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_GRID)),
                ..Default::default()
            })
            .into()
    }

    fn people_grid<'a>(
        &'a self,
        section_label: &'a str,
        clusters: &[&'a FaceClusterSummary],
        cols: usize,
    ) -> Element<'a, Msg> {
        let mut section = column![
            text(section_label).size(TEXT_SM).color(FG_DIM),
        ]
        .spacing(SPACE_2);

        let mut current_row: Vec<Element<Msg>> = Vec::new();
        for cluster in clusters {
            current_row.push(self.person_card(cluster));
            if current_row.len() >= cols {
                section = section.push(row(std::mem::take(&mut current_row)).spacing(TILE_GAP));
            }
        }
        if !current_row.is_empty() {
            section = section.push(row(current_row).spacing(TILE_GAP));
        }

        section.into()
    }

    fn person_card<'a>(&'a self, cluster: &'a FaceClusterSummary) -> Element<'a, Msg> {
        let cluster_id = &cluster.cluster_id;
        let is_renaming = self.faces.rename_cluster_id.as_deref() == Some(cluster_id.as_str());

        let face_img: Element<Msg> = match self.faces.crop_handles.get(cluster_id) {
            Some(handle) => container(
                image(handle.clone())
                    .width(PERSON_CARD_SIZE)
                    .height(PERSON_CARD_SIZE)
                    .content_fit(iced::ContentFit::Cover),
            )
            .style(|_: &Theme| container::Style {
                border: Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            })
            .clip(true)
            .into(),
            None => container(Space::new())
                .width(PERSON_CARD_SIZE)
                .height(PERSON_CARD_SIZE)
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(BG_TILE_LOADING)),
                    border: Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),
        };

        let click_target = button(face_img)
            .on_press(Msg::SidebarItemClicked(SidebarItem::FaceCluster(cluster_id.clone())))
            .style(|_: &Theme, _| button::Style {
                background: None,
                text_color: FG,
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            });

        let label: Element<Msg> = if is_renaming {
            row![
                text_input("Name…", &self.faces.rename_input)
                    .on_input(Msg::RenameFaceClusterInputChanged)
                    .on_submit(Msg::ConfirmRenameFaceCluster)
                    .padding([SPACE_1, SPACE_1_5])
                    .size(TEXT_SM)
                    .width(PERSON_CARD_SIZE),
            ]
            .into()
        } else {
            let display = cluster.name.as_deref().unwrap_or(
                if cluster_id == "face-unknown" { "Unknown" } else { "?" }
            );
            let count = cluster.file_count;
            button(
                column![
                    text(display).size(TEXT_SM).color(FG),
                    text(format!("{count}")).size(TEXT_SM).color(FG_MUTED),
                ]
                .align_x(Alignment::Center),
            )
            .on_press(Msg::RenameFaceCluster(cluster_id.clone()))
            .style(|_: &Theme, _| button::Style {
                background: None,
                text_color: FG,
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            })
            .into()
        };

        column![click_target, label]
            .width(PERSON_CARD_SIZE)
            .align_x(Alignment::Center)
            .spacing(SPACE_1)
            .into()
    }
}
