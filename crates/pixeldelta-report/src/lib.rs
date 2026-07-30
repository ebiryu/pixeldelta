//! Reports for a directory comparison.
//!
//! Takes the outcome of comparing two directories and renders it three ways: a
//! self-contained HTML page for people, JSON for other tools, JUnit XML for CI,
//! and Markdown for a notification. The types are the crate's own, so the
//! comparison engine stays free of a serialization dependency; the caller maps
//! its results onto them.

mod html;
mod json;
mod junit;
mod markdown;

use serde::Serialize;

pub use html::{asset_path, html, local_assets, url_path};
pub use json::json;
pub use junit::junit;
pub use markdown::{markdown, MARKER};

/// Which of an entry's images a source is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Expected,
    Actual,
    Diff,
}

/// Which side of the comparison an entry came out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// In both directories and equal within the threshold.
    Matched,
    /// In both directories, with differences within the allowed ratio.
    Tolerated,
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

/// Which of an entry's images exist, filled per category.
///
/// Not serialized: JSON and JUnit carry no images. `html()` does not read this
/// field directly; a caller's `src` callback consults it to decide whether an
/// entry has a given side at all before answering with a URL for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Images {
    pub expected: bool,
    pub actual: bool,
    pub diff: bool,
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
    /// `[width, height]` of the compared images, when they share a size.
    ///
    /// Used by the HTML to place cluster rectangles over the diff image. Not
    /// serialized: JSON and JUnit report cluster bounds as numbers.
    #[serde(skip)]
    pub image_size: Option<[u32; 2]>,
    #[serde(skip)]
    pub images: Images,
}

/// A whole directory comparison, ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub threshold: f32,
    pub antialiasing: bool,
    pub layout_shift: bool,
    /// The ratio of differing pixels the run allowed before an entry counted
    /// as changed.
    pub tolerance_ratio: f64,
    pub entries: Vec<Entry>,
}

/// How many entries fell into each category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub matched: u32,
    pub tolerated: u32,
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
            tolerated: 0,
            changed: 0,
            size_mismatch: 0,
            added: 0,
            removed: 0,
            passed: true,
        };
        for entry in &self.entries {
            match entry.category {
                Category::Matched => s.matched += 1,
                Category::Tolerated => s.tolerated += 1,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, category: Category) -> Entry {
        Entry {
            path: path.to_owned(),
            category,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: Vec::new(),
            expected_size: None,
            actual_size: None,
            image_size: None,
            images: Images::default(),
        }
    }

    #[test]
    fn summary_counts_tolerated_entries_and_still_passes() {
        let report = Report {
            threshold: 0.1,
            antialiasing: true,
            layout_shift: true,
            tolerance_ratio: 0.05,
            entries: vec![
                entry("a.png", Category::Matched),
                entry("b.png", Category::Tolerated),
            ],
        };

        let summary = report.summary();

        assert_eq!(summary.matched, 1);
        assert_eq!(summary.tolerated, 1);
        assert_eq!(summary.changed, 0);
        assert!(summary.passed);
    }
}
