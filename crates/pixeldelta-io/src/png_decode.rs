use png::{BitDepth, ColorType, Transformations};

use crate::{DecodeError, Decoded};

/// Number of bytes per pixel in the RGBA8 layout the engine works on.
const BYTES_PER_PIXEL: usize = 4;

/// Decodes a PNG into RGBA8.
///
/// The palette, grayscale and 16-bit variants all reach the engine as RGBA8:
/// `EXPAND` resolves the palette and the `tRNS` chunk, `STRIP_16` drops the low
/// byte of 16-bit channels, and `ALPHA` adds an opaque alpha where the image
/// carries none. What is left after that is either RGBA or gray plus alpha, and
/// the second is spread over the color channels here.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    // The decoder seeks over the chunks it skips, which a cursor over the
    // bytes in memory supports and a bare slice does not.
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(Transformations::normalize_to_color8() | Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(malformed)?;

    let (width, height) = reader.info().size();
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or(DecodeError::TooLarge { width, height })?
    ];
    let info = reader.next_frame(&mut buffer).map_err(malformed)?;
    buffer.truncate(info.buffer_size());

    let data = match (info.color_type, info.bit_depth) {
        (ColorType::Rgba, BitDepth::Eight) => buffer,
        (ColorType::GrayscaleAlpha, BitDepth::Eight) => spread_gray(&buffer),
        (color_type, bit_depth) => {
            return Err(DecodeError::Malformed {
                message: format!("{color_type:?} at {bit_depth:?} bits did not normalize to RGBA8"),
            })
        }
    };

    let expected = (info.width as usize)
        .checked_mul(info.height as usize)
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL));
    if expected != Some(data.len()) {
        return Err(DecodeError::Malformed {
            message: format!(
                "{} bytes decoded for {}x{} pixels",
                data.len(),
                info.width,
                info.height
            ),
        });
    }

    Ok(Decoded {
        width: info.width,
        height: info.height,
        data,
    })
}

/// Turns gray-plus-alpha pairs into RGBA quadruples.
fn spread_gray(buffer: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(buffer.len() * 2);
    for pair in buffer.chunks_exact(2) {
        data.extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
    }
    data
}

/// Reports a decoding failure without exposing the `png` error type.
fn malformed(error: png::DecodingError) -> DecodeError {
    DecodeError::Malformed {
        message: error.to_string(),
    }
}
