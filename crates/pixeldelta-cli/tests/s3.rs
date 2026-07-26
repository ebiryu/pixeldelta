//! The S3-compatible backend, driven against a local stub.

mod stub;

use std::path::Path;

use pixeldelta_cli::{Credentials, S3Config, Storage};
use stub::{Reply, Stub};

fn config(endpoint: &str) -> S3Config {
    S3Config {
        bucket: "shots".into(),
        prefix: "pixeldelta".into(),
        region: "us-east-1".into(),
        endpoint: Some(endpoint.to_owned()),
        credentials: Credentials {
            key_id: "AKIDEXAMPLE".into(),
            secret: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: None,
        },
    }
}

#[test]
fn a_stored_key_is_a_head_on_its_manifest() {
    let stub = Stub::start(vec![Reply::status(200)]);
    let storage = Storage::s3(config(&stub.url()));

    let exists = storage.exists("abc123").expect("the check succeeds");

    assert!(exists);
    let requests = stub.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "HEAD");
    assert_eq!(requests[0].target, "/shots/pixeldelta/abc123/manifest.json");
}

#[test]
fn a_missing_manifest_is_not_an_error() {
    let stub = Stub::start(vec![Reply::status(404)]);
    let storage = Storage::s3(config(&stub.url()));

    let exists = storage
        .exists("abc123")
        .expect("a 404 is an answer, not a failure");

    assert!(!exists);
}

#[test]
fn every_request_carries_a_signature() {
    let stub = Stub::start(vec![Reply::status(200)]);
    let storage = Storage::s3(config(&stub.url()));

    storage.exists("abc123").expect("the check succeeds");

    let request = stub.requests().remove(0);
    let authorization = request
        .header("authorization")
        .expect("the request is signed");
    assert!(
        authorization.starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/"),
        "{authorization}"
    );
    assert!(
        authorization.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"),
        "{authorization}"
    );
    assert!(request.header("x-amz-date").is_some());
    assert!(request.header("x-amz-content-sha256").is_some());
}

#[test]
fn storing_a_snapshot_puts_the_images_before_the_manifest() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("a.png"), b"a").expect("the file is written");
    std::fs::create_dir_all(dir.path().join("nested")).expect("the directory is created");
    std::fs::write(dir.path().join("nested/b.png"), b"b").expect("the file is written");
    let stub = Stub::start(vec![
        Reply::status(200),
        Reply::status(200),
        Reply::status(200),
    ]);
    let storage = Storage::s3(config(&stub.url()));

    storage
        .store("abc123", dir.path())
        .expect("the snapshot is stored");

    let requests = stub.requests();
    let targets: Vec<&str> = requests.iter().map(|r| r.target.as_str()).collect();
    assert_eq!(
        targets,
        vec![
            "/shots/pixeldelta/abc123/images/a.png",
            "/shots/pixeldelta/abc123/images/nested/b.png",
            "/shots/pixeldelta/abc123/manifest.json",
        ],
        "the manifest is written last"
    );
    assert!(requests.iter().all(|r| r.method == "PUT"));
    assert_eq!(requests[0].body, b"a");
}

#[test]
fn fetching_reads_the_manifest_and_then_the_images() {
    let dest = tempfile::tempdir().expect("a temporary directory");
    let stub = Stub::start(vec![
        Reply::ok(br#"{"version":1,"files":["a.png","nested/b.png"]}"#),
        Reply::ok(b"a"),
        Reply::ok(b"b"),
    ]);
    let storage = Storage::s3(config(&stub.url()));

    let files = storage
        .fetch("abc123", dest.path())
        .expect("the snapshot is fetched");

    assert_eq!(files, vec!["a.png".to_owned(), "nested/b.png".to_owned()]);
    assert_eq!(read(&dest.path().join("a.png")), b"a");
    assert_eq!(read(&dest.path().join("nested/b.png")), b"b");
    let requests = stub.requests();
    assert!(requests.iter().all(|r| r.method == "GET"));
}

#[test]
fn a_stored_report_answers_with_its_url() {
    let stub = Stub::start(vec![Reply::status(200)]);
    let storage = Storage::s3(config(&stub.url()));

    let url = storage
        .store_report("abc123", b"<html></html>")
        .expect("the report is stored");

    assert_eq!(
        url,
        Some(format!(
            "{}/shots/pixeldelta/abc123/report/index.html",
            stub.url()
        ))
    );
}

#[test]
fn a_refused_request_is_an_error() {
    let stub = Stub::start(vec![Reply::status(403)]);
    let storage = Storage::s3(config(&stub.url()));

    let error = storage
        .fetch("abc123", Path::new("unused"))
        .expect_err("the storage refused");

    assert!(error.to_string().contains("403"), "{error}");
}

/// Without an endpoint the request goes to the bucket's own host, which is the
/// form AWS accepts for buckets made after path style was withdrawn.
#[test]
fn without_an_endpoint_the_bucket_is_the_host() {
    let mut config = config("unused");
    config.endpoint = None;
    let storage = Storage::s3(config);

    assert_eq!(
        storage.object_url("abc123/manifest.json"),
        Some("https://shots.s3.us-east-1.amazonaws.com/pixeldelta/abc123/manifest.json".to_owned())
    );
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()))
}
