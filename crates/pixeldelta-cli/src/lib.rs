//! The operations behind the pixeldelta command-line tool.
//!
//! Argument parsing lives in the binary; this library holds the work each
//! subcommand does, so the operations can be tested without spawning a process.

mod baseline;
mod ci;
mod config;
mod github;
mod http;
mod paths;
mod run;
mod s3;
mod sigv4;
mod storage;

use std::path::Path;

use pixeldelta_core::{compare, CompareOptions, CompareResult, Image, Rect, Verdict};
use pixeldelta_io::{decode_file, encode_png, DecodeError, EncodeError};

pub use baseline::{resolve_baseline, Baseline, BaselineError};
pub use ci::{ci, CiOptions, CiRun};
pub use config::{load_config, Config, ConfigError, Settings};
pub use github::{notify, pull_request_number, GithubConfig, GithubError, Notification};
pub use run::{run_dirs, write_report, RunOptions};
pub use sigv4::Credentials;
pub use storage::{S3Config, Storage, StorageError};

/// How many clusters one entry reports by default, the ones with the most
/// differing pixels.
pub const DEFAULT_MAX_CLUSTERS: usize = 100;

/// A `--ignore-region` value that is not four comma-separated `u32` fields.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("{value} is not a valid ignore region: expected X,Y,W,H")]
pub struct RectParseError {
    value: String,
}

/// Parses a `--ignore-region X,Y,W,H` value into a [`Rect`].
///
/// The four fields are `u32`, separated by commas with no other characters.
/// A width or height of `0` is accepted: `Rect` already treats it as covering
/// no pixels.
pub fn parse_ignore_region(value: &str) -> Result<Rect, RectParseError> {
    let invalid = || RectParseError {
        value: value.to_owned(),
    };
    let fields: Vec<&str> = value.split(',').collect();
    let [x, y, width, height] = fields.as_slice() else {
        return Err(invalid());
    };
    let parse = |s: &str| s.parse::<u32>().map_err(|_| invalid());
    Ok(Rect {
        x: parse(x)?,
        y: parse(y)?,
        width: parse(width)?,
        height: parse(height)?,
    })
}

#[cfg(test)]
mod ignore_region_tests {
    use super::*;

    #[test]
    fn a_valid_value_parses_into_its_four_fields() {
        assert_eq!(
            parse_ignore_region("10,20,30,40"),
            Ok(Rect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            })
        );
    }

    #[test]
    fn a_zero_width_or_height_is_accepted() {
        assert_eq!(
            parse_ignore_region("0,0,0,5"),
            Ok(Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 5,
            })
        );
    }

    #[test]
    fn too_few_fields_is_rejected() {
        assert!(parse_ignore_region("1,2,3").is_err());
    }

    #[test]
    fn too_many_fields_is_rejected() {
        assert!(parse_ignore_region("1,2,3,4,5").is_err());
    }

    #[test]
    fn a_non_numeric_field_is_rejected() {
        assert!(parse_ignore_region("1,2,3,x").is_err());
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(parse_ignore_region("").is_err());
    }
}

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
    ///
    /// `DecodeError` names the reason only. The path is held here because a
    /// `run` decodes every PNG under two directories, where the reason on its
    /// own does not say which file failed.
    #[error("{path}: {source}")]
    Decode {
        path: std::path::PathBuf,
        source: DecodeError,
    },
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
    /// The comment could not be posted.
    #[error(transparent)]
    Github(#[from] GithubError),
    /// The config file could not be read or parsed.
    #[error(transparent)]
    Config(#[from] ConfigError),
}

impl CliError {
    /// Exit code for a run that failed before producing a verdict.
    pub fn exit_code(&self) -> i32 {
        3
    }

    /// Names the file a decode failure came from.
    pub(crate) fn decode(path: &Path, source: DecodeError) -> Self {
        Self::Decode {
            path: path.to_path_buf(),
            source,
        }
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
    let base_image = decode_file(base).map_err(|source| CliError::decode(base, source))?;
    let head_image = decode_file(head).map_err(|source| CliError::decode(head, source))?;

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
