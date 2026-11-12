use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gtk4 as gtk;
use gtk::{gdk, gio, glib, prelude::*};
use insert_core::{
    cancel_effect, command_for_key, CancelEffect, ClipId, Key, KeyContext, MimeMap, Modifiers,
    NameTarget, PanelCommand, PasteAction, PinboardId, Scope, Selection,
};
use time::OffsetDateTime;

use super::card::Card;
use super::menus;
use super::model::{Paste, TrayState};
use crate::app::App;
use crate::services::PasteStatus;
use crate::with_app;

const DELETE_ANIMATION: Duration = Duration::from_millis(160);

impl App {
    pub fn toggle_tray(self: &Rc<Self>, activation_token: Option<&str>) {
        if self.tray.model.borrow().is_shown() {
            self.hide_tray(None);
        } else {
            self.show_tray(activation_token);
        }
    }

    pub fn show_tray(self: &Rc<Self>, activation_token: Option<&str>) {
        if self.tray.model.borrow().is_shown() {
            self.tray.present(activation_token);
            return;
        }
        self.paste_status.set(self.services.paste_status());
        self.tray
            .model
            .borrow_mut()
            .prepare_show(&self.library.borrow());
        self.render_tray();
        self.tray.present(activation_token);
    }

    pub fn hide_tray(self: &Rc<Self>, then: Option<Paste>) {
        if self.tray.model.borrow().state == TrayState::Hidden {
            if then.is_some() {
                self.services.paste_into_front_app();
            }
            return;
        }
        let start = self.tray.model.borrow_mut().begin_hide(then);
        if start {
            self.tray.start_hide();
        }
    }

    pub fn hide_if_focus_lost(self: &Rc<Self>) {
        let lost = !self.tray.window.is_active()
            && !self.tray.is_menu_open()
            && self.tray.model.borrow().is_shown();
        if lost {
            self.hide_tray(None);
        }
    }

    pub fn finish_hide(self: &Rc<Self>) {
        if !matches!(self.tray.model.borrow().state, TrayState::Hiding { .. }) {
            return;
        }
        self.tray.window.set_visible(false);
        let then = self.tray.model.borrow_mut().did_hide();
        if then.is_some() {
            self.services.paste_into_front_app();
        }
    }

    pub fn set_ctrl_held(self: &Rc<Self>, held: bool) {
        let changed = {
            let mut model = self.tray.model.borrow_mut();
            let changed = model.is_shown() && model.ctrl_held != held;
            model.ctrl_held = held;
            changed
        };
        if changed {
            self.render_tray();
        }
    }

    pub fn handle_key(self: &Rc<Self>, key: Key, modifiers: Modifiers) -> bool {
        let command = {
            let model = self.tray.model.borrow();
            let Some(mode) = model.key_mode() else {
                return false;
            };
            let context = KeyContext {
                mode,
                query_is_empty: model.query.is_empty(),
                composing: self.tray.is_composing(),
            };
            command_for_key(key, modifiers, context)
        };
        let Some(command) = command else {
            return false;
        };
        self.run_command(command);
        true
    }

    fn run_command(self: &Rc<Self>, command: PanelCommand) {
        match command {
            PanelCommand::Move(offset) => self.change_selection(|s, v| s.move_by(offset, v)),
            PanelCommand::Extend(offset) => self.change_selection(|s, v| s.extend_by(offset, v)),
            PanelCommand::SelectAll => self.change_selection(|s, v| s.select_all(v)),
            PanelCommand::Activate(action) => {
                let primary = self.tray.model.borrow().selection.primary();
                if let Some(id) = primary {
                    self.activate(id, action);
                }
            }
            PanelCommand::ActivateIndex(index) => {
                let id = self.tray.model.borrow().visible.get(index).copied();
                if let Some(id) = id {
                    self.activate(id, PasteAction::Paste);
                }
            }
            PanelCommand::DeleteSelection => {
                let ids = self.tray.model.borrow().selection.ids().clone();
                self.delete(ids);
            }
            PanelCommand::TogglePreview => {
                {
                    let mut model = self.tray.model.borrow_mut();
                    model.state = match model.state {
                        TrayState::Previewing => TrayState::Browsing,
                        TrayState::Browsing if model.selection.primary().is_some() => {
                            TrayState::Previewing
                        }
                        ref other => other.clone(),
                    };
                }
                self.render_tray();
            }
            PanelCommand::Cancel => {
                let effect = {
                    let model = self.tray.model.borrow();
                    let Some(mode) = model.key_mode() else { return };
                    cancel_effect(mode, &model.query)
                };
                match effect {
                    CancelEffect::Browse => self.browse(),
                    CancelEffect::ClearQuery => self.set_query(String::new()),
                    CancelEffect::HideTray => self.hide_tray(None),
                }
            }
            PanelCommand::SwitchScope(offset) => {
                let scope = {
                    let model = self.tray.model.borrow();
                    self.library.borrow().step_scope(model.scope, offset)
                };
                self.set_scope(scope);
            }
            PanelCommand::OpenSettings => self.open_settings(),
            PanelCommand::Hide => self.hide_tray(None),
        }
    }

