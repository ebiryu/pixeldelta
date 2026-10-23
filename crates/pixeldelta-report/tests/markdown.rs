//! Golden test for the Markdown body a notification carries.

mod sample;

use pixeldelta_report::markdown;

#[test]
fn markdown_matches_the_golden_file() {
    insta::assert_snapshot!(markdown(
        &sample::sample_report(),
        "5046eb28",
        Some("https://reports.example.invalid/5046eb28/report/index.html"),
    ));
}

/// A reader who has no report to open should not be given a link to nothing.
#[test]
fn a_body_without_a_url_carries_no_link() {
    let body = markdown(&sample::sample_report(), "5046eb28", None);

    assert!(!body.contains("]("), "{body}");
}

/// The link is placed before the lists, which are cut off at twenty rows.
#[test]
fn the_link_comes_before_the_lists() {
    let url = "https://reports.example.invalid/index.html";

    let body = markdown(&sample::sample_report(), "5046eb28", Some(url));

    let link = body
        .find(url)
        .unwrap_or_else(|| panic!("the body carries the URL: {body}"));
    let list = body.find("#### changed").expect("the sample has a list");
    assert!(link < list, "{body}");
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

    let body = markdown(&report, "5046eb28", None);

    assert_eq!(body.matches("| `generated/").count(), 19);
    assert!(body.contains("and 11 more"), "{body}");
}
