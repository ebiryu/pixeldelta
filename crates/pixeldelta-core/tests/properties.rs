//! Invariants that hold for any pair of images.
//!
//! The fixtures in `compat.rs` pin the counts for a handful of images. These
//! properties cover the inputs no one thought to write a fixture for: a
//! comparison that reports a difference between an image and itself, or that
//! answers differently depending on the argument order, is wrong whatever the
//! fixtures say.

use pixeldelta_core::{compare, CompareOptions, FailFast, Image, Rect, Verdict};
use proptest::prelude::*;

const MAX_SIDE: u32 = 12;

/// Pixels drawn from a small palette, so that flat areas, edges and
/// semi-transparent pixels all occur often enough to reach every branch of the
/// comparison. Uniformly random bytes would produce neither flat areas nor
/// edges.
fn pixels(count: usize) -> impl Strategy<Value = Vec<u8>> {
    let palette = vec![
        [0u8, 0, 0, 255],
        [255, 255, 255, 255],
        [128, 128, 128, 255],
        [40, 90, 200, 255],
        [255, 255, 255, 128],
        [40, 90, 200, 0],
    ];
    proptest::collection::vec(proptest::sample::select(palette), count)
        .prop_map(|pixels| pixels.concat())
}

/// Two images of the same random dimensions.
fn image_pair() -> impl Strategy<Value = (u32, u32, Vec<u8>, Vec<u8>)> {
    (1..=MAX_SIDE, 1..=MAX_SIDE).prop_flat_map(|(width, height)| {
        let count = (width * height) as usize;
        (Just(width), Just(height), pixels(count), pixels(count))
    })
}

/// Options covering both anti-aliasing settings and the range of thresholds.
fn options() -> impl Strategy<Value = CompareOptions> {
    (0.0f32..=1.0, any::<bool>()).prop_map(|(threshold, detect_antialiasing)| CompareOptions {
        threshold,
        detect_antialiasing,
        ..CompareOptions::default()
    })
}

proptest! {
    #[test]
    fn an_image_never_differs_from_itself(
        (width, height, data, _) in image_pair(),
        opts in options(),
    ) {
        let image = Image::from_rgba8(width, height, &data).unwrap();

        let result = compare(&image, &image, &opts);

        prop_assert_eq!(result.verdict, Verdict::Match);
        prop_assert_eq!(result.diff_pixels, 0);
        prop_assert_eq!(result.diff_ratio, 0.0);
    }

    #[test]
    fn the_argument_order_does_not_matter(
        (width, height, first, second) in image_pair(),
        opts in options(),
    ) {
        let a = Image::from_rgba8(width, height, &first).unwrap();
        let b = Image::from_rgba8(width, height, &second).unwrap();

        prop_assert_eq!(compare(&a, &b, &opts), compare(&b, &a, &opts));
    }

    #[test]
    fn a_wider_ignored_region_never_adds_differences(
        (width, height, first, second) in image_pair(),
        opts in options(),
        (x, y, region_width, region_height) in (0..MAX_SIDE, 0..MAX_SIDE, 0..MAX_SIDE, 0..MAX_SIDE),
        growth in 0..MAX_SIDE,
    ) {
        let a = Image::from_rgba8(width, height, &first).unwrap();
        let b = Image::from_rgba8(width, height, &second).unwrap();
        let inner = Rect { x, y, width: region_width, height: region_height };
        // The same region grown by `growth` on every side.
        let outer = Rect {
            x: x.saturating_sub(growth),
            y: y.saturating_sub(growth),
            width: region_width.saturating_add(2 * growth),
            height: region_height.saturating_add(2 * growth),
        };

        let narrow = compare(&a, &b, &CompareOptions {
            ignore_regions: vec![inner],
            ..opts.clone()
        });
        let wide = compare(&a, &b, &CompareOptions {
            ignore_regions: vec![outer],
            ..opts
        });

        prop_assert!(
            wide.diff_pixels <= narrow.diff_pixels,
            "{:?} left {} differences, {:?} left {}",
            outer, wide.diff_pixels, inner, narrow.diff_pixels
        );
    }

    #[test]
    fn a_fail_fast_scan_stops_one_pixel_past_the_limit(
        (width, height, first, second) in image_pair(),
        opts in options(),
        max_diff_pixels in 0u64..8,
    ) {
        let a = Image::from_rgba8(width, height, &first).unwrap();
        let b = Image::from_rgba8(width, height, &second).unwrap();

        let full = compare(&a, &b, &opts);
        let limited = compare(&a, &b, &CompareOptions {
            fail_fast: Some(FailFast { max_diff_pixels }),
            ..opts
        });

        prop_assert!(limited.diff_pixels <= full.diff_pixels);
        prop_assert!(limited.diff_pixels <= max_diff_pixels + 1);
        prop_assert_eq!(limited.verdict, full.verdict);
        // Stopping means the limit was passed, and passing it means stopping.
        prop_assert_eq!(limited.stopped_early, full.diff_pixels > max_diff_pixels);
    }
}
