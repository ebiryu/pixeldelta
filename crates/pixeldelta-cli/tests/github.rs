//! Posting the notification body as a pull request comment.

mod stub;

use pixeldelta_cli::{notify, pull_request_number, GithubConfig, Notification};
use stub::{Reply, Stub};

const BODY: &str = "<!-- pixeldelta -->\n### pixeldelta: 1 changed\n";

fn config(api_url: &str) -> GithubConfig {
    GithubConfig {
        api_url: api_url.to_owned(),
        repository: "acme/site".into(),
        pull_request: 7,
        token: "ghs_token".into(),
    }
}

#[test]
fn a_first_run_posts_a_new_comment() {
    let stub = Stub::start(vec![
        Reply::ok(b"[]"),
        Reply::ok(br#"{"id":11,"html_url":"https://github.com/acme/site/pull/7#issuecomment-11"}"#),
    ]);

    let result = notify(&config(&stub.url()), BODY).expect("the comment is posted");

    assert_eq!(
        result,
        Notification::Posted {
            url: "https://github.com/acme/site/pull/7#issuecomment-11".to_owned(),
            updated: false,
        }
    );
    let requests = stub.requests();
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].target,
        "/repos/acme/site/issues/7/comments?per_page=100"
    );
    assert_eq!(requests[1].method, "POST");
    assert_eq!(requests[1].target, "/repos/acme/site/issues/7/comments");
    assert!(String::from_utf8_lossy(&requests[1].body).contains("1 changed"));
    assert_eq!(
        requests[1].header("authorization"),
        Some("Bearer ghs_token")
    );
    assert!(requests[1].header("user-agent").is_some());
}

/// Posting a second comment on every run would bury the pull request under a
/// comment per push.
#[test]
fn a_later_run_replaces_the_comment_it_wrote() {
    let stub = Stub::start(vec![
        Reply::ok(
            br#"[{"id":4,"body":"a review comment"},
                {"id":9,"body":"<!-- pixeldelta -->\nolder body"}]"#,
        ),
        Reply::ok(br#"{"id":9,"html_url":"https://github.com/acme/site/pull/7#issuecomment-9"}"#),
    ]);

    let result = notify(&config(&stub.url()), BODY).expect("the comment is updated");

    assert_eq!(
        result,
        Notification::Posted {
            url: "https://github.com/acme/site/pull/7#issuecomment-9".to_owned(),
            updated: true,
        }
    );
    let requests = stub.requests();
    assert_eq!(requests[1].method, "PATCH");
    assert_eq!(requests[1].target, "/repos/acme/site/issues/comments/9");
}

/// Comments by other authors are left alone: the marker is what identifies the
/// one this tool wrote.
#[test]
fn a_comment_without_the_marker_is_left_alone() {
    let stub = Stub::start(vec![
        Reply::ok(br#"[{"id":4,"body":"looks good to me"}]"#),
        Reply::ok(br#"{"id":12,"html_url":"https://example.invalid/12"}"#),
    ]);

    notify(&config(&stub.url()), BODY).expect("the comment is posted");

    let requests = stub.requests();
    assert_eq!(requests[1].method, "POST");
}

/// A pull request from a fork gets a read-only token, and a comparison that
/// ran is not a failure just because its comment could not be posted.
#[test]
fn a_read_only_token_is_reported_rather_than_failing() {
    let stub = Stub::start(vec![Reply::ok(b"[]"), Reply::status(403)]);

    let result = notify(&config(&stub.url()), BODY).expect("a refusal is not an error");

    assert_eq!(result, Notification::Refused);
}

#[test]
fn an_unexpected_status_is_an_error() {
    let stub = Stub::start(vec![Reply::status(500)]);

    let error = notify(&config(&stub.url()), BODY).expect_err("the API failed");

    assert!(error.to_string().contains("500"), "{error}");
}

#[test]
fn the_pull_request_number_comes_from_the_event() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let event = dir.path().join("event.json");
    std::fs::write(&event, br#"{"pull_request":{"number":42}}"#).expect("the file is written");

    assert_eq!(pull_request_number(&event), Some(42));
}

#[test]
fn an_event_without_a_pull_request_has_no_number() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let event = dir.path().join("event.json");
    std::fs::write(&event, br#"{"ref":"refs/heads/main"}"#).expect("the file is written");

    assert_eq!(pull_request_number(&event), None);
    assert_eq!(pull_request_number(&dir.path().join("absent.json")), None);
}
