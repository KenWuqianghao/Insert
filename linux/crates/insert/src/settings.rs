use std::cell::Cell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::{gio, glib, prelude::*};
use insert_core::{HistoryLimit, SourceApp};

use crate::app::App;
use crate::appicon::{desktop_app, desktop_id};
use crate::services::{PasteStatus, Shortcut};
use crate::theme::icon;
use crate::with_app;

const PANES: [(&str, &str, &[&str]); 2] = [
    ("general", "General", &["emblem-system-symbolic", "preferences-system-symbolic"]),
    ("privacy", "Privacy", &["preferences-system-privacy-symbolic", "security-high-symbolic"]),
];

pub struct SettingsWindow {
    pub window: gtk::Window,
    updating: Cell<bool>,
    login: gtk::Switch,
    login_warning: Warning,
    tray_icon: gtk::Switch,
    shortcut_value: gtk::Label,
    shortcut_change: gtk::Button,
    shortcut_row: Row,
    paste: gtk::Switch,
    paste_warning: Warning,
    paste_allow: gtk::Button,
    limit: gtk::DropDown,
    ignored_rows: gtk::Box,
    ignored_warning: Warning,
    pause: gtk::Switch,
}

impl SettingsWindow {
    pub fn new(app: &Rc<App>) -> Rc<Self> {
        let weak = Rc::downgrade(app);
        let window = gtk::Window::builder()
            .application(&app.gtk_app)
            .title("General")
            .resizable(false)
            .hide_on_close(true)
            .build();
        window.add_css_class("settings");

        let stack = gtk::Stack::new();
        // A pane switch that changes the height of this fixed-size window makes it wider on
        // KWin. Both panes keep the height of the taller one.
        stack.set_vhomogeneous(true);
        let switcher = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        for (name, title, icon_names) in PANES {
            let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let image = icon(icon_names);
            image.set_pixel_size(20);
            content.append(&image);
            content.append(&gtk::Label::new(Some(title)));
            let button = gtk::ToggleButton::builder()
                .child(&content)
                .action_name("settings.pane")
                .action_target(&name.to_variant())
                .build();
            button.add_css_class("pane-button");
            switcher.append(&button);
        }
        let pane = gio::SimpleAction::new_stateful(
            "pane",
            Some(glib::VariantTy::STRING),
            &PANES[0].0.to_variant(),
        );
        let stack_ref = stack.clone();
        let window_ref = window.downgrade();
        pane.connect_activate(move |action, param| {
            let Some(name) = param.and_then(|p| p.str()) else { return };
            let Some((_, title, _)) = PANES.iter().find(|(n, _, _)| *n == name) else { return };
            action.set_state(&name.to_variant());
            stack_ref.set_visible_child_name(name);
            if let Some(window) = window_ref.upgrade() {
                window.set_title(Some(title));
            }
        });
        let actions = gio::SimpleActionGroup::new();
        actions.add_action(&pane);
        window.insert_action_group("settings", Some(&actions));
        let header = gtk::HeaderBar::new();
        header.set_decoration_layout(Some(":close"));
        header.set_title_widget(Some(&switcher));
        window.set_titlebar(Some(&header));

        let login = gtk::Switch::new();
        let login_warning = Warning::new("");
        let tray_icon = gtk::Switch::new();
        let shortcut_value = gtk::Label::new(None);
        shortcut_value.add_css_class("keycap");
        let shortcut_change = gtk::Button::with_label("Change…");
        let shortcut_controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        shortcut_controls.append(&shortcut_value);
        shortcut_controls.append(&shortcut_change);
        let shortcut_row = Row::new("Shortcut to open Insert", Some(""), &shortcut_controls);
        let paste = gtk::Switch::new();
        let paste_warning = Warning::new("");
        let paste_allow = gtk::Button::with_label("Allow…");
        paste_warning.root.append(&paste_allow);
        let labels: Vec<String> = HistoryLimit::ALL.iter().map(|l| l.label()).collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let limit = gtk::DropDown::from_strings(&label_refs);

        let general = page();
        general.append(&group_of(&[
            Row::new("Open Insert at login", None, &login).root,
            login_warning.root.clone().upcast(),
            Row::new("Show the icon in the system tray", None, &tray_icon).root,
        ]));
        general.append(&group_of(&[shortcut_row.root.clone()]));
        general.append(&group_of(&[
            Row::new(
                "Paste directly into the active app",
                Some("When this is off, Insert only copies the clip."),
                &paste,
            )
            .root,
            paste_warning.root.clone().upcast(),
        ]));
        general.append(&group_of(&[Row::new(
            "Keep in history",
            Some("Insert deletes the oldest clips first. Pinned clips do not count."),
            &limit,
        )
        .root]));

        let add_app = gtk::Button::with_label("Add App…");
        let ignored_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let ignored_warning = Warning::new(
            "This desktop does not tell Insert which app is in front. Ignored apps need KDE Plasma.",
        );
        let pause = gtk::Switch::new();
        let clear = gtk::Button::with_label("Clear History…");
        let privacy = page();
        privacy.append(&group_of(&[
            Row::new(
                "Ignored apps",
                Some("Insert does not save a copy that you make while one of these apps is in front."),
                &add_app,
            )
            .root,
            ignored_warning.root.clone().upcast(),
            ignored_rows.clone().upcast(),
        ]));
        let footer = gtk::Label::new(Some(
            "Insert always skips items that an app marks as concealed, such as passwords from a password manager.",
        ));
        footer.add_css_class("group-footer");
        footer.set_wrap(true);
        footer.set_max_width_chars(1);
        footer.set_xalign(0.0);
        privacy.append(&footer);
        privacy.append(&group_of(&[Row::new(
            "Pause capture",
            Some("Insert saves no new clips while capture is paused."),
            &pause,
        )
        .root]));
        privacy.append(&group_of(&[Row::new(
            "History",
            Some("Delete all clips that are not pinned."),
            &clear,
        )
        .root]));

        stack.add_named(&general, Some("general"));
        stack.add_named(&privacy, Some("privacy"));
        stack.set_size_request(500, -1);
        window.set_child(Some(&stack));

        let settings = Rc::new(Self {
            window,
            updating: Cell::new(false),
            login,
            login_warning,
            tray_icon,
            shortcut_value,
            shortcut_change,
            shortcut_row,
            paste,
            paste_warning,
            paste_allow,
            limit,
            ignored_rows,
            ignored_warning,
            pause,
        });

        let this = Rc::downgrade(&settings);
        settings.login.connect_active_notify(with_app!(&weak => app, |switch| {
            let Some(this) = this.upgrade() else { return };
            if this.updating.get() {
                return;
            }
            match app.set_launch_at_login(switch.is_active()) {
                Ok(()) => this.login_warning.root.set_visible(false),
                Err(text) => this.login_warning.show(&text),
            }
        }));
        let this = Rc::downgrade(&settings);
        settings.tray_icon.connect_active_notify(with_app!(&weak => app, |switch| {
            if this.upgrade().is_some_and(|this| !this.updating.get()) {
                app.set_show_tray_icon(switch.is_active());
            }
        }));
        settings
            .shortcut_change
            .connect_clicked(with_app!(&weak => app, |_| app.services.change_shortcut()));
        let this = Rc::downgrade(&settings);
        settings.paste.connect_active_notify(with_app!(&weak => app, |switch| {
            if this.upgrade().is_some_and(|this| !this.updating.get()) {
                app.set_paste_directly(switch.is_active());
            }
        }));
        settings
            .paste_allow
            .connect_clicked(with_app!(&weak => app, |_| app.services.request_paste_permission()));
        let this = Rc::downgrade(&settings);
        settings.limit.connect_selected_notify(with_app!(&weak => app, |dropdown| {
            if this.upgrade().is_some_and(|this| !this.updating.get()) {
                if let Some(limit) = HistoryLimit::ALL.get(dropdown.selected() as usize) {
                    app.set_history_limit(*limit);
                }
            }
        }));
        let this = Rc::downgrade(&settings);
        settings.pause.connect_active_notify(with_app!(&weak => app, |switch| {
            if this.upgrade().is_some_and(|this| !this.updating.get()) {
                app.set_paused(switch.is_active());
            }
        }));
        add_app.connect_clicked(with_app!(&weak => app, |button| {
            let parent = button.root().and_downcast::<gtk::Window>();
            show_app_picker(app, parent.as_ref());
        }));
        clear.connect_clicked(with_app!(&weak => app, |_| app.confirm_clear_history()));

        let shortcuts = gtk::ShortcutController::new();
        shortcuts.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<Control>w"),
            Some(gtk::NamedAction::new("window.close")),
        ));
        shortcuts.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<Control>q"),
            Some(gtk::NamedAction::new("app.quit")),
        ));
        settings.window.add_controller(shortcuts);
        settings
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn refresh(&self, app: &Rc<App>) {
        self.updating.set(true);
        let prefs = app.prefs.borrow();
        self.login.set_active(app.services.launch_at_login());
        self.tray_icon.set_active(prefs.show_tray_icon);
        self.paste.set_active(prefs.paste_directly);
        self.pause.set_active(prefs.capture_paused);
        let index = HistoryLimit::ALL
            .iter()
            .position(|limit| *limit == prefs.history_limit)
            .unwrap_or(1);
        self.limit.set_selected(index as u32);

        match &*app.shortcut.borrow() {
            Shortcut::Registered { trigger, can_change } => {
                self.shortcut_value
                    .set_text(trigger.as_deref().unwrap_or("Not set"));
                self.shortcut_value.set_visible(true);
                self.shortcut_change.set_visible(*can_change);
                self.shortcut_row.set_description(if *can_change {
                    None
                } else {
                    Some("Change it in System Settings > Keyboard > Shortcuts.")
                });
            }
            Shortcut::Unavailable => {
                self.shortcut_value.set_visible(false);
                self.shortcut_change.set_visible(false);
                self.shortcut_row.set_description(Some(
                    "This desktop has no shortcut service. Bind the command \"insert --toggle\" to a key in the desktop settings.",
                ));
            }
        }

        match (prefs.paste_directly, app.paste_status.get()) {
            (true, PasteStatus::NeedsPermission) => {
                self.paste_warning
                    .show("Insert needs your permission to send keys to paste.");
                self.paste_allow.set_visible(true);
            }
            (true, PasteStatus::Unavailable) => {
                self.paste_warning
                    .show("This desktop does not let Insert send keys. Insert only copies the clip.");
                self.paste_allow.set_visible(false);
            }
            _ => self.paste_warning.root.set_visible(false),
        }

        self.ignored_warning
            .root
            .set_visible(!app.services.tracks_source_app());
        while let Some(child) = self.ignored_rows.first_child() {
            self.ignored_rows.remove(&child);
        }
        for ignored in &prefs.ignored_apps {
            self.ignored_rows.append(&ignored_row(app, ignored));
        }
        self.updating.set(false);
    }
}

