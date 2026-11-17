//! The clip ring. The user holds its global shortcut, points at a clip, and releases the shortcut to paste
//! the clip. Wayland gives an app no global pointer position, so the ring is a fullscreen transparent window
//! that learns the pointer from its own pointer events.

use std::cell::RefCell;
use std::f64::consts::TAU;
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk4 as gtk;
use gtk::{cairo, gdk, glib, prelude::*};
use insert_core::format::relative_time;
use insert_core::ring::{direction, span, HOLE_RADIUS, INNER_RADIUS, OUTER_RADIUS};
use insert_core::{Bounds, Clip, ClipId, Library, PasteAction, Point, Ring};
use time::OffsetDateTime;

use crate::app::App;
use crate::appicon::app_visual;
use crate::theme::{background_class, named_icon};
use crate::tray::card::{set_class, Card, CARD_SIZE};
use crate::tray::WINDOW_TITLE;
use crate::with_app;

const SIDE: i32 = (OUTER_RADIUS * 2.0) as i32;
const ICON_SIZE: i32 = 30;
/// The kind badge sticks out of the icon by this much at the bottom right.
const BADGE_BLEED: i32 = 4;
const SELECTION_INSET: f64 = 3.0;
const SEGMENT_GAP: f64 = 6.0;
/// The longest wait for the user to release the modifier keys before the paste.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(1);
const HELD_MODIFIERS: gdk::ModifierType = gdk::ModifierType::SHIFT_MASK
    .union(gdk::ModifierType::CONTROL_MASK)
    .union(gdk::ModifierType::ALT_MASK)
    .union(gdk::ModifierType::SUPER_MASK)
    .union(gdk::ModifierType::META_MASK);

enum RingState {
    Hidden,
    /// The window is up and waits for the first pointer position to place the ring.
    Placing,
    Open(Ring),
    /// The user released the shortcut on this clip. The window shows nothing and keeps the keyboard focus
    /// until the user also releases the modifier keys. Else the paste keys combine with them, and a key of
    /// the shortcut that is still down blocks the same key in the paste.
    Releasing(ClipId),
}

pub struct RingWindow {
    window: gtk::Window,
    canvas: gtk::Fixed,
    donut: gtk::Overlay,
    band: gtk::DrawingArea,
    icons: gtk::Fixed,
    title: gtk::Label,
    age: gtk::Label,
    preview: RefCell<Option<(ClipId, gtk::Widget)>>,
    state: RefCell<RingState>,
    app: Weak<App>,
}

impl RingWindow {
    pub fn new(app: &Weak<App>, gtk_app: &gtk::Application) -> Self {
        let window = gtk::Window::builder()
            .application(gtk_app)
            .title(WINDOW_TITLE)
            .decorated(false)
            .hide_on_close(true)
            .build();
        window.add_css_class("ring-window");

        let band = gtk::DrawingArea::new();
        band.set_content_width(SIDE);
        band.set_content_height(SIDE);
        band.set_draw_func(with_app!(app => app, |_, cr, _, _| app.ring.draw_band(cr)));
        let icons = gtk::Fixed::new();
        let title = gtk::Label::new(None);
        title.add_css_class("ring-title");
        let age = gtk::Label::new(None);
        age.add_css_class("ring-age");
        let hole = gtk::Box::new(gtk::Orientation::Vertical, 1);
        hole.set_halign(gtk::Align::Center);
        hole.set_valign(gtk::Align::Center);
        hole.set_size_request((HOLE_RADIUS * 2.0) as i32 - 16, -1);
        for label in [&title, &age] {
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(1);
            label.set_hexpand(true);
            hole.append(label);
        }
        let donut = gtk::Overlay::new();
        donut.add_css_class("ring-donut");
        donut.set_child(Some(&band));
        donut.add_overlay(&icons);
        donut.add_overlay(&hole);

        let canvas = gtk::Fixed::new();
        canvas.add_css_class("ring-canvas");
        canvas.put(&donut, 0.0, 0.0);
        window.set_child(Some(&canvas));

        let ring = Self {
            window,
            canvas,
            donut,
            band,
            icons,
            title,
            age,
            preview: RefCell::new(None),
            state: RefCell::new(RingState::Hidden),
            app: app.clone(),
        };
        ring.connect(app);
        ring
    }

