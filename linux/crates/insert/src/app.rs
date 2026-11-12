use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4 as gtk;
use gtk::{gio, glib, prelude::*};
use insert_core::{
    classify, Capture, Change, HistoryLimit, Library, MimeMap, Paths, Preferences, Skip, SourceApp, Storage,
};
use time::OffsetDateTime;

use crate::services::{
    PasteStatus, ServiceEvent, Services, Shortcut, StatusIcon, StatusMenuCommand,
};
use crate::settings::SettingsWindow;
use crate::tray::{Notices, Tray};

const SAVE_DELAY: Duration = Duration::from_millis(500);

pub struct App {
    pub gtk_app: gtk::Application,
    pub services: Services,
    pub library: RefCell<Library>,
    pub prefs: RefCell<Preferences>,
    paths: Paths,
    pub tray: Tray,
    settings: RefCell<Option<Rc<SettingsWindow>>>,
    captures: async_channel::Sender<(MimeMap, Option<SourceApp>)>,
    save_timer: RefCell<Option<glib::SourceId>>,
    pub library_notice: Cell<bool>,
    pub capture_notice: Cell<bool>,
    pub paste_status: Cell<PasteStatus>,
    pub shortcut: RefCell<Shortcut>,
}

impl App {
    pub fn start(gtk_app: &gtk::Application) -> Rc<Self> {
        let display = gtk::gdk::Display::default().expect("a display");
        let paths = Paths::from_env();
        let (services, events) = Services::start(&display, &paths);
        let prefs = Preferences::load(&paths.preferences_file());
        let opened = Library::open(Storage::new(paths.data_dir.clone()), prefs.history_limit);
        if let Some(backup) = &opened.corrupt_backup {
            eprintln!("insert: index.json did not parse; moved to {}", backup.display());
        }

        let (captures, queued_captures) = async_channel::unbounded();
        let app = Rc::new_cyclic(|weak| App {
            gtk_app: gtk_app.clone(),
            paste_status: Cell::new(services.paste_status()),
            shortcut: RefCell::new(services.shortcut()),
            services,
            library: RefCell::new(opened.library),
            prefs: RefCell::new(prefs),
            paths,
            tray: Tray::new(weak, gtk_app),
            settings: RefCell::new(None),
            captures,
            save_timer: RefCell::new(None),
            library_notice: Cell::new(opened.corrupt_backup.is_some()),
            capture_notice: Cell::new(false),
        });
        app.install_tray_actions();
        app.update_status_icon();

        let weak = Rc::downgrade(&app);
        glib::spawn_future_local(async move {
            while let Ok((offer, source)) = queued_captures.recv().await {
                let Ok(classified) = gio::spawn_blocking(move || classify(offer)).await else {
                    continue;
                };
                let Some(app) = weak.upgrade() else { break };
                app.insert_capture(classified, source);
            }
        });
        let weak = Rc::downgrade(&app);
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                let Some(app) = weak.upgrade() else { break };
                app.on_service_event(event);
            }
        });
        let weak = Rc::downgrade(&app);
        glib::timeout_add_seconds_local(60, move || {
            let Some(app) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if app.tray.model.borrow().is_shown() {
                app.render_tray();
            }
            glib::ControlFlow::Continue
        });
        app
    }

    pub fn on_service_event(self: &Rc<Self>, event: ServiceEvent) {
        match event {
            ServiceEvent::ClipboardChanged { offer } => self.capture(offer),
            ServiceEvent::ShortcutActivated { activation_token } => {
                self.toggle_tray(activation_token.as_deref())
            }
            ServiceEvent::ShortcutChanged(shortcut) => {
                *self.shortcut.borrow_mut() = shortcut;
                self.refresh_settings();
            }
            ServiceEvent::PasteStatusChanged(status) => {
                self.paste_status.set(status);
                self.render_tray();
                self.refresh_settings();
            }
            ServiceEvent::CaptureUnavailable => {
                self.capture_notice.set(true);
                self.render_tray();
            }
            ServiceEvent::StatusMenu(command) => match command {
                StatusMenuCommand::ShowTray => self.show_tray(None),
                StatusMenuCommand::TogglePause => self.toggle_paused(),
                StatusMenuCommand::ClearHistory => self.confirm_clear_history(),
                StatusMenuCommand::OpenSettings => self.open_settings(),
                StatusMenuCommand::Quit => self.quit(),
            },
        }
    }

    pub fn capture(&self, offer: MimeMap) {
        let source = self.services.source_app();
        {
            let prefs = self.prefs.borrow();
            if prefs.capture_paused || prefs.is_ignored(source.as_ref()) {
                return;
            }
        }
        let _ = self.captures.try_send((offer, source));
    }

    fn insert_capture(self: &Rc<Self>, classified: Result<Capture, Skip>, source: Option<SourceApp>) {
        let capture = match classified {
            Ok(capture) => capture,
            Err(skip) => {
                eprintln!("insert: clipboard not saved: {skip:?}");
                return;
            }
        };
        if self.prefs.borrow().capture_paused {
            return;
        }
        let inserted = self
            .library
            .borrow_mut()
            .insert(capture, source, OffsetDateTime::now_utc());
        match inserted {
            Ok(change) => self.apply(change),
            Err(error) => eprintln!("insert: cannot save the clip: {error}"),
        }
    }

    pub fn apply(self: &Rc<Self>, change: Change) {
        if !change.needs_save() {
            return;
        }
        self.tray.drop_cards(&change.removed);
        self.schedule_save();
        self.tray
            .model
            .borrow_mut()
            .reconcile(&self.library.borrow());
        self.render_tray();
    }

    pub fn render_tray(&self) {
        let prefs = self.prefs.borrow();
        self.sync_kind_action();
        self.tray.render(
            &self.library.borrow(),
            Notices {
                paused: prefs.capture_paused && !self.capture_notice.get(),
                needs_paste_permission: prefs.paste_directly
                    && self.paste_status.get() == PasteStatus::NeedsPermission,
                library_unreadable: self.library_notice.get(),
                capture_unavailable: self.capture_notice.get(),
            },
        );
    }

    fn schedule_save(self: &Rc<Self>) {
        if self.save_timer.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(SAVE_DELAY, move || {
            if let Some(app) = weak.upgrade() {
                app.save_timer.borrow_mut().take();
                app.save_now();
            }
        });
        *self.save_timer.borrow_mut() = Some(id);
    }

    fn save_now(&self) {
        if let Err(error) = self.library.borrow().save() {
            eprintln!("insert: cannot save index.json: {error}");
        }
    }

    pub fn flush_save(&self) {
        if let Some(id) = self.save_timer.borrow_mut().take() {
            id.remove();
            self.save_now();
        }
    }

    fn change_prefs(self: &Rc<Self>, change: impl FnOnce(&mut Preferences)) {
        {
            let mut prefs = self.prefs.borrow_mut();
            change(&mut prefs);
            if let Err(error) = prefs.save(&self.paths.preferences_file()) {
                eprintln!("insert: cannot save preferences: {error}");
            }
        }
        self.update_status_icon();
        self.render_tray();
        self.refresh_settings();
    }

    pub fn set_paused(self: &Rc<Self>, paused: bool) {
        self.change_prefs(|prefs| prefs.capture_paused = paused);
    }

    pub fn toggle_paused(self: &Rc<Self>) {
        let paused = self.prefs.borrow().capture_paused;
        self.set_paused(!paused);
    }

    pub fn set_paste_directly(self: &Rc<Self>, on: bool) {
        self.change_prefs(|prefs| prefs.paste_directly = on);
    }

    pub fn set_show_tray_icon(self: &Rc<Self>, on: bool) {
        self.change_prefs(|prefs| prefs.show_tray_icon = on);
    }

    pub fn set_history_limit(self: &Rc<Self>, limit: HistoryLimit) {
        self.change_prefs(|prefs| prefs.history_limit = limit);
        let change = self.library.borrow_mut().set_history_limit(limit);
        self.apply(change);
    }

    pub fn set_launch_at_login(self: &Rc<Self>, on: bool) -> Result<(), String> {
        let result = self.services.set_launch_at_login(on);
        self.refresh_settings();
        result
    }

    pub fn ignore_apps(self: &Rc<Self>, apps: Vec<SourceApp>) {
        self.change_prefs(|prefs| {
            for app in apps {
                prefs.ignore_app(app);
            }
        });
    }

    pub fn unignore_app(self: &Rc<Self>, id: &str) {
        self.change_prefs(|prefs| prefs.unignore_app(id));
    }

    fn update_status_icon(&self) {
        let prefs = self.prefs.borrow();
        self.services.set_status_icon(StatusIcon {
            visible: prefs.show_tray_icon,
            paused: prefs.capture_paused,
        });
    }

    pub fn open_settings(self: &Rc<Self>) {
        self.hide_tray(None);
        let window = self
            .settings
            .borrow_mut()
            .get_or_insert_with(|| SettingsWindow::new(self))
            .clone();
        window.refresh(self);
        window.present();
    }

    fn refresh_settings(self: &Rc<Self>) {
        let window = self.settings.borrow().clone();
        if let Some(window) = window {
            window.refresh(self);
        }
    }

    pub fn confirm_clear_history(self: &Rc<Self>) {
        self.hide_tray(None);
        let dialog = gtk::AlertDialog::builder()
            .modal(true)
            .message("Clear the clipboard history?")
            .detail("Insert deletes all clips that are not in a pinboard. You cannot undo this.")
            .buttons(["Clear History", "Cancel"])
            .default_button(1)
            .cancel_button(1)
            .build();
        let parent = self
            .settings
            .borrow()
            .as_ref()
            .map(|settings| settings.window.clone())
            .filter(|window| window.is_visible());
        let weak = Rc::downgrade(self);
        dialog.choose(parent.as_ref(), gio::Cancellable::NONE, move |choice| {
            let Some(app) = weak.upgrade() else { return };
            if choice == Ok(0) {
                let change = app.library.borrow_mut().clear_history();
                app.apply(change);
            }
        });
    }

    pub fn quit(&self) {
        self.flush_save();
        self.gtk_app.quit();
    }
}
