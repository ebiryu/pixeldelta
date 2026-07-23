//! Estimating how far a cluster's content moved between the two images.
//!
//! A cluster that is a moved element, not a changed one, matches the other
//! image once shifted by the move vector. The search slides the cluster's
//! rectangle over a small window of offsets and reports the one that turns it
//! into a match, following the block-matching sketch in the design.

use crate::color::color_delta;
use crate::image::{Image, BYTES_PER_PIXEL};
use crate::region::Rect;

/// Half-width of the offset window the search covers, in pixels.
const SEARCH_RADIUS: i32 = 8;

/// The offset at which `bounds` of `a` best matches `b`, when that match is
/// close enough to call the cluster a move rather than a change.
///
/// The result is the non-zero offset `(dx, dy)` with the lowest mean color
/// delta over the rectangle, provided that mean is within `max_delta`. Offsets
/// that push the rectangle outside the image are not considered, so that a
/// handful of pixels cannot produce a false match. `None` means no shift in the
/// window turns the region into a match, so it is a genuine change.
pub fn displacement(
    a: &Image<'_>,
    b: &Image<'_>,
    bounds: Rect,
    max_delta: f32,
) -> Option<(i32, i32)> {
    let a_pixels = a.pixels();
    let b_pixels = b.pixels();
    let width = a.width() as i32;
    let height = a.height() as i32;
    let (bx, by) = (bounds.x as i32, bounds.y as i32);
    let (bw, bh) = (bounds.width as i32, bounds.height as i32);
    let area = (bw * bh) as f32;

    let mut best: Option<(f32, (i32, i32))> = None;
    for (dx, dy) in offsets() {
        // The whole rectangle, shifted, has to stay inside the image.
        if bx + dx < 0 || by + dy < 0 || bx + dx + bw > width || by + dy + bh > height {
            continue;
        }

        let mut sum = 0.0;
        for ry in 0..bh {
            for rx in 0..bw {
                let (px, py) = (bx + rx, by + ry);
                let here = (py * width + px) as usize;
                let there = ((py + dy) * width + (px + dx)) as usize;
                sum += color_delta(&a_pixels[here], &b_pixels[there], here * BYTES_PER_PIXEL);
            }
        }

        let mean = sum / area;
        // Offsets come in order of increasing magnitude, so a strict compare
        // keeps the smallest shift among equally good matches.
        if best.is_none_or(|(lowest, _)| mean < lowest) {
            best = Some((mean, (dx, dy)));
        }
    }

    best.filter(|&(mean, _)| mean <= max_delta)
        .map(|(_, offset)| offset)
}

/// The offsets to try, without `(0, 0)`, ordered by increasing magnitude so
/// that the closest of several equal matches is the one kept.
fn offsets() -> impl Iterator<Item = (i32, i32)> {
    let mut all: Vec<(i32, i32)> = (-SEARCH_RADIUS..=SEARCH_RADIUS)
        .flat_map(|dy| (-SEARCH_RADIUS..=SEARCH_RADIUS).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .collect();
    all.sort_by_key(|&(dx, dy)| (dx * dx + dy * dy, dy, dx));
    all.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const RED: [u8; 4] = [220, 40, 40, 255];

    /// A black canvas with a filled rectangle of `color`.
    fn canvas(width: u32, height: u32, block: Rect, color: [u8; 4]) -> Vec<u8> {
        let mut data = Vec::with_capacity((width * height) as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let inside = x >= block.x
                    && x < block.x + block.width
                    && y >= block.y
                    && y < block.y + block.height;
                data.extend_from_slice(if inside { &color } else { &BLACK });
            }
        }
        data
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    // color_delta at threshold 0.1: MAX_COLOR_DELTA * 0.1 * 0.1.
    const MAX_DELTA: f32 = crate::MAX_COLOR_DELTA * 0.01;

    #[test]
    fn a_block_shifted_right_is_found_at_its_offset() {
        // The block sits at x=2 in the base and x=5 in the head: moved by +3.
        let base = canvas(32, 16, rect(2, 4, 4, 4), WHITE);
        let head = canvas(32, 16, rect(5, 4, 4, 4), WHITE);
        let a = Image::from_rgba8(32, 16, &base).unwrap();
        let b = Image::from_rgba8(32, 16, &head).unwrap();
        // The cluster covers both positions.
        let bounds = rect(2, 4, 7, 4);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), Some((3, 0)));
    }

    #[test]
    fn a_block_shifted_down_is_found_at_its_offset() {
        let base = canvas(16, 32, rect(4, 2, 4, 4), WHITE);
        let head = canvas(16, 32, rect(4, 6, 4, 4), WHITE);
        let a = Image::from_rgba8(16, 32, &base).unwrap();
        let b = Image::from_rgba8(16, 32, &head).unwrap();
        let bounds = rect(4, 2, 4, 8);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), Some((0, 4)));
    }

    #[test]
    fn a_recolored_block_in_place_is_not_a_move() {
        // Same position, different color: no offset turns it into a match.
        let base = canvas(32, 16, rect(10, 4, 5, 5), WHITE);
        let head = canvas(32, 16, rect(10, 4, 5, 5), RED);
        let a = Image::from_rgba8(32, 16, &base).unwrap();
        let b = Image::from_rgba8(32, 16, &head).unwrap();
        let bounds = rect(10, 4, 5, 5);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), None);
    }

    #[test]
    fn a_move_beyond_the_search_radius_is_not_found() {
        // Moved by 12, past the radius of 8.
        let base = canvas(48, 16, rect(2, 4, 4, 4), WHITE);
        let head = canvas(48, 16, rect(14, 4, 4, 4), WHITE);
        let a = Image::from_rgba8(48, 16, &base).unwrap();
        let b = Image::from_rgba8(48, 16, &head).unwrap();
        let bounds = rect(2, 4, 16, 4);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), None);
    }
}
