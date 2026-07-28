//! The `ci` operation: resolve, fetch, compare, store.

mod stub;

use std::path::Path;
use std::process::Command;

use pixeldelta_cli::{ci, CiOptions, GithubConfig, Notification, Storage};

#[test]
fn the_first_run_stores_a_snapshot_and_has_nothing_to_compare() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let head = commit(&repo, "first");
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    let run = ci(&options(&repo, &actual, &storage)).expect("the run finishes");

    assert_eq!(run.head, head);
    assert_eq!(run.baseline, None);
    assert!(run.summary.is_none(), "there was nothing to compare");
    assert!(
        storage.exists(&head).expect("the check succeeds"),
        "the snapshot is stored for the next run"
    );
}

#[test]
fn an_unchanged_screenshot_passes_against_the_stored_baseline() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the first run finishes");
    let head = commit(&repo, "second");
    let run = ci(&options(&repo, &actual, &storage)).expect("the second run finishes");

    assert_eq!(run.head, head);
    assert!(run.baseline.is_some());
    let summary = run.summary.expect("the run compared against a baseline");
    assert!(summary.passed);
    assert_eq!(summary.matched, 1);
}

#[test]
fn a_changed_screenshot_fails_against_the_stored_baseline() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    let first = commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the first run finishes");

    write_png(&actual.join("a.png"), 8, 8, [255, 0, 0, 255]);
    write_png(&actual.join("new.png"), 4, 4, [0, 0, 255, 255]);
    commit(&repo, "second");
    let run = ci(&options(&repo, &actual, &storage)).expect("the second run finishes");

    assert_eq!(run.baseline, Some(first));
    let summary = run.summary.expect("the run compared against a baseline");
    assert!(!summary.passed);
    assert_eq!(summary.changed, 1);
    assert_eq!(summary.added, 1);
}

#[test]
fn the_report_is_written_and_stored_when_it_is_asked_for() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the first run finishes");

    write_png(&actual.join("a.png"), 8, 8, [255, 0, 0, 255]);
    let head = commit(&repo, "second");
    let report_dir = dir.path().join("report");
    let json = dir.path().join("result.json");
    let mut opts = options(&repo, &actual, &storage);
    opts.report = Some(&report_dir);
    opts.json = Some(&json);
    let run = ci(&opts).expect("the run finishes");

    assert!(report_dir.join("index.html").is_file());
    assert!(json.is_file());
    assert_eq!(run.report_url, None, "a local directory has no public URL");
    assert!(
        storage_root(dir.path())
            .join(&head)
            .join("report/index.html")
            .is_file(),
        "the report is kept with the snapshot"
    );
}

#[test]
fn without_a_report_flag_nothing_is_written() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    let head = commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the run finishes");

    assert!(!storage_root(dir.path()).join(&head).join("report").exists());
}

/// The job summary file a workflow points at may already hold output from an
/// earlier step, so the body is appended.
#[test]
fn the_notification_body_is_appended_to_the_markdown_file() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    let first = commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the first run finishes");

    write_png(&actual.join("a.png"), 8, 8, [255, 0, 0, 255]);
    commit(&repo, "second");
    let markdown = dir.path().join("summary.md");
    std::fs::write(&markdown, "written by an earlier step\n").expect("the file is written");
    let mut opts = options(&repo, &actual, &storage);
    opts.markdown = Some(&markdown);
    ci(&opts).expect("the run finishes");

    let body = std::fs::read_to_string(&markdown).expect("the file is readable");
    assert!(body.starts_with("written by an earlier step\n"), "{body}");
    assert!(body.contains("<!-- pixeldelta -->"), "{body}");
    assert!(body.contains(&first[..8]), "the baseline is named: {body}");
    assert!(body.contains("| changed | 1 |"), "{body}");
}

/// The comment carries the same body as the markdown file, so a reader sees
/// one account of the run wherever they look.
#[test]
fn the_comment_carries_the_notification_body() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let repo = dir.path().join("repo");
    init(&repo);
    let actual = dir.path().join("actual");
    write_png(&actual.join("a.png"), 8, 8, [0, 128, 0, 255]);
    let storage = storage(dir.path());

    commit(&repo, "first");
    ci(&options(&repo, &actual, &storage)).expect("the first run finishes");

    write_png(&actual.join("a.png"), 8, 8, [255, 0, 0, 255]);
    commit(&repo, "second");
    let stub = stub::Stub::start(vec![
        stub::Reply::ok(b"[]"),
        stub::Reply::ok(br#"{"id":1,"html_url":"https://example.invalid/1"}"#),
    ]);
    let github = GithubConfig {
        api_url: stub.url(),
        repository: "acme/site".into(),
        pull_request: 3,
        token: "ghs_token".into(),
    };
    let mut opts = options(&repo, &actual, &storage);
    opts.github = Some(&github);
    let run = ci(&opts).expect("the run finishes");

    assert_eq!(
        run.comment,
        Some(Notification::Posted {
            url: "https://example.invalid/1".to_owned(),
            updated: false,
        })
    );
    let posted = stub.requests().remove(1);
    let body = String::from_utf8_lossy(&posted.body).into_owned();
    assert!(body.contains("<!-- pixeldelta -->"), "{body}");
    assert!(body.contains("changed"), "{body}");
}

fn options<'a>(repo: &'a Path, actual: &'a Path, storage: &'a Storage) -> CiOptions<'a> {
    CiOptions {
        repo,
        actual,
        storage,
        base_branch: "main",
        history_limit: 50,
        threshold: 0.1,
        antialiasing: true,
        tolerance_ratio: 0.0,
        report: None,
        json: None,
        junit: None,
        markdown: None,
        github: None,
    }
}

fn storage_root(root: &Path) -> std::path::PathBuf {
    root.join("store")
}

fn storage(root: &Path) -> Storage {
    Storage::parse(storage_root(root).to_str().expect("a UTF-8 path")).expect("a directory spec")
}

fn write_png(path: &Path, width: u32, height: u32, color: [u8; 4]) {
    std::fs::create_dir_all(path.parent().expect("the path has a parent"))
        .expect("the directory is created");
    let pixels: Vec<u8> = color
        .iter()
        .copied()
        .cycle()
        .take((width * height * 4) as usize)
        .collect();
    let png = pixeldelta_io::encode_png(width, height, &pixels).expect("the image encodes");
    std::fs::write(path, png).expect("the file is written");
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
