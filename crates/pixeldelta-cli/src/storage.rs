//! Where snapshots are kept between runs.
//!
//! A snapshot is the set of PNGs a run compared, held under a key. The layout
//! is the same for every backend:
//!
//! ```text
//! <root>/<key>/manifest.json           files the snapshot holds
//! <root>/<key>/images/<path>           the PNGs, at their relative paths
//! <root>/<key>/report/index.html
//! <root>/<key>/report/images/diff/<path>   diff images the report references
//! ```
//!
//! A key counts as present when its `manifest.json` is present, which is one
//! object rather than a listing. The manifest is written after the images, so
//! a write that stopped halfway does not read as a complete snapshot.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::paths::collect_pngs;
use crate::s3::S3;

pub use crate::s3::S3Config;

/// Version this build writes and reads.
const MANIFEST_VERSION: u32 = 1;

/// How many object requests run at once.
///
/// Fixed rather than sized to the CPU count: the wait on each request is for
/// a network response, not CPU time, so a low core count should not throttle
/// it down to a couple of requests in flight.
const REQUEST_CONCURRENCY: usize = 16;

/// The pool parallel object requests run on, built the first time it is
/// needed.
///
/// A pool of its own, not rayon's global one: the global pool is sized to the
/// CPU count and is also what comparison work in `run_dirs` uses, so blocking
/// its workers on network responses would stall comparisons waiting for a
/// free worker.
///
/// `None` when the pool could not be built, in which case callers fall back
/// to running the same work sequentially on the calling thread.
fn request_pool() -> Option<&'static rayon::ThreadPool> {
    static POOL: OnceLock<Option<rayon::ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(REQUEST_CONCURRENCY)
            .build()
            .ok()
    })
    .as_ref()
}

/// Runs `f` over `items`, in parallel on the request pool when one is
/// available, sequentially on the calling thread otherwise.
///
/// Stops at the first error; which of several concurrent failures is
/// returned is unspecified.
fn parallel_try_for_each<T, F>(items: &[T], f: F) -> Result<(), StorageError>
where
    T: Sync,
    F: Fn(&T) -> Result<(), StorageError> + Sync,
{
    match request_pool() {
        Some(pool) => pool.install(|| items.par_iter().try_for_each(&f)),
        None => items.iter().try_for_each(f),
    }
}

/// The files a snapshot holds.
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    files: Vec<String>,
}

/// Where snapshots are kept.
#[derive(Debug)]
pub enum Storage {
    /// A directory on the local filesystem, which has no public URL.
    Dir(PathBuf),
    /// A bucket on an S3-compatible service.
    S3(Box<S3>),
}

/// Reasons a snapshot cannot be read or written.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The `--storage` argument does not name a supported location.
    #[error("{spec} is not a storage location: give a local directory path")]
    Location { spec: String },
    /// A key holds characters that do not belong in a path.
    #[error("{key} is not a snapshot key")]
    Key { key: String },
    /// No snapshot is stored under the key.
    #[error("no snapshot is stored for {key}")]
    Missing { key: String },
    /// A file could not be read.
    #[error("{path} could not be read: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A file could not be written.
    #[error("{path} could not be written: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The manifest is not the JSON this build reads.
    #[error("the manifest for {key} could not be read: {source}")]
    Manifest {
        key: String,
        source: serde_json::Error,
    },
    /// The manifest was written by a build that lays snapshots out differently.
    #[error("the manifest for {key} has version {version}, and this build reads version {MANIFEST_VERSION}")]
    ManifestVersion { key: String, version: u32 },
    /// A path in the manifest would write outside the destination.
    #[error("the manifest for {key} holds {path}, which leaves the destination directory")]
    ManifestPath { key: String, path: String },
    /// An object listed in a manifest is not in the storage.
    #[error("{path} is listed in the manifest but is not stored")]
    MissingObject { path: String },
    /// The storage answered a request with a status that is not success.
    #[error("the storage answered {status} for {path}")]
    Status { status: u16, path: String },
    /// The request could not be carried out.
    #[error("{path} could not be requested: {source}")]
    Request {
        path: String,
        source: Box<ureq::Error>,
    },
    /// The storage location names a service this build does not speak to.
    #[error("{variable} is not set, and an S3 storage needs it")]
    MissingEnv { variable: String },
}

