//! Which commit's snapshot a run compares against.
//!
//! The baseline is the newest commit at or below the merge base of HEAD and
//! the base branch that has a snapshot stored. Reading it from the history
//! rather than from a moving pointer keeps the comparison tied to the commit
//! a branch actually forked from.

use std::path::Path;
use std::process::Command;

use crate::storage::{Storage, StorageError};

/// The commits a run works with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    /// The commit being tested, which the new snapshot is stored under.
    pub head: String,
    /// The commit whose snapshot is compared against, if one is stored.
    pub baseline: Option<String>,
}

/// Reasons the baseline cannot be resolved.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    /// The `git` command could not be run.
    #[error("git could not be run: {source}")]
    Spawn { source: std::io::Error },
    /// A git command reported failure.
    #[error("git {command} failed: {message}")]
    Git { command: String, message: String },
    /// The storage could not be asked whether a snapshot exists.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Resolves the baseline for the repository at `repo`.
///
/// `limit` bounds how far back the search walks, since each candidate costs
/// one existence check against the storage.
pub fn resolve_baseline(
    repo: &Path,
    storage: &Storage,
    base_branch: &str,
    limit: usize,
) -> Result<Baseline, BaselineError> {
    let head = git(repo, &["rev-parse", "HEAD"])?;
    let merge_base = git(repo, &["merge-base", "HEAD", base_branch])?;
    let candidates = git(
        repo,
        &[
            "rev-list",
            &format!("--max-count={limit}"),
            merge_base.as_str(),
        ],
    )?;

    for candidate in candidates.lines() {
        // The head commit is skipped: on the base branch the merge base is
        // HEAD itself, and a commit compared against its own snapshot always
        // matches.
        if candidate == head {
            continue;
        }
        if storage.exists(candidate)? {
            return Ok(Baseline {
                head,
                baseline: Some(candidate.to_owned()),
            });
        }
    }

    Ok(Baseline {
        head,
        baseline: None,
    })
}

/// Runs one git command and returns its trimmed standard output.
fn git(repo: &Path, args: &[&str]) -> Result<String, BaselineError> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|source| BaselineError::Spawn { source })?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(BaselineError::Git {
            command: args.join(" "),
            message,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
