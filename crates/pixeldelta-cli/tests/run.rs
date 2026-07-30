//! The run operation behind the `run` subcommand.

use std::path::Path;

use pixeldelta_cli::{run_dirs, write_report, CliError};
use pixeldelta_io::{encode_png, DecodeError, Format};
use pixeldelta_report::Category;

/// A solid-color one-row PNG of the given width.
fn png(color: [u8; 4], width: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    for _ in 0..width {
        rgba.extend_from_slice(&color);
    }
    encode_png(width, 1, &rgba).expect("the row encodes")
}

/// A one-row PNG of the given width, all `rest` except for the first pixel,
/// which is `first`. Used to produce a small, known diff ratio.
fn png_with_one_diff(first: [u8; 4], rest: [u8; 4], width: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    rgba.extend_from_slice(&first);
    for _ in 1..width {
        rgba.extend_from_slice(&rest);
    }
    encode_png(width, 1, &rgba).expect("the row encodes")
}

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Builds an expected/actual pair of directories covering every category.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));

    // matched: same content on both sides.
    write(&e.join("same.png"), &png([10, 20, 30, 255], 4));
    write(&a.join("same.png"), &png([10, 20, 30, 255], 4));
    // changed: different content, same size.
    write(&e.join("nested/diff.png"), &png([255, 0, 0, 255], 4));
    write(&a.join("nested/diff.png"), &png([0, 0, 255, 255], 4));
    // size mismatch: same name, different width.
    write(&e.join("grew.png"), &png([0, 0, 0, 255], 4));
    write(&a.join("grew.png"), &png([0, 0, 0, 255], 8));
    // added: only in actual.
    write(&a.join("new.png"), &png([1, 2, 3, 255], 4));
    // removed: only in expected.
    write(&e.join("gone.png"), &png([4, 5, 6, 255], 4));

    dir
}

fn category_of<'a>(entries: &'a [pixeldelta_report::Entry], path: &str) -> &'a Category {
    &entries
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("no entry for {path}"))
        .category
}

#[test]
fn every_pairing_lands_in_the_right_category() {
    let dir = fixture();
    let report = run_dirs(
        &dir.path().join("expected"),
        &dir.path().join("actual"),
        0.1,
        true,
        0.0,
    )
    .expect("the run finishes");

    assert_eq!(category_of(&report.entries, "same.png"), &Category::Matched);
    assert_eq!(
        category_of(&report.entries, "nested/diff.png"),
        &Category::Changed
    );
    assert_eq!(
        category_of(&report.entries, "grew.png"),
        &Category::SizeMismatch
    );
    assert_eq!(category_of(&report.entries, "new.png"), &Category::Added);
    assert_eq!(category_of(&report.entries, "gone.png"), &Category::Removed);

    let summary = report.summary();
    assert!(!summary.passed);
    assert_eq!(summary.matched, 1);
    assert_eq!(summary.changed, 1);
}

#[test]
fn a_changed_entry_carries_its_diff_image_and_clusters() {
    let dir = fixture();
    let report = run_dirs(
        &dir.path().join("expected"),
        &dir.path().join("actual"),
        0.1,
        true,
        0.0,
    )
    .expect("the run finishes");

    let changed = report
        .entries
        .iter()
        .find(|e| e.path == "nested/diff.png")
        .unwrap();
    assert!(changed.images.expected.is_some());
    assert!(changed.images.actual.is_some());
    assert!(changed.images.diff.is_some());
    assert!(!changed.clusters.is_empty());
    assert_eq!(changed.image_size, Some([4, 1]));
}

#[test]
fn matching_directories_pass() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    write(&e.join("a.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("a.png"), &png([9, 9, 9, 255], 4));

    let report = run_dirs(&e, &a, 0.1, true, 0.0).expect("the run finishes");
    assert!(report.summary().passed);
}

#[test]
fn a_small_difference_is_tolerated_when_the_ratio_allows_it() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    let width = 1000;
    write(&e.join("a.png"), &png([10, 10, 10, 255], width));
    write(
        &a.join("a.png"),
        &png_with_one_diff([200, 0, 0, 255], [10, 10, 10, 255], width),
    );

    let tolerated =
        run_dirs(&e, &a, 0.1, true, 0.01).expect("the run finishes with the ratio allowed");
    assert_eq!(
        category_of(&tolerated.entries, "a.png"),
        &Category::Tolerated
    );
    assert_eq!(tolerated.summary().tolerated, 1);
    assert_eq!(tolerated.summary().changed, 0);
    assert!(tolerated.summary().passed);

    let changed = run_dirs(&e, &a, 0.1, true, 0.0).expect("the run finishes with no tolerance");
    assert_eq!(category_of(&changed.entries, "a.png"), &Category::Changed);
    assert!(!changed.summary().passed);
}

