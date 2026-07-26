//! Golden test for the Markdown body a notification carries.

mod sample;

use pixeldelta_report::markdown;

#[test]
fn markdown_matches_the_golden_file() {
    insta::assert_snapshot!(markdown(&sample::sample_report(), "5046eb28"));
}

/// A body long enough to be rejected by the comment API would deliver nothing,
/// so each category lists at most twenty rows.
#[test]
fn a_long_list_is_cut_off() {
    let mut report = sample::sample_report();
    let changed = report
        .entries
        .iter()
        .find(|entry| entry.category == pixeldelta_report::Category::Changed)
        .expect("the sample holds a changed entry")
        .clone();
    for index in 0..30 {
        let mut entry = changed.clone();
        entry.path = format!("generated/{index}.png");
        report.entries.push(entry);
    }

    let body = markdown(&report, "5046eb28");

    assert_eq!(body.matches("| `generated/").count(), 19);
    assert!(body.contains("and 11 more"), "{body}");
}
