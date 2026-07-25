//! A sample report the golden tests render, covering every category.

use pixeldelta_report::{Category, Cluster, Entry, Images, Report};

pub fn sample_report() -> Report {
    Report {
        threshold: 0.1,
        antialiasing: true,
        layout_shift: true,
        entries: vec![
            Entry {
                path: "components/button/primary.png".into(),
                category: Category::Changed,
                diff_pixels: 1284,
                diff_ratio: 0.0201,
                clusters: vec![
                    Cluster {
                        x: 14,
                        y: 138,
                        width: 96,
                        height: 34,
                        diff_pixels: 612,
                        displacement: Some([0, 4]),
                        ssim: Some(0.981),
                    },
                    Cluster {
                        x: 18,
                        y: 40,
                        width: 132,
                        height: 16,
                        diff_pixels: 672,
                        displacement: None,
                        ssim: Some(0.412),
                    },
                ],
                image_size: Some([320, 200]),
                expected_size: None,
                actual_size: None,
                images: Images {
                    expected: Some(b"EXPECTED-PNG".to_vec()),
                    actual: Some(b"ACTUAL-PNG".to_vec()),
                    diff: Some(b"DIFF-PNG".to_vec()),
                },
            },
            Entry {
                path: "pages/dashboard.png".into(),
                category: Category::SizeMismatch,
                diff_pixels: 0,
                diff_ratio: 0.0,
                clusters: vec![],
                expected_size: Some([1280, 864]),
                actual_size: Some([1280, 912]),
                image_size: None,
                images: Images {
                    expected: Some(b"EXPECTED-PNG".to_vec()),
                    actual: Some(b"ACTUAL-PNG".to_vec()),
                    diff: None,
                },
            },
            Entry {
                path: "components/toast.png".into(),
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
            },
            Entry {
                path: "components/banner-legacy.png".into(),
                category: Category::Removed,
                diff_pixels: 0,
                diff_ratio: 0.0,
                clusters: vec![],
                expected_size: None,
                actual_size: None,
                image_size: None,
                images: Images {
                    expected: Some(b"EXPECTED-PNG".to_vec()),
                    actual: None,
                    diff: None,
                },
            },
            Entry {
                path: "components/checkbox.png".into(),
                category: Category::Matched,
                diff_pixels: 0,
                diff_ratio: 0.0,
                clusters: vec![],
                expected_size: None,
                actual_size: None,
                image_size: None,
                images: Images {
                    expected: Some(b"EXPECTED-PNG".to_vec()),
                    actual: None,
                    diff: None,
                },
            },
        ],
    }
}
