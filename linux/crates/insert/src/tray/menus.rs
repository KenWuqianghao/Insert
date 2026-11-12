use gtk4 as gtk;
use gtk::{gio, prelude::*};
use insert_core::{Clip, ClipKind, Library, Pinboard, PinboardColor};

pub const ALL_KINDS: &str = "all";

pub fn kind_key(kind: Option<ClipKind>) -> String {
    kind.map(|kind| kind.name().to_owned())
        .unwrap_or_else(|| ALL_KINDS.to_owned())
}

pub fn kind_from_key(key: &str) -> Option<ClipKind> {
    ClipKind::ALL.into_iter().find(|kind| kind.name() == key)
}

pub fn filter_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let all = gio::Menu::new();
    all.append(Some("All Types"), Some(&format!("tray.kind::{ALL_KINDS}")));
    menu.append_section(None, &all);
    let kinds = gio::Menu::new();
    for kind in ClipKind::ALL {
        let item = gio::MenuItem::new(Some(kind.name()), None);
        item.set_action_and_target_value(Some("tray.kind"), Some(&kind.name().to_variant()));
        item.set_icon(&gio::ThemedIcon::new(kind.icon_name()));
        kinds.append_item(&item);
    }
    menu.append_section(None, &kinds);
    menu
}

pub fn more_menu(paused: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    let main = gio::Menu::new();
    main.append(Some("Settings…"), Some("app.settings"));
    main.append(
        Some(if paused { "Resume Capture" } else { "Pause Capture" }),
        Some("app.toggle-pause"),
    );
    main.append(Some("Clear History…"), Some("app.clear-history"));
    menu.append_section(None, &main);
    let quit = gio::Menu::new();
    quit.append(Some("Quit Insert"), Some("app.quit"));
    menu.append_section(None, &quit);
    menu
}

pub fn clip_menu(clip: &Clip, library: &Library) -> gio::Menu {
    let id = clip.id.to_string();
    let menu = gio::Menu::new();
    let paste = gio::Menu::new();
    paste.append(Some("Paste"), Some(&format!("tray.paste::{id}")));
    paste.append(Some("Paste as Plain Text"), Some(&format!("tray.paste-plain::{id}")));
    paste.append(Some("Copy"), Some(&format!("tray.copy::{id}")));
    menu.append_section(None, &paste);

    let organize = gio::Menu::new();
    let pin_to = gio::Menu::new();
    let boards = gio::Menu::new();
    for pinboard in library.pinboards() {
        boards.append(
            Some(&pinboard.name),
            Some(&format!("tray.pin-to::{}", pinboard.id.0)),
        );
    }
    pin_to.append_section(None, &boards);
    let new_board = gio::Menu::new();
    new_board.append(Some("New Pinboard…"), Some("tray.pin-new"));
    pin_to.append_section(None, &new_board);
    organize.append_submenu(Some("Pin to"), &pin_to);
    if clip.pinboard_id.is_some() {
        organize.append(Some("Unpin"), Some("tray.unpin"));
    }
    organize.append(Some("Rename…"), Some(&format!("tray.rename-clip::{id}")));
    organize.append(Some("Preview"), Some(&format!("tray.preview::{id}")));
    menu.append_section(None, &organize);

    let delete = gio::Menu::new();
    delete.append(Some("Delete"), Some("tray.delete"));
    menu.append_section(None, &delete);
    menu
}

pub fn pinboard_menu(pinboard: &Pinboard) -> gio::Menu {
    let menu = gio::Menu::new();
    let main = gio::Menu::new();
    main.append(
        Some("Rename…"),
        Some(&format!("tray.rename-pinboard::{}", pinboard.id.0)),
    );
    let colors = gio::Menu::new();
    for color in PinboardColor::ALL {
        colors.append(
            Some(color.name()),
            Some(&format!("tray.pinboard-color::{}", color.name())),
        );
    }
    main.append_submenu(Some("Color"), &colors);
    menu.append_section(None, &main);
    let delete = gio::Menu::new();
    delete.append(
        Some("Delete Pinboard"),
        Some(&format!("tray.delete-pinboard::{}", pinboard.id.0)),
    );
    menu.append_section(None, &delete);
    menu
}

pub fn color_from_name(name: &str) -> Option<PinboardColor> {
    PinboardColor::ALL.into_iter().find(|color| color.name() == name)
}
