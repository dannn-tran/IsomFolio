use super::*;
use iced::widget::{column, row};

impl App {
    pub(super) fn view_add_folder_prompt(&self) -> Element<'_, Msg> {
        let prompt = match &self.add_folder_prompt {
            Some(p) => p,
            None => return Space::new().into(),
        };
        let name = std::path::Path::new(&prompt.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(prompt.path.as_str());

        let glyph = if prompt.recursive { "☑" } else { "☐" };
        let subfolder_label = match prompt.subfolder_count {
            0 => "Include subfolders (none found)".to_string(),
            1 => "Include subfolders (1 found)".to_string(),
            n => format!("Include subfolders ({n} found)"),
        };
        let toggle = button(
            row![
                text(glyph).size(TEXT_BASE).color(FG),
                text(subfolder_label).size(TEXT_MD).color(FG),
            ]
            .spacing(SPACE_1_5)
            .align_y(Alignment::Center),
        )
        .on_press(Msg::AddFolderPromptToggleRecursive)
        .style(ghost_btn_style);

        let body = column![
            text(format!("Add \u{201C}{name}\u{201D} to the library"))
                .size(TEXT_TITLE).color(FG),
            Space::new().height(SPACE_2),
            text("Photos in this folder are indexed. With subfolders included, the whole tree is indexed and shown as a navigable hierarchy in the sidebar.")
                .size(TEXT_SM).color(FG_DIM),
            Space::new().height(SPACE_4),
            toggle,
            Space::new().height(SPACE_4),
            row![
                button(text("Cancel").size(TEXT_BASE))
                    .on_press(Msg::AddFolderCancel)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
                button(text("Add").size(TEXT_BASE))
                    .on_press(Msg::AddFolderConfirm)
                    .style(active_chip_style),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(0)
        .width(440);

        let modal = container(body)
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
                ..Default::default()
            });
        modal_with_backdrop(modal).into()
    }

    pub(super) fn view_shortcut_help(&self) -> Element<'_, Msg> {
        use crate::app::keybinds::{self, Category};

        let bindings = keybinds::bindings();
        let categories = [
            (Category::Navigation, "Navigation"),
            (Category::View, "View"),
            (Category::Culling, "Culling"),
            (Category::Tagging, "Tagging"),
        ];

        let mut col = column![
            row![
                text("Keyboard Shortcuts").size(TEXT_BASE).color(FG),
                Space::new().width(Length::Fill),
                styles::icon_btn("✕", Msg::ToggleShortcutHelp),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACE_2),
        ]
        .spacing(SPACE_2)
        .padding(SPACE_3);

        for (cat, cat_label) in &categories {
            let cat_bindings: Vec<_> = bindings.iter().filter(|b| &b.category == cat).collect();
            if cat_bindings.is_empty() {
                continue;
            }
            col = col.push(text(*cat_label).size(TEXT_SM).color(FG_DIM));
            for bind in cat_bindings {
                let key_str = keybinds::format_key(bind);
                col = col.push(
                    row![
                        container(text(key_str).size(TEXT_SM).color(ACCENT))
                            .width(Length::Fixed(100.0)),
                        text(bind.label).size(TEXT_SM).color(FG),
                    ]
                    .spacing(SPACE_2)
                    .align_y(Alignment::Center),
                );
            }
        }