impl Storage {
    /// Reads the `--storage` argument.
    ///
    /// A spec without a scheme is a directory on the local filesystem. An
    /// `s3://bucket/prefix` spec takes the rest of its settings, including the
    /// credentials, from the environment.
    pub fn parse(spec: &str) -> Result<Self, StorageError> {
        if let Some(rest) = spec.strip_prefix("s3://") {
            let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
            return Ok(Storage::s3(S3Config {
                bucket: bucket.to_owned(),
                prefix: prefix.to_owned(),
                region: env("AWS_REGION").unwrap_or_else(|| "us-east-1".to_owned()),
                endpoint: env("AWS_ENDPOINT_URL"),
                credentials: crate::Credentials {
                    key_id: required_env("AWS_ACCESS_KEY_ID")?,
                    secret: required_env("AWS_SECRET_ACCESS_KEY")?,
                    session_token: env("AWS_SESSION_TOKEN"),
                },
            }));
        }
        if spec.contains("://") {
            return Err(StorageError::Location {
                spec: spec.to_owned(),
            });
        }
        Ok(Storage::Dir(PathBuf::from(spec)))
    }

    /// Builds a storage on an S3-compatible service.
    pub fn s3(config: S3Config) -> Storage {
        Storage::S3(Box::new(S3::new(config)))
    }

    /// Whether a complete snapshot is stored under `key`.
    pub fn exists(&self, key: &str) -> Result<bool, StorageError> {
        validate_key(key)?;
        self.object_exists(&format!("{key}/manifest.json"))
    }

    /// Writes the snapshot stored under `key` into `dest`.
    ///
    /// Returns the relative paths it wrote, in the order the manifest holds.
    pub fn fetch(&self, key: &str, dest: &Path) -> Result<Vec<String>, StorageError> {
        validate_key(key)?;
        let manifest_path = format!("{key}/manifest.json");
        let bytes = self
            .get_object(&manifest_path)?
            .ok_or_else(|| StorageError::Missing {
                key: key.to_owned(),
            })?;

        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|source| StorageError::Manifest {
                key: key.to_owned(),
                source,
            })?;
        if manifest.version != MANIFEST_VERSION {
            return Err(StorageError::ManifestVersion {
                key: key.to_owned(),
                version: manifest.version,
            });
        }

