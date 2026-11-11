//! The global shortcut that toggles the tray, through the XDG GlobalShortcuts portal.

use std::pin::{pin, Pin};
use std::sync::Arc;

use ashpd::desktop::global_shortcuts::{ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut, Shortcut};
use ashpd::desktop::Session;
use ashpd::ActivationToken;
use futures_lite::{future, stream, Stream, StreamExt};

use crate::portal;

pub const SHORTCUT_ID: &str = "toggle-tray";
pub const SHORTCUT_DESCRIPTION: &str = "Show or hide Insert";
pub const PREFERRED_TRIGGER: &str = "CTRL+SHIFT+V";
/// `ConfigureShortcuts` exists from this version of the portal interface.
const CONFIGURE_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEvent {
    /// The user pressed the shortcut. Pass the token to the window that the app presents.
    Activated { activation_token: Option<String> },
    /// The trigger in the form that the desktop shows to the user. Empty when no key is assigned.
    TriggerChanged(String),
    /// The portal closed the session. No more events follow.
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    #[error("the global shortcut is not available: {0}")]
    Unavailable(String),
    #[error("this desktop cannot open its shortcut settings for the app")]
    ConfigureNotSupported,
}

/// A bound shortcut. Drop it to release the shortcut.
pub struct GlobalShortcut {
    portal: GlobalShortcuts,
    session: Arc<Session<GlobalShortcuts>>,
    stop: async_channel::Sender<()>,
}

impl GlobalShortcut {
    /// Binds the shortcut and starts a listener thread that sends to `events`.
    /// The first event is `TriggerChanged` with the trigger that the desktop assigned.
    /// The desktop can show a dialog the first time.
    pub async fn connect(events: async_channel::Sender<ShortcutEvent>) -> Result<Self, ShortcutError> {
        portal::register_app().await;
        let portal = GlobalShortcuts::new().await.map_err(unavailable)?;
        let session = Arc::new(portal.create_session(Default::default()).await.map_err(unavailable)?);

        let activated = portal.receive_activated().await.map_err(unavailable)?;
        let changed = portal.receive_shortcuts_changed().await.map_err(unavailable)?;

        let shortcut = NewShortcut::new(SHORTCUT_ID, SHORTCUT_DESCRIPTION).preferred_trigger(PREFERRED_TRIGGER);
        let bound = portal
            .bind_shortcuts(&session, &[shortcut], None, Default::default())
            .await
            .and_then(|request| request.response())
            .map_err(unavailable)?;
        let _ = events.send(ShortcutEvent::TriggerChanged(trigger_of(bound.shortcuts()))).await;

        let signals: Signals = Box::pin(stream::or(
            activated.filter(|signal| signal.shortcut_id() == SHORTCUT_ID).map(|signal| ShortcutEvent::Activated {
                activation_token: signal
                    .options()
                    .get("activation_token")
                    .and_then(|value| value.downcast_ref::<String>().ok()),
            }),
            changed.map(|signal| ShortcutEvent::TriggerChanged(trigger_of(signal.shortcuts()))),
        ));
        let (stop, stopped) = async_channel::bounded::<()>(1);
        std::thread::Builder::new()
            .name("insert-shortcut".into())
            .spawn({
                let session = Arc::clone(&session);
                move || future::block_on(listen(signals, session, stopped, events))
            })
            .map_err(|error| ShortcutError::Unavailable(error.to_string()))?;
        Ok(Self { portal, session, stop })
    }

    /// True when `configure` can open the shortcut settings of the desktop.
    pub fn can_configure(&self) -> bool {
        self.portal.version() >= CONFIGURE_VERSION
    }

    /// Opens the desktop's dialog to change the trigger. The new trigger arrives as `TriggerChanged`.
    pub async fn configure(&self, activation_token: Option<String>) -> Result<(), ShortcutError> {
        if !self.can_configure() {
            return Err(ShortcutError::ConfigureNotSupported);
        }
        let options =
            ConfigureShortcutsOptions::default().set_activation_token(activation_token.map(ActivationToken::from));
        self.portal.configure_shortcuts(&self.session, None, options).await.map_err(unavailable)
    }

}

impl Drop for GlobalShortcut {
    fn drop(&mut self) {
        self.stop.close();
    }
}

type Signals = Pin<Box<dyn Stream<Item = ShortcutEvent> + Send>>;

async fn listen(
    signals: Signals,
    session: Arc<Session<GlobalShortcuts>>,
    stopped: async_channel::Receiver<()>,
    events: async_channel::Sender<ShortcutEvent>,
) {
    let closed = match session.receive_closed().await {
        Ok(closed) => closed,
        Err(error) => {
            let _ = events.send(ShortcutEvent::Unavailable(error.to_string())).await;
            return;
        }
    };
    let closed = closed.map(|_| ShortcutEvent::Unavailable("the desktop closed the shortcut session".into()));
    let mut incoming = pin!(stream::or(signals, closed));
    loop {
        let next = future::or(incoming.next(), async {
            let _ = stopped.recv().await;
            None
        })
        .await;
        let Some(event) = next else { break };
        let last = matches!(event, ShortcutEvent::Unavailable(_));
        if events.send(event).await.is_err() || last {
            break;
        }
    }
    let _ = session.close().await;
}

fn trigger_of(shortcuts: &[Shortcut]) -> String {
    shortcuts
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID)
        .map(|shortcut| shortcut.trigger_description().to_owned())
        .unwrap_or_default()
}

fn unavailable(error: ashpd::Error) -> ShortcutError {
    ShortcutError::Unavailable(error.to_string())
}
