//! Conversions between the JS option and result shapes and the engine's types.

use napi::bindgen_prelude::*;

use pixeldelta_core::{
    Cluster, CompareOptions, CompareResult, DiffImage, DiffStyle, FailFast, Rect, Verdict,
};

use crate::{
    JsCluster, JsCompareOptions, JsCompareResult, JsDiffImage, JsDiffStyle, JsDisplacement,
    JsFailFast, JsRect,
};

/// Builds engine options from the JS options, filling each unset field with the
/// engine default.
pub fn to_options(js: JsCompareOptions) -> Result<CompareOptions> {
    let defaults = CompareOptions::default();
    Ok(CompareOptions {
        threshold: js.threshold.map(|t| t as f32).unwrap_or(defaults.threshold),
        detect_antialiasing: js.antialiasing.unwrap_or(defaults.detect_antialiasing),
        ignore_regions: js
            .ignore_regions
            .unwrap_or_default()
            .into_iter()
            .map(to_rect)
            .collect(),
        fail_fast: js.fail_fast.map(to_fail_fast),
        cluster: js.cluster.unwrap_or(defaults.cluster),
        layout_shift: js.layout_shift.unwrap_or(defaults.layout_shift),
        diff: js.diff.map(to_diff_style),
    })
}

fn to_diff_style(js: JsDiffStyle) -> DiffStyle {
    let defaults = DiffStyle::default();
    DiffStyle {
        alpha: js.alpha.map(|a| a as f32).unwrap_or(defaults.alpha),
        diff_color: js
            .color
            .and_then(|c| <[u32; 3]>::try_from(c).ok())
            .map(|[r, g, b]| [r as u8, g as u8, b as u8])
            .unwrap_or(defaults.diff_color),
    }
}

fn to_rect(js: JsRect) -> Rect {
    Rect {
        x: js.x,
        y: js.y,
        width: js.width,
        height: js.height,
    }
}

fn to_fail_fast(js: JsFailFast) -> FailFast {
    FailFast {
        max_diff_pixels: u64::from(js.max_diff_pixels),
    }
}

/// Maps an engine result onto the JS result shape.
pub fn to_result(result: CompareResult) -> JsCompareResult {
    JsCompareResult {
        verdict: match result.verdict {
            Verdict::Match => "match",
            Verdict::Differ => "differ",
            Verdict::SizeMismatch => "sizeMismatch",
        }
        .to_owned(),
        diff_pixels: result.diff_pixels as f64,
        diff_ratio: result.diff_ratio,
        stopped_early: result.stopped_early,
        clusters: result.clusters.into_iter().map(to_cluster).collect(),
        diff_image: result.diff_image.map(to_diff_image),
    }
}

fn to_diff_image(diff: DiffImage) -> JsDiffImage {
    JsDiffImage {
        width: diff.width,
        height: diff.height,
        data: diff.data.into(),
    }
}

fn to_cluster(cluster: Cluster) -> JsCluster {
    JsCluster {
        x: cluster.bounds.x,
        y: cluster.bounds.y,
        width: cluster.bounds.width,
        height: cluster.bounds.height,
        diff_pixels: cluster.diff_pixels as f64,
        displacement: cluster
            .displacement
            .map(|(dx, dy)| JsDisplacement { dx, dy }),
        ssim: cluster.ssim,
    }
}
