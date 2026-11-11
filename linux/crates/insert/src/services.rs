//! The one boundary between the UI and the desktop. The UI asks for desktop work only through
//! `Services` and hears about desktop events only through `ServiceEvent`.

use gtk4 as gtk;
use gtk::{gdk, glib, prelude::*};
use insert_core::{MimeMap, SourceApp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteStatus {
    /// Insert can send Ctrl+V to the app in front.
    Ready,
    /// The desktop can send keys, but the user did not allow it yet.
    NeedsPermission,
    /// The desktop has no way for Insert to send keys.
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// The desktop shortcut service knows the "toggle tray" shortcut. `trigger` is its display
    /// text, for example "Ctrl+Shift+V", or `None` when no key is bound.
    Registered {
        trigger: Option<String>,
        /// The desktop can show its own dialog to change the shortcut.
        can_change: bool,
    },
    /// No global shortcut service. The user binds `insert --toggle` in the desktop settings.
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusMenuCommand {
    ShowTray,
    TogglePause,
    ClearHistory,
    OpenSettings,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusIcon {
    pub visible: bool,
    pub paused: bool,
}

#[derive(Clone, Debug)]
pub enum ServiceEvent {
    /// The clipboard has new content from another app. The platform never reports content that
    /// Insert wrote with `set_clipboard`. `concealed` is true when the offer carried a password
    /// manager marker.
    ClipboardChanged { offer: MimeMap, concealed: bool },
    /// The global shortcut fired. Present the tray with the token so the compositor focuses it.
    ShortcutActivated { activation_token: Option<String> },
    ShortcutChanged(Shortcut),
    PasteStatusChanged(PasteStatus),
    StatusMenu(StatusMenuCommand),
}

pub struct Services {
    clipboard: gdk::Clipboard,
    /// Kept so that the event channel stays open while no platform source sends.
    _events: async_channel::Sender<ServiceEvent>,
}

impl Services {
    pub fn start(display: &gdk::Display) -> (Self, async_channel::Receiver<ServiceEvent>) {
        let (sender, receiver) = async_channel::unbounded();
        let services = Self {
            clipboard: display.clipboard(),
            _events: sender,
        };
        (services, receiver)
    }

    pub fn set_clipboard(&self, offer: MimeMap) {
        let providers: Vec<gdk::ContentProvider> = offer
            .0
            .into_iter()
            .map(|(mime, data)| gdk::ContentProvider::for_bytes(&mime, &glib::Bytes::from_owned(data)))
            .collect();
        self.clipboard
            .set_content(Some(&gdk::ContentProvider::new_union(&providers)))
            .unwrap_or_else(|error| eprintln!("insert: cannot set the clipboard: {error}"));
    }

    pub fn send_paste(&self) {
        eprintln!("insert: send Ctrl+V (no key injection in this build)");
    }

    pub fn paste_status(&self) -> PasteStatus {
        PasteStatus::Unavailable
    }

    /// Starts the desktop permission flow. The result arrives as `PasteStatusChanged`.
    pub fn request_paste_permission(&self) {}

    /// The app in front, for the clip source and the ignored apps.
    pub fn source_app(&self) -> Option<SourceApp> {
        None
    }

    /// False when the desktop does not tell Insert which app is in front.
    pub fn tracks_source_app(&self) -> bool {
        false
    }

    pub fn launch_at_login(&self) -> bool {
        false
    }

    /// The error text is shown to the user.
    pub fn set_launch_at_login(&self, _enabled: bool) -> Result<(), String> {
        Err("Launch at login is not available in this build.".to_owned())
    }

    pub fn shortcut(&self) -> Shortcut {
        Shortcut::Unavailable
    }

    /// Opens the desktop dialog to change the shortcut. The result arrives as `ShortcutChanged`.
    pub fn change_shortcut(&self) {}

    pub fn set_status_icon(&self, _icon: StatusIcon) {}
}
