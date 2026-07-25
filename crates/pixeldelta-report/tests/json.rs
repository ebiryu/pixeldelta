//! Golden test for the JSON report.

mod sample;

use pixeldelta_report::json;

#[test]
fn json_matches_the_golden_file() {
    insta::assert_snapshot!(json(&sample::sample_report()));
}
