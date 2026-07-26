//! Walking a directory for the PNGs that make up one side of a comparison.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A directory entry that could not be read.
#[derive(Debug)]
pub(crate) struct WalkError {
    pub(crate) path: PathBuf,
    pub(crate) source: std::io::Error,
}

/// Collects the relative paths of every `.png` under `root`.
///
/// The paths use forward slashes so the same tree pairs and keys the same way
/// on every platform.
pub(crate) fn collect_pngs(root: &Path) -> Result<BTreeSet<String>, WalkError> {
    let mut out = BTreeSet::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<String>) -> Result<(), WalkError> {
    let reader = std::fs::read_dir(dir).map_err(|source| error(dir, source))?;
    for entry in reader {
        let path = entry.map_err(|source| error(dir, source))?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if is_png(&path) {
            let rel = path
                .strip_prefix(root)
                .expect("the walk stays under the root");
            out.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

fn error(path: &Path, source: std::io::Error) -> WalkError {
    WalkError {
        path: path.to_path_buf(),
        source,
    }
}
