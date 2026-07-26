//! The operations behind the pixeldelta command-line tool.
//!
//! Argument parsing lives in the binary; this library holds the work each
//! subcommand does, so the operations can be tested without spawning a process.

mod baseline;
mod ci;
mod paths;
mod run;
mod storage;

use std::path::Path;

use pixeldelta_core::{compare, CompareOptions, CompareResult, Image, Verdict};
use pixeldelta_io::{decode_file, encode_png, DecodeError, EncodeError};

pub use baseline::{resolve_baseline, Baseline, BaselineError};
pub use ci::{ci, CiOptions, CiRun};
pub use run::{run_dirs, write_report};
pub use storage::{Storage, StorageError};

/// Summary of a `compare` run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareRun {
    /// Whether the images matched.
    pub verdict: Verdict,
    /// Number of differing pixels.
    pub diff_pixels: u64,
    /// Differing pixels over compared pixels.
    pub diff_ratio: f64,
}

/// Reasons a subcommand cannot finish.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// An input image could not be read or decoded.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// The diff image could not be encoded.
    #[error(transparent)]
    Encode(#[from] EncodeError),
    /// The diff image could not be written.
    #[error("{path} could not be written: {source}")]
    Write {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// The directory a baseline snapshot is fetched into could not be created.
    #[error("a temporary directory could not be created: {source}")]
    Temp { source: std::io::Error },
    /// The baseline commit could not be resolved.
    #[error(transparent)]
    Baseline(#[from] BaselineError),
    /// A snapshot could not be read or written.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl CliError {
    /// Exit code for a run that failed before producing a verdict.
    pub fn exit_code(&self) -> i32 {
        3
    }
}

/// Exit code standing for a verdict.
///
/// A difference is a non-zero code so a CI step fails on it, and it is kept
/// distinct from the runtime-error code so a script can tell a real difference
/// from a broken input.
pub fn exit_code(verdict: Verdict) -> i32 {
    match verdict {
        Verdict::Match => 0,
        Verdict::Differ => 1,
        Verdict::SizeMismatch => 2,
    }
}

/// Compares two PNG files, optionally writing the diff image to `output`.
///
/// A diff image is rendered and encoded only when `output` is given.
pub fn compare_files(
    base: &Path,
    head: &Path,
    opts: &CompareOptions,
    output: Option<&Path>,
) -> Result<CompareRun, CliError> {
    let base_image = decode_file(base)?;
    let head_image = decode_file(head)?;

    // The diff image is only rendered when it is going to be written.
    let opts = CompareOptions {
        diff: output.map(|_| opts.diff.unwrap_or_default()),
        ..opts.clone()
    };

    let a = Image::from_rgba8(
        base_image.width(),
        base_image.height(),
        base_image.as_rgba8(),
    )
    .expect("the decoder returns a buffer matching its dimensions");
    let b = Image::from_rgba8(
        head_image.width(),
        head_image.height(),
        head_image.as_rgba8(),
    )
    .expect("the decoder returns a buffer matching its dimensions");
    let result = compare(&a, &b, &opts);

    if let (Some(path), Some(diff)) = (output, &result.diff_image) {
        let png = encode_png(diff.width, diff.height, &diff.data)?;
        std::fs::write(path, png).map_err(|source| CliError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(summary(&result))
}

fn summary(result: &CompareResult) -> CompareRun {
    CompareRun {
        verdict: result.verdict,
        diff_pixels: result.diff_pixels,
        diff_ratio: result.diff_ratio,
    }
}