        // Mouse / trackpad gestures and context-menu actions aren't key bindings,
        // so the auto-generated list above misses them — spell them out here.
        let gesture_sections: [(&str, &[(&str, &str)]); 2] = [
            (
                "Mouse & trackpad",
                &[
                    ("Double-click", "Open photo in Loupe"),
                    ("Cmd+Click", "Toggle one photo in/out of selection"),
                    ("Shift+Click", "Select a range"),
                    ("Drag to album", "Add the selected photos to an album"),
                    ("Scroll in Loupe", "Zoom toward the pointer"),
                    ("Drag in Loupe", "Pan when zoomed in"),
                ],
            ),
            (
                "Right-click (or Ctrl+Click)",
                &[
                    ("Folder", "Sync · Remove · Locate missing"),
                    ("Album", "Rename · Duplicate · Delete · Edit criteria"),
                    ("Photos", "Add to album · Import XMP · Copy/Move · Reveal"),
                    ("Person", "Rename · Merge into…"),
                ],
            ),
        ];
        for (title, rows) in gesture_sections {
            col = col.push(text(title).size(TEXT_SM).color(FG_DIM));
            for (key, desc) in rows {
                col = col.push(
                    row![
                        container(text(*key).size(TEXT_SM).color(ACCENT))
                            .width(Length::Fixed(110.0)),
                        text(*desc).size(TEXT_SM).color(FG),
                    ]
                    .spacing(SPACE_2)
                    .align_y(Alignment::Center),
                );
            }
        }

        let panel = container(col)
            .width(Length::Fixed(420.0))
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border {
                    color: BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

        modal_with_backdrop(panel).into()
    }

    /// Permanent-purge confirmation. The one confirm in the app that uses a modal:
    /// deleting files from disk is irreversible, so the scrim guards against an
    /// accidental click-through and forces a deliberate confirm (see
    /// design-system.md → Modal dialogs). Reversible confirms stay inline.
    pub(super) fn purge_confirm_overlay(&self) -> Element<'_, Msg> {
        let n = self.purge_pending.as_ref().map(|t| t.len()).unwrap_or(0);
        let noun = if n == 1 { "photo" } else { "photos" };
        let trash = crate::app::os_trash_name();

        let body = column![
            text(format!("Move to {trash}")).size(TEXT_TITLE).color(FG),
            Space::new().height(SPACE_2),
            text(format!("{n} {noun} will be moved to the {trash}."))
                .size(TEXT_SM).color(FG_DIM),
            Space::new().height(SPACE_1),
            text("Their ratings and tags are removed from the catalog.")
                .size(TEXT_SM).color(ERR),
            Space::new().height(SPACE_4),
            row![
                button(text("Cancel").size(TEXT_BASE))
                    .on_press(Msg::CancelPurge)
                    .style(ghost_btn_style),
                Space::new().width(Length::Fill),
                button(text(format!("Move to {trash}")).size(TEXT_BASE))
                    .on_press(Msg::ConfirmPurge)
                    .style(danger_btn_style),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(0)
        .width(420);

        let modal = container(body)
            .padding(SPACE_6)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(BG_MODAL)),
                border: Border { color: BORDER, width: 1.0, radius: 10.0.into() },
                ..Default::default()
            });
        // Scrim-click cancels — safe, since Cancel is the non-destructive choice.
        modal_with_backdrop_dismiss(modal, Msg::CancelPurge).into()
    }
}

/// Wrap a modal panel with a backdrop that blocks all mouse events from reaching
/// the layers below. The backdrop is darkened (OVERLAY_MEDIUM) and the modal is
/// centered on top. Scrim-click is inert.
fn modal_with_backdrop<'a, E>(modal: E) -> Element<'a, Msg>
where
    E: Into<Element<'a, Msg>>,
{
    modal_with_backdrop_dismiss(modal, Msg::NoOp)
}

/// As [`modal_with_backdrop`], but a scrim-click emits `on_dismiss` (e.g. Cancel)
/// rather than being inert. All other mouse events are still swallowed.
fn modal_with_backdrop_dismiss<'a, E>(modal: E, on_dismiss: Msg) -> Element<'a, Msg>
where
    E: Into<Element<'a, Msg>>,
{
    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(OVERLAY_MEDIUM)),
                ..Default::default()
            }),
    )
    .on_press(on_dismiss)
    .on_release(Msg::NoOp)
    .on_right_press(Msg::NoOp)
    .on_right_release(Msg::NoOp)
    .on_double_click(Msg::NoOp);

    let centered = container(modal.into())
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    iced::widget::stack(vec![backdrop.into(), centered.into()]).into()
}
