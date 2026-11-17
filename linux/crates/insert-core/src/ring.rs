//! The clip ring: a ring of the newest clips around the pointer. The pointer selects the segment in its
//! direction from the center. Coordinates are pixels in the surface that shows the ring, with y down.

use std::f64::consts::{PI, TAU};

use crate::ClipId;

pub const RING_CAPACITY: usize = 8;
pub const INNER_RADIUS: f64 = 72.0;
pub const OUTER_RADIUS: f64 = 132.0;
pub const ICON_RADIUS: f64 = 102.0;
/// The center disc. A pointer in it selects no segment.
pub const HOLE_RADIUS: f64 = 62.0;
pub const PREVIEW_GAP: f64 = 22.0;
const EDGE_MARGIN: f64 = 8.0;
const PREVIEW_INSET: f64 = 12.0;
/// Segment 0 is at 12 o'clock. The next segments follow clockwise, because the y axis points down.
const FIRST_DIRECTION: f64 = -PI / 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// The surface, with its origin at the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub width: f64,
    pub height: f64,
}

/// The angle in radians of the middle of segment `index`.
pub fn direction(index: usize, count: usize) -> f64 {
    FIRST_DIRECTION + index as f64 * span(count)
}

/// The angle in radians of one segment.
pub fn span(count: usize) -> f64 {
    TAU / count.max(1) as f64
}

fn segment_at(point: Point, center: Point, count: usize) -> Option<usize> {
    let (dx, dy) = (point.x - center.x, point.y - center.y);
    if count == 0 || dx.hypot(dy) < HOLE_RADIUS {
        return None;
    }
    let angle = (dy.atan2(dx) - FIRST_DIRECTION).rem_euclid(TAU);
    Some((angle / span(count)).round() as usize % count)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    clips: Vec<ClipId>,
    bounds: Bounds,
    center: Point,
    /// The pointer when the ring opened. It selects nothing until it moves, also when the ring is not
    /// centered on it near an edge of the surface.
    opened_at: Point,
    selection: Option<usize>,
}

impl Ring {
    /// The ring of the first `RING_CAPACITY` clips, centered at `pointer` and moved in so that it fits in
    /// `bounds`. `None` when there are no clips.
    pub fn open(clips: impl IntoIterator<Item = ClipId>, pointer: Point, bounds: Bounds) -> Option<Self> {
        let clips: Vec<ClipId> = clips.into_iter().take(RING_CAPACITY).collect();
        if clips.is_empty() {
            return None;
        }
        let margin = OUTER_RADIUS + EDGE_MARGIN;
        let center = Point { x: clamp(pointer.x, margin, bounds.width - margin), y: clamp(pointer.y, margin, bounds.height - margin) };
        Some(Self { clips, bounds, center, opened_at: pointer, selection: None })
    }

    pub fn clips(&self) -> &[ClipId] {
        &self.clips
    }

    pub fn center(&self) -> Point {
        self.center
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection
    }

    pub fn selected_clip(&self) -> Option<ClipId> {
        self.clips.get(self.selection?).copied()
    }

    /// Selects the segment in the direction of `pointer`. Returns true when the selection changed.
    pub fn track(&mut self, pointer: Point) -> bool {
        if pointer == self.opened_at {
            return false;
        }
        let selection = segment_at(pointer, self.center, self.clips.len());
        let changed = selection != self.selection;
        self.selection = selection;
        changed
    }

    /// True when `point` is on the band of the segments.
    pub fn band_contains(&self, point: Point) -> bool {
        let distance = (point.x - self.center.x).hypot(point.y - self.center.y);
        (INNER_RADIUS..=OUTER_RADIUS).contains(&distance)
    }

    /// The center of the icon of segment `index`.
    pub fn icon_center(&self, index: usize) -> Point {
        let angle = direction(index, self.clips.len());
        Point { x: self.center.x + angle.cos() * ICON_RADIUS, y: self.center.y + angle.sin() * ICON_RADIUS }
    }

    /// The center of the square preview card of the selected clip, with sides of `card_side`. The card goes
    /// outside the ring in the direction of the segment, or in the opposite direction when that does not fit.
    pub fn preview_center(&self, card_side: f64) -> Option<Point> {
        let angle = direction(self.selection?, self.clips.len());
        let toward = self.preview_placement(angle, card_side);
        let away = self.preview_placement(angle + PI, card_side);
        Some(if toward.1 || !away.1 { toward.0 } else { away.0 })
    }

    /// The card center, and true when the card fits with no move.
    fn preview_placement(&self, angle: f64, card_side: f64) -> (Point, bool) {
        let half = card_side / 2.0;
        let card_extent = half * (angle.cos().abs() + angle.sin().abs());
        let reach = OUTER_RADIUS + PREVIEW_GAP + card_extent;
        let wanted = Point { x: self.center.x + angle.cos() * reach, y: self.center.y + angle.sin() * reach };
        let inset = half + PREVIEW_INSET;
        let placed = Point {
            x: clamp(wanted.x, inset, self.bounds.width - inset),
            y: clamp(wanted.y, inset, self.bounds.height - inset),
        };
        let fits = (placed.x - wanted.x).abs() < 0.5 && (placed.y - wanted.y).abs() < 0.5;
        (placed, fits)
    }
}

