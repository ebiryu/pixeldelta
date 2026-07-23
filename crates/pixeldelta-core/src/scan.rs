//! The scan that counts differing pixels, over one thread or several.
//!
//! Work is split into blocks of rows. Blocks share nothing but the count that
//! [`FailFast`](crate::FailFast) checks against, so the only ordering the
//! result depends on is the one a limited scan stops at.

use core::ops::Range;
use core::sync::atomic::{AtomicU64, Ordering};

use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use rayon::slice::ParallelSliceMut;

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

/// Pixels a row is compared for equality at a time.
///
/// Eight pixels are 32 bytes, which the equality check reads as whole machine
/// words rather than pixel by pixel.
const PIXELS_PER_CHUNK: usize = 8;

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

/// Where a block's counts and, when clustering, its diff pixels accumulate.
///
/// The bitmap is the block's own rows, so its first row is subtracted from a
/// pixel's `y` before indexing. When no bitmap is present the scan only counts.
struct Sink<'a> {
    counts: Counts,
    bitmap: Option<&'a mut [bool]>,
    first_row: u32,
    width: u32,
}

impl Sink<'_> {
    /// Counts the pixel at (`x`, `y`) as a difference and, when a bitmap is
    /// present, marks it there.
    fn mark(&mut self, x: u32, y: u32) {
        self.counts.diff_pixels += 1;
        if let Some(bitmap) = self.bitmap.as_deref_mut() {
            let local = (y - self.first_row) as usize * self.width as usize + x as usize;
            bitmap[local] = true;
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

/// Counts the pixels of `a` and `b` that differ, and marks them in `bitmap`
/// when one is given.
///
/// Both images must have the same dimensions, and `bitmap`, when present, must
/// hold one entry per pixel.
pub fn scan(a: &Image<'_>, b: &Image<'_>, params: &Params, bitmap: Option<&mut [bool]>) -> Counts {
    let budget = params.limit.map(|limit| Budget {
        found: AtomicU64::new(0),
        limit,
    });

    if a.pixel_count() < PARALLEL_MIN_PIXELS {
        return scan_rows(a, b, params, budget.as_ref(), 0..a.height(), bitmap, 0);
    }

    let stride = a.width() as usize;
    let block = |first: u32, bitmap: Option<&mut [bool]>| {
        let last = (first + ROWS_PER_BLOCK).min(a.height());
        scan_rows(a, b, params, budget.as_ref(), first..last, bitmap, first)
    };

    // Each block owns a disjoint range of rows, so the bitmap is split into the
    // same ranges and handed out without any sharing.
    match bitmap {
        None => {
            let blocks = a.height().div_ceil(ROWS_PER_BLOCK);
            (0..blocks)
                .into_par_iter()
                .map(|index| block(index * ROWS_PER_BLOCK, None))
                .reduce(Counts::default, Counts::merge)
        }
        Some(bitmap) => bitmap
            .par_chunks_mut(ROWS_PER_BLOCK as usize * stride)
            .enumerate()
            .map(|(index, rows)| block(index as u32 * ROWS_PER_BLOCK, Some(rows)))
            .reduce(Counts::default, Counts::merge),
    }
}

/// Counts the differing pixels of the rows in `rows`, marking them in `bitmap`
/// when present. `first_row` is the row `bitmap` starts at.
fn scan_rows(
    a: &Image<'_>,
    b: &Image<'_>,
    params: &Params,
    budget: Option<&Budget>,
    rows: Range<u32>,
    bitmap: Option<&mut [bool]>,
    first_row: u32,
) -> Counts {
    let mut sink = Sink {
        counts: Counts::default(),
        bitmap,
        first_row,
        width: a.width(),
    };

    for y in rows {
        // Another block may have reached the limit while this one was running.
        if budget.is_some_and(Budget::reached) {
            sink.counts.stopped_early = true;
            return sink.counts;
        }

        // A row an ignored region reaches has to answer for each of its pixels
        // whether it is compared at all, which the chunked path cannot do.
        let outcome = if params.mask.reaches_row(y) {
            scan_row_pixel_by_pixel(a, b, params, budget, y, &mut sink)
        } else {
            let outcome = scan_row_in_chunks(a, b, params, budget, y, &mut sink);
            // Every pixel of the row is compared, up to the one the scan
            // stopped at.
            sink.counts.compared += match outcome {
                Row::Finished => u64::from(a.width()),
                Row::Stopped(x) => u64::from(x) + 1,
            };
            outcome
        };
        if let Row::Stopped(_) = outcome {
            sink.counts.stopped_early = true;
            return sink.counts;
        }
    }

    sink.counts
}

/// Where a row scan ended.
enum Row {
    /// Every pixel of the row was examined.
    Finished,
    /// The limit was reached at this column.
    Stopped(u32),
}

/// Counts the differing pixels of row `y`, [`PIXELS_PER_CHUNK`] at a time.
///
/// Every pixel of the row is compared, and counting them is left to the
/// caller, which knows from the outcome how far the scan got.
fn scan_row_in_chunks(
    a: &Image<'_>,
    b: &Image<'_>,
    params: &Params,
    budget: Option<&Budget>,
    y: u32,
    sink: &mut Sink<'_>,
) -> Row {
    let width = a.width() as usize;
    let row = y as usize * width;
    let (left, right) = (&a.pixels()[row..row + width], &b.pixels()[row..row + width]);
    let (left_chunks, left_tail) = left.as_chunks::<PIXELS_PER_CHUNK>();
    let (right_chunks, right_tail) = right.as_chunks::<PIXELS_PER_CHUNK>();

    for (chunk, (first, second)) in left_chunks.iter().zip(right_chunks).enumerate() {
        // Screenshots repeat far more than they change, so most chunks are
        // byte for byte the same. Comparing all of their bytes at once leaves
        // the metric for the chunks that are not.
        if first == second {
            continue;
        }

        for lane in 0..PIXELS_PER_CHUNK {
            let index = row + chunk * PIXELS_PER_CHUNK + lane;
            let column = (chunk * PIXELS_PER_CHUNK + lane) as u32;
            if color_delta(&first[lane], &second[lane], index * BYTES_PER_PIXEL) > params.max_delta
                && !take_difference(a, b, params, budget, column, y, sink)
            {
                return Row::Stopped(column);
            }
        }
    }

    let offset = width - left_tail.len();
    for (lane, (first, second)) in left_tail.iter().zip(right_tail).enumerate() {
        let index = row + offset + lane;
        let column = (offset + lane) as u32;
        if color_delta(first, second, index * BYTES_PER_PIXEL) > params.max_delta
            && !take_difference(a, b, params, budget, column, y, sink)
        {
            return Row::Stopped(column);
        }
    }

    Row::Finished
}

/// Counts the differing pixels of row `y`, asking the mask about each one.
///
/// Pixels an ignored region covers are neither compared nor counted, so this
/// path counts the compared ones itself.
fn scan_row_pixel_by_pixel(
    a: &Image<'_>,
    b: &Image<'_>,
    params: &Params,
    budget: Option<&Budget>,
    y: u32,
    sink: &mut Sink<'_>,
) -> Row {
    let width = a.width();
    let row = y as usize * width as usize;
    let (left, right) = (a.pixels(), b.pixels());

    for x in 0..width {
        if params.mask.excludes(x, y) {
            continue;
        }
        sink.counts.compared += 1;

        let index = row + x as usize;
        if color_delta(&left[index], &right[index], index * BYTES_PER_PIXEL) > params.max_delta
            && !take_difference(a, b, params, budget, x, y, sink)
        {
            return Row::Stopped(x);
        }
    }

    Row::Finished
}

/// Counts the pixel at (`x`, `y`) as a difference unless it only differs by
/// anti-aliasing.
///
/// The caller has established that the pixel is compared at all: neither path
/// reaches this for a pixel an ignored region covers.
///
/// Returns whether the scan can go on rather than having reached the limit.
fn take_difference(
    a: &Image<'_>,
    b: &Image<'_>,
    params: &Params,
    budget: Option<&Budget>,
    x: u32,
    y: u32,
    sink: &mut Sink<'_>,
) -> bool {
    // Anti-aliasing is decided only for pixels that already differ. It reads
    // eight neighbors per image, so running it on every pixel would cost more
    // than the comparison itself.
    if params.detect_antialiasing && (is_antialiased(a, b, x, y) || is_antialiased(b, a, x, y)) {
        return true;
    }
    sink.mark(x, y);
    !budget.is_some_and(Budget::record)
}
