//! The S3-compatible backend for snapshot storage.
//!
//! Only three operations are used: `HEAD` to see whether a key is stored,
//! `GET` to read an object, and `PUT` to write one. Anything an S3-compatible
//! service has beyond that is not needed to keep snapshots.

use crate::http::{self, Answer};
use crate::sigv4::{self, Credentials};

/// Where an S3-compatible storage lives and how to authenticate to it.
#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    /// Prefix every object sits under, which may be empty.
    pub prefix: String,
    pub region: String,
    /// Set for a service other than AWS, such as R2 or MinIO.
    pub endpoint: Option<String>,
    pub credentials: Credentials,
}

/// A client for one bucket.
///
/// Public because it names a variant of [`crate::Storage`]; the operations on
/// it are reached through that enum.
pub struct S3 {
    config: S3Config,
    agent: ureq::Agent,
}

impl std::fmt::Debug for S3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3")
            .field("bucket", &self.config.bucket)
            .field("prefix", &self.config.prefix)
            .field("region", &self.config.region)
            .field("endpoint", &self.config.endpoint)
            .finish()
    }
}

impl S3 {
    pub(crate) fn new(config: S3Config) -> S3 {
        S3 {
            config,
            agent: http::agent(),
        }
    }

    /// Full URL of an object, where `path` is relative to the prefix.
    pub(crate) fn url(&self, path: &str) -> String {
        match &self.config.endpoint {
            Some(endpoint) => format!(
                "{}/{}/{}",
                endpoint.trim_end_matches('/'),
                self.config.bucket,
                self.encoded_key(path)
            ),
            None => format!("https://{}/{}", self.host(), self.encoded_key(path)),
        }
    }

    /// Sends a request, retrying it on a connection failure or a 5xx.
    ///
    /// `HEAD` and `GET` only read, and `PUT` writes the same key with the
    /// same body on every attempt, so all three are safe to repeat.
    pub(crate) fn send(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<Answer, ureq::Error> {
        http::retry(|| self.send_once(method, path, body))
    }

    /// Signs and sends one attempt.
    ///
    /// Signing happens here rather than in `send` because `x-amz-date` is
    /// part of what the signature covers, and a retried attempt sent later
    /// has to sign its own timestamp.
    fn send_once(&self, method: &str, path: &str, body: &[u8]) -> Result<Answer, ureq::Error> {
        let url = self.url(path);
        let headers = sigv4::sign(
            &sigv4::Request {
                method,
                path: &self.signed_path(path),
                host: &self.host(),
                region: &self.config.region,
                body,
                timestamp: &sigv4::timestamp(now()),
            },
            &self.config.credentials,
        );

        let mut request = match method {
            "HEAD" => self.agent.head(&url),
            "GET" => self.agent.get(&url),
            _ => {
                let mut request = self
                    .agent
                    .put(&url)
                    .header("content-type", content_type(path));
                for (name, value) in &headers {
                    request = request.header(name, value);
                }
                let mut answer = request.send(body)?;
                return Ok(Answer {
                    status: answer.status().as_u16(),
                    body: answer.body_mut().read_to_vec()?,
                });
            }
        };
        for (name, value) in &headers {
            request = request.header(name, value);
        }
        let mut answer = request.call()?;
        Ok(Answer {
            status: answer.status().as_u16(),
            body: answer.body_mut().read_to_vec()?,
        })
    }

    /// Host the request goes to, which the signature covers.
    fn host(&self) -> String {
        match &self.config.endpoint {
            Some(endpoint) => endpoint
                .trim_end_matches('/')
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or(endpoint)
                .to_owned(),
            None => format!(
                "{}.s3.{}.amazonaws.com",
                self.config.bucket, self.config.region
            ),
        }
    }

    /// Object key within the bucket.
    fn object_key(&self, path: &str) -> String {
        match self.config.prefix.trim_matches('/') {
            "" => path.to_owned(),
            prefix => format!("{prefix}/{path}"),
        }
    }

    /// Object key as it appears in a URL path.
    ///
    /// A key holds the name of a compared file, which can carry a space or a
    /// non-ASCII character. `ureq` parses the URL with `http::Uri`, which
    /// rejects a non-ASCII byte, so such a key has to be percent-encoded to be
    /// sent at all.
    ///
    /// This is the only place that encodes: the URL and the path the
    /// signature covers are both built from it, so they cannot disagree, and a
    /// key is not encoded twice.
    fn encoded_key(&self, path: &str) -> String {
        sigv4::encode_path(&self.object_key(path))
    }

    /// Path the signature covers, which holds the bucket when the request goes
    /// to an endpoint rather than to the bucket's own host.
    fn signed_path(&self, path: &str) -> String {
        match &self.config.endpoint {
            Some(_) => format!("/{}/{}", self.config.bucket, self.encoded_key(path)),
            None => format!("/{}", self.encoded_key(path)),
        }
    }
}

/// Media type an object is stored under, taken from the extension of its path.
///
/// An object stored without one comes back as `binary/octet-stream`, which a
/// browser downloads rather than renders.
fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.') {
        Some((_, "html")) => "text/html; charset=utf-8",
        Some((_, "png")) => "image/png",
        Some((_, "json")) => "application/json",
        _ => "application/octet-stream",
    }
}

