//! Comparing a checkout against the snapshot stored for its baseline commit.

use std::path::Path;

use pixeldelta_report::Summary;

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
    )?;
    write_report(&report, None, opts.json, opts.junit)?;

    // The report is stored only when it was asked for, since embedding every
    // image makes it the largest thing the run writes.
    let stored_url = match opts.report {
        Some(dir) => {
            let html = write_html(&report, dir)?;
            opts.storage.store_report(&resolved.head, html.as_bytes())?
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

    opts.storage.store(&resolved.head, opts.actual)?;

    Ok(CiRun {
        head: resolved.head,
        baseline: Some(baseline),
        summary: Some(report.summary()),
        report_url,
        comment,
    })
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
