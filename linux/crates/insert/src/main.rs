mod app;
mod appicon;
mod services;
mod settings;
mod theme;
mod tray;

use std::cell::RefCell;
use std::rc::Rc;

use async_signal::Signal;
use futures_lite::StreamExt as _;
use gtk4 as gtk;
use gtk::{gio, glib, prelude::*};
use insert_core::MimeMap;

use app::App;
use services::ServiceEvent;

/// A signal handler that holds the app weakly. `with_app!(weak => app, |args| body)` binds the
/// live `&Rc<App>` to `app` in the body. When the app is gone, the handler returns the default
/// value, or the value after `or`.
macro_rules! with_app {
    ($weak:expr => $app:ident, |$($arg:pat_param),*| $body:expr) => {
        with_app!($weak => $app or Default::default(), |$($arg),*| $body)
    };
    ($weak:expr => $app:ident or $fallback:expr, |$($arg:pat_param),*| $body:expr) => {{
        let weak = ::std::rc::Weak::clone($weak);
        move |$($arg),*| {
            let Some($app) = weak.upgrade() else {
                return $fallback;
            };
            let $app = &$app;
            $body
        }
    }};
}
pub(crate) use with_app;

const APP_ID: &str = "io.github.kenwuqianghao.Insert";

fn main() -> glib::ExitCode {
    let gtk_app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    gtk_app.add_main_option(
        "toggle",
        glib::Char::from(b't'),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Open or close the tray of the running Insert",
        None,
    );

    let running: Rc<RefCell<Option<Rc<App>>>> = Rc::default();
    gtk_app.connect_startup({
        let running = running.clone();
        move |gtk_app| {
            theme::install(&gtk::gdk::Display::default().expect("a display"));
            let app = App::start(gtk_app);
            install_app_actions(&app);
            if std::env::var_os("INSERT_TEST_HOOKS").is_some_and(|v| v == "1") {
                install_test_hooks(&app);
            }
            std::mem::forget(gtk_app.hold());
            quit_on_signals(&app);
            *running.borrow_mut() = Some(app);
        }
    });
    gtk_app.connect_command_line({
        let running = running.clone();
        move |_, command_line| {
            let Some(app) = running.borrow().clone() else {
                return glib::ExitCode::FAILURE;
            };
            let token = activation_token(command_line);
            if command_line.options_dict().contains("toggle") {
                app.toggle_tray(token.as_deref());
            } else if command_line.is_remote() {
                app.show_tray(token.as_deref());
            }
            glib::ExitCode::SUCCESS
        }
    });
    gtk_app.connect_shutdown(move |_| {
        if let Some(app) = running.borrow().as_ref() {
            app.flush_save();
            app.services.stop();
        }
    });
    gtk_app.run()
}

/// The default action of SIGTERM and SIGINT ends the process before the index save that waits.
fn quit_on_signals(app: &Rc<App>) {
    let mut signals = match async_signal::Signals::new([Signal::Term, Signal::Int]) {
        Ok(signals) => signals,
        Err(error) => {
            eprintln!("insert: cannot handle SIGTERM and SIGINT: {error}");
            return;
        }
    };
    let weak = Rc::downgrade(app);
    glib::spawn_future_local(async move {
        while signals.next().await.is_some() {
            let Some(app) = weak.upgrade() else { return };
            app.quit();
        }
    });
}

fn activation_token(command_line: &gio::ApplicationCommandLine) -> Option<String> {
    let data = command_line.platform_data()?;
    glib::VariantDict::new(Some(&data))
        .lookup::<String>("activation-token")
        .ok()
        .flatten()
}

fn install_app_actions(app: &Rc<App>) {
    let weak = Rc::downgrade(app);
    let add = |name: &str, run: fn(&Rc<App>)| {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(with_app!(&weak => app, |_, _| run(app)));
        app.gtk_app.add_action(&action);
    };
    add("show-tray", |app| app.show_tray(None));
    add("toggle-tray", |app| app.toggle_tray(None));
    add("settings", |app| app.open_settings());
    add("toggle-pause", |app| app.toggle_paused());
    add("clear-history", |app| app.confirm_clear_history());
    add("quit", |app| app.quit());
    app.gtk_app
        .set_accels_for_action("app.settings", &["<Control>comma"]);
}

/// Verification only. `test-key` feeds the tray key path, `test-capture` the capture pipeline,
/// and `test-action` runs a window action such as "tray.new-pinboard" or "settings.pane|privacy",
/// or "clip-menu" and "filter-menu" to open those menus.
fn install_test_hooks(app: &Rc<App>) {
    let weak = Rc::downgrade(app);
    let key = gio::SimpleAction::new("test-key", Some(glib::VariantTy::STRING));
    key.connect_activate(with_app!(&weak => app, |_, param| {
        let Some(spec) = param.and_then(|p| p.str()) else { return };
        match spec {
            "ctrl-down" => app.set_ctrl_held(true),
            "ctrl-up" => app.set_ctrl_held(false),
            _ => match spec.strip_prefix("type:") {
                Some(text) => app.tray.type_text(text),
                None => match tray::parse_test_key(spec) {
                    Some((key, mods)) if !app.handle_key(key, mods) => app.tray.edit_field(key),
                    Some(_) => {}
                    None => eprintln!("insert: test-key: unknown key {spec}"),
                },
            },
        }
    }));
    app.gtk_app.add_action(&key);

    let capture = gio::SimpleAction::new("test-capture", Some(glib::VariantTy::STRING));
    capture.connect_activate(with_app!(&weak => app, |_, param| {
        let Some(spec) = param.and_then(|p| p.str()) else { return };
        let offer = if let Some(path) = spec.strip_prefix("file:") {
            MimeMap(vec![(
                "text/uri-list".to_owned(),
                format!("file://{path}\r\n").into_bytes(),
            )])
        } else if let Some(path) = spec.strip_prefix("image:") {
            match std::fs::read(path) {
                Ok(bytes) => MimeMap(vec![("image/png".to_owned(), bytes)]),
                Err(error) => {
                    eprintln!("insert: test-capture: {error}");
                    return;
                }
            }
        } else {
            MimeMap::from_plain_text(spec)
        };
        app.on_service_event(ServiceEvent::ClipboardChanged {
            offer,
            concealed: false,
        });
    }));
    app.gtk_app.add_action(&capture);

    let action = gio::SimpleAction::new("test-action", Some(glib::VariantTy::STRING));
    action.connect_activate(with_app!(&weak => app, |_, param| {
        let Some(spec) = param.and_then(|p| p.str()) else { return };
        if spec == "clip-menu" {
            let primary = app.tray.model.borrow().selection.primary();
            if let Some((id, card)) = primary.and_then(|id| Some((id, app.tray.card_widget(id)?))) {
                app.open_clip_menu(id, card.upcast_ref(), 120.0, 120.0);
            }
            return;
        }
        if spec == "filter-menu" {
            app.tray.popup_filter_menu();
            return;
        }
        let (name, target) = match spec.split_once('|') {
            Some((name, target)) => (name, Some(target.to_variant())),
            None => (spec, None),
        };
        let window: gtk::Window = if name.starts_with("settings.") {
            match app.gtk_app.windows().into_iter().find(|w| w.has_css_class("settings")) {
                Some(window) => window,
                None => return,
            }
        } else {
            app.tray.window.clone().upcast()
        };
        if let Err(error) = window.activate_action(name, target.as_ref()) {
            eprintln!("insert: test-action: {error}");
        }
    }));
    app.gtk_app.add_action(&action);
}
