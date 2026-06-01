use iced::{
    widget::{button, column, container, image, mouse_area, row, scrollable, text, text_input, Space},
    Alignment, Background, Border, Element, Length, Theme,
};

use isomfolio_core::models::FaceClusterSummary;

use super::styles::{
    ghost_btn_style, BG_GRID, BG_TILE_LOADING, FG, FG_DIM, FG_MUTED,
    SPACE_1, SPACE_1_5, SPACE_2, SPACE_3, SPACE_4, TEXT_BASE, TEXT_SM,
};
use crate::app::{App, Msg, SidebarItem, TILE_GAP};

const PERSON_CARD_SIZE: f32 = 96.0;
const CARD_TOTAL: f32 = PERSON_CARD_SIZE + 28.0;

impl App {
    pub(super) fn view_people_grid(&self) -> Element<'_, Msg> {
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

        let mut content = column![].spacing(SPACE_3).padding([SPACE_3, SPACE_3]);

        if let Some(ref status) = self.faces.status {
            content = content.push(text(status.as_str()).size(TEXT_SM).color(FG_DIM));
        }

        if !named.is_empty() {
            content = content.push(self.people_grid("Named", &named, cols));
        }
        if !unnamed.is_empty() {
            content = content.push(self.people_grid("Unnamed", &unnamed, cols));
        }
        if !unknown.is_empty() {
            content = content.push(self.people_grid("Unknown", &unknown, cols));
        }

        if self.faces.clusters.is_empty() && self.faces.status.is_none() {
            let empty = if self.inference_manifest.is_none() {
                column![
                    text("People needs a face engine").size(TEXT_BASE).color(FG_DIM),
                    text("Install one to detect and group faces — it runs locally, on your machine.")
                        .size(TEXT_SM).color(FG_MUTED),
                    Space::new().height(SPACE_2),
                    button(text("Open Settings → Extensions").size(TEXT_SM))
                        .on_press(Msg::OpenSettings)
                        .style(ghost_btn_style),
                ]
                .spacing(SPACE_1)
                .align_x(Alignment::Center)
            } else {
                column![
                    text("No people found yet").size(TEXT_BASE).color(FG_DIM),
                    text("Click ⟳ in the sidebar to run face detection").size(TEXT_SM).color(FG_MUTED),
                ]
                .spacing(SPACE_1)
                .align_x(Alignment::Center)
            };
            content = content.push(
                container(empty)
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

        container(scrollable_content)
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
        let section = column![
            text(section_label).size(TEXT_SM).color(FG_DIM),
        ]
        .spacing(SPACE_2);

        clusters
            .chunks(cols)
            .fold(section, |acc, chunk| {
                let cells: Vec<Element<Msg>> =
                    chunk.iter().map(|c| self.person_card(c)).collect();
                acc.push(row(cells).spacing(TILE_GAP))
            })
            .into()
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

        let img_btn = button(face_img)
            .on_press(Msg::SidebarItemClicked(SidebarItem::FaceCluster(cluster_id.clone())))
            .style(|_: &Theme, _| button::Style {
                background: None,
                text_color: FG,
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            });

        let cid = cluster_id.clone();
        let click_target = mouse_area(img_btn)
            .on_right_press(Msg::OpenFaceClusterMenu(cid));

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
                if cluster_id == "face-unknown" { "Unknown" } else { "Unnamed" }
            );
            let count = cluster.file_count;
            column![
                text(display).size(TEXT_SM).color(FG),
                text(format!("{count}")).size(TEXT_SM).color(FG_MUTED),
            ]
            .align_x(Alignment::Center)
            .into()
        };

        column![click_target, label]
            .width(PERSON_CARD_SIZE)
            .align_x(Alignment::Center)
            .spacing(SPACE_1)
            .into()
    }
}
