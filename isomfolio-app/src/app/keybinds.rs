use std::sync::LazyLock;

use iced::keyboard;

use super::Msg;

static BINDINGS: LazyLock<Vec<KeyBind>> = LazyLock::new(default_bindings);

pub fn bindings() -> &'static [KeyBind] {
    &BINDINGS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Navigation,
    View,
    Culling,
    Tagging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Named(keyboard::key::Named),
    Char(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mods {
    pub command: bool,
    pub shift: bool,
}

impl Mods {
    pub const NONE: Self = Mods { command: false, shift: false };
    pub const SHIFT: Self = Mods { command: false, shift: true };
    pub const CMD: Self = Mods { command: true, shift: false };
    pub const CMD_SHIFT: Self = Mods { command: true, shift: true };

    pub fn matches(self, m: keyboard::Modifiers) -> bool {
        m.command() == self.command && m.shift() == self.shift
    }
}

pub struct KeyBind {
    pub key: Key,
    pub mods: Mods,
    pub when_ignored: bool,
    pub action: fn() -> Msg,
    pub label: &'static str,
    pub category: Category,
}

pub fn default_bindings() -> Vec<KeyBind> {
    use Category::*;
    use Key::*;
    use keyboard::key::Named;

    vec![
        // Navigation
        KeyBind { key: Named(Named::ArrowLeft),  mods: Mods::NONE, when_ignored: true,  action: || Msg::Navigate { dx: -1, dy: 0 },  label: "Previous",          category: Navigation },
        KeyBind { key: Named(Named::ArrowRight), mods: Mods::NONE, when_ignored: true,  action: || Msg::Navigate { dx: 1, dy: 0 },   label: "Next",              category: Navigation },
        KeyBind { key: Named(Named::ArrowUp),    mods: Mods::NONE, when_ignored: true,  action: || Msg::Navigate { dx: 0, dy: -1 },  label: "Up",                category: Navigation },
        KeyBind { key: Named(Named::ArrowDown),  mods: Mods::NONE, when_ignored: true,  action: || Msg::Navigate { dx: 0, dy: 1 },   label: "Down",              category: Navigation },
        KeyBind { key: Named(Named::ArrowLeft),  mods: Mods::SHIFT, when_ignored: true, action: || Msg::NavigateExtend { dx: -1, dy: 0 }, label: "Extend selection left",  category: Navigation },
        KeyBind { key: Named(Named::ArrowRight), mods: Mods::SHIFT, when_ignored: true, action: || Msg::NavigateExtend { dx: 1, dy: 0 },  label: "Extend selection right", category: Navigation },
        KeyBind { key: Named(Named::ArrowUp),    mods: Mods::SHIFT, when_ignored: true, action: || Msg::NavigateExtend { dx: 0, dy: -1 }, label: "Extend selection up",    category: Navigation },
        KeyBind { key: Named(Named::ArrowDown),  mods: Mods::SHIFT, when_ignored: true, action: || Msg::NavigateExtend { dx: 0, dy: 1 },  label: "Extend selection down",  category: Navigation },
        KeyBind { key: Named(Named::Delete),     mods: Mods::NONE, when_ignored: true,  action: || Msg::DeleteKeyPressed,            label: "Delete (or remove from album)", category: Culling },
        KeyBind { key: Named(Named::Backspace),  mods: Mods::NONE, when_ignored: true,  action: || Msg::DeleteKeyPressed,            label: "Delete (or remove from album)", category: Culling },
        KeyBind { key: Named(Named::Escape),     mods: Mods::NONE, when_ignored: false, action: || Msg::EscapePressed,                label: "Cancel / Back",     category: Navigation },

        // View
        KeyBind { key: Named(Named::Space), mods: Mods::NONE, when_ignored: true, action: || Msg::OpenLoupe,       label: "Toggle Loupe",    category: View },
        KeyBind { key: Char("i"),           mods: Mods::NONE, when_ignored: true, action: || Msg::ToggleDetail,     label: "Toggle Info",     category: View },
        KeyBind { key: Char("="),           mods: Mods::CMD,  when_ignored: true, action: || Msg::TileSizeUp,       label: "Zoom In (grid / loupe)",  category: View },
        KeyBind { key: Char("-"),           mods: Mods::CMD,  when_ignored: true, action: || Msg::TileSizeDown,     label: "Zoom Out (grid / loupe)", category: View },
        KeyBind { key: Char("="),           mods: Mods::NONE, when_ignored: true, action: || Msg::TileSizeUp,       label: "Zoom In (grid / loupe)",  category: View },
        KeyBind { key: Char("+"),           mods: Mods::SHIFT, when_ignored: true, action: || Msg::TileSizeUp,      label: "Zoom In (grid / loupe)",  category: View },
        KeyBind { key: Char("-"),           mods: Mods::NONE, when_ignored: true, action: || Msg::TileSizeDown,     label: "Zoom Out (grid / loupe)", category: View },
        KeyBind { key: Char("e"),           mods: Mods::NONE, when_ignored: true, action: || Msg::TogglePreview,      label: "Toggle Preview",  category: View },
        KeyBind { key: Char("z"),           mods: Mods::NONE, when_ignored: true, action: || Msg::LoupeZoomActual,    label: "Loupe 1:1 / Fit", category: View },
        KeyBind { key: Char("f"),           mods: Mods::NONE, when_ignored: true, action: || Msg::ToggleFilterPanel,  label: "Toggle Filters",  category: View },
        KeyBind { key: Char("\\"),          mods: Mods::NONE, when_ignored: true, action: || Msg::ToggleHideRejects,  label: "Toggle Rejects",  category: View },

        // Culling
        KeyBind { key: Char("p"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetFlag(isomfolio_core::models::Flag::Pick),      label: "Flag Pick",      category: Culling },
        KeyBind { key: Char("x"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetFlag(isomfolio_core::models::Flag::Reject),    label: "Flag Reject",    category: Culling },
        KeyBind { key: Char("u"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetFlag(isomfolio_core::models::Flag::Unflagged), label: "Flag Unflagged", category: Culling },
        KeyBind { key: Char("0"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(None),    label: "Clear Rating", category: Culling },
        KeyBind { key: Char("1"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(Some(1)), label: "1 Star",       category: Culling },
        KeyBind { key: Char("2"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(Some(2)), label: "2 Stars",      category: Culling },
        KeyBind { key: Char("3"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(Some(3)), label: "3 Stars",      category: Culling },
        KeyBind { key: Char("4"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(Some(4)), label: "4 Stars",      category: Culling },
        KeyBind { key: Char("5"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetRating(Some(5)), label: "5 Stars",      category: Culling },
        KeyBind { key: Char("6"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetColorLabel(Some("Red".into())),    label: "Label Red",    category: Culling },
        KeyBind { key: Char("7"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetColorLabel(Some("Yellow".into())), label: "Label Yellow", category: Culling },
        KeyBind { key: Char("8"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetColorLabel(Some("Green".into())),  label: "Label Green",  category: Culling },
        KeyBind { key: Char("9"), mods: Mods::NONE, when_ignored: true, action: || Msg::SetColorLabel(Some("Blue".into())),   label: "Label Blue",   category: Culling },

        // Selection
        KeyBind { key: Char("a"), mods: Mods::CMD,       when_ignored: true, action: || Msg::SelectAll,   label: "Select All",   category: Navigation },
        KeyBind { key: Char("a"), mods: Mods::CMD_SHIFT,  when_ignored: true, action: || Msg::DeselectAll, label: "Deselect All", category: Navigation },

        // Undo/Redo
        KeyBind { key: Char("z"), mods: Mods::CMD,       when_ignored: true, action: || Msg::Undo, label: "Undo", category: Tagging },
        KeyBind { key: Char("z"), mods: Mods::CMD_SHIFT,  when_ignored: true, action: || Msg::Redo, label: "Redo", category: Tagging },

        // Compare
        KeyBind { key: Char("c"), mods: Mods::NONE, when_ignored: true, action: || Msg::OpenCompare, label: "Compare (2 selected)", category: View },
        KeyBind { key: Char("r"), mods: Mods::NONE, when_ignored: true, action: || Msg::OpenResolveStacks, label: "Sift (similar shots)", category: View },
        KeyBind { key: Named(Named::Enter), mods: Mods::NONE, when_ignored: true, action: || Msg::ResolveConfirm, label: "Keep & Next (in Sift)", category: View },

        // Tagging
        KeyBind { key: Char("t"), mods: Mods::NONE, when_ignored: true, action: || Msg::FocusTagInput, label: "Add Tag (focus entry)", category: Tagging },
        KeyBind { key: Char("."), mods: Mods::NONE, when_ignored: true, action: || Msg::RepeatLastTag, label: "Repeat Last Tag", category: Tagging },
        KeyBind { key: Char("b"), mods: Mods::NONE, when_ignored: true, action: || Msg::AddSelectionToTargetAlbum, label: "Add to Target Album", category: Tagging },

        // Sync
        KeyBind { key: Char("r"), mods: Mods::CMD, when_ignored: true, action: || Msg::SyncSelectedFolder, label: "Sync Selected Folder", category: Navigation },

        // Help / Settings
        KeyBind { key: Char("?"), mods: Mods::NONE, when_ignored: true, action: || Msg::ToggleShortcutHelp, label: "Shortcut Help", category: View },
        KeyBind { key: Char(","), mods: Mods::CMD,  when_ignored: true, action: || Msg::OpenSettings,        label: "Settings",       category: View },
    ]
}

pub fn format_key(bind: &KeyBind) -> String {
    let mut parts = Vec::new();
    if bind.mods.command { parts.push("Cmd"); }
    if bind.mods.shift { parts.push("Shift"); }
    match bind.key {
        Key::Named(n) => parts.push(match n {
            keyboard::key::Named::ArrowLeft => "←",
            keyboard::key::Named::ArrowRight => "→",
            keyboard::key::Named::ArrowUp => "↑",
            keyboard::key::Named::ArrowDown => "↓",
            keyboard::key::Named::Space => "Space",
            keyboard::key::Named::Escape => "Esc",
            keyboard::key::Named::Enter => "Enter",
            keyboard::key::Named::Delete => "Del",
            keyboard::key::Named::Backspace => "Backspace",
            _ => "?",
        }),
        Key::Char(c) => parts.push(match c {
            "\\" => "\\",
            "=" => "+",
            "-" => "-",
            "." => ".",
            other => other,
        }),
    }
    parts.join("+")
}

pub fn match_event(
    bindings: &[KeyBind],
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
    ignored: bool,
) -> Option<Msg> {
    for bind in bindings {
        if bind.when_ignored && !ignored {
            continue;
        }
        if !bind.mods.matches(modifiers) {
            continue;
        }
        let matched = match (&bind.key, key) {
            (Key::Named(expected), keyboard::Key::Named(actual)) => expected == actual,
            (Key::Char(expected), keyboard::Key::Character(actual)) => actual.as_str() == *expected,
            _ => false,
        };
        if matched {
            return Some((bind.action)());
        }
    }
    None
}
