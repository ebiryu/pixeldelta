//! Golden test for the JUnit report.

mod sample;

use pixeldelta_report::junit;

#[test]
fn junit_matches_the_golden_file() {
    insta::assert_snapshot!(junit(&sample::sample_report()));
}
