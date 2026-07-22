//! Image comparison engine.
//!
//! The crate compares two decoded RGBA8 buffers and reports how many pixels
//! differ perceptually. It performs no I/O: decoding and encoding belong to the
//! layers above.

mod antialias;
mod color;
mod image;
mod region;

use antialias::is_antialiased;
use color::color_delta;
pub use color::MAX_COLOR_DELTA;
pub use image::{Image, ImageError, BYTES_PER_PIXEL};
use region::Mask;
pub use region::Rect;

/// Options controlling a comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareOptions {
    /// Matching threshold as a fraction of [`MAX_COLOR_DELTA`], in `[0, 1]`.
    ///
    /// A pixel counts as different once its color delta exceeds
    /// `threshold * threshold * MAX_COLOR_DELTA`. Smaller values make the
    /// comparison more sensitive. The default of `0.1` matches pixelmatch.
    /// Values outside `[0, 1]` are clamped, and a value that is not a number
    /// is treated as `0`.
    pub threshold: f32,
    /// Whether pixels that differ only by anti-aliasing are excluded.
    ///
    /// The default of `true` matches pixelmatch, whose `includeAA: false`
    /// default runs the same detector.
    pub detect_antialiasing: bool,
    /// Regions left out of the comparison.
    ///
    /// Pixels inside any of them are neither compared nor counted towards
    /// [`CompareResult::diff_ratio`]. Parts reaching outside the image are
    /// ignored.
    pub ignore_regions: Vec<Rect>,
    /// Limit at which the comparison stops before reaching the last pixel.
    ///
    /// `None`, the default, walks every pixel and reports exact counts.
    pub fail_fast: Option<FailFast>,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            detect_antialiasing: true,
            ignore_regions: Vec::new(),
            fail_fast: None,
        }
    }
}

/// How much difference a comparison looks for before giving up.
///
/// A caller that only needs to know whether two images differ can stop at the
/// first difference instead of counting all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailFast {
    /// Number of differing pixels to tolerate before stopping.
    ///
    /// The scan stops once more than this many pixels have been found, so `0`
    /// stops at the first difference.
    pub max_diff_pixels: u64,
}

impl CompareOptions {
    /// Color delta above which a pixel counts as different.
    ///
    /// Thresholds outside `[0, 1]` are clamped. A threshold that is not a
    /// number becomes `0`, so a misconfigured comparison reports every
    /// difference rather than reporting a match and hiding all of them.
    fn max_delta(&self) -> f32 {
        let t = if self.threshold.is_nan() {
            0.0
        } else {
            self.threshold.clamp(0.0, 1.0)
        };
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
    /// Whether the comparison stopped at the [`FailFast`] limit.
    ///
    /// When it did, `diff_pixels` and `diff_ratio` are lower bounds rather
    /// than counts of the whole image.
    pub stopped_early: bool,
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
            stopped_early: false,
        };
    }

    let max_delta = opts.max_delta();
    let mask = Mask::new(&opts.ignore_regions, a.width(), a.height());
    let (left, right) = (a.pixels(), b.pixels());

    // The scan runs until it has found more differing pixels than the limit
    // tolerates, so a limit of zero stops at the first difference.
    let limit = opts.fail_fast.map_or(u64::MAX, |fail_fast| {
        fail_fast.max_diff_pixels.saturating_add(1)
    });

    let mut diff_pixels = 0u64;
    let mut compared = 0u64;
    let mut stopped_early = false;
    'scan: for y in 0..a.height() {
        let row = y as usize * a.width() as usize;
        for x in 0..a.width() {
            if mask.excludes(x, y) {
                continue;
            }
            compared += 1;

            let index = row + x as usize;
            if color_delta(&left[index], &right[index], index * BYTES_PER_PIXEL) <= max_delta {
                continue;
            }
            // Anti-aliasing is decided only for pixels that already differ. It
            // reads eight neighbors per image, so running it on every pixel
            // would cost more than the comparison itself.
            if opts.detect_antialiasing
                && (is_antialiased(a, b, x, y) || is_antialiased(b, a, x, y))
            {
                continue;
            }
            diff_pixels += 1;
            if diff_pixels >= limit {
                stopped_early = true;
                break 'scan;
            }
        }
    }

    CompareResult {
        verdict: if diff_pixels == 0 {
            Verdict::Match
        } else {
            Verdict::Differ
        },
        diff_pixels,
        diff_ratio: if compared == 0 {
            0.0
        } else {
            diff_pixels as f64 / compared as f64
        },
        stopped_early,
    }
}
