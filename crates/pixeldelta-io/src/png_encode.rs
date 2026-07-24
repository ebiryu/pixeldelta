use png::{BitDepth, ColorType, Encoder};

use crate::EncodeError;

/// Number of bytes per pixel in the RGBA8 layout the engine works on.
const BYTES_PER_PIXEL: usize = 4;

/// Encodes an RGBA8 buffer as PNG bytes.
///
/// The buffer holds `width * height * 4` bytes, row by row, without padding.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL));
    if expected != Some(rgba.len()) {
        return Err(EncodeError::WrongLength {
            expected: expected.unwrap_or(usize::MAX),
            actual: rgba.len(),
        });
    }

    let mut bytes = Vec::new();
    let mut encoder = Encoder::new(&mut bytes, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    // Writing to a `Vec` cannot fail for I/O reasons, so a failure here means the
    // encoder rejected the parameters rather than the sink.
    let mut writer = encoder.write_header().map_err(failed)?;
    writer.write_image_data(rgba).map_err(failed)?;
    writer.finish().map_err(failed)?;

    Ok(bytes)
}

/// Reports an encoding failure without exposing the `png` error type.
fn failed(error: png::EncodingError) -> EncodeError {
    EncodeError::Failed {
        message: error.to_string(),
    }
}
