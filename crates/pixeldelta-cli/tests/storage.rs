//! Snapshot storage: what is stored under a key comes back for that key.

use std::path::Path;

use pixeldelta_cli::Storage;

const KEY: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn a_stored_snapshot_comes_back_with_its_paths() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let source = root.path().join("actual");
    write_png(&source.join("top.png"), b"top");
    write_png(&source.join("nested/inner.png"), b"inner");

    let storage = Storage::parse(root.path().join("store").to_str().expect("a UTF-8 path"))
        .expect("a directory spec");
    storage.store(KEY, &source).expect("the snapshot is stored");

    let dest = root.path().join("baseline");
    let files = storage.fetch(KEY, &dest).expect("the snapshot is fetched");

    assert_eq!(
        files,
        vec!["nested/inner.png".to_owned(), "top.png".to_owned()]
    );
    assert_eq!(read(&dest.join("top.png")), b"top");
    assert_eq!(read(&dest.join("nested/inner.png")), b"inner");
}

#[test]
fn a_key_that_was_never_stored_does_not_exist() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let storage = Storage::parse(root.path().to_str().expect("a UTF-8 path")).expect("a spec");

    assert!(!storage.exists(KEY).expect("the check succeeds"));
}

#[test]
fn a_stored_key_exists() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let source = root.path().join("actual");
    write_png(&source.join("a.png"), b"a");
    let storage = Storage::parse(root.path().join("store").to_str().expect("a UTF-8 path"))
        .expect("a directory spec");

    storage.store(KEY, &source).expect("the snapshot is stored");

    assert!(storage.exists(KEY).expect("the check succeeds"));
}

/// Images without a manifest are a write that did not finish, and picking such
/// a key as the baseline would compare against an incomplete snapshot.
#[test]
fn images_without_a_manifest_do_not_exist() {
    let root = tempfile::tempdir().expect("a temporary directory");
    write_png(&root.path().join(KEY).join("images/a.png"), b"a");
    let storage = Storage::parse(root.path().to_str().expect("a UTF-8 path")).expect("a spec");

    assert!(!storage.exists(KEY).expect("the check succeeds"));
}

#[test]
fn fetching_a_missing_key_fails() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let storage = Storage::parse(root.path().to_str().expect("a UTF-8 path")).expect("a spec");

    let error = storage
        .fetch(KEY, &root.path().join("dest"))
        .expect_err("the key was never stored");

    assert!(error.to_string().contains(KEY), "{error}");
}

/// A manifest is read from the storage, so a path in it must not be able to
/// write outside the destination directory.
#[test]
fn a_manifest_path_leaving_the_destination_is_rejected() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let key_dir = root.path().join(KEY);
    write_png(&key_dir.join("images/escape.png"), b"escape");
    std::fs::write(
        key_dir.join("manifest.json"),
        br#"{"version":1,"files":["../escape.png"]}"#,
    )
    .expect("the manifest is written");
    let storage = Storage::parse(root.path().to_str().expect("a UTF-8 path")).expect("a spec");

    let error = storage
        .fetch(KEY, &root.path().join("dest"))
        .expect_err("the path leaves the destination");

    assert!(error.to_string().contains("../escape.png"), "{error}");
    assert!(!root.path().join("escape.png").exists());
}

#[test]
fn a_report_is_stored_under_the_key() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let storage = Storage::parse(root.path().to_str().expect("a UTF-8 path")).expect("a spec");

    let url = storage
        .store_report(KEY, b"<html></html>", &[])
        .expect("the report is stored");

    assert_eq!(url, None, "a local directory has no public URL");
    assert_eq!(
        read(&root.path().join(KEY).join("report/index.html")),
        b"<html></html>"
    );
}

#[test]
fn a_spec_with_an_unknown_scheme_is_rejected() {
    let error =
        Storage::parse("gs://bucket/prefix").expect_err("only local directory paths are specs");

    assert!(error.to_string().contains("gs://"), "{error}");
}

fn write_png(path: &Path, contents: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("the path has a parent"))
        .expect("the directory is created");
    std::fs::write(path, contents).expect("the file is written");
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}
