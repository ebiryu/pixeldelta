use pixeldelta_core::{compare, CompareOptions, Image, Verdict};

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

    let lenient = compare(&a, &b, &CompareOptions { threshold: 0.1 });
    assert_eq!(lenient.diff_pixels, 0);
    assert_eq!(lenient.verdict, Verdict::Match);

    let strict = compare(&a, &b, &CompareOptions { threshold: 0.0 });
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
        compare(&a, &b, &CompareOptions { threshold: 4.0 }).diff_pixels,
        0
    );
    assert_eq!(
        compare(&a, &b, &CompareOptions { threshold: -1.0 }).diff_pixels,
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
        },
    );

    assert_eq!(result.verdict, Verdict::Differ);
    assert_eq!(result.diff_pixels, 1);
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

    let result = compare(&a, &b, &CompareOptions { threshold: 0.0 });

    assert_eq!(result.verdict, Verdict::Match);
    assert_eq!(result.diff_pixels, 0);
}
