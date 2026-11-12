//! The one boundary between the UI and the desktop. The UI asks for desktop work only through
//! `Services` and hears about desktop events only through `ServiceEvent`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4 as gtk;
use gtk::{gdk, glib, prelude::*};
use insert_core::{select_targets, MimeMap, SourceApp};
use insert_platform::active_app::ActiveAppTracker;
use insert_platform::autostart::{self, Autostart};
use insert_platform::clipboard::{ClipboardConfig, ClipboardEvent, ClipboardWatcher};
use insert_platform::paste::{PasteError, PasteInjector, PasteKeys};
use insert_platform::shortcut::{GlobalShortcut, ShortcutEvent};
use insert_platform::tray_icon::{TrayEvent, TrayIcon};

use crate::tray::WINDOW_TITLE;

/// Without a signal from the desktop, the time for the focus to go back to the target app.
const FOCUS_DELAY: Duration = Duration::from_millis(150);
/// The longest wait for the desktop to say that the target app has the focus.
const FOCUS_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteStatus {
    /// Insert can send the paste keys to the app in front.
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
    /// Insert wrote with `set_clipboard`. The offer can have a password manager marker, which
    /// `classify` checks.
    ClipboardChanged { offer: MimeMap },
    /// The global shortcut fired. Present the tray with the token so the compositor focuses it.
    ShortcutActivated { activation_token: Option<String> },
    ShortcutChanged(Shortcut),
    PasteStatusChanged(PasteStatus),
    /// This desktop does not let Insert watch the clipboard. No `ClipboardChanged` follows.
    CaptureUnavailable,
    StatusMenu(StatusMenuCommand),
}

pub struct Services {
    inner: Rc<Inner>,
}

struct Inner {
    events: async_channel::Sender<ServiceEvent>,
    /// `None` when the desktop has no data-control protocol. Writes then go through GDK.
    watcher: RefCell<Option<ClipboardWatcher>>,
    gdk_clipboard: gdk::Clipboard,
    injector: RefCell<Option<Rc<PasteInjector>>>,
    paste_status: Cell<PasteStatus>,
    global_shortcut: RefCell<Option<Rc<GlobalShortcut>>>,
    shortcut: RefCell<Shortcut>,
    active_app: RefCell<Option<Rc<ActiveAppTracker>>>,
    autostart: Option<Autostart>,
    status_icon: RefCell<Option<TrayIcon>>,
    status_menu: async_channel::Sender<TrayEvent>,
}

impl Services {
    pub fn start(display: &gdk::Display) -> (Self, async_channel::Receiver<ServiceEvent>) {
        let (events, receiver) = async_channel::unbounded();
        let (clipboard_events, clipboard_changes) = async_channel::unbounded();
        let (status_menu, status_menu_events) = async_channel::unbounded();
        let config = ClipboardConfig {
            select_mimes: Box::new(select_targets),
            ..Default::default()
        };
        let watcher = ClipboardWatcher::start(config, clipboard_events)
            .inspect_err(|error| eprintln!("insert: cannot start the clipboard watcher: {error}"))
            .ok();
        let inner = Rc::new(Inner {
            events,
            watcher: RefCell::new(watcher),
            gdk_clipboard: display.clipboard(),
            injector: RefCell::new(None),
            paste_status: Cell::new(PasteStatus::NeedsPermission),
            global_shortcut: RefCell::new(None),
            shortcut: RefCell::new(Shortcut::Unavailable),
            active_app: RefCell::new(None),
            autostart: Autostart::for_user(),
            status_icon: RefCell::new(None),
            status_menu,
        });
        if inner.watcher.borrow().is_none() {
            inner.emit(ServiceEvent::CaptureUnavailable);
        }

        glib::spawn_future_local(Rc::clone(&inner).forward_clipboard(clipboard_changes));
        glib::spawn_future_local(Rc::clone(&inner).forward_status_menu(status_menu_events));
        glib::spawn_future_local(Rc::clone(&inner).track_active_app());
        // The portals take the app id from the first call on the connection, so the two portal
        // users start one after the other.
        glib::spawn_future_local({
            let inner = Rc::clone(&inner);
            async move {
                inner.connect_paste(false).await;
                inner.run_shortcut().await;
            }
        });
        (Self { inner }, receiver)
    }

