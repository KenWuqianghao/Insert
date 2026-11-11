use gtk4 as gtk;
use gtk::{gio, pango, prelude::*};
use insert_core::{format::grapheme_prefix, model::file_name, Clip, ClipContent, ColorValue, Library};
use time::OffsetDateTime;

use crate::appicon::app_visual;
use crate::theme::{appearance, background_class};

pub const CARD_SIZE: i32 = 232;
const HEADER_HEIGHT: i32 = 52;
const APP_ICON_SIZE: i32 = 64;
/// The app icon bleeds this far out of the card's right edge.
const APP_ICON_BLEED: i32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    None,
    Primary,
    Secondary,
}

/// One clip card. The body is built once per clip; `update` refreshes what can change.
pub struct Card {
    pub root: gtk::Box,
    title: gtk::Label,
    subtitle: gtk::Label,
    pin_dot: gtk::Box,
    footer: gtk::Label,
    badge: gtk::Label,
    dot_class: std::cell::RefCell<Option<String>>,
}

impl Card {
    pub fn new(clip: &Clip, library: &Library) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("card");
        root.set_size_request(CARD_SIZE, CARD_SIZE);
        root.set_overflow(gtk::Overflow::Hidden);
        root.set_valign(gtk::Align::Start);
        root.set_halign(gtk::Align::Start);

        let visual = app_visual(clip.source.as_ref());
        let header = gtk::Overlay::new();
        header.set_size_request(-1, HEADER_HEIGHT);
        header.set_overflow(gtk::Overflow::Hidden);
        header.add_css_class("card-header");
        header.add_css_class(&background_class(visual.tint));

        let labels = gtk::Box::new(gtk::Orientation::Vertical, 2);
        labels.set_valign(gtk::Align::Center);
        labels.set_margin_start(14);
        labels.set_margin_end(if visual.icon.is_some() {
            APP_ICON_SIZE - APP_ICON_BLEED + 4
        } else {
            14
        });
        let title = label("card-title", 1);
        let subtitle = label("card-subtitle", 1);
        labels.append(&title);
        labels.append(&subtitle);
        header.set_child(Some(&labels));

        let fixed = gtk::Fixed::new();
        fixed.set_can_target(false);
        match &visual.icon {
            Some(app_icon) => {
                let image = gtk::Image::from_gicon(app_icon);
                image.set_pixel_size(APP_ICON_SIZE);
                image.add_css_class("app-icon");
                fixed.put(
                    &image,
                    f64::from(CARD_SIZE - APP_ICON_SIZE + APP_ICON_BLEED),
                    f64::from(HEADER_HEIGHT - APP_ICON_SIZE) / 2.0,
                );
            }
            None => {
                let image = gtk::Image::from_icon_name(clip.kind().icon_name());
                image.set_pixel_size(22);
                image.add_css_class("kind-icon");
                fixed.put(&image, f64::from(CARD_SIZE - 14 - 22), 15.0);
            }
        }
        header.add_overlay(&fixed);
        root.append(&header);

        let body = body(clip, library);
        body.set_vexpand(true);
        root.append(&body);

        let footer_bar = gtk::CenterBox::new();
        footer_bar.add_css_class("card-footer");
        footer_bar.set_size_request(-1, 28);
        let pin_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        pin_dot.add_css_class("dot");
        pin_dot.set_valign(gtk::Align::Center);
        let footer = label("card-footer-text", 1);
        let badge = gtk::Label::new(None);
        badge.add_css_class("badge");
        badge.set_valign(gtk::Align::Center);
        footer_bar.set_start_widget(Some(&pin_dot));
        footer_bar.set_center_widget(Some(&footer));
        footer_bar.set_end_widget(Some(&badge));
        root.append(&footer_bar);

        let card = Self {
            root,
            title,
            subtitle,
            pin_dot,
            footer,
            badge,
            dot_class: Default::default(),
        };
        card.update(clip, library, OffsetDateTime::now_utc());
        card
    }

    pub fn update(&self, clip: &Clip, library: &Library, now: OffsetDateTime) {
        self.title.set_text(clip.card_title());
        self.subtitle.set_text(&clip.card_subtitle(now));
        self.footer.set_text(&clip.footer());
        let color = clip
            .pinboard_id
            .and_then(|id| library.pinboard(id))
            .map(|pinboard| background_class(pinboard.color.rgb(appearance().dark)));
        let mut current = self.dot_class.borrow_mut();
        if *current != color {
            if let Some(old) = current.take() {
                self.pin_dot.remove_css_class(&old);
            }
            if let Some(new) = &color {
                self.pin_dot.add_css_class(new);
            }
            *current = color;
        }
        self.pin_dot.set_opacity(if current.is_some() { 1.0 } else { 0.0 });
    }

    pub fn set_ring(&self, ring: Ring) {
        set_class(&self.root, "selected", ring != Ring::None);
        set_class(&self.root, "secondary", ring == Ring::Secondary);
    }

    /// `Some(n)` shows "Ctrl+n".
    pub fn set_badge(&self, number: Option<usize>) {
        if let Some(n) = number {
            self.badge.set_text(&format!("Ctrl+{n}"));
        }
        set_class(&self.badge, "shown", number.is_some());
    }

    pub fn set_deleting(&self) {
        self.root.add_css_class("deleting");
        self.root.set_can_target(false);
    }
}