        parallel_try_for_each(&manifest.files, |file| {
            let rel = relative_path(key, file)?;
            let path = format!("{key}/images/{file}");
            let bytes = self
                .get_object(&path)?
                .ok_or(StorageError::MissingObject { path })?;
            write(&dest.join(&rel), &bytes)
        })?;
        Ok(manifest.files)
    }

    /// Stores the PNGs under `dir` as the snapshot for `key`.
    ///
    /// Returns a URL when the backend can serve one.
    pub fn store(&self, key: &str, dir: &Path) -> Result<Option<String>, StorageError> {
        validate_key(key)?;
        let files: Vec<String> = collect_pngs(dir)
            .map_err(|error| StorageError::Read {
                path: error.path,
                source: error.source,
            })?
            .into_iter()
            .collect();

        parallel_try_for_each(&files, |file| {
            let bytes = read(&dir.join(file))?;
            self.put_object(&format!("{key}/images/{file}"), &bytes)
        })?;

        // Written last: a key counts as stored once its manifest is there, so
        // a write that stopped halfway is not picked as a baseline.
        let manifest = Manifest {
            version: MANIFEST_VERSION,
            files,
        };
        let json = serde_json::to_vec(&manifest).expect("a manifest of strings serializes");
        self.put_object(&format!("{key}/manifest.json"), &json)?;
        Ok(None)
    }

    /// Stores the HTML report for `key`, along with the images it references.
    ///
    /// Each entry in `images` is a path below `<key>/report/` (for example
    /// `images/diff/foo.png`) and its bytes. The images are written first and
    /// `index.html` last, so a write that stopped halfway does not read as a
    /// complete report.
    ///
    /// Returns a URL when the backend can serve one.
    pub fn store_report(
        &self,
        key: &str,
        html: &[u8],
        images: &[(String, &[u8])],
    ) -> Result<Option<String>, StorageError> {
        validate_key(key)?;
        parallel_try_for_each(images, |(rel, bytes)| {
            self.put_object(&format!("{key}/report/{rel}"), bytes)
        })?;
        let path = format!("{key}/report/index.html");
        self.put_object(&path, html)?;
        Ok(self.object_url(&path))
    }

    /// Where an object can be read from, when the backend serves URLs.
    pub fn object_url(&self, path: &str) -> Option<String> {
        match self {
            Storage::Dir(_) => None,
            Storage::S3(s3) => Some(s3.url(path)),
        }
    }

    fn object_exists(&self, path: &str) -> Result<bool, StorageError> {
        match self {
            Storage::Dir(root) => Ok(root.join(path).is_file()),
            Storage::S3(s3) => match self.answer(s3, "HEAD", path, &[])?.status {
                200..=299 => Ok(true),
                404 => Ok(false),
                status => Err(StorageError::Status {
                    status,
                    path: path.to_owned(),
                }),
            },
        }
    }

    /// Reads an object, answering `None` when it is not stored.
    fn get_object(&self, path: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match self {
            Storage::Dir(root) => match std::fs::read(root.join(path)) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(source) => Err(StorageError::Read {
                    path: root.join(path),
                    source,
                }),
            },
            Storage::S3(s3) => {
                let answer = self.answer(s3, "GET", path, &[])?;
                match answer.status {
                    200..=299 => Ok(Some(answer.body)),
                    404 => Ok(None),
                    status => Err(StorageError::Status {
                        status,
                        path: path.to_owned(),
                    }),
                }
            }
        }
    }

    fn put_object(&self, path: &str, bytes: &[u8]) -> Result<(), StorageError> {
        match self {
            Storage::Dir(root) => write(&root.join(path), bytes),
            Storage::S3(s3) => {
                let answer = self.answer(s3, "PUT", path, bytes)?;
                match answer.status {
                    200..=299 => Ok(()),
                    status => Err(StorageError::Status {
                        status,
                        path: path.to_owned(),
                    }),
                }
            }
        }
    }

    fn answer(
        &self,
        s3: &S3,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<crate::http::Answer, StorageError> {
        s3.send(method, path, body)
            .map_err(|source| StorageError::Request {
                path: path.to_owned(),
                source: Box::new(source),
            })
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn required_env(name: &str) -> Result<String, StorageError> {
    env(name).ok_or_else(|| StorageError::MissingEnv {
        variable: name.to_owned(),
    })
}

/// Checks that a key can stand as a path component.
///
/// Keys are commit hashes; rejecting anything else keeps a key from reaching
/// outside the storage root.
fn validate_key(key: &str) -> Result<(), StorageError> {
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(StorageError::Key {
            key: key.to_owned(),
        })
    }
}

/// Turns a path from a manifest into one that stays under the destination.
///
/// The manifest comes from the storage, so a path in it is not trusted to be
/// relative or to stay below its root.
fn relative_path(key: &str, file: &str) -> Result<PathBuf, StorageError> {
    let rejected = |path: &str| StorageError::ManifestPath {
        key: key.to_owned(),
        path: path.to_owned(),
    };

    if file.is_empty() || file.contains('\\') || file.contains(':') {
        return Err(rejected(file));
    }
    let mut out = PathBuf::new();
    for part in file.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(rejected(file));
        }
        out.push(part);
    }
    Ok(out)
}

fn read(path: &Path) -> Result<Vec<u8>, StorageError> {
    std::fs::read(path).map_err(|source| StorageError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| StorageError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, bytes).map_err(|source| StorageError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_keeps_its_components() {
        let path = relative_path("key", "a/b.png").expect("the path stays below the destination");

        assert_eq!(path, Path::new("a").join("b.png"));
    }

    #[test]
    fn a_path_climbing_out_is_rejected() {
        for file in ["../a.png", "a/../../b.png", "/a.png", "", "a//b.png"] {
            assert!(
                relative_path("key", file).is_err(),
                "{file} should be rejected"
            );
        }
    }

    #[test]
    fn a_key_that_is_not_a_hash_is_rejected() {
        for key in ["", "..", "a/b", "a b"] {
            assert!(validate_key(key).is_err(), "{key} should be rejected");
        }
        assert!(validate_key("0123abc").is_ok());
    }
}
