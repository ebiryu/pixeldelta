//! Estimating how far a cluster's content moved between the two images.
//!
//! A cluster that is a moved element, not a changed one, matches the other
//! image once shifted by the move vector. The search slides the cluster's
//! rectangle over a small window of offsets and reports the one that turns it
//! into a match, following the block-matching sketch in the design.
//!
//! Measuring every offset over the whole rectangle costs `288 * area`, which
//! grows with the size of the cluster rather than with the amount of
//! difference in it. [`displacement`] instead narrows the offsets through a
//! ladder of sample densities, each cheaper than measuring the full rectangle,
//! and only measures the survivors over the whole rectangle.

use crate::color::color_delta;
use crate::image::{Image, BYTES_PER_PIXEL};
use crate::region::Rect;

/// Half-width of the offset window the search covers, in pixels.
const SEARCH_RADIUS: i32 = 8;

/// Sample budgets and how many offsets survive scoring at each one.
///
/// A rung samples the rectangle down to at most `budget` pixels, scores every
/// surviving offset at that density, and keeps the `keep` best of them for the
/// next rung. The pair is applied in order until a rung's sample would already
/// cover the whole rectangle.
const RUNGS: [(u32, usize); 2] = [(4096, 16), (65536, 4)];

/// The offset at which `bounds` of `a` best matches `b`, when that match is
/// close enough to call the cluster a move rather than a change.
///
/// The result is the non-zero offset `(dx, dy)` with the lowest mean color
/// delta over the rectangle, provided that mean is within `max_delta`. Offsets
/// that push the rectangle outside the image are not considered, so that a
/// handful of pixels cannot produce a false match. `None` means no shift in the
/// window turns the region into a match, so it is a genuine change.
///
/// The accept-or-reject decision is always taken over the whole rectangle: a
/// rung can only drop an offset from consideration, never decide the outcome
/// on a sample. A sample that discards the true offset can turn a move into a
/// reported change, but it cannot turn a change into a reported move.
pub fn displacement(
    a: &Image<'_>,
    b: &Image<'_>,
    bounds: Rect,
    max_delta: f32,
) -> Option<(i32, i32)> {
    let width = a.width() as i32;
    let height = a.height() as i32;
    let (bx, by) = (bounds.x as i32, bounds.y as i32);
    let (bw, bh) = (bounds.width as i32, bounds.height as i32);
    let area = u64::from(bounds.width) * u64::from(bounds.height);

    // Offsets come in order of increasing magnitude; the rank each one carries
    // through the rungs is its position in that order, so the final tie-break
    // still favors the smallest shift.
    let mut candidates: Vec<(usize, (i32, i32))> = offsets()
        .enumerate()
        .filter(|&(_, (dx, dy))| {
            // The whole rectangle, shifted, has to stay inside the image.
            bx + dx >= 0 && by + dy >= 0 && bx + dx + bw <= width && by + dy + bh <= height
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }

    for &(budget, keep) in &RUNGS {
        let step = step_for(area, budget);
        if step == 1 {
            // The sample would cover the whole rectangle, so there is nothing
            // left to narrow: go straight to the full measurement below.
            break;
        }
        if candidates.len() <= keep {
            continue;
        }

        let mut scored: Vec<(f32, usize, (i32, i32))> = candidates
            .iter()
            .map(|&(rank, offset)| (mean_at(a, b, bounds, offset, step), rank, offset))
            .collect();
        scored.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        scored.truncate(keep);
        candidates = scored
            .into_iter()
            .map(|(_, rank, offset)| (rank, offset))
            .collect();
    }

    candidates.sort_by_key(|&(rank, _)| rank);
    let mut best: Option<(f32, (i32, i32))> = None;
    for (_, offset) in candidates {
        let mean = mean_at(a, b, bounds, offset, 1);
        // A strict compare keeps the smallest shift among equally good
        // matches, since candidates are walked in order of increasing
        // magnitude.
        if best.is_none_or(|(lowest, _)| mean < lowest) {
            best = Some((mean, offset));
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

/// Stride at which sampling `area` pixels visits at most `budget` of them: `1`
/// when the area already fits the budget, otherwise the smallest stride whose
/// spacing in both directions brings the sampled count back within it.
fn step_for(area: u64, budget: u32) -> u32 {
    if area <= u64::from(budget) {
        1
    } else {
        (area as f64 / f64::from(budget)).sqrt().ceil() as u32
    }
}

/// Mean color delta between `a` and `b` over `bounds` shifted by `offset`,
/// visiting every `step`-th pixel in both directions. A `step` of `1` visits
/// every pixel in the rectangle, so the same helper serves both the sampled
/// and the whole-rectangle passes.
fn mean_at(a: &Image<'_>, b: &Image<'_>, bounds: Rect, offset: (i32, i32), step: u32) -> f32 {
    let a_pixels = a.pixels();
    let b_pixels = b.pixels();
    let width = a.width() as i32;
    let (bx, by) = (bounds.x as i32, bounds.y as i32);
    let (bw, bh) = (bounds.width as i32, bounds.height as i32);
    let (dx, dy) = offset;
    let step = step as i32;

    let mut sum = 0.0;
    let mut count: u32 = 0;
    let mut ry = 0;
    while ry < bh {
        let mut rx = 0;
        while rx < bw {
            let (px, py) = (bx + rx, by + ry);
            let here = (py * width + px) as usize;
            let there = ((py + dy) * width + (px + dx)) as usize;
            sum += color_delta(&a_pixels[here], &b_pixels[there], here * BYTES_PER_PIXEL);
            count += 1;
            rx += step;
        }
        ry += step;
    }

    sum / count as f32
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

    #[test]
    fn a_large_moved_block_is_found_through_the_sampled_path() {
        // 160x80 content, comfortably over the 4096-pixel first budget, moved
        // by (5, -3). The canvas is large enough to hold both positions and
        // the search window around them.
        let width = 400;
        let height = 300;
        let base = canvas(width, height, rect(100, 100, 160, 80), WHITE);
        let head = canvas(width, height, rect(105, 97, 160, 80), WHITE);
        let a = Image::from_rgba8(width, height, &base).unwrap();
        let b = Image::from_rgba8(width, height, &head).unwrap();
        // The cluster covers both positions.
        let bounds = rect(100, 97, 165, 83);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), Some((5, -3)));
    }

    #[test]
    fn a_large_recolored_block_is_not_a_move() {
        // Same size and position as the moved-block case above, recolored in
        // place instead: no sample density should turn this into a match.
        let width = 400;
        let height = 300;
        let base = canvas(width, height, rect(100, 100, 160, 80), WHITE);
        let head = canvas(width, height, rect(100, 100, 160, 80), RED);
        let a = Image::from_rgba8(width, height, &base).unwrap();
        let b = Image::from_rgba8(width, height, &head).unwrap();
        let bounds = rect(100, 100, 160, 80);

        assert_eq!(displacement(&a, &b, bounds, MAX_DELTA), None);
    }

    #[test]
    fn step_for_is_1_at_or_below_budget_and_grows_with_the_square_root_above_it() {
        assert_eq!(step_for(1, 4096), 1);
        assert_eq!(step_for(4096, 4096), 1);
        // Just over budget: still rounds up to 1 only when the ratio is 1,
        // otherwise the next integer above the square root of the ratio.
        assert_eq!(step_for(4097, 4096), 2);
        // 4x the budget needs a step of 2 to bring the sampled count back to
        // budget.
        assert_eq!(step_for(4096 * 4, 4096), 2);
        // 9x the budget needs a step of 3.
        assert_eq!(step_for(4096 * 9, 4096), 3);
    }
}
