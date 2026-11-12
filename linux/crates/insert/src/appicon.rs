//! The icon and header tint of a clip's source app, resolved one time per app id.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk4 as gtk;
use gtk::{gdk, gdk_pixbuf::Pixbuf, gio, prelude::*};
use insert_core::{app_id_key, header_tint, Rgb, SourceApp, NEUTRAL_TINT};

#[derive(Clone)]
pub struct AppVisual {
    pub tint: Rgb,
    pub icon: Option<gio::Icon>,
}

const NEUTRAL: AppVisual = AppVisual {
    tint: NEUTRAL_TINT,
    icon: None,
};

thread_local! {
    static CACHE: RefCell<HashMap<String, AppVisual>> = RefCell::new(HashMap::new());
}

pub fn app_visual(source: Option<&SourceApp>) -> AppVisual {
    let Some(source) = source else {
        return NEUTRAL;
    };
    if let Some(found) = CACHE.with(|cache| cache.borrow().get(&source.id).cloned()) {
        return found;
    }
    let visual = resolve(&source.id).unwrap_or(NEUTRAL);
    CACHE.with(|cache| cache.borrow_mut().insert(source.id.clone(), visual.clone()));
    visual
}

/// The installed app for a desktop file id.
pub fn desktop_app(id: &str) -> Option<gio::AppInfo> {
    let key = app_id_key(id);
    gio::AppInfo::all()
        .into_iter()
        .find(|info| desktop_id(info).is_some_and(|found| app_id_key(&found) == key))
}

/// The desktop file id of an installed app. GIO adds the `.desktop` file suffix to the id.
pub fn desktop_id(info: &gio::AppInfo) -> Option<String> {
    info.id().map(|id| strip_desktop_suffix(&id).to_owned())
}

fn strip_desktop_suffix(file_name: &str) -> &str {
    file_name.strip_suffix(".desktop").unwrap_or(file_name)
}

fn resolve(id: &str) -> Option<AppVisual> {
    let icon = desktop_app(id)?.icon()?;
    let tint = icon_pixels(&icon)
        .map(|pixels| header_tint(&pixels))
        .unwrap_or(NEUTRAL_TINT);
    Some(AppVisual {
        tint,
        icon: Some(icon),
    })
}

/// The icon as straight RGBA at 24 × 24.
fn icon_pixels(icon: &gio::Icon) -> Option<Vec<u8>> {
    let display = gdk::Display::default()?;
    let paintable = gtk::IconTheme::for_display(&display).lookup_by_gicon(
        icon,
        24,
        1,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    );
    let path = paintable.file()?.path()?;
    let pixbuf = Pixbuf::from_file_at_scale(path, 24, 24, false)
        .ok()?
        .add_alpha(false, 0, 0, 0)
        .ok()?;
    let bytes = pixbuf.read_pixel_bytes();
    let (width, height, stride) = (
        pixbuf.width() as usize,
        pixbuf.height() as usize,
        pixbuf.rowstride() as usize,
    );
    Some(
        (0..height)
            .flat_map(|row| bytes[row * stride..row * stride + width * 4].iter().copied())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_file_suffix_leaves_a_desktop_file_name() {
        assert_eq!(strip_desktop_suffix("com.bitwarden.desktop.desktop"), "com.bitwarden.desktop");
        assert_eq!(strip_desktop_suffix("org.kde.konsole.desktop"), "org.kde.konsole");
    }
}
