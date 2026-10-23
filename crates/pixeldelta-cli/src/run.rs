//! Directory comparison behind the `run` subcommand.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pixeldelta_core::{compare, CompareOptions, DiffStyle, FailFast, Image, Verdict};
use pixeldelta_io::{decode, encode_png, Decoded};
use pixeldelta_report::{Category, Cluster, Entry, Images, Report, Side};
use rayon::prelude::*;

use crate::config::{Config, Settings};
use crate::CliError;

/// What one `run_dirs` call works on, beyond the two directories being
/// compared.
pub struct RunOptions<'a> {
    /// Resolves the threshold, tolerance ratio and ignore regions for each
    /// entry path.
    pub config: &'a Config,
    /// Whether anti-aliasing differences are excluded.
    pub antialiasing: bool,
    /// Clusters an entry reports, the ones with the most differing pixels. 0
    /// reports every cluster.
    pub max_clusters: usize,
    /// Where each entry's images are written as soon as that entry is
    /// compared. `None` produces and drops them without writing.
    pub images_dir: Option<&'a Path>,
}

/// Compares two directories of PNGs, pairing files by their relative path.
///
/// Clustering, the layout-shift search and the diff image are always on: the
/// report shows where each difference sits, whether it moved, and the diff
/// image. `options.config` resolves the threshold, tolerance ratio and ignore
/// regions for each entry path, since an override in the config file can set
/// them differently per path (see `Config::settings`). The report's own
/// `threshold` and `tolerance_ratio` fields carry the run-level base values,
/// `options.config.base()`, not any one entry's resolved values.
///
/// When `options.images_dir` is `Some`, each entry's images are written under
/// it at `pixeldelta_report::asset_path(&entry.path, side)` as soon as that
/// entry is compared, so at most one entry's worth of encoded PNGs per worker
/// thread is held at a time rather than the whole run's. When it is `None`,
/// the bytes are produced and dropped without being written. Either way,
/// `Entry::images` records which sides the entry has.
pub fn run_dirs(expected: &Path, actual: &Path, options: &RunOptions) -> Result<Report, CliError> {
    let expected_files = collect_pngs(expected)?;
    let actual_files = collect_pngs(actual)?;

    // `BTreeSet::union` merges two already-sorted iterators, so this is the
    // lexicographic order of the relative paths. Materializing it into a
    // `Vec` first gives the parallel map below an indexed source, which is
    // what lets its result preserve that order.
    let rels: Vec<&str> = expected_files
        .union(&actual_files)
        .map(String::as_str)
        .collect();

    // One path's read-decode-compare work runs per Rayon worker thread, so the
    // number of decoded image buffers held at once is bounded by the pool
    // size rather than by the number of paths.
    let results: Vec<Result<Entry, CliError>> = rels
        .par_iter()
        .map(|&rel| {
            let in_expected = expected_files.contains(rel);
            let in_actual = actual_files.contains(rel);
            match (in_expected, in_actual) {
                (true, true) => {
                    let settings = options.config.settings(rel);
                    let opts = full_options(&settings, options.antialiasing);
                    let probe = probe_options(&settings, options.antialiasing);
                    compare_pair(
                        rel,
                        &expected.join(rel),
                        &actual.join(rel),
                        &opts,
                        &probe,
                        settings.tolerance_ratio,
                        options.max_clusters,
                        options.images_dir,
                    )
                }
                (false, true) => {
                    only_one(rel, &actual.join(rel), Category::Added, options.images_dir)
                }
                (true, false) => only_one(
                    rel,
                    &expected.join(rel),
                    Category::Removed,
                    options.images_dir,
                ),
                (false, false) => unreachable!("a path came from one of the two sets"),
            }
        })
        .collect();

    // `results` is in path order because the map above is indexed, so walking
    // it in order and returning on the first error reports the first failure
    // in path order rather than whichever worker thread failed first.
    let mut entries = Vec::with_capacity(results.len());
    for result in results {
        entries.push(result?);
    }

    let base = options.config.base();
    Ok(Report {
        threshold: base.threshold,
        antialiasing: options.antialiasing,
        layout_shift: true,
        tolerance_ratio: base.tolerance_ratio,
        entries,
    })
}

/// Builds the options a differing pair is compared with: clustering, the
/// layout-shift search and the diff image are always requested.
fn full_options(settings: &Settings, antialiasing: bool) -> CompareOptions {
    CompareOptions {
        threshold: settings.threshold,
        detect_antialiasing: antialiasing,
        ignore_regions: settings.ignore_regions.clone(),
        cluster: true,
        layout_shift: true,
        diff: Some(DiffStyle::default()),
        ..Default::default()
    }
}

/// Builds the options that decide whether a pair differs at all, before the
/// comparison that fills an entry runs. It counts nothing and stops at the
/// first differing pixel, so it neither allocates the per-pixel bitmap nor
/// renders a diff image for a pair that turns out to match.
fn probe_options(settings: &Settings, antialiasing: bool) -> CompareOptions {
    CompareOptions {
        threshold: settings.threshold,
        detect_antialiasing: antialiasing,
        ignore_regions: settings.ignore_regions.clone(),
        fail_fast: Some(FailFast { max_diff_pixels: 0 }),
        ..Default::default()
    }
}

