use png::{BitDepth, ColorType, Transformations};

use crate::{DecodeError, Decoded};

/// Number of bytes per pixel in the RGBA8 layout the engine works on.
const BYTES_PER_PIXEL: usize = 4;

/// Number of bytes a palette entry occupies in a PLTE chunk.
const PALETTE_ENTRY: usize = 3;

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
    if let Some(palette) = &reader.info().palette {
        check_palette(palette.len())?;
    }

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

/// Rejects a PLTE chunk whose length cannot describe whole palette entries.
///
/// `png` bounds the chunk to 3..=768 bytes but does not require the length to
/// be a whole number of entries. Expanding the palette then walks it three
/// bytes at a time and reads past the end on the one or two bytes left over,
/// which panics, and a panic would reach the Node binding as an aborted
/// process rather than an exception.
fn check_palette(len: usize) -> Result<(), DecodeError> {
    if !len.is_multiple_of(PALETTE_ENTRY) {
        return Err(DecodeError::Malformed {
            message: format!(
                "the palette holds {len} bytes, which is not {PALETTE_ENTRY} per entry"
            ),
        });
    }
    Ok(())
}

/// Turns gray-plus-alpha pairs into RGBA quadruples.
fn spread_gray(buffer: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(buffer.len() * 2);
    for pair in buffer.as_chunks::<2>().0 {
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
