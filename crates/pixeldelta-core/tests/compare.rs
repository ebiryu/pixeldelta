use pixeldelta_core::{compare, CompareOptions, FailFast, Image, Rect, Verdict};

/// Builds a solid image of `width` x `height` opaque pixels.
fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    rgba.iter()
        .copied()
        .cycle()
        .take(width as usize * height as usize * 4)
        .collect()
}

#[test]
fn identical_images_match() {
    let buf = solid(4, 4, [10, 20, 30, 255]);
    let a = Image::from_rgba8(4, 4, &buf).unwrap();
    let b = Image::from_rgba8(4, 4, &buf).unwrap();

    let result = compare(&a, &b, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
    assert_eq!(result.diff_ratio, 0.0);
}

#[test]
fn a_single_changed_pixel_is_counted() {
    let base = solid(4, 4, [0, 0, 0, 255]);
    let mut changed = base.clone();
    changed[4..8].copy_from_slice(&[255, 255, 255, 255]);
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(&a, &b, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 1);
    assert_eq!(result.diff_ratio, 1.0 / 16.0);
}

#[test]
fn differing_dimensions_report_a_size_mismatch() {
    let small = solid(2, 2, [0, 0, 0, 255]);
    let large = solid(2, 3, [0, 0, 0, 255]);
    let a = Image::from_rgba8(2, 2, &small).unwrap();
    let b = Image::from_rgba8(2, 3, &large).unwrap();

    let result = compare(&a, &b, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::SizeMismatch);
    assert_eq!(result.diff_pixels, 0);
}

#[test]
fn the_threshold_decides_whether_a_small_delta_counts() {
    let base = solid(2, 2, [120, 120, 120, 255]);
    let mut nudged = base.clone();
    nudged[0..4].copy_from_slice(&[124, 124, 124, 255]);
    let a = Image::from_rgba8(2, 2, &base).unwrap();
    let b = Image::from_rgba8(2, 2, &nudged).unwrap();

    let lenient = compare(
        &a,
        &b,
        &CompareOptions {
            threshold: 0.1,
            ..CompareOptions::default()
        },
    );
    assert_eq!(lenient.diff_pixels, 0);
    assert_eq!(lenient.verdict, Verdict::Match);

    let strict = compare(
        &a,
        &b,
        &CompareOptions {
            threshold: 0.0,
            ..CompareOptions::default()
        },
    );
    assert_eq!(strict.diff_pixels, 1);
    assert_eq!(strict.verdict, Verdict::Differ);
}

#[test]
fn a_threshold_outside_the_unit_range_is_clamped() {
    let base = solid(2, 2, [0, 0, 0, 255]);
    let mut changed = base.clone();
    changed[0..4].copy_from_slice(&[255, 255, 255, 255]);
    let a = Image::from_rgba8(2, 2, &base).unwrap();
    let b = Image::from_rgba8(2, 2, &changed).unwrap();

    // Above 1.0 nothing can exceed the threshold, below 0.0 anything can.
    assert_eq!(
        compare(
            &a,
            &b,
            &CompareOptions {
                threshold: 4.0,
                ..CompareOptions::default()
            }
        )
        .diff_pixels,
        0
    );
    assert_eq!(
        compare(
            &a,
            &b,
            &CompareOptions {
                threshold: -1.0,
                ..CompareOptions::default()
            }
        )
        .diff_pixels,
        1
    );
}

#[test]
fn a_threshold_that_is_not_a_number_does_not_hide_differences() {
    let base = solid(2, 2, [0, 0, 0, 255]);
    let mut changed = base.clone();
    changed[0..4].copy_from_slice(&[255, 255, 255, 255]);
    let a = Image::from_rgba8(2, 2, &base).unwrap();
    let b = Image::from_rgba8(2, 2, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            threshold: f32::NAN,
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 1);
}

/// Builds an 8x8 image split into a black and a white half, with column 3
/// blended to `edge` so that the boundary can be moved by part of a pixel.
fn split(edge: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 * 8 * 4);
    for _ in 0..8 {
        for x in 0..8u32 {
            let level = match x {
                0..=2 => 0,
                3 => edge,
                _ => 255,
            };
            buf.extend_from_slice(&[level, level, level, 255]);
        }
    }
    buf
}

