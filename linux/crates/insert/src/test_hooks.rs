use std::rc::Rc;

use gtk4 as gtk;
use gtk::{gio, glib, prelude::*};
use insert_core::MimeMap;

use crate::app::App;
use crate::services::ServiceEvent;
use crate::{tray, with_app};

pub fn install(app: &Rc<App>) {
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
        app.on_service_event(ServiceEvent::ClipboardChanged { offer });
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