    fn change_selection(self: &Rc<Self>, change: impl FnOnce(&mut Selection, &[ClipId])) {
        {
            let model = &mut *self.tray.model.borrow_mut();
            change(&mut model.selection, &model.visible);
        }
        self.render_tray();
    }

    fn browse(self: &Rc<Self>) {
        self.tray.model.borrow_mut().state = TrayState::Browsing;
        self.render_tray();
        self.tray.focus_field();
    }

    pub fn activate(self: &Rc<Self>, id: ClipId, action: PasteAction) {
        let payload = self.library.borrow().payload(id);
        let Ok(payload) = payload else {
            self.hide_tray(None);
            WidgetExt::display(&self.tray.window).beep();
            return;
        };
        let offer = match (action, payload.plain_text()) {
            (PasteAction::PastePlainText, Some(text)) => MimeMap::from_plain_text(&text),
            _ => payload.for_clipboard(),
        };
        let copied = self.services.set_clipboard(offer);
        if let Err(error) = &copied {
            eprintln!("insert: cannot set the clipboard: {error}");
        }
        let change = self.library.borrow_mut().move_to_front(id, OffsetDateTime::now_utc());
        self.apply(change);
        let paste = copied.is_ok()
            && action != PasteAction::Copy
            && self.prefs.borrow().paste_directly
            && self.services.paste_status() == PasteStatus::Ready;
        self.hide_tray(paste.then_some(Paste));
    }

    fn delete(self: &Rc<Self>, ids: HashSet<ClipId>) {
        {
            let mut model = self.tray.model.borrow_mut();
            if ids.is_empty() || !model.deleting.is_empty() {
                return;
            }
            model.selection = model.selection.after_deleting(&ids, &model.visible);
            model.deleting = ids.clone();
        }
        self.render_tray();
        let app = Rc::downgrade(self);
        glib::timeout_add_local_once(DELETE_ANIMATION, move || {
            let Some(app) = app.upgrade() else { return };
            app.tray.model.borrow_mut().deleting.clear();
            let change = app.library.borrow_mut().delete(&ids);
            app.apply(change);
        });
    }

    pub fn set_query(self: &Rc<Self>, query: String) {
        {
            let mut model = self.tray.model.borrow_mut();
            if model.query == query {
                return;
            }
            model.query = query;
            model.reset_selection(&self.library.borrow());
        }
        self.render_tray();
    }

    fn set_scope(self: &Rc<Self>, scope: Scope) {
        {
            let mut model = self.tray.model.borrow_mut();
            model.scope = scope;
            model.reset_selection(&self.library.borrow());
        }
        self.render_tray();
    }

    fn set_kind_filter(self: &Rc<Self>, kind: Option<insert_core::ClipKind>) {
        {
            let mut model = self.tray.model.borrow_mut();
            model.kind = kind;
            model.reset_selection(&self.library.borrow());
        }
        self.render_tray();
    }

    pub fn bind_card(self: &Rc<Self>, item: &gtk::ListItem) {
        let Some(id) = super::item_clip_id(item) else { return };
        if !self.tray.has_card(id) {
            let library = self.library.borrow();
            let Some(clip) = library.clip(id) else { return };
            let card = Card::new(clip, &library);
            let click = gtk::GestureClick::new();
            click.set_button(0);
            click.connect_pressed(with_app!(&Rc::downgrade(self) => app, |gesture, presses, x, y| {
                app.click_card(id, gesture, presses, x, y);
            }));
            card.root.add_controller(click);
            self.tray.insert_card(id, card);
        }
        self.tray.attach_card(id, item);
        self.tray
            .restyle(&self.tray.model.borrow(), &self.library.borrow());
    }

