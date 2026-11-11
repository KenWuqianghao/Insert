//! The StatusNotifierItem icon and its menu.

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};

use crate::APP_ID;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowTray,
    TogglePause,
    ClearHistory,
    OpenSettings,
    Quit,
}

#[derive(Debug, thiserror::Error)]
pub enum TrayIconError {
    #[error("the system tray icon is not available: {0}")]
    Unavailable(String),
}

/// The icon is visible while this value is alive.
pub struct TrayIcon {
    handle: Handle<InsertTray>,
}

impl TrayIcon {
    /// Shows the icon. When no system tray is running, the icon appears as soon as one starts.
    pub fn spawn(events: async_channel::Sender<TrayEvent>, paused: bool) -> Result<Self, TrayIconError> {
        InsertTray { events, paused }
            .assume_sni_available(true)
            .spawn()
            .map(|handle| Self { handle })
            .map_err(|error| TrayIconError::Unavailable(error.to_string()))
    }

    /// Changes the icon and the label of the pause item.
    pub fn set_paused(&self, paused: bool) {
        self.handle.update(|tray| tray.paused = paused);
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.handle.shutdown().wait();
    }
}

struct InsertTray {
    events: async_channel::Sender<TrayEvent>,
    paused: bool,
}

impl InsertTray {
    fn item(label: &str, event: TrayEvent) -> MenuItem<Self> {
        StandardItem {
            label: label.into(),
            activate: Box::new(move |tray: &mut Self| {
                let _ = tray.events.try_send(event);
            }),
            ..Default::default()
        }
        .into()
    }
}

impl ksni::Tray for InsertTray {
    // The macOS menu bar item opens its menu on a click.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        APP_ID.into()
    }

    fn title(&self) -> String {
        if self.paused { "Insert, capture is paused" } else { "Insert" }.into()
    }

    fn icon_name(&self) -> String {
        if self.paused { "media-playback-pause-symbolic" } else { "edit-paste-symbolic" }.into()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            Self::item("Show Insert", TrayEvent::ShowTray),
            Self::item(if self.paused { "Resume Capture" } else { "Pause Capture" }, TrayEvent::TogglePause),
            Self::item("Clear History…", TrayEvent::ClearHistory),
            MenuItem::Separator,
            Self::item("Settings…", TrayEvent::OpenSettings),
            MenuItem::Separator,
            Self::item("Quit Insert", TrayEvent::Quit),
        ]
    }
}
