//! The compare operation behind the `compare` subcommand.

use std::path::Path;

use pixeldelta_cli::{compare_files, exit_code, CliError};
use pixeldelta_core::Verdict;
use pixeldelta_io::{decode_file, encode_png};

/// Writes a one-row PNG to `path` and returns its path back.
fn write_png(path: &Path, rgba: &[u8]) {
    let png = encode_png((rgba.len() / 4) as u32, 1, rgba).expect("the row encodes");
    std::fs::write(path, png).expect("the fixture is written");
}

#[test]
fn identical_images_report_a_match() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("base.png");
    let head = dir.path().join("head.png");
    write_png(&base, &[10, 20, 30, 255, 40, 50, 60, 255]);
    write_png(&head, &[10, 20, 30, 255, 40, 50, 60, 255]);

    let run = compare_files(&base, &head, &Default::default(), None).expect("the comparison runs");

    assert_eq!(run.verdict, Verdict::Match);
    assert_eq!(run.diff_pixels, 0);
    assert_eq!(exit_code(run.verdict), 0);
}

#[test]
fn differing_images_report_a_difference() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("base.png");
    let head = dir.path().join("head.png");
    write_png(&base, &[255, 0, 0, 255]);
    write_png(&head, &[0, 0, 255, 255]);

    let run = compare_files(&base, &head, &Default::default(), None).expect("the comparison runs");

    assert_eq!(run.verdict, Verdict::Differ);
    assert_eq!(run.diff_pixels, 1);
    assert_eq!(exit_code(run.verdict), 1);
}

#[test]
fn images_of_different_sizes_report_a_size_mismatch() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("base.png");
    let head = dir.path().join("head.png");
    write_png(&base, &[0, 0, 0, 255]);
    write_png(&head, &[0, 0, 0, 255, 0, 0, 0, 255]);

    let run = compare_files(&base, &head, &Default::default(), None).expect("the comparison runs");

    assert_eq!(run.verdict, Verdict::SizeMismatch);
    assert_eq!(exit_code(run.verdict), 2);
}

#[test]
fn the_output_path_receives_a_decodable_diff_png() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("base.png");
    let head = dir.path().join("head.png");
    let output = dir.path().join("diff.png");
    write_png(&base, &[255, 0, 0, 255, 0, 255, 0, 255]);
    write_png(&head, &[0, 0, 255, 255, 0, 255, 0, 255]);

    compare_files(&base, &head, &Default::default(), Some(&output)).expect("the comparison runs");

    let diff = decode_file(&output).expect("the diff image decodes");
    assert_eq!((diff.width(), diff.height()), (2, 1));
    // The changed first pixel is painted the default red.
    assert_eq!(&diff.as_rgba8()[0..4], &[255, 0, 0, 255]);
}

#[test]
fn a_missing_input_is_an_error() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("absent.png");
    let head = dir.path().join("head.png");
    write_png(&head, &[0, 0, 0, 255]);

    let error: CliError = compare_files(&base, &head, &Default::default(), None)
        .expect_err("a missing file is reported");

    assert_eq!(error.exit_code(), 3);
    assert!(
        matches!(&error, CliError::Decode { path, .. } if path == &base),
        "expected the error to name the missing file, got {error}"
    );
}

/// The error names which of the two inputs failed to decode, which the reason
/// on its own does not say.
#[test]
fn a_broken_input_names_the_file_it_came_from() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let base = dir.path().join("base.png");
    let head = dir.path().join("head.png");
    write_png(&base, &[0, 0, 0, 255]);
    std::fs::write(&head, b"not an image at all").expect("the fixture is writable");

    let error: CliError = compare_files(&base, &head, &Default::default(), None)
        .expect_err("a broken file is reported");

    assert!(
        matches!(&error, CliError::Decode { path, .. } if path == &head),
        "expected the error to name the broken file, got {error}"
    );
}