    fn click_card(self: &Rc<Self>, id: ClipId, gesture: &gtk::GestureClick, presses: i32, x: f64, y: f64) {
        let state = gesture.current_event_state();
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        if gesture.current_button() == gdk::BUTTON_SECONDARY {
            if let Some(widget) = gesture.widget() {
                self.open_clip_menu(id, &widget, x, y);
            }
            return;
        }
        if gesture.current_button() != gdk::BUTTON_PRIMARY {
            return;
        }
        if presses == 1 {
            self.change_selection(|selection, visible| {
                if shift {
                    selection.select_range(id, visible);
                } else if ctrl {
                    selection.toggle(id, visible);
                } else {
                    *selection = Selection::only(Some(id));
                }
            });
        } else if presses == 2 && !shift && !ctrl {
            self.activate(id, PasteAction::Paste);
        }
    }

    pub fn open_clip_menu(self: &Rc<Self>, id: ClipId, widget: &gtk::Widget, x: f64, y: f64) {
        let menu = {
            let library = self.library.borrow();
            let Some(clip) = library.clip(id) else { return };
            let mut model = self.tray.model.borrow_mut();
            model.menu_targets = if model.selection.contains(id) {
                model.selection.ids().clone()
            } else {
                HashSet::from([id])
            };
            menus::clip_menu(clip, &library)
        };
        self.tray.popup_menu(&menu, widget, x, y);
    }

    pub fn open_pinboard_menu(self: &Rc<Self>, id: PinboardId, widget: &gtk::Widget, x: f64, y: f64) {
        let menu = {
            let library = self.library.borrow();
            let Some(pinboard) = library.pinboard(id) else { return };
            self.tray.menu_pinboard.set(Some(id));
            if let Some(action) = self.tray.action("pinboard-color") {
                action.set_state(&pinboard.color.name().to_variant());
            }
            menus::pinboard_menu(pinboard)
        };
        self.tray.popup_menu(&menu, widget, x, y);
    }

    fn begin_naming(self: &Rc<Self>, target: NameTarget) {
        if !self.tray.model.borrow().is_shown() {
            return;
        }
        self.tray.model.borrow_mut().state = TrayState::Naming(target);
        self.render_tray();
        self.tray.focus_field();
    }

    pub fn commit_name(self: &Rc<Self>, text: &str) {
        let TrayState::Naming(target) = self.tray.model.borrow().state.clone() else {
            return;
        };
        let commit = self.library.borrow_mut().commit_name(&target, text);
        {
            let mut model = self.tray.model.borrow_mut();
            model.state = TrayState::Browsing;
            if let Some(scope) = commit.switch_to {
                model.scope = scope;
            }
        }
        self.apply(commit.change);
        if commit.switch_to.is_some() {
            self.tray
                .model
                .borrow_mut()
                .reset_selection(&self.library.borrow());
            self.render_tray();
        }
        self.tray.focus_field();
    }

    fn set_pinboard_of_targets(self: &Rc<Self>, pinboard: Option<PinboardId>) {
        let targets = self.tray.model.borrow().menu_targets.clone();
        let change = self.library.borrow_mut().set_pinboard(pinboard, &targets);
        self.apply(change);
    }

    pub fn install_tray_actions(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let clip_action = |name: &str, run: fn(&Rc<App>, ClipId)| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            action.connect_activate(with_app!(&weak => app, |_, param| {
                if let Some(id) = param.and_then(parse_clip_id) {
                    run(app, id);
                }
            }));
            self.tray.add_action(&action);
        };
        clip_action("paste", |app, id| app.activate(id, PasteAction::Paste));
        clip_action("paste-plain", |app, id| app.activate(id, PasteAction::PastePlainText));
        clip_action("copy", |app, id| app.activate(id, PasteAction::Copy));
        clip_action("rename-clip", |app, id| app.begin_naming(NameTarget::Clip(id)));
        clip_action("preview", |app, id| {
            let mut model = app.tray.model.borrow_mut();
            model.selection = Selection::only(Some(id));
            model.state = TrayState::Previewing;
            drop(model);
            app.render_tray();
        });

