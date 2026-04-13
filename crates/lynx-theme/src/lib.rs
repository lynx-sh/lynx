pub mod color;
pub mod colors;
pub mod loader;
pub mod ls_colors;
pub mod patch;
pub mod schema;
pub mod segments;
pub mod terminal;

pub use loader::{list, load, load_from_path, parse_and_validate};
pub use schema::{
    SegmentColor, SegmentLayout, SegmentOrder, SegmentVisibility, StatusIcon, Theme, ThemeMeta,
    KNOWN_SEGMENTS,
};
