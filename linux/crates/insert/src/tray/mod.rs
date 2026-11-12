//! The tray: a transparent full-screen window with the card tray at the bottom edge.

mod card;
mod control;
mod keys;
mod menus;
pub mod model;
mod preview;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Weak;

use gtk4 as gtk;
use gtk::{gdk, gio, glib, prelude::*};
use insert_core::{empty_state, ClipId, Key, Library, PinboardId, Scope};
use time::OffsetDateTime;

use crate::app::App;
use crate::with_app;
use crate::theme::{appearance, background_class, icon};
use card::{set_class, Card, Ring};
pub use keys::parse_test_key;
use model::{TrayModel, TrayState};

/// The desktop finds the tray window by this title to give it the focus and its role.
pub const WINDOW_TITLE: &str = "Insert";
const SHOW_MS: u32 = 220;
const HIDE_MS: u32 = 160;
const MODE_MS: u32 = 200;
const STRIP_HEIGHT: i32 = 270;
const MENU_FOCUS_GRACE: std::time::Duration = std::time::Duration::from_millis(150);

/// Which notices the top bar shows.
#[derive(Clone, Copy)]
pub struct Notices {
    pub paused: bool,
    pub needs_paste_permission: bool,
    pub library_unreadable: bool,
    pub capture_unavailable: bool,
}

/// A notice pill in the top bar.
struct Notice {
    root: gtk::Box,
}

impl Notice {
    fn new(icon_names: &[&str], text: &str, button: &str, action: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        root.add_css_class("notice");
        root.set_valign(gtk::Align::Center);
        let image = icon(icon_names);
        image.add_css_class("notice-icon");
        root.append(&image);
        root.append(&gtk::Label::new(Some(text)));
        let button = gtk::Button::with_label(button);
        button.add_css_class("notice-button");
        button.set_action_name(Some(action));
        button.set_valign(gtk::Align::Center);
        root.append(&button);
        Self { root }
    }
}

pub struct Tray {
    pub window: gtk::ApplicationWindow,
    pub model: RefCell<TrayModel>,
    /// True while `render` changes widgets, so their change signals are ignored.
    rendering: Cell<bool>,
    composing: Cell<bool>,
    revealer: gtk::Revealer,
    panel: gtk::Box,
    preview_revealer: gtk::Revealer,
    preview_frame: gtk::ScrolledWindow,
    preview_slot: gtk::Box,
    previewed: Cell<Option<ClipId>>,
    search: gtk::Text,
    search_clear: gtk::Button,
    tabs: gtk::Box,
    tabs_shown: RefCell<Option<(Scope, Vec<(String, String)>)>>,
    pause_notice: Notice,
    paste_notice: Notice,
    library_notice: Notice,
    capture_notice: Notice,
    filter_button: gtk::MenuButton,
    filter_label: gtk::Label,
    more_button: gtk::MenuButton,
    more_paused: Cell<Option<bool>>,
    strip: gtk::Stack,
    list: gtk::ListView,
    ids: gtk::StringList,
    empty_icon: gtk::Image,
    empty_title: gtk::Label,
    empty_detail: gtk::Label,
    prompt_revealer: gtk::Revealer,
    prompt_title: gtk::Label,
    prompt_entry: gtk::Entry,
    /// The cards that a row shows now.
    cards: RefCell<HashMap<ClipId, Card>>,
    /// The list row that shows each bound card now.
    bound: RefCell<HashMap<ClipId, glib::WeakRef<gtk::ListItem>>>,
    /// New clips that play the insert animation when their card is bound.
    appearing: RefCell<HashSet<ClipId>>,
    actions: gio::SimpleActionGroup,
    /// The pinboard whose tab menu is open.
    pub menu_pinboard: Cell<Option<PinboardId>>,
    menu_open: Cell<bool>,
    app: Weak<App>,
}