/// Seconds since the Unix epoch.
///
/// A clock before the epoch would only make the signature stale, which the
/// service reports, so it reads as zero rather than failing here.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(endpoint: Option<&str>) -> S3Config {
        S3Config {
            bucket: "shots".into(),
            prefix: "pixeldelta".into(),
            region: "us-east-1".into(),
            endpoint: endpoint.map(str::to_owned),
            credentials: Credentials {
                key_id: "id".into(),
                secret: "secret".into(),
                session_token: None,
            },
        }
    }

    #[test]
    fn an_endpoint_puts_the_bucket_in_the_path() {
        let s3 = S3::new(config(Some("https://example.invalid")));

        assert_eq!(
            s3.url("abc/manifest.json"),
            "https://example.invalid/shots/pixeldelta/abc/manifest.json"
        );
        assert_eq!(s3.host(), "example.invalid");
        assert_eq!(
            s3.signed_path("abc/manifest.json"),
            "/shots/pixeldelta/abc/manifest.json"
        );
    }

    #[test]
    fn without_an_endpoint_the_bucket_is_the_host() {
        let s3 = S3::new(config(None));

        assert_eq!(
            s3.url("abc/manifest.json"),
            "https://shots.s3.us-east-1.amazonaws.com/pixeldelta/abc/manifest.json"
        );
        assert_eq!(
            s3.signed_path("abc/manifest.json"),
            "/pixeldelta/abc/manifest.json"
        );
    }

    #[test]
    fn an_empty_prefix_leaves_the_key_alone() {
        let mut config = config(None);
        config.prefix = String::new();
        let s3 = S3::new(config);

        assert_eq!(s3.object_key("abc/manifest.json"), "abc/manifest.json");
    }

    #[test]
    fn a_non_ascii_name_is_encoded_in_the_url() {
        let s3 = S3::new(config(None));

        assert_eq!(
            s3.url("abc/トップ.png"),
            "https://shots.s3.us-east-1.amazonaws.com/pixeldelta/abc/%E3%83%88%E3%83%83%E3%83%97.png"
        );
    }

    /// The signature covers the path the request line carries. `+` is the
    /// case that passes `http::Uri` unchanged, so an encoding applied to only
    /// one of the two is read by the service as a bad signature rather than
    /// as a malformed URL.
    #[test]
    fn the_signed_path_and_the_url_carry_the_same_key() {
        let s3 = S3::new(config(Some("https://example.invalid")));

        assert_eq!(
            s3.signed_path("abc/a+b.png"),
            "/shots/pixeldelta/abc/a%2Bb.png"
        );
        assert_eq!(
            s3.url("abc/a+b.png"),
            "https://example.invalid/shots/pixeldelta/abc/a%2Bb.png"
        );
    }
}
