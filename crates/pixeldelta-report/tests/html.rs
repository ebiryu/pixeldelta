//! Golden test for the HTML report.

mod sample;

use pixeldelta_report::html;

#[test]
fn html_matches_the_golden_file() {
    insta::assert_snapshot!(html(&sample::sample_report()));
}