#[test]
fn a_blended_edge_is_a_difference_only_when_detection_is_off() {
    let base = split(0);
    let shifted = split(128);
    let a = Image::from_rgba8(8, 8, &base).unwrap();
    let b = Image::from_rgba8(8, 8, &shifted).unwrap();

    let detected = compare(&a, &b, &CompareOptions::default());
    assert_eq!(detected.verdict, Verdict::Match);
    assert_eq!(detected.diff_pixels, 0);

    let raw = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            ..CompareOptions::default()
        },
    );
    assert_eq!(raw.verdict, Verdict::Differ);
    assert_eq!(raw.diff_pixels, 8);
}

#[test]
fn detection_keeps_a_change_that_fills_an_area() {
    // A block large enough that its pixels have three equal neighbors, which
    // is what separates content from a blended edge.
    let base = solid(8, 8, [255, 255, 255, 255]);
    let mut changed = base.clone();
    for y in 2..6 {
        for x in 2..6 {
            let pos = (y * 8 + x) * 4;
            changed[pos..pos + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    let a = Image::from_rgba8(8, 8, &base).unwrap();
    let b = Image::from_rgba8(8, 8, &changed).unwrap();

    let result = compare(&a, &b, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 16);
}

/// Builds a 4x4 black image with the four pixels of the top left quadrant
/// turned white.
fn quadrant_changed() -> (Vec<u8>, Vec<u8>) {
    let base = solid(4, 4, [0, 0, 0, 255]);
    let mut changed = base.clone();
    for y in 0..2 {
        for x in 0..2 {
            let pos = (y * 4 + x) * 4;
            changed[pos..pos + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    (base, changed)
}

#[test]
fn a_difference_inside_an_ignored_region_is_not_counted() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            ignore_regions: vec![Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            }],
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
}

#[test]
fn an_ignored_region_leaves_the_difference_beside_it() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            // Covers the left column of the changed quadrant only.
            ignore_regions: vec![Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 2,
            }],
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 2);
    // Two of the sixteen pixels were left out of the comparison.
    assert_eq!(result.diff_ratio, 2.0 / 14.0);
}

#[test]
fn an_ignored_region_reaching_outside_the_image_is_clipped() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            ignore_regions: vec![Rect {
                x: 0,
                y: 0,
                width: u32::MAX,
                height: u32::MAX,
            }],
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
    assert_eq!(result.diff_ratio, 0.0);
}

#[test]
fn an_ignored_region_still_provides_neighbors_to_the_detector() {
    // The blended column keeps its dark side, which lies inside the ignored
    // region. Without those neighbors it would have only brighter ones and
    // would no longer read as an edge.
    let base = split(0);
    let shifted = split(128);
    let a = Image::from_rgba8(8, 8, &base).unwrap();
    let b = Image::from_rgba8(8, 8, &shifted).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            ignore_regions: vec![Rect {
                x: 0,
                y: 0,
                width: 3,
                height: 8,
            }],
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
}

#[test]
fn a_full_comparison_is_not_marked_as_stopped() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.diff_pixels, 4);
    assert!(!result.stopped_early);
}

#[test]
fn the_scan_stops_once_the_limit_is_passed() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            fail_fast: Some(FailFast { max_diff_pixels: 1 }),
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 2);
    assert!(result.stopped_early);
}

#[test]
fn a_limit_the_image_stays_under_leaves_the_counts_exact() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            fail_fast: Some(FailFast { max_diff_pixels: 4 }),
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.diff_pixels, 4);
    assert_eq!(result.diff_ratio, 4.0 / 16.0);
    assert!(!result.stopped_early);
}

#[test]
fn a_limit_of_zero_stops_at_the_first_difference() {
    let (base, changed) = quadrant_changed();
    let a = Image::from_rgba8(4, 4, &base).unwrap();
    let b = Image::from_rgba8(4, 4, &changed).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            detect_antialiasing: false,
            fail_fast: Some(FailFast { max_diff_pixels: 0 }),
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 1);
    // The first pixel of the image is the first difference.
    assert_eq!(result.diff_ratio, 1.0);
    assert!(result.stopped_early);
}