    fn connect(&self, app: &Weak<App>) {
        let motion = gtk::EventControllerMotion::new();
        motion.connect_enter(with_app!(app => app, |_, x, y| app.point_ring(Point { x, y })));
        motion.connect_motion(with_app!(app => app, |_, x, y| app.point_ring(Point { x, y })));
        self.window.add_controller(motion);

        let click = gtk::GestureClick::new();
        click.set_button(0);
        click.connect_pressed(with_app!(app => app, |gesture, _, x, y| {
            app.click_ring(gesture.current_button(), Point { x, y });
        }));
        self.window.add_controller(click);

        // The portal reports the release of the shortcut. The window can also get it, while it has the focus.
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(with_app!(app => app or glib::Propagation::Proceed, |_, keyval, _, _| {
            match keyval {
                gdk::Key::Escape => app.close_ring(),
                gdk::Key::Return | gdk::Key::KP_Enter => app.release_ring(),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        }));
        keys.connect_key_released(with_app!(app => app, |_, _, _, _| app.release_ring()));
        keys.connect_modifiers(with_app!(app => app or glib::Propagation::Proceed, |_, state| {
            if !state.intersects(HELD_MODIFIERS) {
                app.finish_ring_release();
            }
            glib::Propagation::Proceed
        }));
        self.window.add_controller(keys);

        self.window.connect_is_active_notify(with_app!(app => app, |window| {
            if !window.is_active() {
                app.close_ring();
            }
        }));
        self.window.connect_close_request(with_app!(app => app or glib::Propagation::Stop, |_| {
            app.close_ring();
            glib::Propagation::Stop
        }));
    }

    pub fn is_hidden(&self) -> bool {
        matches!(*self.state.borrow(), RingState::Hidden)
    }

    fn present(&self, activation_token: Option<&str>) {
        *self.state.borrow_mut() = RingState::Placing;
        if let Some(token) = activation_token {
            self.window.set_startup_id(token);
        }
        self.window.fullscreen();
        self.window.present();
        // GTK sends no enter signal when the window shows again under a pointer that did not move, so the
        // ring also reads the pointer position on each frame until it has one.
        self.window.add_tick_callback(with_app!(&self.app => app or glib::ControlFlow::Break, |window, _| {
            if !matches!(*app.ring.state.borrow(), RingState::Placing) {
                return glib::ControlFlow::Break;
            }
            if let Some(at) = pointer_position(window) {
                app.point_ring(at);
            }
            glib::ControlFlow::Continue
        }));
    }

    fn hide(&self) {
        if matches!(self.state.replace(RingState::Hidden), RingState::Hidden) {
            return;
        }
        self.window.set_visible(false);
        self.canvas.remove_css_class("closing");
        self.donut.remove_css_class("shown");
        while let Some(child) = self.icons.first_child() {
            self.icons.remove(&child);
        }
        self.show_preview(None);
    }

    fn modifiers_held(&self) -> bool {
        WidgetExt::display(&self.window)
            .default_seat()
            .and_then(|seat| seat.keyboard())
            .is_some_and(|keyboard| keyboard.modifier_state().intersects(HELD_MODIFIERS))
    }

    fn bounds(&self) -> Bounds {
        Bounds { width: f64::from(self.window.width()), height: f64::from(self.window.height()) }
    }

    fn render_open(&self, library: &Library) {
        let state = self.state.borrow();
        let RingState::Open(ring) = &*state else { return };
        let center = ring.center();
        self.canvas.move_(&self.donut, center.x - OUTER_RADIUS, center.y - OUTER_RADIUS);
        for (index, id) in ring.clips().iter().enumerate() {
            let Some(clip) = library.clip(*id) else { continue };
            let icon = clip_icon(clip);
            let at = ring.icon_center(index);
            let half = f64::from(ICON_SIZE) / 2.0;
            self.icons.put(&icon, at.x - center.x + OUTER_RADIUS - half, at.y - center.y + OUTER_RADIUS - half);
        }
        self.donut.add_css_class("shown");
        drop(state);
        self.render_selection(library);
    }

    fn render_selection(&self, library: &Library) {
        let state = self.state.borrow();
        let RingState::Open(ring) = &*state else { return };
        let mut icon = self.icons.first_child();
        let mut index = 0;
        while let Some(widget) = icon {
            set_class(&widget, "selected", ring.selection() == Some(index));
            icon = widget.next_sibling();
            index += 1;
        }
        self.band.queue_draw();

        let selected = ring.selected_clip().and_then(|id| library.clip(id));
        self.title.set_text(selected.map(|clip| clip.card_title()).unwrap_or_default());
        self.age.set_text(&selected.map(|clip| relative_time(clip.created_at, OffsetDateTime::now_utc())).unwrap_or_default());

        let card = selected.zip(ring.preview_center(f64::from(CARD_SIZE)));
        self.show_preview(card.map(|(clip, at)| (clip, library, at)));
    }

    fn show_preview(&self, card: Option<(&Clip, &Library, Point)>) {
        let mut shown = self.preview.borrow_mut();
        if shown.as_ref().map(|(id, _)| *id) != card.map(|(clip, _, _)| clip.id) {
            if let Some((_, widget)) = shown.take() {
                self.canvas.remove(&widget);
            }
            if let Some((clip, library, _)) = card {
                let root = Card::new(clip, library).root;
                root.add_css_class("ring-preview");
                root.set_can_target(false);
                self.canvas.put(&root, 0.0, 0.0);
                *shown = Some((clip.id, root.upcast()));
            }
        }
        if let (Some((_, widget)), Some((_, _, at))) = (&*shown, card) {
            let half = f64::from(CARD_SIZE) / 2.0;
            self.canvas.move_(widget, at.x - half, at.y - half);
        }
    }

    /// Thin glass on the band and on the center disc, with hairlines at the edges and a lighter selected
    /// segment. Like the tray in the dark appearance, in any appearance.
    fn draw_band(&self, cr: &cairo::Context) {
        let state = self.state.borrow();
        let RingState::Open(ring) = &*state else { return };
        let c = OUTER_RADIUS;
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        cr.arc(c, c, OUTER_RADIUS, 0.0, TAU);
        cr.new_sub_path();
        cr.arc(c, c, INNER_RADIUS, 0.0, TAU);
        cr.new_sub_path();
        cr.arc(c, c, HOLE_RADIUS, 0.0, TAU);
        cr.set_source_rgba(40.0 / 255.0, 40.0 / 255.0, 42.0 / 255.0, 0.94);
        let _ = cr.fill();

        cr.set_source_rgba(1.0, 1.0, 1.0, 0.16);
        cr.set_line_width(1.0);
        for radius in [OUTER_RADIUS - 0.5, INNER_RADIUS, HOLE_RADIUS] {
            cr.new_sub_path();
            cr.arc(c, c, radius, 0.0, TAU);
        }
        let _ = cr.stroke();

        let Some(index) = ring.selection() else { return };
        let count = ring.clips().len();
        let (inner, outer) = (INNER_RADIUS + SELECTION_INSET, OUTER_RADIUS - SELECTION_INSET);
        if count == 1 {
            cr.arc(c, c, outer, 0.0, TAU);
            cr.new_sub_path();
            cr.arc(c, c, inner, 0.0, TAU);
        } else {
            let (middle, half) = (direction(index, count), span(count) / 2.0);
            let gap = |radius: f64| SEGMENT_GAP / 2.0 / radius;
            cr.arc(c, c, outer, middle - half + gap(outer), middle + half - gap(outer));
            cr.arc_negative(c, c, inner, middle + half - gap(inner), middle - half + gap(inner));
            cr.close_path();
        }
        let _ = cr.fill();
    }
}

fn pointer_position(window: &gtk::Window) -> Option<Point> {
    let pointer = WidgetExt::display(window).default_seat()?.pointer()?;
    let (x, y, _) = window.surface()?.device_position(&pointer)?;
    Some(Point { x, y })
}

/// The app icon of the clip with a badge for its kind. A clip with no app icon gets a square in the app tint.
fn clip_icon(clip: &Clip) -> gtk::Widget {
    let visual = app_visual(clip.source.as_ref());
    let face: gtk::Widget = match &visual.icon {
        Some(icon) => {
            let image = gtk::Image::from_gicon(icon);
            image.set_pixel_size(ICON_SIZE);
            image.upcast()
        }
        None => {
            let square = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            square.add_css_class("ring-icon-square");
            square.add_css_class(&background_class(visual.tint));
            square.upcast()
        }
    };
    face.set_size_request(ICON_SIZE, ICON_SIZE);
    face.set_halign(gtk::Align::Start);
    face.set_valign(gtk::Align::Start);

    let badge = named_icon(clip.kind().icon_name());
    badge.set_pixel_size(8);
    badge.add_css_class("ring-badge");
    badge.set_halign(gtk::Align::End);
    badge.set_valign(gtk::Align::End);

    let icon = gtk::Overlay::new();
    icon.add_css_class("ring-icon");
    icon.set_size_request(ICON_SIZE + BADGE_BLEED, ICON_SIZE + BADGE_BLEED);
    icon.set_child(Some(&face));
    icon.add_overlay(&badge);
    icon.upcast()
}

impl App {
    /// The portal can send the press again while the user holds the keys. The ring ignores it while it is up.
    pub fn open_ring(self: &Rc<Self>, activation_token: Option<&str>) {
        if !self.ring.is_hidden() || self.tray.model.borrow().is_shown() || self.library.borrow().clips().is_empty() {
            return;
        }
        self.ring.present(activation_token);
    }

