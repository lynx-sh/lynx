pub mod color;
pub mod loader;
pub mod schema;
pub mod terminal;

pub use loader::{builtin_content, list, load, load_from_path, parse_and_validate};
pub use schema::{
    SegmentColor, SegmentConfig, SegmentLayout, SegmentOrder, StatusIcon, Theme, ThemeMeta,
    KNOWN_SEGMENTS,
};
