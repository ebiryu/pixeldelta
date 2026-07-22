//! The scan that counts differing pixels, over one thread or several.
//!
//! Work is split into blocks of rows. Blocks share nothing but the count that
//! [`FailFast`](crate::FailFast) checks against, so the only ordering the
//! result depends on is the one a limited scan stops at.

use core::ops::Range;
use core::sync::atomic::{AtomicU64, Ordering};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::antialias::is_antialiased;
use crate::color::color_delta;
use crate::image::{Image, BYTES_PER_PIXEL};
use crate::region::Mask;

/// Rows one block covers.
///
/// A block has to be long enough to cover the cost of handing it to another
/// thread, and short enough that the last block does not decide when the whole
/// scan finishes.
const ROWS_PER_BLOCK: u32 = 64;

/// Pixel count below which the scan stays on the calling thread.
///
/// Below it, distributing the work costs more than the work itself.
const PARALLEL_MIN_PIXELS: u64 = 256 * 1024;

/// What a scan counted.
#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    /// Pixels whose color delta exceeded the threshold.
    pub diff_pixels: u64,
    /// Pixels that were compared, which excludes the masked ones.
    pub compared: u64,
    /// Whether a block stopped at the limit.
    pub stopped_early: bool,
}

impl Counts {
    fn merge(self, other: Self) -> Self {
        Self {
            diff_pixels: self.diff_pixels + other.diff_pixels,
            compared: self.compared + other.compared,
            stopped_early: self.stopped_early || other.stopped_early,
        }
    }
}

/// What the scan compares against, derived from the caller's options.
pub struct Params {
    /// Color delta above which a pixel counts as different.
    pub max_delta: f32,
    /// Whether differing pixels are checked for anti-aliasing.
    pub detect_antialiasing: bool,
    /// The regions left out of the comparison.
    pub mask: Mask,
    /// Number of differing pixels at which the scan stops, if there is one.
    pub limit: Option<u64>,
}

/// Differing pixels found so far, and the count at which the scan stops.
///
/// Blocks read it to notice that another block has already reached the limit.
struct Budget {
    found: AtomicU64,
    limit: u64,
}

impl Budget {
    fn reached(&self) -> bool {
        self.found.load(Ordering::Relaxed) >= self.limit
    }

    /// Records one differing pixel and reports whether the limit is now met.
    fn record(&self) -> bool {
        self.found.fetch_add(1, Ordering::Relaxed) + 1 >= self.limit
    }
}

/// Counts the pixels of `a` and `b` that differ.
///
/// Both images must have the same dimensions.
pub fn scan(a: &Image<'_>, b: &Image<'_>, params: &Params) -> Counts {
    let budget = params.limit.map(|limit| Budget {
        found: AtomicU64::new(0),
        limit,
    });

    if a.pixel_count() < PARALLEL_MIN_PIXELS {
        return scan_rows(a, b, params, budget.as_ref(), 0..a.height());
    }

    let blocks = a.height().div_ceil(ROWS_PER_BLOCK);
    (0..blocks)
        .into_par_iter()
        .map(|block| {
            let first = block * ROWS_PER_BLOCK;
            let last = (first + ROWS_PER_BLOCK).min(a.height());
            scan_rows(a, b, params, budget.as_ref(), first..last)
        })
        .reduce(Counts::default, Counts::merge)
}

/// Counts the differing pixels of the rows in `rows`.
fn scan_rows(
    a: &Image<'_>,
    b: &Image<'_>,
    params: &Params,
    budget: Option<&Budget>,
    rows: Range<u32>,
) -> Counts {
    let (left, right) = (a.pixels(), b.pixels());
    let width = a.width();
    let mut counts = Counts::default();

    for y in rows {
        // Another block may have reached the limit while this one was running.
        if budget.is_some_and(Budget::reached) {
            counts.stopped_early = true;
            return counts;
        }

        let row = y as usize * width as usize;
        for x in 0..width {
            if params.mask.excludes(x, y) {
                continue;
            }
            counts.compared += 1;

            let index = row + x as usize;
            if color_delta(&left[index], &right[index], index * BYTES_PER_PIXEL) <= params.max_delta
            {
                continue;
            }
            // Anti-aliasing is decided only for pixels that already differ. It
            // reads eight neighbors per image, so running it on every pixel
            // would cost more than the comparison itself.
            if params.detect_antialiasing
                && (is_antialiased(a, b, x, y) || is_antialiased(b, a, x, y))
            {
                continue;
            }
            counts.diff_pixels += 1;
            if budget.is_some_and(Budget::record) {
                counts.stopped_early = true;
                return counts;
            }
        }
    }

    counts
}
