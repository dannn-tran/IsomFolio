use iced::{
    widget::{button, column, container, row, text, text_input, Space},
    Alignment, Color, Element, Length, Theme,
};

use isomfolio_core::models::{Flag, RatingFilter, TagMatch};

use super::styles::{
    active_chip_style, color_label_swatch, danger_btn_style, ghost_btn_style,
    COLOR_LABELS, ERR, FG, FG_DIM, SPACE_0_5, SPACE_1, SPACE_1_5, SPACE_2,
    STAR_GOLD, TEXT_SM, TEXT_XS,
};
use crate::app::{
    parse_date_str, App, DatePreset, Msg, RatingCmp,
};

/// Width of the criteria-panel label column (px).
const FILTER_LABEL_W: f32 = 48.0;

/// One criteria row: `[label] [controls]`. The controls block fills the
/// remaining width so wrapping chip rows and full-width inputs both align to a
/// single left edge.
fn filter_field<'a>(label: &str, content: Element<'a, Msg>) -> Element<'a, Msg> {
    row![
        container(text(label.to_string()).size(TEXT_XS).color(FG_DIM)).width(FILTER_LABEL_W),
        container(content).width(Length::Fill),
    ]
    .spacing(SPACE_1_5)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// A uniform text chip (toggle/segment). Same size and padding everywhere so the
/// panel reads as one control family, not a grab-bag.
fn txt_chip<'a>(label: String, active: bool, msg: Msg) -> Element<'a, Msg> {
    button(text(label).size(TEXT_XS))
        .on_press(msg)
        .padding([SPACE_0_5, SPACE_1_5])
        .style(if active { active_chip_style } else { ghost_btn_style })
        .into()
}

/// A uniform glyph chip with a hover tooltip (the glyph carries the meaning, so
/// the tip is how it's discovered). Colour is fixed (a swatch / star / flag).
fn glyph_chip<'a>(glyph: &str, hint: String, active: bool, color: Color, msg: Msg) -> Element<'a, Msg> {
    super::styles::tip(
        button(text(glyph.to_string()).size(TEXT_SM).color(color))
            .on_press(msg)
            .padding([SPACE_0_5, SPACE_1])
            .style(if active { active_chip_style } else { ghost_btn_style }),
        hint,
        super::styles::TipPos::Right,
    )
}

impl App {
    pub(super) fn view_sidebar_filters(&self) -> Element<'_, Msg> {
        let cur = self.filters.rating;
        let cmp = self.filters.rating_cmp;

        // Uniform two-column grid: every row is `filter_field(label, controls)`,
        // so labels share a left column and controls a single left edge.
        let mut col = column![].spacing(SPACE_2).padding([SPACE_1, SPACE_0_5]);

        // Flags
        let flags = row![
            glyph_chip("✓", "Show picks".into(), self.filters.flags.allows(Flag::Pick), FG, Msg::ToggleFlagFilter(Flag::Pick)),
            glyph_chip("○", "Show unflagged".into(), self.filters.flags.allows(Flag::Unflagged), FG, Msg::ToggleFlagFilter(Flag::Unflagged)),
            glyph_chip("✕", "Show rejects".into(), self.filters.flags.allows(Flag::Reject), FG, Msg::ToggleFlagFilter(Flag::Reject)),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("Flags", flags.into()));

        // Rating — comparator, then 1–5, then unrated.
        let mut rating = row![].spacing(SPACE_1);
        for (c, hint) in [
            (RatingCmp::AtLeast, "At least"),
            (RatingCmp::Exactly, "Exactly"),
            (RatingCmp::AtMost, "At most"),
        ] {
            rating = rating.push(glyph_chip(c.symbol(), hint.to_string(), cmp == c, FG_DIM, Msg::SetRatingCmp(c)));
        }
        for n in 1..=5i32 {
            let active = matches!(cur,
                RatingFilter::AtLeast(v) | RatingFilter::Exactly(v) | RatingFilter::AtMost(v) if v == n);
            let msg = if active { RatingFilter::Any } else { cmp.apply(n) };
            rating = rating.push(glyph_chip(&n.to_string(), format!("{} {n}★", cmp.symbol()), active, STAR_GOLD, Msg::SetRatingFilter(msg)));
        }
        let unrated = matches!(cur, RatingFilter::Unrated);
        let unrated_msg = if unrated { RatingFilter::Any } else { RatingFilter::Unrated };
        rating = rating.push(glyph_chip("0", "Unrated only".into(), unrated, FG_DIM, Msg::SetRatingFilter(unrated_msg)));
        col = col.push(filter_field("Rating", rating.wrap().into()));

