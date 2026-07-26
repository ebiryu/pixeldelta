//! Where snapshots are kept between runs.
//!
//! A snapshot is the set of PNGs a run compared, held under a key. The layout
//! is the same for every backend:
//!
//! ```text
//! <root>/<key>/manifest.json    files the snapshot holds
//! <root>/<key>/images/<path>    the PNGs, at their relative paths
//! <root>/<key>/report/index.html
//! ```
//!
//! A key counts as present when its `manifest.json` is present, which is one
//! object rather than a listing. The manifest is written after the images, so
//! a write that stopped halfway does not read as a complete snapshot.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::collect_pngs;

/// Version this build writes and reads.
const MANIFEST_VERSION: u32 = 1;

/// The files a snapshot holds.
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    files: Vec<String>,
}

/// Where snapshots are kept.
#[derive(Debug, Clone)]
pub enum Storage {
    /// A directory on the local filesystem, which has no public URL.
    Dir(PathBuf),
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
}

impl Storage {
    /// Reads the `--storage` argument.
    ///
    /// A spec without a scheme is a directory on the local filesystem.
    pub fn parse(spec: &str) -> Result<Self, StorageError> {
        if spec.contains("://") {
            return Err(StorageError::Location {
                spec: spec.to_owned(),
            });
        }
        Ok(Storage::Dir(PathBuf::from(spec)))
    }

    /// Whether a complete snapshot is stored under `key`.
    pub fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let Storage::Dir(root) = self;
        Ok(key_dir(root, key)?.join("manifest.json").is_file())
    }

    /// Writes the snapshot stored under `key` into `dest`.
    ///
    /// Returns the relative paths it wrote, in the order the manifest holds.
    pub fn fetch(&self, key: &str, dest: &Path) -> Result<Vec<String>, StorageError> {
        let Storage::Dir(root) = self;
        let dir = key_dir(root, key)?;
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.is_file() {
            return Err(StorageError::Missing {
                key: key.to_owned(),
            });
        }

        let bytes = read(&manifest_path)?;
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

        for file in &manifest.files {
            let rel = relative_path(key, file)?;
            let bytes = read(&dir.join("images").join(&rel))?;
            write(&dest.join(&rel), &bytes)?;
        }
        Ok(manifest.files)
    }

    /// Stores the PNGs under `dir` as the snapshot for `key`.
    ///
    /// Returns a URL when the backend can serve one.
    pub fn store(&self, key: &str, dir: &Path) -> Result<Option<String>, StorageError> {
        let Storage::Dir(root) = self;
        let target = key_dir(root, key)?;
        let files: Vec<String> = collect_pngs(dir)
            .map_err(|error| StorageError::Read {
                path: error.path,
                source: error.source,
            })?
            .into_iter()
            .collect();

        for file in &files {
            let bytes = read(&dir.join(file))?;
            write(&target.join("images").join(file), &bytes)?;
        }

        let manifest = Manifest {
            version: MANIFEST_VERSION,
            files,
        };
        let json = serde_json::to_vec(&manifest).expect("a manifest of strings serializes");
        write(&target.join("manifest.json"), &json)?;
        Ok(None)
    }

    /// Stores the HTML report for `key`.
    ///
    /// Returns a URL when the backend can serve one.
    pub fn store_report(&self, key: &str, html: &[u8]) -> Result<Option<String>, StorageError> {
        let Storage::Dir(root) = self;
        write(&key_dir(root, key)?.join("report").join("index.html"), html)?;
        Ok(None)
    }
}

/// Resolves the directory holding one key.
///
/// Keys are commit hashes; rejecting anything else keeps a key from reaching
/// outside the storage root.
fn key_dir(root: &Path, key: &str) -> Result<PathBuf, StorageError> {
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(StorageError::Key {
            key: key.to_owned(),
        });
    }
    Ok(root.join(key))
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
        let root = Path::new("/store");

        for key in ["", "..", "a/b", "a b"] {
            assert!(key_dir(root, key).is_err(), "{key} should be rejected");
        }
        assert!(key_dir(root, "0123abc").is_ok());
    }
}