impl Tray {
    pub fn new(app: &Weak<App>, gtk_app: &gtk::Application) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(gtk_app)
            .title(WINDOW_TITLE)
            .decorated(false)
            .build();
        window.add_css_class("tray-window");
        window.set_hide_on_close(true);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let backdrop = gtk::Box::new(gtk::Orientation::Vertical, 0);
        backdrop.set_vexpand(true);
        root.append(&backdrop);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("tray");

        let preview_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // The preview height is fixed by the screen, whatever the content asks for.
        let preview_frame = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .propagate_natural_height(false)
            .child(&preview_slot)
            .build();
        let preview_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(MODE_MS)
            .child(&preview_frame)
            .vexpand(false)
            .build();
        panel.append(&preview_revealer);

        let top_bar = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        top_bar.add_css_class("top-bar");
        top_bar.set_size_request(-1, 54);
        top_bar.set_margin_start(22);
        top_bar.set_margin_end(22);
        top_bar.set_margin_top(6);

        let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_box.add_css_class("search");
        search_box.set_size_request(240, 30);
        search_box.set_valign(gtk::Align::Center);
        search_box.set_hexpand(false);
        let magnifier = icon(&["system-search-symbolic", "edit-find-symbolic"]);
        magnifier.add_css_class("secondary");
        search_box.append(&magnifier);
        let search = gtk::Text::new();
        search.set_placeholder_text(Some("Search"));
        search.set_hexpand(true);
        search_box.append(&search);
        let search_clear = gtk::Button::from_icon_name("edit-clear-symbolic");
        search_clear.add_css_class("plain-button");
        search_clear.set_focusable(false);
        search_clear.set_visible(false);
        search_box.append(&search_clear);
        top_bar.append(&search_box);

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let tabs_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .child(&tabs)
            .valign(gtk::Align::Center)
            .build();
        top_bar.append(&tabs_scroller);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        top_bar.append(&spacer);

        let library_notice = Notice::new(
            &["dialog-warning-symbolic"],
            "Insert could not read its library and started empty.",
            "OK",
            "tray.dismiss-library-notice",
        );
        let capture_notice = Notice::new(
            &["dialog-warning-symbolic"],
            "This desktop does not let Insert watch the clipboard.",
            "OK",
            "tray.dismiss-capture-notice",
        );
        let pause_notice = Notice::new(
            &["media-playback-pause-symbolic"],
            "Capture is paused",
            "Resume",
            "tray.resume",
        );
        let paste_notice = Notice::new(
            &["dialog-warning-symbolic"],
            "Copy only. Allow Insert to send keys to paste directly.",
            "Allow…",
            "tray.allow-paste",
        );
        for notice in [&library_notice, &capture_notice, &pause_notice, &paste_notice] {
            top_bar.append(&notice.root);
        }

        let filter_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        filter_content.append(&icon(&[
            "view-filter-symbolic",
            "funnel-symbolic",
            "view-list-symbolic",
        ]));
        let filter_label = gtk::Label::new(None);
        filter_label.add_css_class("filter-label");
        filter_content.append(&filter_label);
        let filter_button = gtk::MenuButton::builder()
            .child(&filter_content)
            .menu_model(&menus::filter_menu())
            .tooltip_text("Filter by type")
            .valign(gtk::Align::Center)
            .focus_on_click(false)
            .build();
        filter_button.add_css_class("bar-button");
        top_bar.append(&filter_button);

        let more_button = gtk::MenuButton::builder()
            .icon_name("view-more-horizontal-symbolic")
            .always_show_arrow(false)
            .valign(gtk::Align::Center)
            .focus_on_click(false)
            .build();
        more_button.add_css_class("bar-button");
        top_bar.append(&more_button);
        panel.append(&top_bar);