        // Colour
        let mut colour = row![].spacing(SPACE_1);
        for name in COLOR_LABELS {
            let active = self.filters.color.as_deref() == Some(name);
            let msg = if active { None } else { Some(name.to_string()) };
            colour = colour.push(glyph_chip("●", format!("Colour: {name}"), active, color_label_swatch(name), Msg::SetColorFilter(msg)));
        }
        col = col.push(filter_field("Colour", colour.wrap().into()));

        // Tags — match toggle, then a wrapped row of tag chips + the add input.
        let is_any = self.filters.tag_match == TagMatch::Any;
        let match_row = row![
            txt_chip("All".into(), !is_any, Msg::SetTagMatch(TagMatch::All)),
            txt_chip("Any".into(), is_any, Msg::SetTagMatch(TagMatch::Any)),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("Tags", match_row.into()));

        let tag_chip = |tag: &str, negated: bool| -> Element<'_, Msg> {
            let style: fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style =
                if negated { danger_btn_style } else { active_chip_style };
            let label = if negated { format!("−{tag}") } else { tag.to_string() };
            row![
                button(text(label).size(TEXT_XS))
                    .on_press(Msg::ToggleFilterTagNegate(tag.to_string()))
                    .padding([SPACE_0_5, SPACE_1])
                    .style(style),
                button(text("×").size(TEXT_XS))
                    .on_press(Msg::RemoveFilterTag(tag.to_string()))
                    .padding([SPACE_0_5, SPACE_1])
                    .style(style),
            ]
            .spacing(1.0)
            .into()
        };
        let mut tag_items = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for tag in &self.filters.tags {
            tag_items = tag_items.push(tag_chip(tag, false));
        }
        for tag in &self.filters.exclude_tags {
            tag_items = tag_items.push(tag_chip(tag, true));
        }
        tag_items = tag_items.push(
            text_input("+ tag", &self.filters.tag_input)
                .on_input(Msg::FilterTagInputChanged)
                .on_submit(Msg::AddFilterTag)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(90),
        );
        col = col.push(filter_field("", tag_items.wrap().into()));

        // Date range
        let from_err = !self.filters.date_from.is_empty()
            && parse_date_str(&self.filters.date_from).is_none();
        let to_err = !self.filters.date_to.is_empty()
            && parse_date_str(&self.filters.date_to).is_none();
        col = col.push(filter_field(
            "From",
            text_input("YYYY-MM-DD", &self.filters.date_from)
                .on_input(Msg::FilterDateFromChanged)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(Length::Fill)
                .into(),
        ));
        col = col.push(filter_field(
            "To",
            text_input("YYYY-MM-DD", &self.filters.date_to)
                .on_input(Msg::FilterDateToChanged)
                .padding([SPACE_0_5, SPACE_1_5])
                .size(TEXT_XS)
                .width(Length::Fill)
                .into(),
        ));
        if from_err || to_err {
            col = col.push(filter_field("", text("Format: YYYY-MM-DD").size(TEXT_XS).color(ERR).into()));
        }
        let mut presets = row![].spacing(SPACE_1);
        for (label, preset) in [
            ("7d", DatePreset::Last7),
            ("30d", DatePreset::Last30),
            ("Month", DatePreset::ThisMonth),
            ("Year", DatePreset::ThisYear),
        ] {
            presets = presets.push(txt_chip(label.into(), false, Msg::SetDatePreset(preset)));
        }
        col = col.push(filter_field("", presets.wrap().into()));

        // Type
        let mut types = row![].spacing(SPACE_1);
        for ext in ["jpg", "png", "webp", "gif"] {
            let active = self.filters.exts.contains(ext);
            types = types.push(txt_chip(format!(".{}", ext.to_uppercase()), active, Msg::ToggleFilterFileType(ext.to_string())));
        }
        col = col.push(filter_field("Type", types.wrap().into()));

        // GPS
        let gps = row![
            txt_chip("Any".into(), self.filters.has_location.is_none(), Msg::SetLocationFilter(None)),
            txt_chip("Yes".into(), self.filters.has_location == Some(true), Msg::SetLocationFilter(Some(true))),
            txt_chip("No".into(), self.filters.has_location == Some(false), Msg::SetLocationFilter(Some(false))),
        ]
        .spacing(SPACE_1);
        col = col.push(filter_field("GPS", gps.into()));

        // Person (only when named clusters exist)
        let named: Vec<&isomfolio_core::models::FaceClusterSummary> = self
            .faces.clusters.iter()
            .filter(|c| c.name.is_some())
            .collect();
        if !named.is_empty() {
            let mut people = row![txt_chip("Any".into(), self.filters.person.is_none(), Msg::SetPersonFilter(None))]
                .spacing(SPACE_1);
            for c in named {
                let name = c.name.clone().unwrap_or_default();
                let active = self.filters.person.as_deref() == Some(c.cluster_id.as_str());
                people = people.push(txt_chip(name, active, Msg::SetPersonFilter(Some(c.cluster_id.clone()))));
            }
            col = col.push(filter_field("Person", people.wrap().into()));
        }

        // Added within
        let mut added = row![].spacing(SPACE_1);
        for (label, days) in [("Any", None), ("7d", Some(7)), ("30d", Some(30))] {
            added = added.push(txt_chip(label.into(), self.filters.added_within_days == days, Msg::SetAddedWithinFilter(days)));
        }
        col = col.push(filter_field("Added", added.into()));

        // Camera (only when cameras exist)
        if !self.cameras.is_empty() {
            let mut cams = row![txt_chip("Any".into(), self.filters.camera.is_none(), Msg::SetCameraFilter(None))]
                .spacing(SPACE_1);
            for cam in &self.cameras {
                let active = self.filters.camera.as_deref() == Some(cam.as_str());
                cams = cams.push(txt_chip(cam.clone(), active, Msg::SetCameraFilter(Some(cam.clone()))));
            }
            col = col.push(filter_field("Camera", cams.wrap().into()));
        }

        // Clear / Smart Album actions — a full-width row under the grid.
        if self.has_active_filters() {
            let is_smart = self.current_album_is_smart();
            let mut actions = row![
                button(text("Clear").size(TEXT_XS)).on_press(Msg::ClearFilters).style(ghost_btn_style),
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_1)
            .align_y(Alignment::Center);

            if is_smart {
                if self.smart_album_dirty {
                    actions = actions.push(text("Unsaved").size(TEXT_XS).color(ERR));
                }
                actions = actions.push(
                    button(text("Update").size(TEXT_XS)).on_press(Msg::UpdateSmartAlbum).style(active_chip_style),
                );
            } else if let Some(ref name_input) = self.filters.save_smart_input {
                actions = actions
                    .push(
                        text_input("Album name…", name_input)
                            .id(crate::app::input_ids::save_smart_album())
                            .on_input(Msg::SmartAlbumNameChanged)
                            .on_submit(Msg::ConfirmSmartAlbum)
                            .padding([SPACE_0_5, SPACE_1_5])
                            .size(TEXT_XS)
                            .width(Length::Fill),
                    )
                    .push(button(text("Save").size(TEXT_XS)).on_press(Msg::ConfirmSmartAlbum).style(active_chip_style));
            } else {
                actions = actions.push(
                    button(text("Save Smart…").size(TEXT_XS)).on_press(Msg::SaveAsSmartAlbum).style(ghost_btn_style),
                );
            }
            col = col.push(container(actions).padding([SPACE_1, 0.0]));
        }

        container(col).width(Length::Fill).into()
    }
}