    /// Unloads what Insert left in the desktop. Call it once, before the app exits.
    pub fn stop(&self) {
        self.inner.status_icon.borrow_mut().take();
        self.inner.global_shortcut.borrow_mut().take();
        if let Some(tracker) = self.inner.active_app.borrow_mut().take() {
            futures_lite::future::block_on(tracker.stop());
        }
    }

    /// The error text is for the log.
    pub fn set_clipboard(&self, offer: MimeMap) -> Result<(), String> {
        if let Some(watcher) = &*self.inner.watcher.borrow() {
            return watcher.set_clipboard(offer.0).map_err(|error| error.to_string());
        }
        let providers: Vec<gdk::ContentProvider> = offer
            .0
            .into_iter()
            .map(|(mime, data)| gdk::ContentProvider::for_bytes(&mime, &glib::Bytes::from_owned(data)))
            .collect();
        self.inner
            .gdk_clipboard
            .set_content(Some(&gdk::ContentProvider::new_union(&providers)))
            .map_err(|error| error.to_string())
    }

    /// Sends the paste keys to the app that gets the focus when the tray is gone. Call it after
    /// the tray window hides. A terminal gets Shift+Insert, each other app Ctrl+V. If the desktop
    /// reports no other app in front within `FOCUS_TIMEOUT`, or the primary selection for a
    /// terminal cannot be set, no key is sent.
    pub fn paste_into_front_app(&self) {
        let Some(injector) = self.inner.injector.borrow().clone() else {
            return;
        };
        let tracker = self.inner.active_app.borrow().clone();
        let inner = Rc::clone(&self.inner);
        glib::spawn_future_local(async move {
            let target = match &tracker {
                Some(tracker) => {
                    let focus = tracker.external_window_active();
                    if glib::future_with_timeout(FOCUS_TIMEOUT, focus).await.is_err() {
                        eprintln!("insert: the target app did not get the focus in time; the clip is only copied");
                        return;
                    }
                    tracker.current()
                }
                None => {
                    glib::timeout_future(FOCUS_DELAY).await;
                    None
                }
            };
            let keys = PasteKeys::for_app(target.as_ref());
            // A terminal that pastes the primary selection would paste old content.
            if keys == PasteKeys::ShiftInsert {
                let mirrored = match &*inner.watcher.borrow() {
                    Some(watcher) => watcher.mirror_to_primary().map_err(|error| error.to_string()),
                    None => Err("this desktop does not let Insert set it".to_owned()),
                };
                if let Err(error) = mirrored {
                    eprintln!("insert: cannot set the primary selection: {error}; the clip is only copied");
                    return;
                }
            }
            if let Err(error) = injector.send_paste(keys).await {
                eprintln!("insert: cannot send the paste keys: {error}");
                inner.injector.borrow_mut().take();
                inner.set_paste_status(PasteStatus::NeedsPermission);
            }
        });
    }

    pub fn paste_status(&self) -> PasteStatus {
        self.inner.paste_status.get()
    }

    /// Starts the desktop permission flow. The result arrives as `PasteStatusChanged`.
    pub fn request_paste_permission(&self) {
        let inner = Rc::clone(&self.inner);
        glib::spawn_future_local(async move { inner.connect_paste(true).await });
    }

    /// The app in front, for the clip source and the ignored apps.
    pub fn source_app(&self) -> Option<SourceApp> {
        Some(self.inner.active_app.borrow().as_ref()?.current()?.source)
    }

    /// False when the desktop does not tell Insert which app is in front.
    pub fn tracks_source_app(&self) -> bool {
        self.inner.active_app.borrow().is_some()
    }

    pub fn launch_at_login(&self) -> bool {
        self.inner.autostart.as_ref().is_some_and(Autostart::is_enabled)
    }

    /// The error text is shown to the user.
    pub fn set_launch_at_login(&self, enabled: bool) -> Result<(), String> {
        let autostart = self
            .inner
            .autostart
            .as_ref()
            .ok_or("Insert cannot find your configuration folder.")?;
        let result = if enabled {
            // A first start with no arguments opens no window.
            autostart::program().and_then(|program| autostart.enable(&program, &[]))
        } else {
            autostart.disable()
        };
        result.map_err(|error| format!("Insert cannot change {}: {error}", autostart.path().display()))
    }

    pub fn shortcut(&self) -> Shortcut {
        self.inner.shortcut.borrow().clone()
    }

