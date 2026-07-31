//! Comparing a checkout against the snapshot stored for its baseline commit.

use std::path::{Path, PathBuf};

use pixeldelta_report::{Entry, Side, Summary};

use crate::baseline::resolve_baseline;
use crate::github::{notify, GithubConfig, Notification};
use crate::run::{run_dirs, write_html, write_report};
use crate::storage::Storage;
use crate::CliError;

/// What one `ci` run works on.
pub struct CiOptions<'a> {
    /// Repository the baseline is read from.
    pub repo: &'a Path,
    /// Directory of images produced by this checkout.
    pub actual: &'a Path,
    /// Where snapshots are kept.
    pub storage: &'a Storage,
    /// Branch the baseline is looked for below.
    pub base_branch: &'a str,
    /// How many commits the baseline search walks.
    pub history_limit: usize,
    /// Color delta a pixel must exceed to count.
    pub threshold: f32,
    /// Whether anti-aliasing differences are excluded.
    pub antialiasing: bool,
    /// Fraction of an image's pixels that may differ and still count as
    /// tolerated rather than changed.
    pub tolerance_ratio: f64,
    /// Clusters an entry reports, the ones with the most differing pixels. 0
    /// reports every cluster.
    pub max_clusters: usize,
    /// Write the HTML report into this directory as index.html.
    pub report: Option<&'a Path>,
    /// Where the report will be readable, for runs that publish it somewhere
    /// the storage does not serve. Takes precedence over the URL the storage
    /// answers with.
    pub report_url: Option<&'a str>,
    /// Write the JSON report to this path.
    pub json: Option<&'a Path>,
    /// Write the JUnit XML report to this path.
    pub junit: Option<&'a Path>,
    /// Append the notification body to this path, which a workflow points at
    /// its job summary file. Nothing is appended when there was no baseline,
    /// since no comparison happened.
    pub markdown: Option<&'a Path>,
    /// Post the notification body as a pull request comment. Nothing is posted
    /// when there was no baseline.
    pub github: Option<&'a GithubConfig>,
}

/// What one `ci` run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiRun {
    /// Commit the new snapshot was stored under.
    pub head: String,
    /// Commit that was compared against, if a snapshot was found.
    pub baseline: Option<String>,
    /// Counts by category, absent when there was no baseline to compare with.
    pub summary: Option<Summary>,
    /// Where the report can be read, when an address for it is known.
    pub report_url: Option<String>,
    /// What posting the comment came to, when one was asked for.
    pub comment: Option<Notification>,
}

/// Compares `actual` against the snapshot of the baseline commit, then stores
/// `actual` as the snapshot for the current commit.
///
/// A checkout with no stored baseline is not a regression: the snapshot is
/// stored and the run reports no comparison.
pub fn ci(opts: &CiOptions) -> Result<CiRun, CliError> {
    let resolved = resolve_baseline(
        opts.repo,
        opts.storage,
        opts.base_branch,
        opts.history_limit,
    )?;

    let Some(baseline) = resolved.baseline else {
        opts.storage.store(&resolved.head, opts.actual)?;
        return Ok(CiRun {
            head: resolved.head,
            baseline: None,
            summary: None,
            report_url: None,
            comment: None,
        });
    };

    let expected = tempfile::tempdir().map_err(|source| CliError::Temp { source })?;
    opts.storage.fetch(&baseline, expected.path())?;

    let report = run_dirs(
        expected.path(),
        opts.actual,
        opts.threshold,
        opts.antialiasing,
        opts.tolerance_ratio,
        opts.max_clusters,
        opts.report,
    )?;
    write_report(&report, None, opts.json, opts.junit)?;

    // The report kept with the snapshot points expected and actual at the
    // snapshot's own images by relative path (see `storage_assets`), so the
    // snapshot has to be written before the report is.
    opts.storage.store(&resolved.head, opts.actual)?;

    // The report is stored only when it was asked for, since its diff images
    // are the largest thing the run writes.
    let stored_url = match opts.report {
        Some(dir) => {
            write_html(&report, dir)?;
            let html = pixeldelta_report::html(&report, storage_assets(&baseline));
            let diff_images = diff_images(&report, dir);
            opts.storage
                .store_report(&resolved.head, html.as_bytes(), &diff_images)?
        }
        None => None,
    };
    let report_url = published_url(opts.report_url, stored_url);

    // Rendered once and used by both routes, so the comment and the job
    // summary carry the same account of the run.
    let body = match (opts.markdown, opts.github) {
        (None, None) => None,
        _ => Some(pixeldelta_report::markdown(
            &report,
            &baseline,
            report_url.as_deref(),
        )),
    };
    if let (Some(path), Some(body)) = (opts.markdown, &body) {
        append(path, body)?;
    }
    let comment = match (opts.github, &body) {
        (Some(config), Some(body)) => Some(notify(config, body)?),
        _ => None,
    };

    Ok(CiRun {
        head: resolved.head,
        baseline: Some(baseline),
        summary: Some(report.summary()),
        report_url,
        comment,
    })
}

