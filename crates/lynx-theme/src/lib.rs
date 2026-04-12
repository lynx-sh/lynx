pub mod color;
pub mod loader;
pub mod patch;
pub mod schema;
pub mod terminal;

pub use loader::{list, load, load_from_path, parse_and_validate};
pub use schema::{
    SegmentColor, SegmentLayout, SegmentOrder, SegmentVisibility, StatusIcon, Theme, ThemeMeta,
    KNOWN_SEGMENTS,
};