    pub fn point_ring(self: &Rc<Self>, pointer: Point) {
        let library = self.library.borrow();
        let mut state = self.ring.state.borrow_mut();
        match &mut *state {
            RingState::Hidden => {}
            RingState::Placing => {
                let clips = library.clips().iter().map(|clip| clip.id);
                if let Some(ring) = Ring::open(clips, pointer, self.ring.bounds()) {
                    *state = RingState::Open(ring);
                    drop(state);
                    self.ring.render_open(&library);
                }
            }
            RingState::Open(ring) => {
                if ring.track(pointer) {
                    drop(state);
                    self.ring.render_selection(&library);
                }
            }
            RingState::Releasing(_) => {}
        }
    }

    /// Pastes the selected clip when the user also releases the modifier keys. With no selection, the ring stays
    /// open, as on macOS: a click on a clip pastes it, and `Esc` or a click outside the band closes the ring.
    pub fn release_ring(self: &Rc<Self>) {
        let selected = match &*self.ring.state.borrow() {
            RingState::Open(ring) => match ring.selected_clip() {
                Some(id) => Some(id),
                None => return,
            },
            RingState::Releasing(_) => return,
            RingState::Hidden | RingState::Placing => None,
        };
        let Some(id) = selected else {
            self.ring.hide();
            return;
        };
        *self.ring.state.borrow_mut() = RingState::Releasing(id);
        self.ring.canvas.add_css_class("closing");
        if self.ring.modifiers_held() {
            let app = Rc::downgrade(self);
            glib::timeout_add_local_once(RELEASE_TIMEOUT, move || {
                if let Some(app) = app.upgrade() {
                    app.finish_ring_release();
                }
            });
        } else {
            self.finish_ring_release();
        }
    }

    fn finish_ring_release(self: &Rc<Self>) {
        let RingState::Releasing(id) = *self.ring.state.borrow() else { return };
        self.ring.hide();
        match self.copy_clip(id, PasteAction::Paste) {
            Ok(true) => self.services.paste_into_front_app(),
            Ok(false) => {}
            Err(()) => WidgetExt::display(&self.ring.window).beep(),
        }
    }

    /// Closes the ring with no paste, or ends the wait of a release at once.
    pub fn close_ring(self: &Rc<Self>) {
        if matches!(*self.ring.state.borrow(), RingState::Releasing(_)) {
            self.finish_ring_release();
        } else {
            self.ring.hide();
        }
    }

    fn click_ring(self: &Rc<Self>, button: u32, at: Point) {
        let on_selection = matches!(&*self.ring.state.borrow(), RingState::Open(ring) if ring.selection().is_some() && ring.band_contains(at));
        if button == gdk::BUTTON_PRIMARY && on_selection {
            self.release_ring();
        } else {
            self.close_ring();
        }
    }
}
