//! Key injection for "paste directly" through the XDG RemoteDesktop portal.

use std::fs;
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{PersistMode, ResponseError, Session};
use insert_core::app_id_key;

use crate::portal;

const KEYSYM_CONTROL_L: i32 = 0xffe3;
const KEYSYM_SHIFT_L: i32 = 0xffe1;
const KEYSYM_SHIFT_R: i32 = 0xffe2;
const KEYSYM_V: i32 = 0x0076;
const KEYSYM_INSERT: i32 = 0xff63;

/// Terminals use Ctrl+V for a control character.
const TERMINAL_APP_IDS: &[&str] = &[
    "konsole",
    "org.kde.konsole",
    "yakuake",
    "org.kde.yakuake",
    "gnome-terminal",
    "gnome-terminal-server",
    "org.gnome.terminal",
    "org.gnome.console",
    "kgx",
    "org.gnome.ptyxis",
    "kitty",
    "alacritty",
    "foot",
    "footclient",
    "wezterm",
    "org.wezfurlong.wezterm",
    "ghostty",
    "com.mitchellh.ghostty",
    "xterm",
    "tilix",
    "com.gexperts.tilix",
    "terminator",
];

/// The keys that paste in the app that has the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKeys {
    CtrlV,
    /// For a terminal. Ctrl+Shift+V is not usable: it is the default shortcut of Insert, and the desktop
    /// takes a global shortcut before the terminal gets the key. Some terminals paste the primary selection
    /// on Shift+Insert, so the caller must put the content there too.
    ShiftInsert,
}

impl PasteKeys {
    /// `app_id` is the desktop file id or the window class of the app.
    pub fn for_app(app_id: Option<&str>) -> Self {
        match app_id {
            Some(id) if TERMINAL_APP_IDS.contains(&app_id_key(id).as_str()) => Self::ShiftInsert,
            _ => Self::CtrlV,
        }
    }

    /// The modifier key, then the key.
    fn keysyms(self) -> (i32, i32) {
        match self {
            Self::CtrlV => (KEYSYM_CONTROL_L, KEYSYM_V),
            Self::ShiftInsert => (KEYSYM_SHIFT_L, KEYSYM_INSERT),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteStatus {
    Ready,
    /// The user must allow input injection. Call `PasteInjector::connect(true)` to show the dialog.
    NeedsPermission,
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasteError {
    #[error("the user did not allow input injection")]
    NeedsPermission,
    #[error("input injection is not available: {0}")]
    Unavailable(String),
}

impl PasteStatus {
    pub fn of(result: &Result<PasteInjector, PasteError>) -> Self {
        match result {
            Ok(_) => Self::Ready,
            Err(PasteError::NeedsPermission) => Self::NeedsPermission,
            Err(PasteError::Unavailable(reason)) => Self::Unavailable(reason.clone()),
        }
    }
}

/// An open RemoteDesktop session with a keyboard device.
pub struct PasteInjector {
    portal: RemoteDesktop,
    session: Session<RemoteDesktop>,
}

impl PasteInjector {
    /// Opens the session. The portal shows its permission dialog when the stored restore token is missing or
    /// no longer valid. With `interactive` false and no stored token, this returns `NeedsPermission` and
    /// shows no dialog.
    pub async fn connect(interactive: bool) -> Result<Self, PasteError> {
        let token_file = restore_token_file();
        let token = token_file.as_ref().and_then(|path| fs::read_to_string(path).ok());
        let token = token.as_deref().map(str::trim).filter(|token| !token.is_empty());
        if !interactive && token.is_none() {
            return Err(PasteError::NeedsPermission);
        }

        portal::register_app().await;
        let portal = RemoteDesktop::new().await.map_err(unavailable)?;
        let session = portal.create_session(Default::default()).await.map_err(unavailable)?;
        let devices = SelectDevicesOptions::default()
            .set_devices(DeviceType::KEYBOARD)
            .set_persist_mode(PersistMode::ExplicitlyRevoked)
            .set_restore_token(token);
        portal
            .select_devices(&session, devices)
            .await
            .and_then(|request| request.response())
            .map_err(unavailable)?;
        let started = portal
            .start(&session, None, Default::default())
            .await
            .and_then(|request| request.response())
            .map_err(|error| match error {
                ashpd::Error::Response(ResponseError::Cancelled) => PasteError::NeedsPermission,
                other => unavailable(other),
            })?;
        if !started.devices().contains(DeviceType::KEYBOARD) {
            let _ = session.close().await;
            return Err(PasteError::NeedsPermission);
        }

        // A restore token is valid for one session only. The portal gives the next one here.
        if let Some(path) = token_file {
            let stored = match started.restore_token() {
                Some(token) => write_private(&path, token),
                None => remove_if_present(&path),
            };
            if let Err(error) = stored {
                log::warn!("cannot store the portal restore token at {}: {error}", path.display());
            }
        }
        Ok(Self { portal, session })
    }

    /// Presses `keys` in the focused window.
    pub async fn send_paste(&self, keys: PasteKeys) -> Result<(), PasteError> {
        let (modifier, key) = keys.keysyms();
        // The user can still hold Shift from Shift+Return. With it, Ctrl+V becomes Ctrl+Shift+V, the shortcut
        // that opens Insert again.
        let shift_release = [(KEYSYM_SHIFT_L, KeyState::Released), (KEYSYM_SHIFT_R, KeyState::Released)];
        let paste = [
            (modifier, KeyState::Pressed),
            (key, KeyState::Pressed),
            (key, KeyState::Released),
            (modifier, KeyState::Released),
        ];
        for (keysym, state) in shift_release.into_iter().chain(paste) {
            self.portal
                .notify_keyboard_keysym(&self.session, keysym, state, Default::default())
                .await
                .map_err(unavailable)?;
        }
        Ok(())
    }

    pub async fn close(self) {
        let _ = self.session.close().await;
    }
}

fn unavailable(error: ashpd::Error) -> PasteError {
    PasteError::Unavailable(error.to_string())
}

fn restore_token_file() -> Option<PathBuf> {
    let state_home = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state_home.join("insert").join("remote-desktop-token"))
}

fn write_private(path: &std::path::Path, content: &str) -> io::Result<()> {
    use std::io::Write as _;
    fs::create_dir_all(path.parent().expect("the token file has a parent"))?;
    let temp = path.with_extension("tmp");
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp)?;
    file.write_all(content.as_bytes())?;
    fs::rename(&temp, path)
}

fn remove_if_present(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_gets_shift_insert() {
        assert_eq!(PasteKeys::for_app(Some("org.kde.konsole")), PasteKeys::ShiftInsert);
        assert_eq!(PasteKeys::for_app(Some("Alacritty")), PasteKeys::ShiftInsert);
        assert_eq!(PasteKeys::for_app(Some("org.kde.kwrite")), PasteKeys::CtrlV);
        assert_eq!(PasteKeys::for_app(None), PasteKeys::CtrlV);
    }
}
