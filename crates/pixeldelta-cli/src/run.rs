//! Directory comparison behind the `run` subcommand.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pixeldelta_core::{compare, CompareOptions, DiffStyle, Image, Verdict};
use pixeldelta_io::{decode, encode_png, Decoded};
use pixeldelta_report::{Category, Cluster, Entry, Images, Report};

use crate::CliError;

/// Compares two directories of PNGs, pairing files by their relative path.
///
/// Clustering, the layout-shift search and the diff image are always on: the
/// report shows where each difference sits, whether it moved, and the diff
/// image. `threshold` and `antialiasing` come from the caller.
pub fn run_dirs(
    expected: &Path,
    actual: &Path,
    threshold: f32,
    antialiasing: bool,
) -> Result<Report, CliError> {
    let expected_files = collect_pngs(expected)?;
    let actual_files = collect_pngs(actual)?;

    let opts = CompareOptions {
        threshold,
        detect_antialiasing: antialiasing,
        cluster: true,
        layout_shift: true,
        diff: Some(DiffStyle::default()),
        ..Default::default()
    };

    let mut entries = Vec::new();
    for rel in expected_files.union(&actual_files) {
        let in_expected = expected_files.contains(rel);
        let in_actual = actual_files.contains(rel);
        let entry = match (in_expected, in_actual) {
            (true, true) => compare_pair(rel, &expected.join(rel), &actual.join(rel), &opts)?,
            (false, true) => only_one(rel, &actual.join(rel), Category::Added)?,
            (true, false) => only_one(rel, &expected.join(rel), Category::Removed)?,
            (false, false) => unreachable!("a path came from one of the two sets"),
        };
        entries.push(entry);
    }

    Ok(Report {
        threshold,
        antialiasing,
        layout_shift: true,
        entries,
    })
}

/// Compares one file present in both directories.
fn compare_pair(
    rel: &str,
    expected_path: &Path,
    actual_path: &Path,
    opts: &CompareOptions,
) -> Result<Entry, CliError> {
    let expected_bytes =
        std::fs::read(expected_path).map_err(|source| read_error(expected_path, source))?;
    let actual_bytes =
        std::fs::read(actual_path).map_err(|source| read_error(actual_path, source))?;
    let expected = decode(&expected_bytes)?;
    let actual = decode(&actual_bytes)?;

    if expected.width() != actual.width() || expected.height() != actual.height() {
        return Ok(Entry {
            path: rel.to_owned(),
            category: Category::SizeMismatch,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: Vec::new(),
            expected_size: Some([expected.width(), expected.height()]),
            actual_size: Some([actual.width(), actual.height()]),
            image_size: None,
            images: Images {
                expected: Some(expected_bytes),
                actual: Some(actual_bytes),
                diff: None,
            },
        });
    }

    let a = image(&expected);
    let b = image(&actual);
    let result = compare(&a, &b, opts);

    if result.verdict == Verdict::Match {
        return Ok(Entry {
            path: rel.to_owned(),
            category: Category::Matched,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: Vec::new(),
            expected_size: None,
            actual_size: None,
            image_size: None,
            images: Images {
                expected: Some(expected_bytes),
                actual: None,
                diff: None,
            },
        });
    }

    let diff = result
        .diff_image
        .expect("the diff image was requested for the comparison");
    let diff_png = encode_png(diff.width, diff.height, &diff.data)?;
    Ok(Entry {
        path: rel.to_owned(),
        category: Category::Changed,
        diff_pixels: result.diff_pixels,
        diff_ratio: result.diff_ratio,
        clusters: result.clusters.iter().map(to_cluster).collect(),
        expected_size: None,
        actual_size: None,
        image_size: Some([expected.width(), expected.height()]),
        images: Images {
            expected: Some(expected_bytes),
            actual: Some(actual_bytes),
            diff: Some(diff_png),
        },
    })
}

/// Builds an entry for a file present on only one side.
fn only_one(rel: &str, path: &Path, category: Category) -> Result<Entry, CliError> {
    let bytes = std::fs::read(path).map_err(|source| read_error(path, source))?;
    // Decoding it validates the file and reports a broken PNG here rather than
    // in the browser.
    decode(&bytes)?;
    let images = match category {
        Category::Added => Images {
            expected: None,
            actual: Some(bytes),
            diff: None,
        },
        _ => Images {
            expected: Some(bytes),
            actual: None,
            diff: None,
        },
    };
    Ok(Entry {
        path: rel.to_owned(),
        category,
        diff_pixels: 0,
        diff_ratio: 0.0,
        clusters: Vec::new(),
        expected_size: None,
        actual_size: None,
        image_size: None,
        images,
    })
}

fn image(decoded: &Decoded) -> Image<'_> {
    Image::from_rgba8(decoded.width(), decoded.height(), decoded.as_rgba8())
        .expect("the decoder returns a buffer matching its dimensions")
}

fn to_cluster(c: &pixeldelta_core::Cluster) -> Cluster {
    Cluster {
        x: c.bounds.x,
        y: c.bounds.y,
        width: c.bounds.width,
        height: c.bounds.height,
        diff_pixels: c.diff_pixels,
        displacement: c.displacement.map(|(dx, dy)| [dx, dy]),
        ssim: c.ssim,
    }
}

/// Writes each output that was requested.
pub fn write_report(
    report: &Report,
    report_dir: Option<&Path>,
    json_path: Option<&Path>,
    junit_path: Option<&Path>,
) -> Result<(), CliError> {
    if let Some(dir) = report_dir {
        std::fs::create_dir_all(dir).map_err(|source| write_error(dir, source))?;
        let index = dir.join("index.html");
        std::fs::write(&index, pixeldelta_report::html(report))
            .map_err(|source| write_error(&index, source))?;
    }
    if let Some(path) = json_path {
        std::fs::write(path, pixeldelta_report::json(report))
            .map_err(|source| write_error(path, source))?;
    }
    if let Some(path) = junit_path {
        std::fs::write(path, pixeldelta_report::junit(report))
            .map_err(|source| write_error(path, source))?;
    }
    Ok(())
}

/// Collects the relative paths of every `.png` under `root`.
fn collect_pngs(root: &Path) -> Result<BTreeSet<String>, CliError> {
    crate::paths::collect_pngs(root).map_err(|error| read_error(&error.path, error.source))
}

fn read_error(path: &Path, source: std::io::Error) -> CliError {
    CliError::Decode(pixeldelta_io::DecodeError::Read {
        path: PathBuf::from(path),
        source,
    })
}

fn write_error(path: &Path, source: std::io::Error) -> CliError {
    CliError::Write {
        path: PathBuf::from(path),
        source,
    }
}
