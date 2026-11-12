use std::cell::RefCell;
use std::collections::BTreeSet;

use gtk4 as gtk;
use gtk::{gdk, gio, glib, prelude::*};
use insert_core::Rgb;

const DEFAULT_ACCENT: Rgb = Rgb { r: 0x35, g: 0x84, b: 0xE4 };

#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    pub dark: bool,
    pub accent: Rgb,
}

thread_local! {
    static APPEARANCE: RefCell<Appearance> = const {
        RefCell::new(Appearance { dark: false, accent: DEFAULT_ACCENT })
    };
    static COLOR_CLASSES: RefCell<(gtk::CssProvider, BTreeSet<String>)> =
        RefCell::new((gtk::CssProvider::new(), BTreeSet::new()));
}

pub fn appearance() -> Appearance {
    APPEARANCE.with(|a| *a.borrow())
}

pub fn install(display: &gdk::Display) {
    let appearance = read_appearance();
    APPEARANCE.with(|a| *a.borrow_mut() = appearance);
    gtk::Settings::for_display(display).set_gtk_application_prefer_dark_theme(appearance.dark);

    let provider = gtk::CssProvider::new();
    provider.load_from_string(&stylesheet(appearance));
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    COLOR_CLASSES.with(|classes| {
        gtk::style_context_add_provider_for_display(
            display,
            &classes.borrow().0,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

pub fn background_class(rgb: Rgb) -> String {
    let name = format!("bg-{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b);
    COLOR_CLASSES.with(|classes| {
        let (provider, names) = &mut *classes.borrow_mut();
        if names.insert(name.clone()) {
            let css: String = names
                .iter()
                .map(|n| format!(".{n} {{ background-color: #{}; }}\n", &n[3..]))
                .collect();
            provider.load_from_string(&css);
        }
    });
    name
}

pub fn icon(names: &[&str]) -> gtk::Image {
    gtk::Image::from_gicon(&gio::ThemedIcon::from_names(names))
}

/// Breeze names for the Adwaita icon names that Breeze does not have.
const ALTERNATES: &[(&str, &[&str])] = &[
    ("view-pin-symbolic", &["pin-symbolic", "window-pin-symbolic"]),
    ("format-text-rich-symbolic", &["format-text-bold-symbolic"]),
    ("preferences-system-privacy-symbolic", &["security-high-symbolic"]),
];

pub fn named_gicon(name: &str) -> gio::ThemedIcon {
    let mut names = vec![name];
    if let Some((_, alternates)) = ALTERNATES.iter().find(|(n, _)| *n == name) {
        names.extend_from_slice(alternates);
    }
    gio::ThemedIcon::from_names(&names)
}

pub fn named_icon(name: &str) -> gtk::Image {
    gtk::Image::from_gicon(&named_gicon(name))
}

fn read_appearance() -> Appearance {
    let scheme = read_portal_setting("color-scheme").and_then(|v| v.get::<u32>());
    let accent = read_portal_setting("accent-color")
        .and_then(|v| v.get::<(f64, f64, f64)>())
        .filter(|(r, g, b)| [r, g, b].iter().all(|c| (0.0..=1.0).contains(*c)))
        .map(|(r, g, b)| Rgb {
            r: (r * 255.0).round() as u8,
            g: (g * 255.0).round() as u8,
            b: (b * 255.0).round() as u8,
        });
    Appearance {
        dark: scheme == Some(1),
        accent: accent.unwrap_or(DEFAULT_ACCENT),
    }
}

fn read_portal_setting(key: &str) -> Option<glib::Variant> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let reply = bus
        .call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            Some(&("org.freedesktop.appearance", key).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let mut value = reply.child_value(0);
    while value.type_() == glib::VariantTy::VARIANT {
        value = value.as_variant()?;
    }
    Some(value)
}

fn stylesheet(appearance: Appearance) -> String {
    let Rgb { r, g, b } = appearance.accent;
    let (fg, tray, card) = if appearance.dark {
        ("255,255,255", "rgba(40,40,42,0.94)", "#2B2B2B")
    } else {
        ("0,0,0", "rgba(242,242,245,0.94)", "#FFFFFF")
    };
    format!(
        "@define-color accent rgb({r},{g},{b});\n\
         @define-color fg rgb({fg});\n\
         @define-color card {card};\n\
         @define-color tray {tray};\n\
         {}",
        include_str!("style.css")
    )
}
