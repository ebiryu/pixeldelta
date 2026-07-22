//! Image comparison engine.
//!
//! The crate compares two decoded RGBA8 buffers and reports how many pixels
//! differ perceptually. It performs no I/O: decoding and encoding belong to the
//! layers above.

mod color;
mod image;

use color::color_delta;
pub use color::MAX_COLOR_DELTA;
pub use image::{Image, ImageError, BYTES_PER_PIXEL};

/// Options controlling a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareOptions {
    /// Matching threshold as a fraction of [`MAX_COLOR_DELTA`], in `[0, 1]`.
    ///
    /// A pixel counts as different once its color delta exceeds
    /// `threshold * threshold * MAX_COLOR_DELTA`. Smaller values make the
    /// comparison more sensitive. The default of `0.1` matches pixelmatch.
    pub threshold: f32,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self { threshold: 0.1 }
    }
}

impl CompareOptions {
    /// Color delta above which a pixel counts as different.
    fn max_delta(&self) -> f32 {
        let t = self.threshold.clamp(0.0, 1.0);
        MAX_COLOR_DELTA * t * t
    }
}

/// Outcome of a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No pixel exceeded the threshold.
    Match,
    /// At least one pixel exceeded the threshold.
    Differ,
    /// The images have different dimensions and were not compared.
    SizeMismatch,
}

/// Result of a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareResult {
    /// Whether the images match.
    pub verdict: Verdict,
    /// Number of pixels whose color delta exceeded the threshold.
    pub diff_pixels: u64,
    /// `diff_pixels` divided by the number of compared pixels, or `0.0` when
    /// nothing was compared.
    pub diff_ratio: f64,
}

/// Compares two images pixel by pixel.
///
/// Images of different dimensions are not compared: the result carries
/// [`Verdict::SizeMismatch`] and no pixel counts.
pub fn compare(a: &Image<'_>, b: &Image<'_>, opts: &CompareOptions) -> CompareResult {
    if !a.same_size_as(b) {
        return CompareResult {
            verdict: Verdict::SizeMismatch,
            diff_pixels: 0,
            diff_ratio: 0.0,
        };
    }

    let max_delta = opts.max_delta();
    let (left, _) = a.as_bytes().as_chunks::<BYTES_PER_PIXEL>();
    let (right, _) = b.as_bytes().as_chunks::<BYTES_PER_PIXEL>();

    let mut diff_pixels = 0u64;
    for (pa, pb) in left.iter().zip(right) {
        if color_delta(pa, pb) > max_delta {
            diff_pixels += 1;
        }
    }

    let total = a.pixel_count();
    CompareResult {
        verdict: if diff_pixels == 0 {
            Verdict::Match
        } else {
            Verdict::Differ
        },
        diff_pixels,
        diff_ratio: if total == 0 {
            0.0
        } else {
            diff_pixels as f64 / total as f64
        },
    }
}
