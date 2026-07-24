//! Rendering a diff image from a comparison.

use pixeldelta_core::{compare, CompareOptions, DiffStyle, Image};

fn row(data: &[u8]) -> Image<'_> {
    Image::from_rgba8((data.len() / 4) as u32, 1, data).expect("a valid row")
}

#[test]
fn no_diff_image_is_rendered_unless_asked() {
    let a = row(&[0, 0, 0, 255, 255, 0, 0, 255]);
    let b = row(&[0, 0, 0, 255, 0, 0, 255, 255]);

    let result = compare(&a, &b, &CompareOptions::default());

    assert!(result.diff_image.is_none());
}

#[test]
fn differing_pixels_are_painted_the_diff_color() {
    let a = row(&[0, 0, 0, 255, 255, 0, 0, 255]);
    let b = row(&[0, 0, 0, 255, 0, 0, 255, 255]);
    let opts = CompareOptions {
        diff: Some(DiffStyle::default()),
        ..Default::default()
    };

    let diff = compare(&a, &b, &opts)
        .diff_image
        .expect("a diff image was asked for");

    assert_eq!((diff.width, diff.height), (2, 1));
    // The changed pixel carries the default diff color, opaque.
    assert_eq!(&diff.data[4..8], &[255, 0, 0, 255]);
    // The unchanged black pixel is dimmed toward white: gray, lighter than the
    // original, and opaque.
    let (r, g, b_, alpha) = (diff.data[0], diff.data[1], diff.data[2], diff.data[3]);
    assert_eq!((g, b_), (r, r));
    assert!(
        r > 128,
        "the background pixel {r} was not dimmed toward white"
    );
    assert_eq!(alpha, 255);
}

#[test]
fn a_matching_comparison_renders_only_background() {
    let a = row(&[10, 20, 30, 255, 40, 50, 60, 255]);
    let opts = CompareOptions {
        diff: Some(DiffStyle::default()),
        ..Default::default()
    };

    let diff = compare(&a, &a, &opts)
        .diff_image
        .expect("a diff image was asked for");

    // No pixel differs, so none carries the diff color.
    for pixel in diff.data.chunks_exact(4) {
        assert_ne!(pixel, [255, 0, 0, 255]);
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }
}
