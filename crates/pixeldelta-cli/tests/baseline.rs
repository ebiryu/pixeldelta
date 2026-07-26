//! Baseline resolution: which stored snapshot a run compares against.

use std::path::Path;
use std::process::Command;

use pixeldelta_cli::{resolve_baseline, Storage};

/// A feature branch compares against the newest snapshot at or below the
/// merge base, not against a commit that only exists on the base branch after
/// the branch point.
#[test]
fn the_newest_stored_ancestor_of_the_merge_base_wins() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let first = commit(&repo, "first");
    let second = commit(&repo, "second");
    git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit(&repo, "on the branch");
    git(&repo, &["checkout", "-q", "main"]);
    let after_branch = commit(&repo, "after the branch point");
    git(&repo, &["checkout", "-q", "feature"]);

    let storage = storage(dir.path());
    for key in [&first, &second, &after_branch] {
        store(&storage, key);
    }

    let resolved = resolve_baseline(&repo, &storage, "main", 50).expect("the history is readable");

    assert_eq!(resolved.baseline, Some(second));
}

/// The merge base of the base branch with itself is HEAD, and comparing a
/// commit against its own snapshot always matches.
#[test]
fn the_head_commit_is_not_its_own_baseline() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let first = commit(&repo, "first");
    let head = commit(&repo, "second");

    let storage = storage(dir.path());
    store(&storage, &first);
    store(&storage, &head);

    let resolved = resolve_baseline(&repo, &storage, "main", 50).expect("the history is readable");

    assert_eq!(resolved.head, head);
    assert_eq!(resolved.baseline, Some(first));
}

#[test]
fn a_history_without_a_stored_snapshot_has_no_baseline() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    commit(&repo, "first");
    commit(&repo, "second");

    let resolved =
        resolve_baseline(&repo, &storage(dir.path()), "main", 50).expect("the history is readable");

    assert_eq!(resolved.baseline, None);
}

/// Walking the whole history costs one existence check per commit, so the
/// search stops after the given number of commits.
#[test]
fn the_search_stops_at_the_history_limit() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let oldest = commit(&repo, "first");
    commit(&repo, "second");
    commit(&repo, "third");

    let storage = storage(dir.path());
    store(&storage, &oldest);

    let far = resolve_baseline(&repo, &storage, "main", 50).expect("the history is readable");
    let near = resolve_baseline(&repo, &storage, "main", 2).expect("the history is readable");

    assert_eq!(far.baseline, Some(oldest));
    assert_eq!(near.baseline, None);
}

#[test]
fn an_unknown_base_branch_is_an_error() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    commit(&repo, "first");

    let error = resolve_baseline(&repo, &storage(dir.path()), "release", 50)
        .expect_err("the branch does not exist");

    assert!(error.to_string().contains("release"), "{error}");
}

#[test]
fn a_directory_outside_a_repository_is_an_error() {
    let dir = tempfile::tempdir().expect("a temporary directory");

    let error = resolve_baseline(dir.path(), &storage(dir.path()), "main", 50)
        .expect_err("the directory is not a repository");

    assert!(error.to_string().contains("git"), "{error}");
}

fn storage(root: &Path) -> Storage {
    Storage::parse(root.join("store").to_str().expect("a UTF-8 path")).expect("a directory spec")
}

/// Stores a snapshot holding one file, which is enough for the key to exist.
fn store(storage: &Storage, key: &str) {
    let source = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(source.path().join("a.png"), b"a").expect("the file is written");
    storage
        .store(key, source.path())
        .expect("the snapshot is stored");
}

fn init(repo: &Path) {
    std::fs::create_dir_all(repo).expect("the repository directory is created");
    git(repo, &["-c", "init.defaultBranch=main", "init", "-q"]);
    git(repo, &["config", "user.name", "pixeldelta test"]);
    git(repo, &["config", "user.email", "test@example.invalid"]);
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["commit", "-q", "--allow-empty", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
