//! Reports for a directory comparison.
//!
//! Takes the outcome of comparing two directories and renders it three ways: a
//! self-contained HTML page for people, JSON for other tools, and JUnit XML for
//! CI. The types are the crate's own, so the comparison engine stays free of a
//! serialization dependency; the caller maps its results onto them.

mod json;

use serde::Serialize;

pub use json::json;

/// Which side of the comparison an entry came out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// In both directories and equal within the threshold.
    Matched,
    /// In both directories with differences.
    Changed,
    /// In both directories but of different dimensions.
    SizeMismatch,
    /// Only in the actual directory.
    Added,
    /// Only in the expected directory.
    Removed,
}

/// A connected group of differing pixels within an entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cluster {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub diff_pixels: u64,
    /// The offset the group moved by, `[dx, dy]`, when the layout-shift search
    /// found one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displacement: Option<[i32; 2]>,
    /// Structural similarity to the other image over the group's rectangle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssim: Option<f64>,
}

/// PNG bytes for the images an entry shows, filled per category.
///
/// Not serialized: JSON and JUnit carry no images, and the HTML embeds these as
/// data URIs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Images {
    pub expected: Option<Vec<u8>>,
    pub actual: Option<Vec<u8>>,
    pub diff: Option<Vec<u8>>,
}

/// One compared path.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Path relative to the compared directories.
    pub path: String,
    pub category: Category,
    pub diff_pixels: u64,
    pub diff_ratio: f64,
    pub clusters: Vec<Cluster>,
    /// `[width, height]` of each side, present only for a size mismatch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_size: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_size: Option<[u32; 2]>,
    #[serde(skip)]
    pub images: Images,
}

/// A whole directory comparison, ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub threshold: f32,
    pub antialiasing: bool,
    pub layout_shift: bool,
    pub entries: Vec<Entry>,
}

/// How many entries fell into each category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub matched: u32,
    pub changed: u32,
    pub size_mismatch: u32,
    pub added: u32,
    pub removed: u32,
    /// Whether every entry matched.
    pub passed: bool,
}

impl Report {
    /// Counts the entries by category.
    pub fn summary(&self) -> Summary {
        let mut s = Summary {
            matched: 0,
            changed: 0,
            size_mismatch: 0,
            added: 0,
            removed: 0,
            passed: true,
        };
        for entry in &self.entries {
            match entry.category {
                Category::Matched => s.matched += 1,
                Category::Changed => s.changed += 1,
                Category::SizeMismatch => s.size_mismatch += 1,
                Category::Added => s.added += 1,
                Category::Removed => s.removed += 1,
            }
        }
        s.passed = s.changed == 0 && s.size_mismatch == 0 && s.added == 0 && s.removed == 0;
        s
    }
}