#[test]
fn write_report_emits_the_requested_files() {
    let dir = fixture();
    let out = dir.path().join("out");
    let report = run_dirs(
        &dir.path().join("expected"),
        &dir.path().join("actual"),
        0.1,
        true,
        0.0,
    )
    .unwrap();

    write_report(
        &report,
        Some(&out),
        Some(&dir.path().join("result.json")),
        Some(&dir.path().join("result.xml")),
    )
    .expect("the files are written");

    assert!(out.join("index.html").is_file());
    assert!(dir.path().join("result.json").is_file());
    assert!(dir.path().join("result.xml").is_file());
}

#[test]
fn write_report_writes_diff_images_as_files_and_the_html_has_no_data_uri() {
    let dir = fixture();
    let out = dir.path().join("out");
    let report = run_dirs(
        &dir.path().join("expected"),
        &dir.path().join("actual"),
        0.1,
        true,
        0.0,
    )
    .unwrap();

    write_report(&report, Some(&out), None, None).expect("the files are written");

    assert!(out.join("images/diff/nested/diff.png").is_file());
    let html = std::fs::read_to_string(out.join("index.html")).expect("the file is readable");
    assert!(!html.contains("data:image"), "{html}");
}

/// Entries come out in the lexicographic order of their relative path, not in
/// whatever order the pairs were written to disk or finished comparing in.
///
/// The names below are picked so that lexicographic order differs from both
/// the order they are written here and, since the comparisons can run in
/// parallel, from whatever order finishes first.
#[test]
fn entries_are_in_lexicographic_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));

    // Written zzz, mmm, aaa, yyy, bbb; lexicographically aaa < bbb < mmm < yyy < zzz.
    write(&a.join("zzz_top.png"), &png([1, 2, 3, 255], 4));
    write(&e.join("mmm_dir/nnn.png"), &png([255, 0, 0, 255], 4));
    write(&a.join("mmm_dir/nnn.png"), &png([0, 0, 255, 255], 4));
    write(&e.join("aaa_top.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("aaa_top.png"), &png([9, 9, 9, 255], 4));
    write(&e.join("yyy_top.png"), &png([4, 5, 6, 255], 4));
    write(&e.join("bbb_top.png"), &png([0, 0, 0, 255], 4));
    write(&a.join("bbb_top.png"), &png([0, 0, 0, 255], 8));

    let report = run_dirs(&e, &a, 0.1, true, 0.0).expect("the run finishes");

    let paths: Vec<&str> = report.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        vec![
            "aaa_top.png",
            "bbb_top.png",
            "mmm_dir/nnn.png",
            "yyy_top.png",
            "zzz_top.png",
        ]
    );
}

/// When several files fail to decode, the reported error is the first one in
/// lexicographic path order, not whichever comparison happens to fail first.
///
/// `decode` reports neither a path nor any other detail identifying its input
/// once the bytes are ruled out as a PNG, so the two broken files are given
/// bytes that fail in distinguishable ways: the lexicographically first,
/// `aaa_broken.png`, has no recognizable image signature at all
/// (`DecodeError::UnsupportedFormat`), while the second, `zzz_broken.png`, has
/// a PNG signature followed by nothing (`DecodeError::Malformed`). Getting the
/// `UnsupportedFormat` variant back confirms the first one in path order is
/// the one that surfaced.
#[test]
fn the_reported_error_names_the_first_broken_file_in_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));

    write(&e.join("aaa_broken.png"), b"not an image at all");
    write(&a.join("aaa_broken.png"), b"not an image at all");
    write(&e.join("mmm_good.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("mmm_good.png"), &png([9, 9, 9, 255], 4));
    write(
        &e.join("zzz_broken.png"),
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
    );
    write(
        &a.join("zzz_broken.png"),
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
    );

    let error = run_dirs(&e, &a, 0.1, true, 0.0).expect_err("a broken PNG fails the run");

    assert!(
        matches!(
            error,
            CliError::Decode(DecodeError::UnsupportedFormat {
                format: Format::Unknown
            })
        ),
        "expected the UnsupportedFormat error from aaa_broken.png, got {error}"
    );
}