/// Compares one file present in both directories.
///
/// `probe` decides whether the pair differs at all and `opts` produces what a
/// differing entry holds, so the pairs that match are never measured with the
/// second one.
#[allow(clippy::too_many_arguments)]
fn compare_pair(
    rel: &str,
    expected_path: &Path,
    actual_path: &Path,
    opts: &CompareOptions,
    probe: &CompareOptions,
    tolerance_ratio: f64,
    max_clusters: usize,
    images_dir: Option<&Path>,
) -> Result<Entry, CliError> {
    let expected_bytes =
        std::fs::read(expected_path).map_err(|source| read_error(expected_path, source))?;
    let actual_bytes =
        std::fs::read(actual_path).map_err(|source| read_error(actual_path, source))?;
    let expected =
        decode(&expected_bytes).map_err(|source| CliError::decode(expected_path, source))?;
    let actual = decode(&actual_bytes).map_err(|source| CliError::decode(actual_path, source))?;

    if expected.width() != actual.width() || expected.height() != actual.height() {
        write_image(images_dir, rel, Side::Expected, &expected_bytes)?;
        write_image(images_dir, rel, Side::Actual, &actual_bytes)?;
        return Ok(Entry {
            path: rel.to_owned(),
            category: Category::SizeMismatch,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: Vec::new(),
            omitted_clusters: 0,
            expected_size: Some([expected.width(), expected.height()]),
            actual_size: Some([actual.width(), actual.height()]),
            image_size: None,
            images: Images {
                expected: true,
                actual: true,
                diff: false,
            },
        });
    }

    let a = image(&expected);
    let b = image(&actual);

    if compare(&a, &b, probe).verdict == Verdict::Match {
        write_image(images_dir, rel, Side::Expected, &expected_bytes)?;
        return Ok(Entry {
            path: rel.to_owned(),
            category: Category::Matched,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: Vec::new(),
            omitted_clusters: 0,
            expected_size: None,
            actual_size: None,
            image_size: None,
            images: Images {
                expected: true,
                actual: false,
                diff: false,
            },
        });
    }

    let result = compare(&a, &b, opts);
    let diff = result
        .diff_image
        .expect("the diff image was requested for the comparison");
    let diff_png = encode_png(diff.width, diff.height, &diff.data)?;
    // An entry within the allowed ratio still keeps its diff image and
    // clusters: the report shows what changed even though it does not fail.
    let category = if tolerance_ratio > 0.0 && result.diff_ratio <= tolerance_ratio {
        Category::Tolerated
    } else {
        Category::Changed
    };
    write_image(images_dir, rel, Side::Expected, &expected_bytes)?;
    write_image(images_dir, rel, Side::Actual, &actual_bytes)?;
    write_image(images_dir, rel, Side::Diff, &diff_png)?;
    let (clusters, omitted_clusters) = pixeldelta_report::cap_clusters(
        result.clusters.iter().map(to_cluster).collect(),
        max_clusters,
    );
    Ok(Entry {
        path: rel.to_owned(),
        category,
        diff_pixels: result.diff_pixels,
        diff_ratio: result.diff_ratio,
        clusters,
        omitted_clusters,
        expected_size: None,
        actual_size: None,
        image_size: Some([expected.width(), expected.height()]),
        images: Images {
            expected: true,
            actual: true,
            diff: true,
        },
    })
}

/// Builds an entry for a file present on only one side.
fn only_one(
    rel: &str,
    path: &Path,
    category: Category,
    images_dir: Option<&Path>,
) -> Result<Entry, CliError> {
    let bytes = std::fs::read(path).map_err(|source| read_error(path, source))?;
    // Decoding it validates the file and reports a broken PNG here rather than
    // in the browser.
    decode(&bytes).map_err(|source| CliError::decode(path, source))?;
    let images = match category {
        Category::Added => {
            write_image(images_dir, rel, Side::Actual, &bytes)?;
            Images {
                expected: false,
                actual: true,
                diff: false,
            }
        }
        _ => {
            write_image(images_dir, rel, Side::Expected, &bytes)?;
            Images {
                expected: true,
                actual: false,
                diff: false,
            }
        }
    };
    Ok(Entry {
        path: rel.to_owned(),
        category,
        diff_pixels: 0,
        diff_ratio: 0.0,
        clusters: Vec::new(),
        omitted_clusters: 0,
        expected_size: None,
        actual_size: None,
        image_size: None,
        images,
    })
}

/// Writes `bytes` under `dir` at the entry's path for `side`, when a
/// destination was given.
///
/// Several worker threads call this at once for different entries, but never
/// for the same path, so no two calls race on the same file.
fn write_image(dir: Option<&Path>, rel: &str, side: Side, bytes: &[u8]) -> Result<(), CliError> {
    let Some(dir) = dir else {
        return Ok(());
    };
    let path = dir.join(pixeldelta_report::asset_path(rel, side));
    write_asset(&path, bytes)
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

/// Writes the HTML report as `index.html` in `dir`.
///
/// The images it references are expected to already be on disk under `dir`,
/// at the paths `run_dirs`'s `images_dir` argument writes them to. Writing
/// `index.html` last keeps the same invariant a stored snapshot's manifest
/// has: a directory without it is an incomplete report, not one merely
/// missing a few images.
pub(crate) fn write_html(report: &Report, dir: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(dir).map_err(|source| write_error(dir, source))?;
    let html = pixeldelta_report::html(report, pixeldelta_report::local_assets);
    let index = dir.join("index.html");
    std::fs::write(&index, &html).map_err(|source| write_error(&index, source))?;
    Ok(())
}

fn write_asset(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| write_error(parent, source))?;
    }
    std::fs::write(path, bytes).map_err(|source| write_error(path, source))
}

/// Writes each output that was requested.
pub fn write_report(
    report: &Report,
    report_dir: Option<&Path>,
    json_path: Option<&Path>,
    junit_path: Option<&Path>,
) -> Result<(), CliError> {
    if let Some(dir) = report_dir {
        write_html(report, dir)?;
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
    CliError::decode(path, pixeldelta_io::DecodeError::Read { source })
}

fn write_error(path: &Path, source: std::io::Error) -> CliError {
    CliError::Write {
        path: PathBuf::from(path),
        source,
    }
}
