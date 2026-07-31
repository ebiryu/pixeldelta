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

/// An expected/actual pair of one-row PNGs of the given width, where `actual`
/// differs from `expected` only within `ranges` (each a half-open `[start,
/// end)` span of pixel indices). Ranges separated by at least one matching
/// pixel form distinct eight-connected clusters, since a single row only
/// connects horizontally.
fn row_with_clusters(width: u32, ranges: &[(u32, u32)]) -> (Vec<u8>, Vec<u8>) {
    let base = [10, 10, 10, 255];
    let diff = [200, 0, 0, 255];
    let mut expected_rgba = Vec::new();
    let mut actual_rgba = Vec::new();
    for x in 0..width {
        expected_rgba.extend_from_slice(&base);
        let differs = ranges.iter().any(|&(start, end)| x >= start && x < end);
        actual_rgba.extend_from_slice(if differs { &diff } else { &base });
    }
    (
        encode_png(width, 1, &expected_rgba).expect("the row encodes"),
        encode_png(width, 1, &actual_rgba).expect("the row encodes"),
    )
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
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
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
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes");

    let changed = report
        .entries
        .iter()
        .find(|e| e.path == "nested/diff.png")
        .unwrap();
    assert!(changed.images.expected);
    assert!(changed.images.actual);
    assert!(changed.images.diff);
    assert!(!changed.clusters.is_empty());
    assert_eq!(changed.image_size, Some([4, 1]));
}

#[test]
fn matching_directories_pass() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    write(&e.join("a.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("a.png"), &png([9, 9, 9, 255], 4));

    let report = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes");
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

    let tolerated = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.01,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes with the ratio allowed");
    assert_eq!(
        category_of(&tolerated.entries, "a.png"),
        &Category::Tolerated
    );
    assert_eq!(tolerated.summary().tolerated, 1);
    assert_eq!(tolerated.summary().changed, 0);
    assert!(tolerated.summary().passed);

    let changed = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes with no tolerance");
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
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        Some(&out),
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
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        Some(&out),
    )
    .unwrap();

    // `run_dirs` writes each entry's images as soon as that entry is
    // compared, so the diff image is already on disk before `write_report`
    // (and its `write_html`) ever runs.
    assert!(out.join("images/diff/nested/diff.png").is_file());

    write_report(&report, Some(&out), None, None).expect("the files are written");

    let html = std::fs::read_to_string(out.join("index.html")).expect("the file is readable");
    assert!(html.contains("images/diff/nested/diff.png"), "{html}");
    assert!(!html.contains("data:image"), "{html}");
}

#[test]
fn run_dirs_with_no_images_dir_writes_nothing() {
    let dir = fixture();
    let out = dir.path().join("out");
    std::fs::create_dir_all(&out).unwrap();

    run_dirs(
        &dir.path().join("expected"),
        &dir.path().join("actual"),
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes");

    assert_eq!(
        std::fs::read_dir(&out).unwrap().count(),
        0,
        "no images_dir was given, so nothing should have been written to out"
    );
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

    let report = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect("the run finishes");

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
/// Both broken files are given the same bytes, so only the path in the error
/// tells which of the two surfaced.
#[test]
fn the_reported_error_names_the_first_broken_file_in_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));

    write(&e.join("aaa_broken.png"), b"not an image at all");
    write(&a.join("aaa_broken.png"), b"not an image at all");
    write(&e.join("mmm_good.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("mmm_good.png"), &png([9, 9, 9, 255], 4));
    write(&e.join("zzz_broken.png"), b"not an image at all");
    write(&a.join("zzz_broken.png"), b"not an image at all");

    let error = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect_err("a broken PNG fails the run");

    assert!(
        matches!(
            &error,
            CliError::Decode {
                path,
                source: DecodeError::UnsupportedFormat {
                    format: Format::Unknown,
                },
            } if path == &e.join("aaa_broken.png")
        ),
        "expected the error to name expected/aaa_broken.png, got {error}"
    );
}

/// A file present on one side only is decoded as well, and its failure names
/// the path it came from.
#[test]
fn a_broken_file_on_one_side_only_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));

    write(&e.join("kept.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("kept.png"), &png([9, 9, 9, 255], 4));
    write(&a.join("added_broken.png"), b"not an image at all");

    let error = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect_err("a broken PNG fails the run");

    assert!(
        matches!(&error, CliError::Decode { path, .. } if path == &a.join("added_broken.png")),
        "expected the error to name actual/added_broken.png, got {error}"
    );
}

/// A directory that cannot be read is reported with the path that failed,
/// through the same error as a file that cannot be decoded.
#[test]
fn an_unreadable_input_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    write(&a.join("only.png"), &png([9, 9, 9, 255], 4));

    let error = run_dirs(
        &e,
        &a,
        0.1,
        true,
        0.0,
        pixeldelta_cli::DEFAULT_MAX_CLUSTERS,
        None,
    )
    .expect_err("a missing directory fails the run");

    assert!(
        matches!(
            &error,
            CliError::Decode {
                path,
                source: DecodeError::Read { .. },
            } if path == &e
        ),
        "expected the error to name the missing expected directory, got {error}"
    );
}

/// The eight clusters `row_with_clusters` shared by these two tests produces:
/// six single-pixel clusters and three wider ones, so a cap smaller than the
/// total exercises both which ones are kept and how many are left out.
const CLUSTER_RANGES: [(u32, u32); 8] = [
    (2, 3),   // 1px
    (5, 6),   // 1px
    (10, 15), // 5px, the largest
    (20, 21), // 1px
    (30, 34), // 4px, the second largest
    (40, 41), // 1px
    (50, 53), // 3px, the third largest
    (60, 61), // 1px
];

#[test]
fn max_clusters_caps_the_reported_clusters_to_the_largest() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    let (expected_bytes, actual_bytes) = row_with_clusters(64, &CLUSTER_RANGES);
    write(&e.join("row.png"), &expected_bytes);
    write(&a.join("row.png"), &actual_bytes);

    let report = run_dirs(&e, &a, 0.1, true, 0.0, 3, None).expect("the run finishes");

    let entry = report
        .entries
        .iter()
        .find(|entry| entry.path == "row.png")
        .expect("the row is compared");

    assert_eq!(entry.clusters.len(), 3);
    assert_eq!(entry.omitted_clusters, 5);

    let mut diffs: Vec<u64> = entry.clusters.iter().map(|c| c.diff_pixels).collect();
    diffs.sort_unstable();
    assert_eq!(diffs, vec![3, 4, 5]);
}

#[test]
fn a_max_clusters_of_zero_reports_every_cluster() {
    let dir = tempfile::tempdir().unwrap();
    let (e, a) = (dir.path().join("expected"), dir.path().join("actual"));
    let (expected_bytes, actual_bytes) = row_with_clusters(64, &CLUSTER_RANGES);
    write(&e.join("row.png"), &expected_bytes);
    write(&a.join("row.png"), &actual_bytes);

    let report = run_dirs(&e, &a, 0.1, true, 0.0, 0, None).expect("the run finishes");

    let entry = report
        .entries
        .iter()
        .find(|entry| entry.path == "row.png")
        .expect("the row is compared");

    assert_eq!(entry.clusters.len(), 8);
    assert_eq!(entry.omitted_clusters, 0);
}
