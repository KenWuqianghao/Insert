pub mod capture;
pub mod format;
pub mod keys;
pub mod library;
pub mod mime;
pub mod model;
pub mod prefs;
pub mod selection;
pub mod storage;
pub mod tint;

pub use capture::{classify, Capture, Skip, MAX_CAPTURE_BYTES, THUMBNAIL_MAX_SIDE};
pub use keys::{
    cancel_effect, command_for_key, CancelEffect, Key, KeyContext, Modifiers, PanelCommand,
    PasteAction, TrayMode,
};
pub use library::{
    empty_state, Change, EmptyState, Library, NameCommit, NameTarget, Opened, Scope,
};
pub use mime::{
    is_secret_hint, select_targets, MimeMap, KDE_PASSWORD_HINT, MAX_READ_BYTES, UTF8_TEXT,
};
pub use model::{
    app_id_key, Clip, ClipContent, ClipId, ClipKind, ColorValue, ContentHash, Pinboard, PinboardColor,
    PinboardId, PixelSize, Rgb, SourceApp, TextSnippet,
};
pub use prefs::{HistoryLimit, Preferences};
pub use selection::Selection;
pub use storage::{LibraryIndex, LoadedIndex, Paths, Storage};
pub use tint::{header_tint, NEUTRAL_TINT};
