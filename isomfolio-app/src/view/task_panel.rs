use super::*;
use iced::widget::{column, row};

impl App {
    fn active_tasks(&self) -> Vec<TaskView> {
        let mut tasks = Vec::new();

        if self.faces.is_clustering {
            tasks.push(TaskView {
                title: "Finding people".into(),
                detail: self.faces.status.clone().unwrap_or_default(),
                progress: match self.faces.progress {
                    Some(r) => TaskProgress::Determinate(r),
                    None => TaskProgress::Indeterminate,
                },
                failed: false,
                done: false,
                dismiss: None,
            });
        }

        if self.is_syncing {
            let detail = if self.status.starts_with("Syncing") {
                self.status.clone()
            } else {
                "Scanning…".into()
            };
            tasks.push(TaskView {
                title: "Sync".into(),
                detail,
                progress: TaskProgress::Indeterminate,
                failed: false,
                done: false,
                dismiss: None,
            });
        }

        if self.thumb_ctx.total > 0 {
            let total = self.thumb_ctx.total;
            let done = total.saturating_sub(self.thumb_ctx.pending);
            let ratio = done as f32 / total.max(1) as f32;
            let eta = if done >= 3 {
                self.thumb_ctx
                    .start_at
                    .map(|s| {
                        let elapsed = s.elapsed().as_secs_f64();
                        let secs =
                            (self.thumb_ctx.pending as f64 / (done as f64 / elapsed)).ceil() as u64;
                        if secs < 60 {
                            format!(" · ~{secs}s")
                        } else {
                            format!(" · ~{}m{}s", secs / 60, secs % 60)
                        }
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            tasks.push(TaskView {
                title: "Thumbnails".into(),
                detail: format!("{done} / {total}{eta}"),
                progress: TaskProgress::Determinate(ratio),
                failed: false,
                done: false,
                dismiss: None,
            });
        }

        for task in &self.bg_tasks {
            let failed = task.failed.is_some();
            tasks.push(TaskView {
                title: task.label.clone(),
                detail: task.failed.clone().unwrap_or_default(),
                progress: match task.progress {
                    Some(r) => TaskProgress::Determinate(r),
                    None => TaskProgress::Indeterminate,
                },
                failed,
                done: false,
                dismiss: failed.then_some(task.id),
            });
        }

        // Recently-finished tasks linger with a ✓ until they expire.
        for done in &self.completed_tasks {
            tasks.push(TaskView {
                title: done.title.clone(),
                detail: done.detail.clone(),
                progress: TaskProgress::Determinate(1.0),
                failed: false,
                done: true,
                dismiss: None,
            });
        }

        tasks
    }

    pub(super) fn view_task_panel(&self) -> Element<'_, Msg> {
        let open = self.task_panel_open;
        let tasks = self.active_tasks();

        // Collapsed pill — shows when panel is minimised.
        if !open {
            let n = tasks.len();
            let label = if n == 1 { "1 task".to_string() } else { format!("{n} tasks") };
            let pill = container(
                button(
                    row![
                        text("◌").size(TEXT_XS).color(FG_DIM),
                        text(label).size(TEXT_XS).color(FG_DIM),
                        Space::new().width(Length::Fill),
                        icons::icon(icons::Icon::ChevronUp, FG_DIM),
                    ]
                    .spacing(SPACE_1)
                    .align_y(Alignment::Center),
                )
                .on_press(Msg::ToggleTaskPanel)
                .style(|_: &Theme, _| button::Style {
                    background: None,
                    text_color: FG_DIM,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                })
                .width(Length::Fill),
            )
            .width(160)
            .padding([SPACE_0_5, SPACE_1_5])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PANEL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow { color: OVERLAY_LIGHT, offset: Vector::new(0.0, 2.0), blur_radius: 8.0 },
                ..Default::default()
            });

            return container(pill)
                .width(Length::Fill).height(Length::Fill)
                .align_x(Alignment::End).align_y(Alignment::End)
                .padding(iced::Padding { top: 0.0, right: SPACE_3, bottom: 38.0, left: 0.0 })
                .into();
        }

        // Expanded panel — build rows for each active task.
        let mut col = column![].spacing(0);

        // Header
        col = col.push(
            row![
                text("Tasks").size(TEXT_XS).color(FG_DIM),
                Space::new().width(Length::Fill),
                styles::icon_btn_svg(icons::Icon::ChevronDown, Msg::ToggleTaskPanel),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACE_1),
        );

        for task in tasks {
            col = col.push(Space::new().height(SPACE_1));
            col = col.push(task_row(task));
        }

        let panel = container(col.padding([SPACE_1, SPACE_1_5]))
            .width(210)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_PANEL)),
                border: Border { color: BORDER, width: 1.0, radius: 6.0.into() },
                shadow: Shadow { color: OVERLAY_LIGHT, offset: Vector::new(0.0, 3.0), blur_radius: 10.0 },
                ..Default::default()
            });

        container(panel)
            .width(Length::Fill).height(Length::Fill)
            .align_x(Alignment::End).align_y(Alignment::End)
            .padding(iced::Padding { top: 0.0, right: SPACE_3, bottom: 38.0, left: 0.0 })
            .into()
    }
}