struct Row {
    root: gtk::Widget,
    description: gtk::Label,
}

impl Row {
    fn new(title: &str, description: Option<&str>, control: &impl IsA<gtk::Widget>) -> Self {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.add_css_class("setting-row");
        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_hexpand(true);
        labels.set_valign(gtk::Align::Center);
        let title_label = gtk::Label::new(Some(title));
        title_label.set_xalign(0.0);
        title_label.add_css_class("row-title");
        let description_label = gtk::Label::new(description);
        description_label.set_xalign(0.0);
        description_label.set_wrap(true);
        description_label.set_max_width_chars(1);
        description_label.add_css_class("row-description");
        description_label.set_visible(description.is_some_and(|d| !d.is_empty()));
        labels.append(&title_label);
        labels.append(&description_label);
        row.append(&labels);
        control.set_valign(gtk::Align::Center);
        row.append(control);
        Self {
            root: row.upcast(),
            description: description_label,
        }
    }

    fn set_description(&self, text: Option<&str>) {
        self.description.set_text(text.unwrap_or_default());
        self.description.set_visible(text.is_some());
    }
}

struct Warning {
    root: gtk::Box,
    text: gtk::Label,
}

impl Warning {
    fn new(text: &str) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        root.add_css_class("setting-row");
        root.add_css_class("setting-warning");
        root.append(&icon(&["dialog-warning-symbolic"]));
        let label = gtk::Label::new(Some(text));
        label.set_wrap(true);
        label.set_max_width_chars(1);
        label.set_xalign(0.0);
        label.set_hexpand(true);
        root.append(&label);
        root.set_visible(false);
        Self { root, text: label }
    }

    fn show(&self, text: &str) {
        self.text.set_text(text);
        self.root.set_visible(true);
    }
}

