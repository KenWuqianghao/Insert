use gtk4 as gtk;
use gtk::{gdk, glib, pango, prelude::*};
use insert_core::{
    format::grapheme_prefix, model::file_path, Clip, ClipContent, Library, TextSnippet,
};

use super::card::{color_body, file_icon, label};
use crate::appicon::app_visual;

const FULL_TEXT_LIMIT: usize = 200_000;

/// The preview of one clip: a header row and a content box that fills the rest.
pub fn build(clip: &Clip, library: &Library) -> gtk::Widget {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_start(22);
    column.set_margin_end(22);
    column.set_margin_top(18);
    column.set_margin_bottom(6);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    if let Some(app_icon) = app_visual(clip.source.as_ref()).icon {
        let image = gtk::Image::from_gicon(&app_icon);
        image.set_pixel_size(28);
        header.append(&image);
    }
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_hexpand(true);
    let title = label("preview-title", 1);
    title.set_text(&clip.preview_title());
    let subtitle = label("preview-subtitle", 1);
    subtitle.set_text(&clip.preview_subtitle());
    titles.append(&title);
    titles.append(&subtitle);
    header.append(&titles);
    let hint = gtk::Label::new(Some("Press Space to close"));
    hint.add_css_class("preview-hint");
    header.append(&hint);
    column.append(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("preview-box");
    content.set_overflow(gtk::Overflow::Hidden);
    content.set_vexpand(true);
    let inner = content_for(clip, library);
    inner.set_vexpand(true);
    content.append(&inner);
    column.append(&content);
    column.upcast()
}

fn content_for(clip: &Clip, library: &Library) -> gtk::Widget {
    match &clip.content {
        ClipContent::Text(snippet) | ClipContent::RichText(snippet) => {
            text_view(&full_text(clip, snippet, library))
        }
        ClipContent::Image(_) => {
            let picture = image_texture(clip, library)
                .map(|texture| gtk::Picture::for_paintable(&texture))
                .unwrap_or_else(|| gtk::Picture::for_filename(library.thumbnail_path(clip.id)));
            picture.set_content_fit(gtk::ContentFit::Contain);
            picture.set_can_shrink(true);
            picture.set_margin_start(12);
            picture.set_margin_end(12);
            picture.set_margin_top(12);
            picture.set_margin_bottom(12);
            picture.upcast()
        }
        ClipContent::Link { url, title } => {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
            column.set_valign(gtk::Align::Center);
            column.set_margin_start(24);
            column.set_margin_end(24);
            let link_icon = gtk::Image::from_icon_name("insert-link-symbolic");
            link_icon.set_pixel_size(30);
            link_icon.add_css_class("accent-icon");
            column.append(&link_icon);
            if let Some(title) = title {
                let heading = gtk::Label::new(Some(title));
                heading.add_css_class("preview-link-title");
                heading.set_wrap(true);
                heading.set_justify(gtk::Justification::Center);
                column.append(&heading);
            }
            let full = gtk::Label::new(Some(url));
            full.add_css_class("preview-link-url");
            full.set_selectable(true);
            full.set_wrap(true);
            full.set_wrap_mode(pango::WrapMode::Char);
            full.set_justify(gtk::Justification::Center);
            column.append(&full);
            column.upcast()
        }
        ClipContent::Files(uris) => {
            let list = gtk::Box::new(gtk::Orientation::Vertical, 10);
            list.set_margin_start(16);
            list.set_margin_end(16);
            list.set_margin_top(16);
            list.set_margin_bottom(16);
            for uri in uris {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                let image = gtk::Image::from_gicon(&file_icon(uri));
                image.set_pixel_size(32);
                row.append(&image);
                let path = file_path(uri);
                let path = std::path::Path::new(&path);
                let names = gtk::Box::new(gtk::Orientation::Vertical, 0);
                names.set_valign(gtk::Align::Center);
                let name = gtk::Label::new(path.file_name().and_then(|n| n.to_str()));
                name.add_css_class("preview-file-name");
                let parent = gtk::Label::new(path.parent().and_then(|p| p.to_str()));
                parent.add_css_class("preview-file-folder");
                for label in [&name, &parent] {
                    label.set_xalign(0.0);
                    label.set_ellipsize(pango::EllipsizeMode::Middle);
                    names.append(label);
                }
                row.append(&names);
                list.append(&row);
            }
            scrolled(&list)
        }
        ClipContent::Color(color) => {
            let area = color_body(color, "preview-color-hex");
            let rgb = color.rgb8();
            let footer = gtk::Label::new(Some(&format!("RGB {}, {}, {}", rgb[0], rgb[1], rgb[2])));
            footer.add_css_class("preview-color-rgb");
            footer.set_margin_bottom(18);
            area.downcast_ref::<gtk::Box>()
                .expect("color_body is a box")
                .append(&footer);
            area
        }
        ClipContent::Pdf | ClipContent::Media | ClipContent::Data => {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
            column.set_valign(gtk::Align::Center);
            let image = gtk::Image::from_icon_name(clip.kind().icon_name());
            image.set_pixel_size(44);
            image.add_css_class("secondary");
            let text = gtk::Label::new(Some(&format!(
                "{}, {}",
                clip.content.display_title(),
                clip.footer()
            )));
            text.add_css_class("preview-placeholder");
            column.append(&image);
            column.append(&text);
            column.upcast()
        }
    }
}

/// The index keeps the first 8,000 characters. Longer text comes from the payload.
fn full_text(clip: &Clip, snippet: &TextSnippet, library: &Library) -> String {
    if snippet.character_count <= TextSnippet::LIMIT {
        return snippet.text.clone();
    }
    library
        .payload(clip.id)
        .ok()
        .and_then(|payload| payload.plain_text())
        .map(|text| grapheme_prefix(&text, FULL_TEXT_LIMIT).to_owned())
        .unwrap_or_else(|| snippet.text.clone())
}

fn image_texture(clip: &Clip, library: &Library) -> Option<gdk::Texture> {
    let payload = library.payload(clip.id).ok()?;
    gdk::Texture::from_bytes(&glib::Bytes::from(payload.image()?)).ok()
}

fn text_view(text: &str) -> gtk::Widget {
    let view = gtk::TextView::new();
    view.add_css_class("preview-text");
    view.set_editable(false);
    view.set_cursor_visible(false);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_pixels_inside_wrap(3);
    view.set_pixels_below_lines(3);
    view.set_left_margin(16);
    view.set_right_margin(16);
    view.set_top_margin(16);
    view.set_bottom_margin(16);
    view.set_can_focus(false);
    view.buffer().set_text(text);
    scrolled(&view)
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_child(Some(child));
    scroller.upcast()
}