pub fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

pub fn label(class: &str, lines: i32) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class(class);
    label.set_xalign(0.0);
    if lines == 1 {
        label.set_single_line_mode(true);
        label.set_ellipsize(pango::EllipsizeMode::End);
    } else {
        label.set_wrap(true);
        label.set_wrap_mode(pango::WrapMode::WordChar);
        label.set_lines(lines);
        label.set_ellipsize(pango::EllipsizeMode::End);
    }
    label
}

fn body(clip: &Clip, library: &Library) -> gtk::Widget {
    match &clip.content {
        ClipContent::Text(snippet) | ClipContent::RichText(snippet) => {
            let text = label("card-text", 8);
            text.set_text(grapheme_prefix(&snippet.text, 600).trim());
            text.set_yalign(0.0);
            text.set_valign(gtk::Align::Start);
            text.set_margin_start(14);
            text.set_margin_end(14);
            text.set_margin_top(12);
            text.upcast()
        }
        ClipContent::Link { url, .. } => {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
            column.set_margin_start(14);
            column.set_margin_end(14);
            column.set_margin_top(14);
            let link_icon = gtk::Image::from_icon_name("insert-link-symbolic");
            link_icon.set_pixel_size(18);
            link_icon.set_halign(gtk::Align::Start);
            link_icon.add_css_class("accent-icon");
            let heading = label("card-link-title", 2);
            heading.set_text(&clip.content.display_title());
            let full = label("card-link-url", 3);
            full.set_wrap_mode(pango::WrapMode::Char);
            full.set_text(url);
            column.append(&link_icon);
            column.append(&heading);
            column.append(&full);
            column.upcast()
        }
        ClipContent::Image(_) => {
            let path = library.thumbnail_path(clip.id);
            if path.exists() {
                let picture = gtk::Picture::for_filename(path);
                picture.set_content_fit(gtk::ContentFit::Cover);
                picture.set_can_shrink(true);
                picture.upcast()
            } else {
                placeholder("Image")
            }
        }
        ClipContent::Color(color) => color_body(color, "card-color-hex"),
        ClipContent::Files(uris) => files_body(uris),
        ClipContent::Pdf => {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
            column.set_valign(gtk::Align::Center);
            let image = gtk::Image::from_icon_name(clip.kind().icon_name());
            image.set_pixel_size(36);
            image.add_css_class("secondary");
            let text = gtk::Label::new(Some("PDF Document"));
            text.add_css_class("card-placeholder");
            column.append(&image);
            column.append(&text);
            column.upcast()
        }
        ClipContent::Media => placeholder("Media"),
        ClipContent::Data => placeholder("Clipboard Data"),
    }
}

pub fn color_body(color: &ColorValue, text_class: &str) -> gtk::Widget {
    let rgb8 = color.rgb8();
    let area = gtk::Box::new(gtk::Orientation::Vertical, 4);
    area.add_css_class(&background_class(insert_core::Rgb {
        r: rgb8[0],
        g: rgb8[1],
        b: rgb8[2],
    }));
    area.add_css_class(if color.is_light() { "on-light" } else { "on-dark" });
    let hex = gtk::Label::new(Some(&color.hex()));
    hex.add_css_class(text_class);
    hex.set_vexpand(true);
    area.append(&hex);
    area.upcast()
}

fn files_body(uris: &[String]) -> gtk::Widget {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_valign(gtk::Align::Center);
    column.set_margin_start(14);
    column.set_margin_end(14);
    if let Some(first) = uris.first() {
        let image = gtk::Image::from_gicon(&file_icon(first));
        image.set_pixel_size(56);
        column.append(&image);
    }
    let names = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for uri in uris.iter().take(3) {
        let name = gtk::Label::new(Some(&file_name(uri)));
        name.add_css_class("card-file-name");
        name.set_ellipsize(pango::EllipsizeMode::Middle);
        names.append(&name);
    }
    column.append(&names);
    if uris.len() > 3 {
        let more = gtk::Label::new(Some(&format!("and {} more", uris.len() - 3)));
        more.add_css_class("card-more");
        column.append(&more);
    }
    column.upcast()
}

pub fn file_icon(uri: &str) -> gio::Icon {
    let (content_type, _) = gio::content_type_guess(Some(file_name(uri)), None);
    gio::content_type_get_icon(&content_type)
}

fn placeholder(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("card-placeholder");
    label.set_valign(gtk::Align::Center);
    label.upcast()
}
