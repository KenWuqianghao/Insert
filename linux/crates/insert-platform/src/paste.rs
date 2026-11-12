use std::fs;
use std::io;
use std::path::Path;

use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{PersistMode, ResponseError, Session};
use insert_core::storage::write_private_atomic;

use crate::active_app::ActiveApp;
use crate::portal;

const KEYSYM_CONTROL_L: i32 = 0xffe3;
const KEYSYM_CONTROL_R: i32 = 0xffe4;
const KEYSYM_SHIFT_L: i32 = 0xffe1;
const KEYSYM_SHIFT_R: i32 = 0xffe2;
const KEYSYM_V: i32 = 0x0076;
const KEYSYM_INSERT: i32 = 0xff63;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKeys {
    CtrlV,
    /// For a terminal. Ctrl+Shift+V is not usable: it is the default shortcut of Insert, and the desktop
    /// takes a global shortcut before the terminal gets the key. Some terminals paste the primary selection
    /// on Shift+Insert, so the caller must put the content there too.
    ShiftInsert,
}

impl PasteKeys {
    pub fn for_app(app: Option<&ActiveApp>) -> Self {
        match app {
            Some(app) if app.is_terminal => Self::ShiftInsert,
            _ => Self::CtrlV,
        }
    }

    fn keysyms(self) -> (i32, i32) {
        match self {
            Self::CtrlV => (KEYSYM_CONTROL_L, KEYSYM_V),
            Self::ShiftInsert => (KEYSYM_SHIFT_L, KEYSYM_INSERT),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasteError {
    #[error("the user did not allow input injection")]
    NeedsPermission,
    #[error("input injection is not available: {0}")]
    Unavailable(String),
}

pub struct PasteInjector {
    portal: RemoteDesktop,
    session: Session<RemoteDesktop>,
}

impl PasteInjector {
    /// Opens the session. The portal shows its permission dialog when the restore token in `token_file` is
    /// missing or no longer valid. With `interactive` false and no stored token, this returns `NeedsPermission`
    /// and shows no dialog.
    pub async fn connect(interactive: bool, token_file: &Path) -> Result<Self, PasteError> {
        let token = fs::read_to_string(token_file).ok();
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
        let started = portal.start(&session, None, Default::default()).await.and_then(|request| request.response());
        // A refused token must go. Else the dialog comes back at each start.
        let started = match started {
            Ok(started) if started.devices().contains(DeviceType::KEYBOARD) => started,
            Ok(_) => {
                let _ = session.close().await;
                store_token(token_file, None);
                return Err(PasteError::NeedsPermission);
            }
            Err(ashpd::Error::Response(ResponseError::Cancelled)) => {
                store_token(token_file, None);
                return Err(PasteError::NeedsPermission);
            }
            Err(other) => return Err(unavailable(other)),
        };
        // A restore token is valid for one session only. The portal gives the next one here.
        store_token(token_file, started.restore_token());
        Ok(Self { portal, session })
    }

    pub async fn send_paste(&self, keys: PasteKeys) -> Result<(), PasteError> {
        let (modifier, key) = keys.keysyms();
        // The user can still hold Shift from Shift+Return. With it, Ctrl+V becomes Ctrl+Shift+V, the shortcut
        // that opens Insert again. The user can also still hold Ctrl from Ctrl+1, and Ctrl+Shift+Insert is not
        // a paste in a terminal.
        let release = [KEYSYM_SHIFT_L, KEYSYM_SHIFT_R, KEYSYM_CONTROL_L, KEYSYM_CONTROL_R]
            .map(|keysym| (keysym, KeyState::Released));
        let paste = [
            (modifier, KeyState::Pressed),
            (key, KeyState::Pressed),
            (key, KeyState::Released),
            (modifier, KeyState::Released),
        ];
        for (keysym, state) in release.into_iter().chain(paste) {
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

fn store_token(path: &Path, token: Option<&str>) {
    let stored = match token {
        Some(token) => write_private_atomic(path, token.as_bytes()),
        None => remove_if_present(path),
    };
    if let Err(error) = stored {
        log::warn!("cannot store the portal restore token at {}: {error}", path.display());
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
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
        let app = |is_terminal| ActiveApp {
            source: insert_core::SourceApp { id: "x".into(), name: "x".into() },
            is_terminal,
        };
        assert_eq!(PasteKeys::for_app(Some(&app(true))), PasteKeys::ShiftInsert);
        assert_eq!(PasteKeys::for_app(Some(&app(false))), PasteKeys::CtrlV);
        assert_eq!(PasteKeys::for_app(None), PasteKeys::CtrlV);
    }
}
