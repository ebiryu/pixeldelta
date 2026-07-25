use std::fmt::Write;

use crate::{Category, Entry, Report};

/// Renders the report as JUnit XML.
///
/// Each entry is a `testcase`; every category but `matched` is a `failure`, so
/// a CI runner that reads JUnit fails the build on any difference and names the
/// path that caused it.
pub fn junit(report: &Report) -> String {
    // Every entry that did not match is a failure.
    let failures = report.entries.len() as u32 - report.summary().matched;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        out,
        "<testsuite name=\"pixeldelta\" tests=\"{}\" failures=\"{}\">",
        report.entries.len(),
        failures,
    )
    .unwrap();

    for entry in &report.entries {
        match failure_message(entry) {
            None => writeln!(out, "  <testcase name=\"{}\"/>", escape(&entry.path)).unwrap(),
            Some(message) => {
                writeln!(out, "  <testcase name=\"{}\">", escape(&entry.path)).unwrap();
                writeln!(out, "    <failure message=\"{}\"/>", escape(&message)).unwrap();
                writeln!(out, "  </testcase>").unwrap();
            }
        }
    }

    out.push_str("</testsuite>\n");
    out
}

/// The failure line for an entry, or `None` when it matched.
fn failure_message(entry: &Entry) -> Option<String> {
    match entry.category {
        Category::Matched => None,
        Category::Changed => Some(format!(
            "changed: {} pixels ({:.4}%)",
            entry.diff_pixels,
            entry.diff_ratio * 100.0
        )),
        Category::SizeMismatch => {
            let e = entry.expected_size.unwrap_or([0, 0]);
            let a = entry.actual_size.unwrap_or([0, 0]);
            Some(format!(
                "size mismatch: {}x{} vs {}x{}",
                e[0], e[1], a[0], a[1]
            ))
        }
        Category::Added => Some("added: only in actual".to_owned()),
        Category::Removed => Some("removed: only in expected".to_owned()),
    }
}

/// Escapes the five XML metacharacters in an attribute value.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}