        let ids = gtk::StringList::new(&[]);
        let factory = gtk::SignalListItemFactory::new();
        let list = gtk::ListView::new(Some(gtk::NoSelection::new(Some(ids.clone()))), Some(factory.clone()));
        list.set_orientation(gtk::Orientation::Horizontal);
        list.set_can_focus(false);
        list.set_focusable(false);
        list.add_css_class("strip");
        let list_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&list)
            .build();

        let empty = gtk::Box::new(gtk::Orientation::Vertical, 8);
        empty.set_valign(gtk::Align::Center);
        empty.set_margin_bottom(16);
        let empty_icon = gtk::Image::new();
        empty_icon.set_pixel_size(34);
        empty_icon.add_css_class("tertiary");
        let empty_title = gtk::Label::new(None);
        empty_title.add_css_class("empty-title");
        let empty_detail = gtk::Label::new(None);
        empty_detail.add_css_class("empty-detail");
        empty.append(&empty_icon);
        empty.append(&empty_title);
        empty.append(&empty_detail);

        let strip = gtk::Stack::new();
        strip.set_size_request(-1, STRIP_HEIGHT);
        strip.add_named(&list_scroller, Some("cards"));
        strip.add_named(&empty, Some("empty"));
        panel.append(&strip);

        let prompt_title = gtk::Label::new(None);
        prompt_title.add_css_class("prompt-title");
        prompt_title.set_xalign(0.0);
        let prompt_entry = gtk::Entry::new();
        prompt_entry.set_placeholder_text(Some("Name"));
        prompt_entry.add_css_class("prompt-entry");
        let hint = gtk::Label::new(Some("Press Return to save. Press Esc to cancel."));
        hint.add_css_class("prompt-hint");
        hint.set_xalign(0.0);
        let prompt_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
        prompt_box.add_css_class("prompt");
        prompt_box.set_size_request(320, -1);
        prompt_box.set_halign(gtk::Align::Center);
        prompt_box.set_valign(gtk::Align::Center);
        prompt_box.append(&prompt_title);
        prompt_box.append(&prompt_entry);
        prompt_box.append(&hint);
        let dim = gtk::Box::new(gtk::Orientation::Vertical, 0);
        dim.add_css_class("prompt-dim");
        dim.append(&prompt_box);
        prompt_box.set_vexpand(true);
        let prompt_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .transition_duration(MODE_MS)
            .child(&dim)
            .build();
        prompt_revealer.set_can_target(false);

        let panel_overlay = gtk::Overlay::new();
        panel_overlay.set_child(Some(&panel));
        panel_overlay.add_overlay(&prompt_revealer);
        panel_overlay.set_margin_top(24);

        let revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideUp)
            .transition_duration(SHOW_MS)
            .child(&panel_overlay)
            .vexpand(false)
            .build();
        root.append(&revealer);
        window.set_child(Some(&root));

        let tray = Self {
            window,
            model: RefCell::new(TrayModel::new()),
            rendering: Cell::new(false),
            composing: Cell::new(false),
            revealer,
            panel,
            preview_revealer,
            preview_frame,
            preview_slot,
            previewed: Cell::new(None),
            search,
            search_clear,
            tabs,
            tabs_shown: RefCell::new(None),
            pause_notice,
            paste_notice,
            library_notice,
            capture_notice,
            filter_button,
            filter_label,
            more_button,
            more_paused: Cell::new(None),
            strip,
            list,
            ids,
            empty_icon,
            empty_title,
            empty_detail,
            prompt_revealer,
            prompt_title,
            prompt_entry,
            cards: RefCell::new(HashMap::new()),
            bound: RefCell::new(HashMap::new()),
            appearing: RefCell::new(HashSet::new()),
            actions: gio::SimpleActionGroup::new(),
            menu_pinboard: Cell::new(None),
            menu_open: Cell::new(false),
            app: app.clone(),
        };
        tray.connect(app, &backdrop, &factory);
        tray
    }

    fn connect(&self, app: &Weak<App>, backdrop: &gtk::Box, factory: &gtk::SignalListItemFactory) {
        let click = gtk::GestureClick::new();
        click.connect_released(with_app!(app => app, |_, _, _, _| app.hide_tray(None)));
        backdrop.add_controller(click);

        // A menu popup takes the keyboard, and the window reports itself inactive while it is
        // open. Focus loss counts only when no menu is open.
        self.window
            .connect_is_active_notify(with_app!(app => app, |_| app.hide_if_focus_lost()));
        for button in [&self.filter_button, &self.more_button] {
            button.connect_active_notify(with_app!(app => app, |button| {
                app.tray.set_menu_open(button.is_active());
            }));
        }
        self.window.connect_close_request(with_app!(app => app or glib::Propagation::Stop, |_| {
            app.hide_tray(None);
            glib::Propagation::Stop
        }));
        self.revealer.connect_child_revealed_notify(with_app!(app => app, |revealer| {
            if !revealer.is_child_revealed() {
                app.finish_hide();
            }
        }));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(with_app!(app => app or glib::Propagation::Proceed, |_, keyval, _, state| {
            match keys::translate(keyval, state) {
                Some((key, mods)) if app.handle_key(key, mods) => glib::Propagation::Stop,
                _ => glib::Propagation::Proceed,
            }
        }));
        keys.connect_modifiers(with_app!(app => app or glib::Propagation::Proceed, |_, state| {
            app.set_ctrl_held(state.contains(gdk::ModifierType::CONTROL_MASK));
            glib::Propagation::Proceed
        }));
        self.window.add_controller(keys);

        self.search.connect_changed(with_app!(app => app, |text| {
            if !app.tray.rendering.get() {
                app.set_query(text.text().to_string());
            }
        }));
        self.search.connect_preedit_changed(with_app!(app => app, |_, preedit| {
            app.tray.composing.set(!preedit.is_empty());
        }));
        self.search_clear
            .connect_clicked(with_app!(app => app, |_| app.set_query(String::new())));
        self.prompt_entry
            .connect_activate(with_app!(app => app, |entry| app.commit_name(&entry.text())));
        if let Some(text) = self.prompt_entry.delegate().and_downcast::<gtk::Text>() {
            text.connect_preedit_changed(with_app!(app => app, |_, preedit| {
                app.tray.composing.set(!preedit.is_empty());
            }));
        }

        factory.connect_bind(with_app!(app => app, |_, item| {
            app.bind_card(item.downcast_ref::<gtk::ListItem>().expect("list item"));
        }));
        factory.connect_unbind(with_app!(app => app, |_, item| {
            app.tray
                .unbind_card(item.downcast_ref::<gtk::ListItem>().expect("list item"));
        }));

        self.window.insert_action_group("tray", Some(&self.actions));
    }

    pub fn add_action(&self, action: &gio::SimpleAction) {
        self.actions.add_action(action);
    }

    pub fn action(&self, name: &str) -> Option<gio::SimpleAction> {
        self.actions.lookup_action(name).and_downcast()
    }

    pub fn present(&self, activation_token: Option<&str>) {
        if let Some(token) = activation_token {
            self.window.set_startup_id(token);
        }
        self.window.fullscreen();
        self.window.present();
        self.panel.add_css_class("shown");
        self.revealer.set_transition_duration(SHOW_MS);
        self.revealer.set_reveal_child(true);
        self.focus_field();
    }

    /// Starts the hide animation. The app's `finish_hide` runs when it ends, at once if the
    /// window is not mapped.
    pub fn start_hide(&self) {
        self.panel.remove_css_class("shown");
        self.revealer.set_transition_duration(HIDE_MS);
        self.revealer.set_reveal_child(false);
    }

    pub fn focus_field(&self) {
        match self.model.borrow().state {
            TrayState::Naming(_) => self.prompt_entry.grab_focus_without_selecting(),
            _ => self.search.grab_focus_without_selecting(),
        };
    }

    /// Test hook: types into the focused text field, as the keyboard would.
    pub fn type_text(&self, text: &str) {
        if let Some(editable) = self.focused_editable() {
            let mut position = editable.position();
            editable.insert_text(text, &mut position);
            editable.set_position(position);
        }
    }

    /// Test hook: what the focused field does with a key that ran no command.
    pub fn edit_field(&self, key: Key) {
        let Some(editable) = self.focused_editable() else { return };
        match key {
            Key::Backspace => {
                let position = editable.position();
                if position > 0 {
                    editable.delete_text(position - 1, position);
                }
            }
            Key::Space => self.type_text(" "),
            Key::Return => editable.emit_by_name::<()>("activate", &[]),
            _ => {}
        }
    }

    fn focused_editable(&self) -> Option<gtk::Editable> {
        GtkWindowExt::focus(&self.window).and_downcast::<gtk::Editable>()
    }

    /// After a menu closes, focus comes back to the tray unless the user clicked another app.
    pub fn set_menu_open(&self, open: bool) {
        self.menu_open.set(open);
        if !open {
            let app = self.app.clone();
            glib::timeout_add_local_once(MENU_FOCUS_GRACE, move || {
                if let Some(app) = app.upgrade() {
                    app.hide_if_focus_lost();
                }
            });
        }
    }

    pub fn popup_filter_menu(&self) {
        self.filter_button.popup();
    }

    pub fn is_menu_open(&self) -> bool {
        self.menu_open.get()
    }

    pub fn is_composing(&self) -> bool {
        self.composing.get()
    }

    /// Brings every widget in line with the model and the library.
    pub fn render(&self, library: &Library, notices: Notices) {
        self.rendering.set(true);
        let model = self.model.borrow();

        if self.search.text() != model.query {
            self.search.set_text(&model.query);
            self.search.set_position(-1);
        }
        self.search_clear.set_visible(!model.query.is_empty());

        self.render_tabs(library, model.scope);
        self.pause_notice.root.set_visible(notices.paused);
        self.paste_notice.root.set_visible(notices.needs_paste_permission);
        self.library_notice.root.set_visible(notices.library_unreadable);
        self.capture_notice.root.set_visible(notices.capture_unavailable);
        match model.kind {
            Some(kind) => {
                self.filter_label.set_text(kind.name());
                self.filter_label.set_visible(true);
            }
            None => self.filter_label.set_visible(false),
        }
        set_class(&self.filter_button, "active", model.kind.is_some());
        if self.more_paused.get() != Some(notices.paused) {
            self.more_paused.set(Some(notices.paused));
            self.more_button.set_menu_model(Some(&menus::more_menu(notices.paused)));
        }

        let shown_ids: Vec<String> = (0..self.ids.n_items())
            .filter_map(|i| self.ids.string(i).map(|s| s.to_string()))
            .collect();
        let wanted: Vec<String> = model.visible.iter().map(ClipId::to_string).collect();
        drop(model);
        if shown_ids != wanted {
            if !shown_ids.is_empty() {
                let shown: HashSet<ClipId> = shown_ids
                    .iter()
                    .filter_map(|id| uuid::Uuid::parse_str(id).ok().map(ClipId))
                    .collect();
                let mut appearing = self.appearing.borrow_mut();
                appearing.clear();
                appearing.extend(
                    self.model
                        .borrow()
                        .visible
                        .iter()
                        .filter(|id| !shown.contains(id)),
                );
            }
            let refs: Vec<&str> = wanted.iter().map(String::as_str).collect();
            self.ids.splice(0, self.ids.n_items(), &refs);
        }
        let model = self.model.borrow();
        if model.visible.is_empty() {
            let state = empty_state(model.scope, model.kind, &model.query);
            self.empty_icon
                .set_from_gicon(&crate::theme::named_gicon(state.icon_name()));
            self.empty_title.set_text(state.title());
            self.empty_detail.set_text(state.detail());
            self.strip.set_visible_child_name("empty");
        } else {
            self.strip.set_visible_child_name("cards");
        }
        self.restyle(&model, library);
        if let Some(index) = model
            .selection
            .primary()
            .and_then(|primary| model.visible.iter().position(|id| *id == primary))
        {
            self.list.scroll_to(index as u32, gtk::ListScrollFlags::NONE, None);
        }

        let previewing = model.state == TrayState::Previewing;
        let preview_id = model.selection.primary().filter(|_| previewing);
        if preview_id.is_some() && preview_id != self.previewed.get() {
            while let Some(child) = self.preview_slot.first_child() {
                self.preview_slot.remove(&child);
            }
            if let Some(clip) = preview_id.and_then(|id| library.clip(id)) {
                self.preview_slot.append(&preview::build(clip, library));
            }
            self.previewed.set(preview_id);
        }
        if preview_id.is_none() {
            self.previewed.set(None);
        }
        if previewing {
            let screen = self.window.height().max(600);
            let height = (f64::from(screen) * 0.46).min(480.0) as i32;
            self.preview_frame.set_min_content_height(height);
            self.preview_frame.set_max_content_height(height);
        }
        self.preview_revealer.set_reveal_child(previewing);

        let naming = match &model.state {
            TrayState::Naming(target) => Some(target.clone()),
            _ => None,
        };
        if let Some(target) = &naming {
            if !self.prompt_revealer.reveals_child() {
                self.prompt_title.set_text(target.prompt_title());
                self.prompt_entry.set_text(&library.current_name(target));
            }
        }
        self.prompt_revealer.set_reveal_child(naming.is_some());
        self.prompt_revealer.set_can_target(naming.is_some());
        drop(model);
        self.rendering.set(false);
    }

    fn render_tabs(&self, library: &Library, scope: Scope) {
        let key: Vec<(String, String)> = library
            .pinboards()
            .iter()
            .map(|p| (p.name.clone(), p.color.name().to_owned()))
            .collect();
        if self.tabs_shown.borrow().as_ref() == Some(&(scope, key.clone())) {
            return;
        }
        *self.tabs_shown.borrow_mut() = Some((scope, key));
        while let Some(child) = self.tabs.first_child() {
            self.tabs.remove(&child);
        }
        let dark = appearance().dark;
        for tab_scope in library.scopes() {
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let name = match tab_scope {
                Scope::History => "Clipboard".to_owned(),
                Scope::Pinboard(id) => {
                    let pinboard = library.pinboard(id).expect("scopes lists known pinboards");
                    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    dot.add_css_class("dot");
                    dot.add_css_class(&background_class(pinboard.color.rgb(dark)));
                    dot.set_valign(gtk::Align::Center);
                    content.append(&dot);
                    pinboard.name.clone()
                }
            };
            content.append(&gtk::Label::new(Some(&name)));
            let tab = gtk::Button::builder()
                .child(&content)
                .focus_on_click(false)
                .focusable(false)
                .build();
            tab.add_css_class("scope-tab");
            set_class(&tab, "selected", tab_scope == scope);
            let target = match tab_scope {
                Scope::History => "history".to_owned(),
                Scope::Pinboard(id) => id.0.to_string(),
            };
            tab.set_action_name(Some("tray.scope"));
            tab.set_action_target_value(Some(&target.to_variant()));
            if let Scope::Pinboard(id) = tab_scope {
                let right = gtk::GestureClick::new();
                right.set_button(gdk::BUTTON_SECONDARY);
                right.connect_pressed(with_app!(&self.app => app, |gesture, _, x, y| {
                    if let Some(tab) = gesture.widget() {
                        app.open_pinboard_menu(id, &tab, x, y);
                    }
                }));
                tab.add_controller(right);
            }
            self.tabs.append(&tab);
        }
        let plus = gtk::Button::from_icon_name("list-add-symbolic");
        plus.add_css_class("plus-button");
        plus.set_tooltip_text(Some("New pinboard"));
        plus.set_focusable(false);
        plus.set_action_name(Some("tray.new-pinboard"));
        self.tabs.append(&plus);
    }

    /// Puts the selection ring and the number badge on each card that a row shows now.
    pub fn restyle(&self, model: &TrayModel, library: &Library) {
        let cards = self.cards.borrow();
        let now = OffsetDateTime::now_utc();
        for id in self.bound.borrow().keys() {
            let Some(card) = cards.get(id) else { continue };
            let ring = if model.selection.primary() == Some(*id) {
                Ring::Primary
            } else if model.selection.contains(*id) {
                Ring::Secondary
            } else {
                Ring::None
            };
            card.set_ring(ring);
            let index = model.visible.iter().position(|v| v == id);
            card.set_badge(index.filter(|i| model.ctrl_held && *i < 9).map(|i| i + 1));
            if let Some(clip) = library.clip(*id) {
                card.update(clip, library, now);
            }
            if model.deleting.contains(id) {
                card.set_deleting();
            }
        }
    }

    pub fn card_widget(&self, id: ClipId) -> Option<gtk::Box> {
        self.cards.borrow().get(&id).map(|card| card.root.clone())
    }

    pub fn has_card(&self, id: ClipId) -> bool {
        self.cards.borrow().contains_key(&id)
    }

    pub fn insert_card(&self, id: ClipId, card: Card) {
        self.cards.borrow_mut().insert(id, card);
    }

    pub fn drop_cards(&self, ids: &[ClipId]) {
        let mut cards = self.cards.borrow_mut();
        for id in ids {
            cards.remove(id);
        }
    }

    /// Shows the card of `id` in `item`, and takes it from the row that showed it before.
    pub fn attach_card(&self, id: ClipId, item: &gtk::ListItem) {
        let Some(root) = self.card_widget(id) else { return };
        if self.appearing.borrow_mut().remove(&id) {
            root.add_css_class("appear");
            let weak = root.downgrade();
            glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                if let Some(root) = weak.upgrade() {
                    root.remove_css_class("appear");
                }
            });
        }
        let previous = self.bound.borrow_mut().insert(id, item.downgrade());
        if let Some(previous) = previous.and_then(|weak| weak.upgrade()) {
            if &previous != item {
                previous.set_child(gtk::Widget::NONE);
            }
        }
        item.set_activatable(false);
        item.set_selectable(false);
        item.set_focusable(false);
        item.set_child(Some(&root));
    }

    /// A card lives while a row shows it. The list view binds only the rows near the visible
    /// part of the strip, so the cards and their textures stay few.
    fn unbind_card(&self, item: &gtk::ListItem) {
        let Some(id) = item_clip_id(item) else { return };
        let shown_here = {
            let mut bound = self.bound.borrow_mut();
            let shown_here = bound.get(&id).and_then(|weak| weak.upgrade()).as_ref() == Some(item);
            if shown_here {
                bound.remove(&id);
            }
            shown_here
        };
        item.set_child(gtk::Widget::NONE);
        if shown_here {
            self.cards.borrow_mut().remove(&id);
        }
    }

    pub fn popup_menu(&self, model: &gio::Menu, from: &impl IsA<gtk::Widget>, x: f64, y: f64) {
        let point = from
            .compute_point(&self.panel, &gtk::graphene::Point::new(x as f32, y as f32))
            .unwrap_or_else(|| gtk::graphene::Point::new(0.0, 0.0));
        let popover = gtk::PopoverMenu::from_model(Some(model));
        popover.set_parent(&self.panel);
        popover.set_has_arrow(false);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(point.x() as i32, point.y() as i32, 1, 1)));
        popover.connect_closed(with_app!(&self.app => app, |popover| {
            app.tray.set_menu_open(false);
            let popover = popover.clone();
            glib::idle_add_local_once(move || popover.unparent());
        }));
        self.set_menu_open(true);
        popover.popup();
    }
}

pub fn item_clip_id(item: &gtk::ListItem) -> Option<ClipId> {
    let text = item.item().and_downcast::<gtk::StringObject>()?.string();
    uuid::Uuid::parse_str(&text).ok().map(ClipId)
}

