//! The Markdown body a GitHub notification carries.
//!
//! One body serves every route: a pull request comment, a check run, and the
//! job summary all take Markdown, and a reader who saw one should not find a
//! different account in another.

use std::fmt::Write;

use crate::{Category, Cluster, Entry, Report};

/// Marker the body opens with, so a later run can find the comment it wrote
/// and replace it rather than adding another.
pub const MARKER: &str = "<!-- pixeldelta -->";

/// Rows one category lists before it is cut off.
///
/// A body over the comment API's length limit is rejected, and a rejected
/// comment delivers nothing.
const MAX_ROWS: usize = 20;

/// Renders the notification body for a comparison against `baseline`.
///
/// `report_url` is where the HTML report can be read. The body names counts
/// and paths but carries no image, so without the link a reader has no way
/// from here to the difference itself.
pub fn markdown(report: &Report, baseline: &str, report_url: Option<&str>) -> String {
    let summary = report.summary();
    let mut out = String::new();

    out.push_str(MARKER);
    out.push('\n');
    let _ = writeln!(
        out,
        "### pixeldelta: {}\n",
        if summary.passed {
            "no visual differences".to_owned()
        } else {
            headline(report)
        }
    );

    out.push_str("| category | count |\n| --- | --: |\n");
    for (name, count) in [
        ("changed", summary.changed),
        ("added", summary.added),
        ("removed", summary.removed),
        ("size mismatch", summary.size_mismatch),
        ("tolerated", summary.tolerated),
        ("matched", summary.matched),
    ] {
        let _ = writeln!(out, "| {name} | {count} |");
    }

    out.push('\n');
    let _ = write!(
        out,
        "Compared against `{baseline}` with threshold {}, anti-aliasing {}",
        report.threshold,
        if report.antialiasing {
            "excluded"
        } else {
            "counted"
        },
    );
    if report.tolerance_ratio > 0.0 {
        let _ = write!(out, ", tolerance {}", report.tolerance_ratio);
    }
    out.push_str(".\n");

    // Ahead of the lists, which are cut off at MAX_ROWS, so a run with many
    // changes does not put the link behind the cut.
    if let Some(url) = report_url {
        let _ = writeln!(out, "\n[Open the report]({url})");
    }

    changed_table(&mut out, report);
    path_list(&mut out, report, Category::SizeMismatch, "size mismatch");
    path_list(&mut out, report, Category::Added, "added");
    path_list(&mut out, report, Category::Removed, "removed");

    out
}

/// Sums up the categories that fail a run.
fn headline(report: &Report) -> String {
    let summary = report.summary();
    let parts = [
        (summary.changed, "changed"),
        (summary.added, "added"),
        (summary.removed, "removed"),
        (summary.size_mismatch, "size mismatch"),
    ];
    parts
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, name)| format!("{count} {name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Lists the changed entries with what their clusters say.
fn changed_table(out: &mut String, report: &Report) {
    let entries = of_category(report, Category::Changed);
    if entries.is_empty() {
        return;
    }

    out.push_str("\n#### changed\n\n");
    out.push_str(
        "| path | diff pixels | ratio | clusters | min ssim |\n| --- | --: | --: | --- | --: |\n",
    );
    for entry in entries.iter().take(MAX_ROWS) {
        let _ = writeln!(
            out,
            "| `{}` | {} | {:.2}% | {} | {} |",
            entry.path,
            entry.diff_pixels,
            entry.diff_ratio * 100.0,
            clusters(&entry.clusters),
            min_ssim(&entry.clusters),
        );
    }
    remainder(out, entries.len());
}

/// Describes an entry's clusters as how many moved and how many did not.
///
/// A cluster carries a displacement when the layout-shift search matched the
/// content at an offset, which separates a move from a change in place.
fn clusters(clusters: &[Cluster]) -> String {
    if clusters.is_empty() {
        return "-".to_owned();
    }

    let mut moved: Vec<&Cluster> = Vec::new();
    let mut changed = 0;
    for cluster in clusters {
        match cluster.displacement {
            Some(_) => moved.push(cluster),
            None => changed += 1,
        }
    }

    let mut parts = Vec::new();
    if !moved.is_empty() {
        // Clusters that shifted together carry the same offset, and repeating
        // it once per cluster says nothing more than the count already does.
        let mut offsets: Vec<[i32; 2]> = Vec::new();
        for cluster in &moved {
            if let Some(offset) = cluster.displacement {
                if !offsets.contains(&offset) {
                    offsets.push(offset);
                }
            }
        }
        let shown: Vec<String> = offsets
            .iter()
            .take(3)
            .map(|[dx, dy]| format!("{dx:+}, {dy:+}"))
            .collect();
        parts.push(format!("{} moved ({})", moved.len(), shown.join("; ")));
    }
    if changed > 0 {
        parts.push(format!("{changed} changed"));
    }
    parts.join(", ")
}

/// The lowest similarity among an entry's clusters.
///
/// The value is reported as measured rather than turned into a label: the
/// boundary between a change of color and a change of structure is not fixed
/// by a measurement yet.
fn min_ssim(clusters: &[Cluster]) -> String {
    let lowest = clusters
        .iter()
        .filter_map(|cluster| cluster.ssim)
        .fold(f64::INFINITY, f64::min);
    if lowest.is_finite() {
        format!("{lowest:.3}")
    } else {
        "-".to_owned()
    }
}

/// Lists the paths of one category.
fn path_list(out: &mut String, report: &Report, category: Category, title: &str) {
    let entries = of_category(report, category);
    if entries.is_empty() {
        return;
    }

    let _ = write!(out, "\n#### {title}\n\n");
    for entry in entries.iter().take(MAX_ROWS) {
        let _ = writeln!(out, "- `{}`{}", entry.path, sizes(entry));
    }
    remainder(out, entries.len());
}

/// Reports both dimensions for a size mismatch, and nothing otherwise.
fn sizes(entry: &Entry) -> String {
    match (entry.expected_size, entry.actual_size) {
        (Some([ew, eh]), Some([aw, ah])) => format!(" — {ew}x{eh} to {aw}x{ah}"),
        _ => String::new(),
    }
}

fn remainder(out: &mut String, total: usize) {
    if total > MAX_ROWS {
        let _ = writeln!(out, "\nand {} more", total - MAX_ROWS);
    }
}

fn of_category(report: &Report, category: Category) -> Vec<&Entry> {
    report
        .entries
        .iter()
        .filter(|entry| entry.category == category)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moved(offset: Option<[i32; 2]>) -> Cluster {
        Cluster {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            diff_pixels: 40,
            displacement: offset,
            ssim: Some(0.9),
        }
    }

    #[test]
    fn clusters_sharing_an_offset_report_it_once() {
        let summary = clusters(&[moved(Some([0, 4])), moved(Some([0, 4]))]);

        assert_eq!(summary, "2 moved (+0, +4)");
    }

    #[test]
    fn clusters_with_different_offsets_report_each() {
        let summary = clusters(&[moved(Some([0, 4])), moved(Some([2, 0])), moved(None)]);

        assert_eq!(summary, "2 moved (+0, +4; +2, +0), 1 changed");
    }
}