fn page() -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 16);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_margin_top(20);
    page.set_margin_bottom(20);
    page
}

fn group_of(rows: &[gtk::Widget]) -> gtk::Box {
    let group = gtk::Box::new(gtk::Orientation::Vertical, 0);
    group.add_css_class("group");
    for row in rows {
        group.append(row);
    }
    group
}

fn ignored_row(app: &Rc<App>, ignored: &SourceApp) -> gtk::Widget {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("setting-row");
    if let Some(app_icon) = desktop_app(&ignored.id).and_then(|info| info.icon()) {
        let image = gtk::Image::from_gicon(&app_icon);
        image.set_pixel_size(20);
        row.append(&image);
    }
    let name = gtk::Label::new(Some(&ignored.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    row.append(&name);
    let remove = gtk::Button::from_icon_name("list-remove-symbolic");
    remove.add_css_class("flat");
    remove.set_tooltip_text(Some(&format!("Remove {}", ignored.name)));
    let id = ignored.id.clone();
    let weak = Rc::downgrade(app);
    remove.connect_clicked(move |_| {
        if let Some(app) = weak.upgrade() {
            let id = id.clone();
            glib::idle_add_local_once(move || app.unignore_app(&id));
        }
    });
    row.append(&remove);
    row.upcast()
}

fn show_app_picker(app: &Rc<App>, parent: Option<&gtk::Window>) {
    let dialog = gtk::Window::builder()
        .title("Add Ignored Apps")
        .modal(true)
        .default_width(380)
        .default_height(480)
        .build();
    dialog.set_transient_for(parent);
    let search = gtk::SearchEntry::new();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Multiple);
    let mut apps: Vec<gio::AppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter(|info| info.should_show())
        .collect();
    apps.sort_by_key(|info| info.display_name().to_lowercase());
    let mut entries = Vec::new();
    for info in apps {
        let Some(id) = desktop_id(&info) else {
            continue;
        };
        if app.prefs.borrow().ignores_id(&id) {
            continue;
        }
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_start(8);
        row.set_margin_end(8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        if let Some(app_icon) = info.icon() {
            let image = gtk::Image::from_gicon(&app_icon);
            image.set_pixel_size(24);
            row.append(&image);
        }
        let name = info.display_name().to_string();
        row.append(&gtk::Label::new(Some(&name)));
        let list_row = gtk::ListBoxRow::builder().child(&row).build();
        list.append(&list_row);
        entries.push((list_row, SourceApp { id, name }));
    }
    let entries = Rc::new(entries);
    {
        let entries = entries.clone();
        list.set_filter_func(glib::clone!(
            #[weak]
            search,
            #[upgrade_or]
            true,
            move |row| {
                let query = search.text().to_lowercase();
                entries
                    .iter()
                    .find(|(r, _)| r == row)
                    .is_none_or(|(_, app)| app.name.to_lowercase().contains(&query))
            }
        ));
    }
    search.connect_search_changed(glib::clone!(
        #[weak]
        list,
        move |_| list.invalidate_filter()
    ));

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .child(&list)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let ignore = gtk::Button::with_label("Ignore");
    ignore.add_css_class("suggested-action");
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.append(&cancel);
    buttons.append(&ignore);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    column.append(&search);
    column.append(&scroller);
    column.append(&buttons);
    dialog.set_child(Some(&column));

    cancel.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| dialog.close()
    ));
    let weak = Rc::downgrade(app);
    ignore.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        list,
        move |_| {
            let chosen: Vec<SourceApp> = list
                .selected_rows()
                .iter()
                .filter_map(|row| entries.iter().find(|(r, _)| r == row))
                .map(|(_, app)| app.clone())
                .collect();
            if let Some(app) = weak.upgrade() {
                app.ignore_apps(chosen);
            }
            dialog.close();
        }
    ));
    dialog.present();
}
