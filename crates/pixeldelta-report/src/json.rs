use serde::Serialize;

use crate::{Entry, Report};

/// The JSON shape: the parameters, the category counts, then every entry.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport<'a> {
    threshold: f32,
    antialiasing: bool,
    layout_shift: bool,
    summary: crate::Summary,
    entries: &'a [Entry],
}

/// Renders the report as pixeldelta's own JSON schema, including the cluster
/// information of each entry.
pub fn json(report: &Report) -> String {
    let view = JsonReport {
        threshold: report.threshold,
        antialiasing: report.antialiasing,
        layout_shift: report.layout_shift,
        summary: report.summary(),
        entries: &report.entries,
    };
    serde_json::to_string_pretty(&view).expect("the report serializes")
}
