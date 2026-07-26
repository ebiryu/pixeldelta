//! Posting the notification body as a pull request comment.

use std::path::Path;

use serde::Deserialize;

/// Where to post and what to authenticate with.
#[derive(Debug, Clone)]
pub struct GithubConfig {
    /// Base of the REST API, which differs on GitHub Enterprise.
    pub api_url: String,
    /// `owner/repo`.
    pub repository: String,
    pub pull_request: u64,
    pub token: String,
}

/// What posting the body came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    /// The body is on the pull request.
    Posted { url: String, updated: bool },
    /// The token may not write to this repository.
    Refused,
}

/// Reasons the comment cannot be posted.
#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    /// The API answered with a status the caller cannot act on.
    #[error("GitHub answered {status} for {target}")]
    Status { status: u16, target: String },
    /// The request could not be carried out.
    #[error("{target} could not be requested: {source}")]
    Request {
        target: String,
        source: Box<ureq::Error>,
    },
    /// The answer is not the JSON this build reads.
    #[error("the answer from {target} could not be read: {source}")]
    Answer {
        target: String,
        source: serde_json::Error,
    },
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
struct Posted {
    #[serde(default)]
    html_url: String,
}

/// Posts `body` on the pull request, replacing the comment a previous run left.
///
/// The comment to replace is the one whose body opens with the marker the body
/// carries.
pub fn notify(config: &GithubConfig, body: &str) -> Result<Notification, GithubError> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build(),
    );

    let list = format!(
        "{}/repos/{}/issues/{}/comments?per_page=100",
        config.api_url.trim_end_matches('/'),
        config.repository,
        config.pull_request
    );
    let answer = send(&agent, config, "GET", &list, None)?;
    let existing: Vec<Comment> = read_json(&list, &answer)?;
    let mine = existing
        .iter()
        .find(|comment| comment.body.starts_with(pixeldelta_report::MARKER));

    let payload = serde_json::json!({ "body": body }).to_string();
    let (method, target) = match mine {
        Some(comment) => (
            "PATCH",
            format!(
                "{}/repos/{}/issues/comments/{}",
                config.api_url.trim_end_matches('/'),
                config.repository,
                comment.id
            ),
        ),
        None => (
            "POST",
            format!(
                "{}/repos/{}/issues/{}/comments",
                config.api_url.trim_end_matches('/'),
                config.repository,
                config.pull_request
            ),
        ),
    };

    let answer = send(&agent, config, method, &target, Some(payload.as_bytes()))?;
    // A pull request from a fork carries a read-only token. The comparison
    // itself ran, so its outcome still reaches the run's exit code and job
    // summary.
    if answer.status == 403 {
        return Ok(Notification::Refused);
    }
    let posted: Posted = read_json(&target, &answer)?;
    Ok(Notification::Posted {
        url: posted.html_url,
        updated: mine.is_some(),
    })
}

/// Reads the pull request number from the event payload a workflow run holds.
///
/// Answers `None` when the run is not on a pull request, or when the file is
/// not there to read.
pub fn pull_request_number(event_path: &Path) -> Option<u64> {
    #[derive(Deserialize)]
    struct Event {
        pull_request: Option<PullRequest>,
    }
    #[derive(Deserialize)]
    struct PullRequest {
        number: u64,
    }

    let bytes = std::fs::read(event_path).ok()?;
    let event: Event = serde_json::from_slice(&bytes).ok()?;
    event.pull_request.map(|pull_request| pull_request.number)
}

struct Answer {
    status: u16,
    body: Vec<u8>,
}

fn send(
    agent: &ureq::Agent,
    config: &GithubConfig,
    method: &str,
    target: &str,
    body: Option<&[u8]>,
) -> Result<Answer, GithubError> {
    let failed = |source: ureq::Error| GithubError::Request {
        target: target.to_owned(),
        source: Box::new(source),
    };

    let mut answer = match body {
        Some(body) => {
            let request = match method {
                "PATCH" => agent.patch(target),
                _ => agent.post(target),
            };
            headers(request, config)
                .header("content-type", "application/json")
                .send(body)
                .map_err(failed)?
        }
        None => headers(agent.get(target), config).call().map_err(failed)?,
    };

    let status = answer.status().as_u16();
    let body = answer.body_mut().read_to_vec().map_err(failed)?;
    if !(200..300).contains(&status) && status != 403 {
        return Err(GithubError::Status {
            status,
            target: target.to_owned(),
        });
    }
    Ok(Answer { status, body })
}

fn headers<T>(request: ureq::RequestBuilder<T>, config: &GithubConfig) -> ureq::RequestBuilder<T> {
    request
        .header("authorization", &format!("Bearer {}", config.token))
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .header("user-agent", "pixeldelta")
}

fn read_json<T: serde::de::DeserializeOwned>(
    target: &str,
    answer: &Answer,
) -> Result<T, GithubError> {
    serde_json::from_slice(&answer.body).map_err(|source| GithubError::Answer {
        target: target.to_owned(),
        source,
    })
}
