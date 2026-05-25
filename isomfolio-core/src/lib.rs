pub mod addon;
pub mod app_paths;
pub mod catalog;
pub mod file_index;
pub mod indexing;
pub mod metadata;
pub mod models;
pub mod path_utils;
pub mod search;
pub mod storage;

pub use catalog::Catalog;
pub use models::*;
pub use app_paths::*;
pub use rusqlite::Connection;
