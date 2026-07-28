//! Golden test for the HTML report.

mod sample;

use pixeldelta_report::{html, local_assets, Category, Entry, Images, Report};

#[test]
fn html_matches_the_golden_file() {
    insta::assert_snapshot!(html(&sample::sample_report(), local_assets));
}

#[test]
fn images_are_referenced_by_path_rather_than_embedded() {
    let out = html(&sample::sample_report(), local_assets);

    assert!(out.contains("images/diff/"), "{out}");
    assert!(!out.contains("data:image"), "{out}");
}

#[test]
fn an_img_tag_carries_loading_lazy() {
    let out = html(&sample::sample_report(), local_assets);

    assert!(out.contains("loading=\"lazy\""), "{out}");
}

#[test]
fn a_path_with_a_space_and_non_ascii_is_percent_encoded_in_the_src() {
    let report = Report {
        threshold: 0.1,
        antialiasing: true,
        layout_shift: true,
        tolerance_ratio: 0.0,
        entries: vec![Entry {
            path: "café shot.png".into(),
            category: Category::Added,
            diff_pixels: 0,
            diff_ratio: 0.0,
            clusters: vec![],
            expected_size: None,
            actual_size: None,
            image_size: None,
            images: Images {
                expected: None,
                actual: Some(b"ACTUAL-PNG".to_vec()),
                diff: None,
            },
        }],
    };

    let out = html(&report, local_assets);

    assert!(out.contains("images/actual/caf%C3%A9%20shot.png"), "{out}");
}
