use std::pin::{pin, Pin};
use std::sync::Arc;

use ashpd::desktop::global_shortcuts::{ConfigureShortcutsOptions, GlobalShortcuts, NewShortcut, Shortcut};
use ashpd::desktop::Session;
use futures_lite::{future, stream, Stream, StreamExt};

use crate::portal;

/// The shortcuts that Insert binds, all in one portal session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutId {
    ToggleTray,
    /// Hold to open the clip ring. Release to paste the selected clip.
    ClipRing,
}

impl ShortcutId {
    pub const ALL: [Self; 2] = [Self::ToggleTray, Self::ClipRing];

    pub fn id(self) -> &'static str {
        match self {
            Self::ToggleTray => "toggle-tray",
            Self::ClipRing => "clip-ring",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ToggleTray => "Show or hide Insert",
            Self::ClipRing => "Hold to open the clip ring",
        }
    }

    pub fn preferred_trigger(self) -> &'static str {
        match self {
            Self::ToggleTray => "CTRL+SHIFT+V",
            Self::ClipRing => "CTRL+ALT+V",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shortcut| shortcut.id() == id)
    }
}

/// `ConfigureShortcuts` exists from this version of the portal interface.
const CONFIGURE_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEvent {
    /// The user pressed the shortcut. Pass the token to the window that the app presents.
    /// Key autorepeat can send this again while the user holds the keys.
    Activated { id: ShortcutId, activation_token: Option<String> },
    /// The user released the shortcut.
    Deactivated(ShortcutId),
    /// The trigger in the form that the desktop shows to the user. Empty when no key is assigned.
    TriggerChanged { id: ShortcutId, trigger: String },
    /// The portal closed the session. No more events follow.
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    #[error("the global shortcuts are not available: {0}")]
    Unavailable(String),
    #[error("this desktop cannot open its shortcut settings for the app")]
    ConfigureNotSupported,
}

/// The bound shortcuts. Drop it to release them.
pub struct GlobalShortcut {
    portal: GlobalShortcuts,
    session: Arc<Session<GlobalShortcuts>>,
    stop: async_channel::Sender<()>,
}

impl GlobalShortcut {
    /// Binds all shortcuts of `ShortcutId::ALL` and starts a listener thread that sends to `events`.
    /// The first events are one `TriggerChanged` for each shortcut, with the trigger that the desktop
    /// assigned. The desktop can show a dialog the first time.
    pub async fn connect(events: async_channel::Sender<ShortcutEvent>) -> Result<Self, ShortcutError> {
        portal::register_app().await;
        let portal = GlobalShortcuts::new().await.map_err(unavailable)?;
        let session = Arc::new(portal.create_session(Default::default()).await.map_err(unavailable)?);

        let activated = portal.receive_activated().await.map_err(unavailable)?;
        let deactivated = portal.receive_deactivated().await.map_err(unavailable)?;
        let changed = portal.receive_shortcuts_changed().await.map_err(unavailable)?;

        let shortcuts = ShortcutId::ALL
            .map(|id| NewShortcut::new(id.id(), id.description()).preferred_trigger(id.preferred_trigger()));
        let bound = portal
            .bind_shortcuts(&session, &shortcuts, None, Default::default())
            .await
            .and_then(|request| request.response())
            .map_err(unavailable)?;
        for event in triggers_of(bound.shortcuts()) {
            let _ = events.send(event).await;
        }

        let activated = activated.filter_map(|signal| {
            Some(ShortcutEvent::Activated {
                id: ShortcutId::from_id(signal.shortcut_id())?,
                activation_token: signal
                    .options()
                    .get("activation_token")
                    .and_then(|value| value.downcast_ref::<String>().ok()),
            })
        });
        let deactivated = deactivated
            .filter_map(|signal| ShortcutId::from_id(signal.shortcut_id()).map(ShortcutEvent::Deactivated));
        let changed = changed.flat_map(|signal| stream::iter(triggers_of(signal.shortcuts())));
        let signals: Signals = Box::pin(stream::or(stream::or(activated, deactivated), changed));
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

    pub fn can_configure(&self) -> bool {
        self.portal.version() >= CONFIGURE_VERSION
    }

    /// Opens the desktop's dialog to change the triggers. The new triggers arrive as `TriggerChanged`.
    pub async fn configure(&self) -> Result<(), ShortcutError> {
        if !self.can_configure() {
            return Err(ShortcutError::ConfigureNotSupported);
        }
        self.portal
            .configure_shortcuts(&self.session, None, ConfigureShortcutsOptions::default())
            .await
            .map_err(unavailable)
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

/// One event for each shortcut of Insert. A shortcut that `shortcuts` does not list has no trigger.
fn triggers_of(shortcuts: &[Shortcut]) -> Vec<ShortcutEvent> {
    ShortcutId::ALL
        .into_iter()
        .map(|id| ShortcutEvent::TriggerChanged {
            id,
            trigger: shortcuts
                .iter()
                .find(|shortcut| shortcut.id() == id.id())
                .map(|shortcut| shortcut.trigger_description().to_owned())
                .unwrap_or_default(),
        })
        .collect()
}

fn unavailable(error: ashpd::Error) -> ShortcutError {
    ShortcutError::Unavailable(error.to_string())
}