/// Resolves images for the report kept with the snapshot.
///
/// Expected and actual point at the snapshots the storage already holds
/// rather than being re-uploaded, since `<key>/report/index.html` and
/// `<key>/images/<path>` share the same layout for both the Dir and S3
/// backends. The diff image is stored alongside the report itself.
fn storage_assets(baseline: &str) -> impl Fn(&Entry, Side) -> Option<String> + '_ {
    move |entry, side| {
        let held = match side {
            Side::Expected => entry.images.expected,
            Side::Actual => entry.images.actual,
            Side::Diff => entry.images.diff,
        };
        if !held {
            return None;
        }
        Some(match side {
            Side::Expected => format!(
                "../../{baseline}/images/{}",
                pixeldelta_report::url_path(&entry.path)
            ),
            Side::Actual => format!("../images/{}", pixeldelta_report::url_path(&entry.path)),
            Side::Diff => {
                pixeldelta_report::url_path(&pixeldelta_report::asset_path(&entry.path, Side::Diff))
            }
        })
    }
}

/// Collects the diff image each entry holds, as a path below the report
/// directory and the local file `run_dirs` already wrote it to under
/// `report_dir`.
///
/// Expected and actual are left out: they are already in the storage as
/// snapshots, so re-uploading them alongside the report would be redundant.
fn diff_images(report: &pixeldelta_report::Report, report_dir: &Path) -> Vec<(String, PathBuf)> {
    report
        .entries
        .iter()
        .filter(|entry| entry.images.diff)
        .map(|entry| {
            let rel = pixeldelta_report::asset_path(&entry.path, Side::Diff);
            let local = report_dir.join(&rel);
            (rel, local)
        })
        .collect()
}

/// Where the report can be read: the value given from outside, and otherwise
/// the one the storage answered with.
///
/// Only an S3-compatible storage builds a URL, and a run that publishes the
/// report elsewhere knows an address the storage cannot.
fn published_url(given: Option<&str>, stored: Option<String>) -> Option<String> {
    given.map(str::to_owned).or(stored)
}

/// Appends the body to a file, creating it when it is not there.
///
/// The job summary file a workflow points at may already hold output from an
/// earlier step.
fn append(path: &Path, body: &str) -> Result<(), CliError> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CliError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(body.as_bytes())
        .map_err(|source| CliError::Write {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::published_url;

    #[test]
    fn the_given_url_wins_over_the_stored_one() {
        let url = published_url(
            Some("https://pages.example.invalid/report/"),
            Some("https://bucket.example.invalid/abc/report/index.html".to_owned()),
        );

        assert_eq!(
            url.as_deref(),
            Some("https://pages.example.invalid/report/")
        );
    }

    #[test]
    fn the_stored_url_is_used_when_none_is_given() {
        let stored = "https://bucket.example.invalid/abc/report/index.html";

        let url = published_url(None, Some(stored.to_owned()));

        assert_eq!(url.as_deref(), Some(stored));
    }
}
