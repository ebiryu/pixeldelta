//! Structural similarity between a cluster's content in the two images.
//!
//! Where the diff pixel count says how much changed, this says how much the
//! change altered the structure of the region. A moved element matched at its
//! displacement scores near one; a recolored or reshaped region scores lower.
//! It is the single-window SSIM of Wang et al. over the cluster rectangle, on
//! luminance.

use crate::image::Image;
use crate::region::Rect;

// Stabilizing constants for eight-bit luminance, from the SSIM paper with the
// usual K1 and K2 and a dynamic range of 255.
const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

// Rec. 601 luma weights.
const R_TO_L: f64 = 0.299;
const G_TO_L: f64 = 0.587;
const B_TO_L: f64 = 0.114;

/// Structural similarity of `bounds` in `a` against the same rectangle in `b`
/// shifted by `offset`, on luminance.
///
/// The result is in `[-1, 1]`, reaching `1` when the two regions are identical.
/// `offset` must keep the rectangle inside `b`, which the layout-shift search
/// guarantees for the displacement it returns.
pub fn similarity(a: &Image<'_>, b: &Image<'_>, bounds: Rect, offset: (i32, i32)) -> f64 {
    let a_pixels = a.pixels();
    let b_pixels = b.pixels();
    let width = a.width() as i32;
    let (bx, by) = (bounds.x as i32, bounds.y as i32);
    let (bw, bh) = (bounds.width as i32, bounds.height as i32);
    let (dx, dy) = offset;

    let (mut sum_a, mut sum_b) = (0.0, 0.0);
    let (mut sum_aa, mut sum_bb, mut sum_ab) = (0.0, 0.0, 0.0);
    for ry in 0..bh {
        for rx in 0..bw {
            let (px, py) = (bx + rx, by + ry);
            let here = (py * width + px) as usize;
            let there = ((py + dy) * width + (px + dx)) as usize;
            let la = luminance(&a_pixels[here]);
            let lb = luminance(&b_pixels[there]);
            sum_a += la;
            sum_b += lb;
            sum_aa += la * la;
            sum_bb += lb * lb;
            sum_ab += la * lb;
        }
    }

    let n = (bw * bh) as f64;
    let mean_a = sum_a / n;
    let mean_b = sum_b / n;
    let var_a = sum_aa / n - mean_a * mean_a;
    let var_b = sum_bb / n - mean_b * mean_b;
    let covar = sum_ab / n - mean_a * mean_b;

    let luminance_term = (2.0 * mean_a * mean_b + C1) / (mean_a * mean_a + mean_b * mean_b + C1);
    let structure_term = (2.0 * covar + C2) / (var_a + var_b + C2);
    luminance_term * structure_term
}

/// Rec. 601 luminance of an RGBA pixel, ignoring its alpha.
fn luminance(pixel: &[u8; 4]) -> f64 {
    f64::from(pixel[0]) * R_TO_L + f64::from(pixel[1]) * G_TO_L + f64::from(pixel[2]) * B_TO_L
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: [u8; 4] = [0, 0, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

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

    #[test]
    fn a_region_compared_to_itself_scores_one() {
        let pixels = canvas(20, 20, rect(4, 4, 8, 8), WHITE);
        let image = Image::from_rgba8(20, 20, &pixels).unwrap();

        let score = similarity(&image, &image, rect(2, 2, 12, 12), (0, 0));

        assert!((score - 1.0).abs() < 1e-9, "score {score} is not one");
    }

    #[test]
    fn a_block_matched_at_its_displacement_scores_near_one() {
        let base = canvas(40, 20, rect(4, 6, 6, 6), WHITE);
        let head = canvas(40, 20, rect(9, 6, 6, 6), WHITE); // moved by (5, 0)
        let a = Image::from_rgba8(40, 20, &base).unwrap();
        let b = Image::from_rgba8(40, 20, &head).unwrap();

        // At the displacement the content lines up, so the score is one; in
        // place it is not.
        let matched = similarity(&a, &b, rect(4, 6, 6, 6), (5, 0));
        let in_place = similarity(&a, &b, rect(4, 6, 6, 6), (0, 0));

        assert!((matched - 1.0).abs() < 1e-9, "matched {matched}");
        assert!(
            in_place < matched,
            "in place {in_place} not below {matched}"
        );
    }

    #[test]
    fn opposite_uniform_regions_score_low() {
        // A white region against a black one: same lack of structure, opposite
        // brightness, so the score is pulled down by the luminance term.
        let white = canvas(16, 16, rect(0, 0, 16, 16), WHITE);
        let black = canvas(16, 16, rect(0, 0, 0, 0), BLACK);
        let a = Image::from_rgba8(16, 16, &white).unwrap();
        let b = Image::from_rgba8(16, 16, &black).unwrap();

        let score = similarity(&a, &b, rect(4, 4, 8, 8), (0, 0));

        assert!(score < 0.05, "score {score} is not low");
    }
}
