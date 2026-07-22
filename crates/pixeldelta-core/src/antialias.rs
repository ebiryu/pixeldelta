//! Detection of pixels that differ only because of anti-aliasing.
//!
//! Rendering the same content on two machines shifts glyph and edge coverage by
//! a fraction of a pixel, which produces differences that no one looking at the
//! two images would call a change. The detector recognizes such pixels from
//! their 3x3 neighborhood, following Vyšniauskas' anti-aliased pixel and
//! intensity slope detector.

use crate::color::brightness_delta;
use crate::image::{Image, BYTES_PER_PIXEL};

/// Whether the pixel at (`x`, `y`) sits on an anti-aliased edge of `subject`.
///
/// `other` is the image `subject` is being compared against: the extreme
/// neighbor has to look like solid content in both images, otherwise a genuine
/// change that happens to have a brighter and a darker neighbor would pass as
/// anti-aliasing.
///
/// Both images must have the dimensions of `subject`, and (`x`, `y`) must be
/// inside them.
pub fn is_antialiased(subject: &Image<'_>, other: &Image<'_>, x: u32, y: u32) -> bool {
    let width = subject.width();
    let pixels = subject.pixels();
    let (left, top, right, bottom) = neighborhood(subject, x, y);
    let center = &pixels[index(width, x, y)];
    let center_offset = index(width, x, y) * BYTES_PER_PIXEL;

    // A pixel on the border has fewer than eight neighbors. The missing ones
    // count as one equal neighbor, which is what keeps a border pixel of a
    // solid area from being read as an edge.
    let mut equal = u32::from(x == left || x == right || y == top || y == bottom);
    let (mut darkest, mut brightest) = (0.0f32, 0.0f32);
    let (mut darkest_at, mut brightest_at) = ((0, 0), (0, 0));

    for nx in left..=right {
        for ny in top..=bottom {
            if (nx, ny) == (x, y) {
                continue;
            }
            let delta = brightness_delta(center, &pixels[index(width, nx, ny)], center_offset);
            if delta == 0.0 {
                equal += 1;
                // Three equal neighbors make this a flat area, not an edge.
                if equal > 2 {
                    return false;
                }
            } else if delta < darkest {
                darkest = delta;
                darkest_at = (nx, ny);
            } else if delta > brightest {
                brightest = delta;
                brightest_at = (nx, ny);
            }
        }
    }

    // An anti-aliased pixel is a blend, so it lies between a darker and a
    // brighter neighbor. Having only one side means it is an extreme itself.
    if darkest == 0.0 || brightest == 0.0 {
        return false;
    }

    is_solid(subject, darkest_at) && is_solid(other, darkest_at)
        || is_solid(subject, brightest_at) && is_solid(other, brightest_at)
}

/// Whether the pixel at `at` has three or more neighbors of its exact color.
///
/// Such a pixel belongs to a filled area rather than to a blended edge.
fn is_solid(image: &Image<'_>, (x, y): (u32, u32)) -> bool {
    let width = image.width();
    let pixels = image.pixels();
    let (left, top, right, bottom) = neighborhood(image, x, y);
    let color = &pixels[index(width, x, y)];

    let mut equal = u32::from(x == left || x == right || y == top || y == bottom);
    for nx in left..=right {
        for ny in top..=bottom {
            if (nx, ny) == (x, y) {
                continue;
            }
            if &pixels[index(width, nx, ny)] == color {
                equal += 1;
                if equal > 2 {
                    return true;
                }
            }
        }
    }
    false
}

/// Bounds of the 3x3 neighborhood of (`x`, `y`), clipped to the image.
fn neighborhood(image: &Image<'_>, x: u32, y: u32) -> (u32, u32, u32, u32) {
    (
        x.saturating_sub(1),
        y.saturating_sub(1),
        x.saturating_add(1).min(image.width().saturating_sub(1)),
        y.saturating_add(1).min(image.height().saturating_sub(1)),
    )
}

/// Index of the pixel at (`x`, `y`) in a row-major buffer of `width` columns.
fn index(width: u32, x: u32, y: u32) -> usize {
    y as usize * width as usize + x as usize
}
