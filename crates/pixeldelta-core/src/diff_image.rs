use crate::color::luminance;
use crate::image::{Image, BYTES_PER_PIXEL};

/// Colors and dimming used when rendering a diff image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffStyle {
    /// How far unchanged pixels are pulled toward white, in `[0, 1]`.
    ///
    /// `0` leaves the background at its luminance, `1` turns it white. The
    /// default of `0.1` matches pixelmatch and keeps the differences legible
    /// against a faint copy of the original.
    pub alpha: f32,
    /// Color painted over differing pixels. The default is red.
    pub diff_color: [u8; 3],
}

impl Default for DiffStyle {
    fn default() -> Self {
        Self {
            alpha: 0.1,
            diff_color: [255, 0, 0],
        }
    }
}

/// A rendered diff image in RGBA8.
///
/// Pixels are stored row by row, four bytes per pixel, matching the layout the
/// engine compares. Encoding it to a file belongs to the layer above.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel bytes, `width * height * 4` of them.
    pub data: Vec<u8>,
}

/// Renders a diff image from the first image and the per-pixel difference flags.
///
/// Differing pixels take [`DiffStyle::diff_color`]; the rest show `a` dimmed
/// toward white, so the differences stand out against a faint copy of the
/// original. `diff` holds one flag per pixel of `a`, in the same order.
pub(crate) fn render(a: &Image<'_>, diff: &[bool], style: &DiffStyle) -> DiffImage {
    let pixels = a.pixels();
    let mut data = Vec::with_capacity(pixels.len() * BYTES_PER_PIXEL);
    let [dr, dg, db] = style.diff_color;
    for (pixel, &differs) in pixels.iter().zip(diff) {
        if differs {
            data.extend_from_slice(&[dr, dg, db, 255]);
        } else {
            let gray = background(pixel, style.alpha);
            data.extend_from_slice(&[gray, gray, gray, 255]);
        }
    }
    DiffImage {
        width: a.width(),
        height: a.height(),
        data,
    }
}

/// Luminance of a pixel pulled toward white by `alpha`, weighted by its own
/// alpha so a transparent pixel leaves the background white.
fn background(pixel: &[u8; 4], alpha: f32) -> u8 {
    let weight = alpha * f32::from(pixel[3]) / 255.0;
    let value = 255.0 + (luminance(pixel) - 255.0) * weight;
    value.round().clamp(0.0, 255.0) as u8
}
