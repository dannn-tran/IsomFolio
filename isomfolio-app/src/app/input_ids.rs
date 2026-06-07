//! Stable `text_input::Id`s for fields that should grab focus the moment they
//! appear. The view tags the input with `.id(...)` and the message handler that
//! reveals it returns `text_input::focus(...)` with the matching id, so opening
//! a rename/create field puts the cursor in it without a manual click.

use iced::advanced::widget::Id;

macro_rules! input_id {
    ($name:ident, $key:literal) => {
        pub fn $name() -> Id {
            Id::new($key)
        }
    };
}

input_id!(create_album, "input-create-album");
input_id!(rename_album, "input-rename-album");
input_id!(create_shelf, "input-create-shelf");
input_id!(rename_shelf, "input-rename-shelf");
input_id!(rename_face, "input-rename-face");
input_id!(rename_tag, "input-rename-tag");
input_id!(new_catalog, "input-new-catalog");
input_id!(save_smart_album, "input-save-smart-album");