/// `f64::clamp` panics when the surface is smaller than the ring. Then the ring goes to the middle.
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if min > max {
        (min + max) / 2.0
    } else {
        value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Bounds = Bounds { width: 1280.0, height: 800.0 };

    fn ids(count: usize) -> Vec<ClipId> {
        (0..count).map(|_| ClipId(uuid::Uuid::new_v4())).collect()
    }

    fn at(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// A point at `distance` from `center` in the middle of segment `index` of 8.
    fn toward(center: Point, index: usize, distance: f64) -> Point {
        let angle = direction(index, 8);
        at(center.x + angle.cos() * distance, center.y + angle.sin() * distance)
    }

    #[test]
    fn the_ring_holds_the_first_eight_clips_and_needs_one() {
        let clips = ids(11);
        let ring = Ring::open(clips.clone(), at(640.0, 400.0), SCREEN).unwrap();
        assert_eq!(ring.clips(), &clips[..RING_CAPACITY]);
        assert_eq!(Ring::open(Vec::new(), at(640.0, 400.0), SCREEN), None);
    }

    #[test]
    fn the_ring_is_centered_at_the_pointer_and_moved_in_at_an_edge() {
        let ring = |x, y| Ring::open(ids(3), at(x, y), SCREEN).unwrap().center();
        assert_eq!(ring(640.0, 400.0), at(640.0, 400.0));
        assert_eq!(ring(0.0, 0.0), at(140.0, 140.0));
        assert_eq!(ring(1280.0, 790.0), at(1140.0, 660.0));
        let tiny = Ring::open(ids(1), at(5.0, 5.0), Bounds { width: 100.0, height: 100.0 }).unwrap();
        assert_eq!(tiny.center(), at(50.0, 50.0));
    }

    #[test]
    fn the_pointer_selects_the_segment_in_its_direction() {
        let center = at(640.0, 400.0);
        let mut ring = Ring::open(ids(8), center, SCREEN).unwrap();
        assert_eq!(ring.selection(), None);
        for index in 0..8 {
            assert!(ring.track(toward(center, index, 100.0)));
            assert_eq!(ring.selection(), Some(index));
            assert_eq!(ring.selected_clip(), Some(ring.clips()[index]));
        }
        assert!(!ring.track(toward(center, 7, 300.0)), "a pointer out of the ring keeps its direction");
        assert!(ring.track(at(650.0, 410.0)), "the center disc clears the selection");
        assert_eq!(ring.selected_clip(), None);
    }

    #[test]
    fn a_segment_boundary_goes_to_the_nearest_segment_middle() {
        let center = at(640.0, 400.0);
        let mut ring = Ring::open(ids(4), center, SCREEN).unwrap();
        ring.track(at(center.x + 100.0, center.y - 99.0));
        assert_eq!(ring.selection(), Some(1), "just right of the boundary between 12 and 3 o'clock");
        ring.track(at(center.x - 100.0, center.y - 101.0));
        assert_eq!(ring.selection(), Some(0), "the wrap from the last segment to the first");
    }

    #[test]
    fn the_pointer_selects_nothing_until_it_moves_from_where_the_ring_opened() {
        let mut ring = Ring::open(ids(8), at(0.0, 0.0), SCREEN).unwrap();
        assert!(!ring.track(at(0.0, 0.0)));
        assert_eq!(ring.selection(), None);
        assert!(ring.track(at(1.0, 1.0)));
        assert_eq!(ring.selection(), Some(7), "the segment toward the top left corner");
    }

    #[test]
    fn only_the_band_takes_a_click() {
        let center = at(640.0, 400.0);
        let ring = Ring::open(ids(8), center, SCREEN).unwrap();
        assert!(ring.band_contains(toward(center, 2, ICON_RADIUS)));
        assert!(!ring.band_contains(toward(center, 2, HOLE_RADIUS)));
        assert!(!ring.band_contains(toward(center, 2, OUTER_RADIUS + 1.0)));
    }

    #[test]
    fn the_preview_goes_outside_the_ring_and_flips_when_it_does_not_fit() {
        let card = 232.0;
        let center = at(640.0, 400.0);
        let mut ring = Ring::open(ids(8), center, SCREEN).unwrap();
        assert_eq!(ring.preview_center(card), None);
        ring.track(toward(center, 2, 100.0));
        assert_eq!(ring.preview_center(card), Some(at(640.0 + OUTER_RADIUS + PREVIEW_GAP + card / 2.0, 400.0)));

        let mut low = Ring::open(ids(8), at(640.0, 800.0), SCREEN).unwrap();
        let low_center = low.center();
        low.track(toward(low_center, 4, 100.0));
        let preview = low.preview_center(card).unwrap();
        assert!(preview.y < low_center.y - OUTER_RADIUS, "the card for 6 o'clock goes above the ring at the bottom edge");
    }
}
