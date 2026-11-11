//! Domain logic for Insert on Linux. No GTK, no D-Bus, no Wayland.

pub mod capture;
pub mod format;
pub mod mime;
pub mod model;

pub use capture::{classify, Capture, Skip, MAX_CAPTURE_BYTES, THUMBNAIL_MAX_SIDE};
pub use mime::{select_targets, MimeMap, UTF8_TEXT};
pub use model::{
    Clip, ClipContent, ClipId, ClipKind, ColorValue, ContentHash, PixelSize, Pinboard, PinboardColor, PinboardId, Rgb,
    SourceApp, TextSnippet,
};
