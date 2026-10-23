//! Image decoding for pixeldelta.
//!
//! Reads PNG files and byte buffers into the RGBA8 layout the comparison
//! engine works on. The engine itself performs no I/O, so every caller that
//! starts from a file or a `Buffer` comes through here.

mod format;
mod png_decode;
mod png_encode;

pub use format::Format;
pub use png_encode::encode as encode_png;

use std::path::Path;

/// A decoded RGBA8 image.
///
/// Pixels are stored row by row, four bytes per pixel, without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Decoded {
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Pixel bytes, `width * height * 4` of them.
    pub fn as_rgba8(&self) -> &[u8] {
        &self.data
    }

    /// Takes the pixel bytes out of the image.
    pub fn into_rgba8(self) -> Vec<u8> {
        self.data
    }
}

/// Reasons an image cannot be decoded.
///
/// No variant carries the path of the input. `decode` is given bytes, which
/// have no path, so the caller that named the file is the one that names it
/// in its own error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// The file could not be read.
    #[error("the file could not be read: {source}")]
    Read { source: std::io::Error },
    /// The bytes are an image of a format that is not supported.
    #[error("{format} images are not supported")]
    UnsupportedFormat { format: Format },
    /// The bytes claim to be a PNG but do not decode as one.
    #[error("the PNG could not be decoded: {message}")]
    Malformed { message: String },
    /// The image is larger than the address space can hold.
    #[error("{width}x{height} pixels do not fit in memory")]
    TooLarge { width: u32, height: u32 },
}

/// Reasons an image cannot be encoded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    /// The buffer does not hold `width * height * 4` bytes.
    #[error("{expected} bytes expected for the given dimensions, {actual} given")]
    WrongLength { expected: usize, actual: usize },
    /// The encoder rejected the image.
    #[error("the PNG could not be encoded: {message}")]
    Failed { message: String },
}

/// Decodes an image from its bytes.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    match Format::of(bytes) {
        Format::Png => png_decode::decode(bytes),
        format => Err(DecodeError::UnsupportedFormat { format }),
    }
}

/// Decodes an image from a file.
///
/// The error names the reason but not `path`; the caller holds it.
pub fn decode_file(path: &Path) -> Result<Decoded, DecodeError> {
    let bytes = std::fs::read(path).map_err(|source| DecodeError::Read { source })?;
    decode(&bytes)
}
