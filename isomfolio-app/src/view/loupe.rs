use super::*;
use iced::widget::{column, row};

impl App {
    pub(super) fn view_preview(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe.idx.min(total.saturating_sub(1));

        let img_handle: Option<image::Handle> = self.loupe.full_res.as_ref()
            .and_then(|(full_idx, handle)| if *full_idx == idx { Some(handle.clone()) } else { None });

        let preview_img: Element<Msg> = match img_handle {
            Some(handle) => image(handle)
                .content_fit(iced::ContentFit::Contain)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => {
                let thumb_path = self.files.get(idx).and_then(|f| {
                    match self.thumbnails.get(&f.id) {
                        Some(isomfolio_core::models::ThumbnailState::Ready(p)) => Some(p.clone()),
                        _ => None,
                    }
                });
                match thumb_path {
                    Some(p) => image(image::Handle::from_path(p))
                        .content_fit(iced::ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    None => container(text("Loading…").size(TEXT_MD).color(FG_DIM))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .into(),
                }
            }
        };

        let preview_area = container(preview_img)
            .width(Length::Fill)
            .height(Length::FillPortion(3))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 })),
                ..Default::default()
            });

        let grid_strip = container(self.view_grid())
            .height(Length::FillPortion(2));

        column![preview_area, grid_strip]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub(super) fn view_loupe(&self) -> Element<'_, Msg> {
        let total = self.files.len();
        let idx = self.loupe.idx.min(total.saturating_sub(1));

        let img_handle: Option<image::Handle> = if let Some((full_idx, handle)) = &self.loupe.full_res {
            if *full_idx == idx {
                Some(handle.clone())
            } else {
                None
            }
        } else {
            None
        };

        let img_element: Element<Msg> = match img_handle {
            Some(handle) => loupe_image::LoupeImage::new(
                handle,
                self.loupe.zoom,
                self.loupe.pan,
                |scale, pan| Msg::LoupeZoomChanged { scale, pan },
                |viewport, native| Msg::LoupeGeometry { viewport, native },
            )
            .into(),
            None => {
                if let Some(file) = self.files.get(idx) {
                    let thumb = self
                        .thumbnails
                        .get(&file.id)
                        .and_then(|s| {
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

        let filename = self.files.get(idx).map(|f| f.name.as_str()).unwrap_or("");
        let counter = if total > 0 {
            format!("{} / {}", idx + 1, total)
        } else {
            String::new()
        };

        let top_bar = container(
            row![
                styles::icon_btn("✕", Msg::OpenLoupe),
                Space::new().width(Length::Fill),
                text(filename).size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                text(counter).size(TEXT_MD).color(FG_DIM),
            ]
            .spacing(SPACE_2_5)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_1_5, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        let zoomed = self.loupe.zoom > crate::app::LOUPE_ZOOM_MIN;
        let tip = |el, label: &'static str| styles::tip(el, label, styles::TipPos::Top);
        let zoom_cluster = row![
            tip(
                styles::icon_btn_svg_color(icons::Icon::ZoomOut, Msg::LoupeZoomBy(0.8), FG),
                "Zoom out (−)",
            ),
            tip(
                styles::icon_btn_svg_color(icons::Icon::ZoomIn, Msg::LoupeZoomBy(1.25), FG),
                "Zoom in (+)",
            ),
            tip(
                button(text("1:1").size(TEXT_MD)).on_press(Msg::LoupeZoomActual).style(ghost_btn_style),
                "Actual pixels (Z)",
            ),
            tip(
                button(text("Fit").size(TEXT_MD).color(if zoomed { FG } else { FG_DIM }))
                    .on_press(Msg::LoupeZoomReset).style(ghost_btn_style),
                "Fit to window",
            ),
            tip(
                styles::icon_btn_color(
                    if self.loupe.lock_zoom { "🔒" } else { "🔓" },
                    Msg::ToggleLoupeZoomLock,
                    if self.loupe.lock_zoom { ACCENT } else { FG_DIM },
                ),
                "Lock zoom across photos",
            ),
            tip(
                styles::icon_btn(if self.fullscreen { "⤢" } else { "⛶" }, Msg::ToggleFullscreen),
                "Toggle fullscreen",
            ),
        ]
        .spacing(SPACE_2)
        .align_y(Alignment::Center);

        let bottom_bar = container(
            row![
                Space::new().width(Length::Fill),
                button(text("‹ Prev").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: -1, dy: 0 })
                    .style(ghost_btn_style),
                button(text("Next ›").size(TEXT_BASE))
                    .on_press(Msg::Navigate { dx: 1, dy: 0 })
                    .style(ghost_btn_style),
                Space::new().width(SPACE_3),
                zoom_cluster,
                Space::new().width(Length::Fill),
            ]
            .spacing(SPACE_3)
            .align_y(Alignment::Center),
        )
        .padding([SPACE_2, SPACE_3])
        .width(Length::Fill)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(OVERLAY_HEAVY)),
            ..Default::default()
        });

        let hud_bar = {
            use isomfolio_core::models::Flag as F;
            let flag = self.files.get(idx).map(|f| f.flag).unwrap_or(F::Unflagged);
            let rating = self.file_ratings.get(
                self.files.get(idx).map(|f| f.id.as_str()).unwrap_or("")
            ).copied().unwrap_or(0);

            let flag_btn = |glyph: &'static str, key: &'static str, f: F, active_color: Color, is_current: bool| -> Element<Msg> {
                button(
                    column![
                        text(glyph).size(TEXT_MD),
                        text(key).size(TEXT_SM).color(if is_current {
                            Color { a: 0.7, ..Color::WHITE }
                        } else {
                            Color { a: 0.35, ..Color::WHITE }
                        }),
                    ]
                    .align_x(Alignment::Center)
                    .spacing(1.0),
                )
                .on_press(Msg::SetFlag(f))
                .style(move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(if is_current {
                        Color { r: active_color.r, g: active_color.g, b: active_color.b, a: 0.55 }
                    } else {
                        Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 }
                    })),
                    text_color: if is_current { Color::WHITE } else { FG_DIM },
                    border: Border { radius: 4.0.into(), ..Default::default() },
                    shadow: iced::Shadow::default(),
                    snap: false,
                })
                .into()
            };

            let flag_row = row![
                flag_btn("✓", "P", F::Pick,      ACCENT, flag == F::Pick),
                flag_btn("○", "U", F::Unflagged, FG_DIM, flag == F::Unflagged),
                flag_btn("✕", "X", F::Reject,    ERR,    flag == F::Reject),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center);

            let mut rating_row = row![].spacing(SPACE_0_5);
            for star in 1..=5i32 {
                let filled = rating >= star;
                rating_row = rating_row.push(
                    button(
                        text(if filled { "★" } else { "☆" })
                            .size(TEXT_STAR)
                            .color(if filled { STAR_GOLD } else { Color { r: FG.r, g: FG.g, b: FG.b, a: 0.4 } }),
                    )
                    .on_press(Msg::SetRating(if rating == star { None } else { Some(star) }))
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: FG,
                        border: Border::default(),
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }),
                );
            }

            let cur_label = self
                .files
                .get(idx)
                .and_then(|f| self.file_labels.get(&f.id))
                .cloned();
            let mut color_row = row![].spacing(SPACE_1).align_y(Alignment::Center);
            for name in styles::COLOR_LABELS {
                let active = cur_label.as_deref() == Some(name);
                let swatch = styles::color_label_swatch(name);
                color_row = color_row.push(styles::tip(
                    styles::icon_btn_styled(
                        "●",
                        Msg::SetColorLabel(Some(name.to_string())),
                        move |_: &Theme, _| button::Style {
                            background: if active { Some(Background::Color(Color { a: 0.25, ..swatch })) } else { None },
                            text_color: swatch,
                            border: Border { radius: 4.0.into(), ..Default::default() },
                            shadow: iced::Shadow::default(),
                            snap: false,
                        },
                    ),
                    format!("Label {name}"),
                    styles::TipPos::Top,
                ));
            }

            container(
                row![
                    flag_row,
                    Space::new().width(Length::Fill),
                    color_row,
                    Space::new().width(Length::Fill),
                    rating_row,
                ]
                .align_y(Alignment::Center)
                .spacing(SPACE_2),
            )
            .padding([SPACE_1_5, SPACE_3])
            .width(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_MEDIUM)),
                ..Default::default()
            })
        };

        // When the full-res decode failed for this photo, explain why over the
        // (pixelated thumbnail) fallback instead of leaving it silently broken.
        let middle: Element<Msg> = match self.loupe.load_error.as_ref().filter(|(e, _)| *e == idx) {
            Some((_, err)) => {
                let path = self.files.get(idx).map(|f| f.disk_path());
                stack![
                    img_element,
                    container(self.loupe_error_card(err, path))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center),
                ]
                .into()
            }
            None => img_element,
        };

        let main_col: Element<Msg> = column![
            top_bar,
            middle,
            hud_bar,
            self.view_loupe_filmstrip(idx),
            bottom_bar
        ]
        .into();

        container(main_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.03,
                    g: 0.03,
                    b: 0.03,
                    a: 1.0,
                })),
                ..Default::default()
            })
            .into()
    }

    /// Explanatory card shown over the loupe when the full-res decode failed —
    /// states the reason and offers concrete resolution actions (open privacy
    /// settings for a permission denial, reveal the file in Finder).
    fn loupe_error_card<'a>(
        &self,
        err: &crate::app::LoupeLoadError,
        path: Option<String>,
    ) -> Element<'a, Msg> {
        let mut col = column![
            text("⚠").size(TEXT_TITLE).color(WARN),
            text("Can't open this photo").size(TEXT_BASE).color(FG),
            text(err.filename.clone()).size(TEXT_SM).color(FG_DIM),
            text(err.message.clone()).size(TEXT_SM).color(FG_DIM),
        ]
        .spacing(SPACE_2)
        .align_x(Alignment::Center);

        let mut actions = row![].spacing(SPACE_2).align_y(Alignment::Center);
        if err.permission {
            actions = actions.push(
                button(text("Open Privacy Settings").size(TEXT_MD))
                    .on_press(Msg::OpenPrivacySettings)
                    .style(active_chip_style),
            );
        }
        if let Some(p) = path {
            actions = actions.push(
                button(text("Show in Finder").size(TEXT_MD))
                    .on_press(Msg::ShowInFinder(vec![p]))
                    .style(ghost_btn_style),
            );
        }
        col = col.push(actions);

        container(col)
            .width(Length::Fixed(360.0))
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
                ..Default::default()
            })
            .into()
    }

    /// Horizontal thumbnail strip under the loupe image: a window of neighbours
    /// centred on the current photo, current one ringed, click to jump. Windowed
    /// (not the whole library) to keep the widget count bounded.
    fn view_loupe_filmstrip(&self, current: usize) -> Element<'_, Msg> {
        const THUMB: f32 = 56.0;
        const WINDOW: usize = 14;
        let total = self.files.len();
        // A window of the whole view around the current photo.
        let indices: Vec<usize> = {
            let lo = current.saturating_sub(WINDOW);
            let hi = (current + WINDOW + 1).min(total);
            (lo..hi).collect()
        };

        let mut strip = row![].spacing(SPACE_1).align_y(Alignment::Center);
        for i in indices {
            let file = &self.files[i];
            let is_cur = i == current;
            let thumb: Element<Msg> = match self.thumbnails.get(&file.id) {
                Some(isomfolio_core::models::ThumbnailState::Ready(p)) => {
                    image(image::Handle::from_path(p))
                        .width(THUMB)
                        .height(THUMB)
                        .content_fit(iced::ContentFit::Cover)
                        .into()
                }
                _ => Space::new().width(THUMB).height(THUMB).into(),
            };
            let ring = if is_cur { ACCENT } else { Color::TRANSPARENT };
            strip = strip.push(
                button(
                    container(thumb)
                        .style(move |_: &Theme| container::Style {
                            border: Border { color: ring, width: 2.0, radius: 3.0.into() },
                            ..Default::default()
                        })
                        .clip(true),
                )
                .padding(0)
                .on_press(Msg::LoupeJumpTo(i))
                .style(|_: &Theme, _| button::Style {
                    background: None,
                    text_color: FG,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }),
            );
        }

        container(strip)
            .width(Length::Fill)
            .height(Length::Fixed(THUMB + SPACE_2 * 2.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .clip(true)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_HEAVY)),
                ..Default::default()
            })
            .into()
    }
}
