mod app;
mod appicon;
mod services;
mod settings;
#[cfg(feature = "test-hooks")]
mod test_hooks;
mod theme;
mod tray;

use std::cell::RefCell;
use std::rc::Rc;

use async_signal::Signal;
use futures_lite::StreamExt as _;
use gtk4 as gtk;
use gtk::{gio, glib, prelude::*};

use app::App;

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

fn main() -> glib::ExitCode {
    let gtk_app = gtk::Application::builder()
        .application_id(insert_platform::APP_ID)
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
            #[cfg(feature = "test-hooks")]
            if std::env::var_os("INSERT_TEST_HOOKS").is_some_and(|v| v == "1") {
                test_hooks::install(&app);
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