/// One uniform progress entry — the only thing the task panel renders.
enum TaskProgress {
    /// Known fraction 0.0–1.0.
    Determinate(f32),
    /// In progress, amount unknown (engine starting, scanning, …).
    Indeterminate,
}

struct TaskView {
    title: String,
    detail: String,
    progress: TaskProgress,
    failed: bool,
    /// Finished: render a ✓ and no progress bar.
    done: bool,
    dismiss: Option<crate::app::BgTaskId>,
}

/// A 2px-high colored segment occupying `portion` of the track width.
fn bar_segment(portion: u16, color: Color) -> Element<'static, Msg> {
    container(Space::new())
        .width(Length::FillPortion(portion.max(1)))
        .height(2)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border { radius: 1.0.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

fn task_row(task: TaskView) -> Element<'static, Msg> {
    let label_color = if task.failed { ERR } else { FG };

    let title_text = if task.done {
        format!("✓ {}", task.title)
    } else {
        task.title.clone()
    };
    let title_color = if task.done { ACCENT } else { label_color };

    let mut header = row![
        text(title_text).size(TEXT_SM).color(title_color),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACE_1);

    // Finished: just the ✓ title + optional detail, no progress bar.
    if task.done {
        let mut col = column![header].spacing(SPACE_0_5);
        if !task.detail.is_empty() {
            col = col.push(text(task.detail).size(TEXT_XS).color(FG_MUTED));
        }
        return col.into();
    }

    if let Some(id) = task.dismiss {
        header = header.push(styles::icon_btn("✕", Msg::BgTaskDismissed(id)));
    }

    let mut col = column![header].spacing(SPACE_0_5);

    if task.failed {
        if !task.detail.is_empty() {
            col = col.push(text(task.detail).size(TEXT_XS).color(ERR));
        }
        return col.into();
    }

    // Determinate fills proportionally; indeterminate floats a centered segment
    // so it reads as "working, amount unknown" rather than a partial fill.
    let bar = match task.progress {
        TaskProgress::Determinate(ratio) => {
            let filled = (ratio.clamp(0.0, 1.0) * 1000.0) as u16;
            row![
                bar_segment(filled, ACCENT),
                bar_segment(1000u16.saturating_sub(filled), BG_PROGRESS_TRACK),
            ]
        }
        TaskProgress::Indeterminate => row![
            bar_segment(35, BG_PROGRESS_TRACK),
            bar_segment(30, ACCENT),
            bar_segment(35, BG_PROGRESS_TRACK),
        ],
    }
    .width(Length::Fill);
    col = col.push(bar);

    if !task.detail.is_empty() {
        col = col.push(text(task.detail).size(TEXT_XS).color(FG_MUTED));
    }

    col.into()
}