    /// Opens the desktop dialog to change the shortcut. The result arrives as `ShortcutChanged`.
    pub fn change_shortcut(&self) {
        let Some(shortcut) = self.inner.global_shortcut.borrow().clone() else {
            return;
        };
        glib::spawn_future_local(async move {
            if let Err(error) = shortcut.configure(None).await {
                eprintln!("insert: cannot open the shortcut settings: {error}");
            }
        });
    }

    pub fn set_status_icon(&self, icon: StatusIcon) {
        let mut current = self.inner.status_icon.borrow_mut();
        match (&*current, icon.visible) {
            (Some(shown), true) => shown.set_paused(icon.paused),
            (Some(_), false) => *current = None,
            (None, false) => {}
            (None, true) => {
                *current = TrayIcon::spawn(self.inner.status_menu.clone(), icon.paused)
                    .inspect_err(|error| eprintln!("insert: {error}"))
                    .ok();
            }
        }
    }
}

impl Inner {
    fn emit(&self, event: ServiceEvent) {
        // The receiver lives as long as the app.
        let _ = self.events.try_send(event);
    }

    fn set_paste_status(&self, status: PasteStatus) {
        self.paste_status.set(status);
        self.emit(ServiceEvent::PasteStatusChanged(status));
    }

    async fn connect_paste(&self, interactive: bool) {
        let result = PasteInjector::connect(interactive).await;
        let status = match &result {
            Ok(_) => PasteStatus::Ready,
            Err(PasteError::NeedsPermission) => PasteStatus::NeedsPermission,
            Err(PasteError::Unavailable(reason)) => {
                eprintln!("insert: cannot send keys on this desktop: {reason}");
                PasteStatus::Unavailable
            }
        };
        *self.injector.borrow_mut() = result.ok().map(Rc::new);
        self.set_paste_status(status);
    }

    async fn run_shortcut(&self) {
        let (sender, events) = async_channel::unbounded();
        let shortcut = match GlobalShortcut::connect(sender).await {
            Ok(shortcut) => Rc::new(shortcut),
            Err(error) => {
                eprintln!("insert: {error}");
                return;
            }
        };
        let can_change = shortcut.can_configure();
        *self.global_shortcut.borrow_mut() = Some(shortcut);
        while let Ok(event) = events.recv().await {
            let state = match event {
                ShortcutEvent::Activated { activation_token } => {
                    self.emit(ServiceEvent::ShortcutActivated { activation_token });
                    continue;
                }
                ShortcutEvent::TriggerChanged(trigger) => Shortcut::Registered {
                    trigger: Some(trigger).filter(|trigger| !trigger.is_empty()),
                    can_change,
                },
                ShortcutEvent::Unavailable(reason) => {
                    eprintln!("insert: the global shortcut stopped: {reason}");
                    self.global_shortcut.borrow_mut().take();
                    Shortcut::Unavailable
                }
            };
            *self.shortcut.borrow_mut() = state.clone();
            self.emit(ServiceEvent::ShortcutChanged(state));
        }
    }

    async fn track_active_app(self: Rc<Self>) {
        match ActiveAppTracker::start(WINDOW_TITLE).await {
            Ok(tracker) => *self.active_app.borrow_mut() = Some(Rc::new(tracker)),
            Err(error) => eprintln!("insert: {error}"),
        }
    }

    async fn forward_clipboard(self: Rc<Self>, changes: async_channel::Receiver<ClipboardEvent>) {
        while let Ok(change) = changes.recv().await {
            match change {
                ClipboardEvent::Changed { mimes } => {
                    self.emit(ServiceEvent::ClipboardChanged { offer: MimeMap(mimes) });
                }
                ClipboardEvent::Unavailable { reason } => {
                    eprintln!("insert: clipboard capture is not available: {reason}");
                    self.watcher.borrow_mut().take();
                    self.emit(ServiceEvent::CaptureUnavailable);
                }
            }
        }
    }

    async fn forward_status_menu(self: Rc<Self>, commands: async_channel::Receiver<TrayEvent>) {
        while let Ok(command) = commands.recv().await {
            self.emit(ServiceEvent::StatusMenu(match command {
                TrayEvent::ShowTray => StatusMenuCommand::ShowTray,
                TrayEvent::TogglePause => StatusMenuCommand::TogglePause,
                TrayEvent::ClearHistory => StatusMenuCommand::ClearHistory,
                TrayEvent::OpenSettings => StatusMenuCommand::OpenSettings,
                TrayEvent::Quit => StatusMenuCommand::Quit,
            }));
        }
    }
}
