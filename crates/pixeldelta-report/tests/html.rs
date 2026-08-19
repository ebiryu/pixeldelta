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
fn the_viewer_caps_its_width_from_the_image_aspect_ratio() {
    let out = html(&sample::sample_report(), local_assets);

    // The sample's changed entry is 320x200.
    assert!(out.contains("--cap:calc(52vh * 320 / 200)"), "{out}");
}

#[test]
fn a_viewer_without_an_image_size_carries_no_cap() {
    let mut report = sample::sample_report();
    for entry in &mut report.entries {
        entry.image_size = None;
    }

    let out = html(&report, local_assets);

    // The stylesheet names the property, so only the attribute is checked.
    assert!(!out.contains("style=\"--cap"), "{out}");
    assert!(!out.contains("--iw:"), "{out}");
}

#[test]
fn the_viewer_bar_carries_a_zoom_control_pressed_at_fit_by_default() {
    let out = html(&sample::sample_report(), local_assets);

    // The stylesheet also names `data-zoom` in its selectors, so the button
    // markup itself, not just the attribute, is asserted on.
    assert!(
        out.contains("<button aria-pressed=\"true\" data-zoom=\"fit\">fit</button>"),
        "{out}"
    );
    assert!(out.contains("data-zoom=\"1\">1:1</button>"), "{out}");
    assert!(out.contains("data-zoom=\"2\">2x</button>"), "{out}");
    assert!(out.contains("data-zoom=\"4\">4x</button>"), "{out}");
}

#[test]
fn the_stage_carries_iw_from_the_image_size() {
    let out = html(&sample::sample_report(), local_assets);

    // The sample's changed entry is 320x200.
    assert!(
        out.contains("--cap:calc(52vh * 320 / 200);--iw:320px"),
        "{out}"
    );
}

#[test]
fn a_cluster_rect_is_emitted_inside_the_same_frame_as_the_diff_image() {
    let out = html(&sample::sample_report(), local_assets);

    let frame_start = out.find("<div class=\"frame\">").expect("a frame div");
    let diff_img = out[frame_start..]
        .find("images/diff/")
        .expect("the diff image after the frame");
    let cluster_rect = out[frame_start..]
        .find("cluster-rect")
        .expect("a cluster rect after the frame");
    assert!(diff_img < cluster_rect, "{out}");
}

#[test]
fn a_caption_sits_outside_the_pane_it_names() {
    let out = html(&sample::sample_report(), local_assets);

    // Inside the pane the caption is drawn over the screenshot's top-left
    // corner, which is as likely to hold a difference as anywhere else.
    assert!(
        out.contains("<span class=\"cap\">expected</span><div class=\"pane\">"),
        "{out}"
    );
    assert!(
        !out.contains("<div class=\"pane\"><span class=\"cap\">"),
        "{out}"
    );
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
            omitted_clusters: 0,
            expected_size: None,
            actual_size: None,
            image_size: None,
            images: Images {
                expected: false,
                actual: true,
                diff: false,
            },
        }],
    };

    let out = html(&report, local_assets);

    assert!(out.contains("images/actual/caf%C3%A9%20shot.png"), "{out}");
}
