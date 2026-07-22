use pixeldelta_core::{compare, CompareOptions, Image, Rect, Verdict};

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