        let pinboard_action = |name: &str, run: fn(&Rc<App>, PinboardId)| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            action.connect_activate(with_app!(&weak => app, |_, param| {
                if let Some(id) = param.and_then(parse_pinboard_id) {
                    run(app, id);
                }
            }));
            self.tray.add_action(&action);
        };
        pinboard_action("pin-to", |app, id| app.set_pinboard_of_targets(Some(id)));
        pinboard_action("rename-pinboard", |app, id| {
            app.begin_naming(NameTarget::Pinboard(id))
        });
        pinboard_action("delete-pinboard", |app, id| {
            let change = app.library.borrow_mut().delete_pinboard(id);
            app.apply(change);
        });

        let simple = |name: &str, run: fn(&Rc<App>)| {
            let action = gio::SimpleAction::new(name, None);
            action.connect_activate(with_app!(&weak => app, |_, _| run(app)));
            self.tray.add_action(&action);
        };
        simple("pin-new", |app| {
            let pinning = app.tray.model.borrow().menu_targets.clone();
            app.begin_naming(NameTarget::NewPinboard { pinning });
        });
        simple("unpin", |app| app.set_pinboard_of_targets(None));
        simple("delete", |app| {
            let targets = app.tray.model.borrow().menu_targets.clone();
            app.delete(targets);
        });
        simple("new-pinboard", |app| {
            app.begin_naming(NameTarget::NewPinboard {
                pinning: HashSet::new(),
            })
        });
        simple("resume", |app| app.set_paused(false));
        simple("allow-paste", |app| {
            app.hide_tray(None);
            app.services.request_paste_permission();
        });
        simple("dismiss-library-notice", |app| {
            app.library_notice.set(false);
            app.render_tray();
        });
        simple("dismiss-capture-notice", |app| {
            app.capture_notice.set(false);
            app.render_tray();
        });

        let scope = gio::SimpleAction::new("scope", Some(glib::VariantTy::STRING));
        scope.connect_activate(with_app!(&weak => app, |_, param| {
            let Some(target) = param.and_then(|p| p.str()) else { return };
            let scope = match target {
                "history" => Scope::History,
                other => match uuid::Uuid::parse_str(other) {
                    Ok(id) => Scope::Pinboard(PinboardId(id)),
                    Err(_) => return,
                },
            };
            app.set_scope(scope);
        }));
        self.tray.add_action(&scope);

        let kind = gio::SimpleAction::new_stateful(
            "kind",
            Some(glib::VariantTy::STRING),
            &menus::ALL_KINDS.to_variant(),
        );
        kind.connect_activate(with_app!(&weak => app, |action, param| {
            let Some(key) = param.and_then(|p| p.str()) else { return };
            action.set_state(&key.to_variant());
            app.set_kind_filter(menus::kind_from_key(key));
        }));
        self.tray.add_action(&kind);

        let color = gio::SimpleAction::new_stateful(
            "pinboard-color",
            Some(glib::VariantTy::STRING),
            &"".to_variant(),
        );
        color.connect_activate(with_app!(&weak => app, |action, param| {
            let Some(color) = param.and_then(|p| p.str()).and_then(menus::color_from_name) else {
                return;
            };
            action.set_state(&color.name().to_variant());
            if let Some(id) = app.tray.menu_pinboard.get() {
                let change = app.library.borrow_mut().set_pinboard_color(id, color);
                app.apply(change);
            }
        }));
        self.tray.add_action(&color);
    }

    pub fn sync_kind_action(&self) {
        if let Some(action) = self.tray.action("kind") {
            let kind = self.tray.model.borrow().kind;
            action.set_state(&menus::kind_key(kind).to_variant());
        }
    }
}

fn parse_clip_id(param: &glib::Variant) -> Option<ClipId> {
    uuid::Uuid::parse_str(param.str()?).ok().map(ClipId)
}

fn parse_pinboard_id(param: &glib::Variant) -> Option<PinboardId> {
    uuid::Uuid::parse_str(param.str()?).ok().map(PinboardId)
}