#[test]
fn comparison_is_commutative() {
    let base = solid(3, 3, [10, 200, 40, 255]);
    let mut other = base.clone();
    other[0..4].copy_from_slice(&[200, 10, 40, 255]);
    other[16..20].copy_from_slice(&[0, 0, 0, 255]);
    let a = Image::from_rgba8(3, 3, &base).unwrap();
    let b = Image::from_rgba8(3, 3, &other).unwrap();
    let opts = CompareOptions::default();

    assert_eq!(compare(&a, &b, &opts), compare(&b, &a, &opts));
}

#[test]
fn transparent_pixels_are_compared_over_the_same_background() {
    // Two fully transparent pixels are equal regardless of their color bytes.
    let a_buf = [255u8, 0, 0, 0];
    let b_buf = [0u8, 0, 255, 0];
    let a = Image::from_rgba8(1, 1, &a_buf).unwrap();
    let b = Image::from_rgba8(1, 1, &b_buf).unwrap();

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            threshold: 0.0,
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
}

/// Width and height of the images used for the cases below.
///
/// The scan splits an image of this many pixels into blocks handed to several
/// threads, while the images in the other tests are small enough to stay on
/// one. The counts here are what says the split changes nothing.
const LARGE_SIDE: u32 = 1024;

/// A `LARGE_SIDE` square, and a copy of it whose every `nth` pixel is white.
fn large_pair(nth: usize) -> (Vec<u8>, Vec<u8>) {
    let base = solid(LARGE_SIDE, LARGE_SIDE, [0, 0, 0, 255]);
    let mut changed = base.clone();
    for pixel in changed.chunks_exact_mut(4).step_by(nth) {
        pixel.copy_from_slice(&[255, 255, 255, 255]);
    }
    (base, changed)
}

#[test]
fn an_image_split_across_threads_counts_every_difference() {
    let (base, changed) = large_pair(64);
    let a = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &base).unwrap();
    let b = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &changed).unwrap();
    let pixels = (LARGE_SIDE * LARGE_SIDE) as u64;

    let result = compare(&a, &b, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, pixels / 64);
    assert_eq!(result.diff_ratio, (pixels / 64) as f64 / pixels as f64);
}

#[test]
fn an_image_split_across_threads_matches_itself() {
    let (base, _) = large_pair(64);
    let a = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &base).unwrap();

    let result = compare(&a, &a, &CompareOptions::default());

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
    assert_eq!(result.diff_ratio, 0.0);
}

#[test]
fn an_ignored_region_holds_across_threads() {
    let (base, changed) = large_pair(64);
    let a = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &base).unwrap();
    let b = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &changed).unwrap();
    // The top half, which covers whole blocks as well as the boundary of one.
    let ignored = Rect {
        x: 0,
        y: 0,
        width: LARGE_SIDE,
        height: LARGE_SIDE / 2,
    };
    let pixels = (LARGE_SIDE * LARGE_SIDE) as u64;

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            ignore_regions: vec![ignored],
            ..CompareOptions::default()
        },
    );

    assert_eq!(result.diff_pixels, pixels / 64 / 2);
    assert_eq!(
        result.diff_ratio,
        (pixels / 64 / 2) as f64 / (pixels / 2) as f64
    );
}

#[test]
fn a_limited_scan_of_a_large_image_stops_past_the_limit() {
    let (base, changed) = large_pair(64);
    let a = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &base).unwrap();
    let b = Image::from_rgba8(LARGE_SIDE, LARGE_SIDE, &changed).unwrap();
    let limit = 100;

    let result = compare(
        &a,
        &b,
        &CompareOptions {
            fail_fast: Some(FailFast {
                max_diff_pixels: limit,
            }),
            ..CompareOptions::default()
        },
    );

    // Blocks running at once can pass the limit together, so the count is a
    // lower bound rather than the limit plus one.
    assert!(result.stopped_early);
    assert!(result.diff_pixels > limit, "{} pixels", result.diff_pixels);
    assert!(result.diff_pixels < (LARGE_SIDE * LARGE_SIDE) as u64 / 64);
    assert_eq!(result.verdict, Verdict::Differ);
}
