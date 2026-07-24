//! Node.js binding for pixeldelta.
//!
//! Exposes [`compare`] and [`compareSync`] over the comparison engine. Each
//! takes two images as file paths or buffers, decodes them, and compares them.
//! The async form runs the decode and compare on a worker thread so the JS
//! event loop stays free.

use napi::bindgen_prelude::*;
use napi::Task;
use napi_derive::napi;

use pixeldelta_core::{compare as core_compare, CompareOptions};
use pixeldelta_io::{decode, decode_file, Decoded};

mod convert;
use convert::to_result;

/// An image argument: a path to a file, or its encoded bytes.
type Source = Either<String, Uint8Array>;

/// Options controlling a comparison.
///
/// Every field is optional and falls back to the engine default. The names
/// mirror `CompareOptions` in the Rust API with the anti-aliasing flag renamed
/// to the word pixelmatch users know it by.
#[napi(object)]
#[derive(Default)]
pub struct JsCompareOptions {
    /// Matching threshold in `[0, 1]`. Defaults to `0.1`, as in pixelmatch.
    pub threshold: Option<f64>,
    /// Whether pixels differing only by anti-aliasing are excluded. Defaults to
    /// `true`, matching pixelmatch's `includeAA: false`.
    pub antialiasing: Option<bool>,
    /// Regions left out of the comparison.
    pub ignore_regions: Option<Vec<JsRect>>,
    /// Stops the scan once more than `maxDiffPixels` pixels differ.
    pub fail_fast: Option<JsFailFast>,
    /// Whether differing pixels are grouped into clusters. Defaults to `false`.
    pub cluster: Option<bool>,
    /// Whether each cluster is searched for the offset it moved by. Defaults to
    /// `false`.
    pub layout_shift: Option<bool>,
}

/// A rectangle in pixel coordinates, covering `[x, x + width) x [y, y + height)`.
#[napi(object)]
pub struct JsRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The fail-fast limit.
#[napi(object)]
pub struct JsFailFast {
    /// Number of differing pixels to tolerate before stopping.
    pub max_diff_pixels: u32,
}

/// Result of a comparison.
#[napi(object)]
pub struct JsCompareResult {
    /// `'match'`, `'differ'` or `'sizeMismatch'`.
    pub verdict: String,
    /// Number of differing pixels. A lower bound when `stoppedEarly` is set.
    pub diff_pixels: f64,
    /// `diffPixels` over the number of compared pixels, or `0` when none were.
    pub diff_ratio: f64,
    /// Whether the scan stopped at the fail-fast limit.
    pub stopped_early: bool,
    /// Connected groups of differing pixels. Empty unless `cluster` or
    /// `layoutShift` was set.
    pub clusters: Vec<JsCluster>,
}

/// A connected group of differing pixels.
#[napi(object)]
pub struct JsCluster {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub diff_pixels: f64,
    /// The offset the group moved by, when the layout-shift search found one.
    pub displacement: Option<JsDisplacement>,
    /// Structural similarity to the other image over the group's rectangle.
    pub ssim: Option<f64>,
}

/// An offset in pixels.
#[napi(object)]
pub struct JsDisplacement {
    pub dx: i32,
    pub dy: i32,
}

/// Decodes and compares two images off the JS thread.
pub struct CompareTask {
    a: OwnedSource,
    b: OwnedSource,
    options: CompareOptions,
}

/// A decoded-or-not image source owned by the task, so no JS value is held
/// across the thread boundary.
enum OwnedSource {
    Path(String),
    Bytes(Vec<u8>),
}

impl OwnedSource {
    fn from(source: Source) -> Self {
        match source {
            Either::A(path) => Self::Path(path),
            Either::B(bytes) => Self::Bytes(bytes.to_vec()),
        }
    }

    fn decode(&self) -> std::result::Result<Decoded, pixeldelta_io::DecodeError> {
        match self {
            Self::Path(path) => decode_file(std::path::Path::new(path)),
            Self::Bytes(bytes) => decode(bytes),
        }
    }
}

impl Task for CompareTask {
    type Output = JsCompareResult;
    type JsValue = JsCompareResult;

    fn compute(&mut self) -> Result<Self::Output> {
        let a = self.a.decode().map_err(decode_error)?;
        let b = self.b.decode().map_err(decode_error)?;
        run(&a, &b, &self.options)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Compares two images, returning a promise.
#[napi(ts_return_type = "Promise<JsCompareResult>")]
pub fn compare(
    a: Source,
    b: Source,
    options: Option<JsCompareOptions>,
) -> Result<AsyncTask<CompareTask>> {
    Ok(AsyncTask::new(CompareTask {
        a: OwnedSource::from(a),
        b: OwnedSource::from(b),
        options: convert::to_options(options.unwrap_or_default())?,
    }))
}

/// Compares two images on the calling thread.
#[napi(js_name = "compareSync")]
pub fn compare_sync(
    a: Source,
    b: Source,
    options: Option<JsCompareOptions>,
) -> Result<JsCompareResult> {
    let options = convert::to_options(options.unwrap_or_default())?;
    let a = OwnedSource::from(a).decode().map_err(decode_error)?;
    let b = OwnedSource::from(b).decode().map_err(decode_error)?;
    run(&a, &b, &options)
}

/// Wraps the decoded buffers and runs the comparison.
fn run(a: &Decoded, b: &Decoded, options: &CompareOptions) -> Result<JsCompareResult> {
    let a = pixeldelta_core::Image::from_rgba8(a.width(), a.height(), a.as_rgba8())
        .map_err(image_error)?;
    let b = pixeldelta_core::Image::from_rgba8(b.width(), b.height(), b.as_rgba8())
        .map_err(image_error)?;
    Ok(to_result(core_compare(&a, &b, options)))
}

/// Turns a decode failure into a JS exception carrying its message.
fn decode_error(error: pixeldelta_io::DecodeError) -> Error {
    Error::from_reason(error.to_string())
}

/// Turns a buffer-wrapping failure into a JS exception.
fn image_error(error: pixeldelta_core::ImageError) -> Error {
    Error::from_reason(error.to_string())
}
