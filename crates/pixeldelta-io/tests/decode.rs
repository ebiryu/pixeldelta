//! Decoding of the PNG color types, all of which reach the engine as RGBA8.

use std::io::Cursor;

use pixeldelta_io::{decode, decode_file, DecodeError, Format};

/// Encodes `data` as a PNG of the given color type and bit depth.
fn encode(
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    palette: Option<Vec<u8>>,
    trns: Option<Vec<u8>>,
    data: &[u8],
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    if let Some(palette) = palette {
        encoder.set_palette(palette);
    }
    if let Some(trns) = trns {
        encoder.set_trns(trns);
    }
    let mut writer = encoder.write_header().expect("header is writable");
    writer.write_image_data(data).expect("data is writable");
    writer.finish().expect("stream closes");
    out
}

#[test]
fn rgba8_keeps_every_channel() {
    let pixels = [1, 2, 3, 4, 250, 251, 252, 253];
    let png = encode(
        2,
        1,
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        None,
        None,
        &pixels,
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!((decoded.width(), decoded.height()), (2, 1));
    assert_eq!(decoded.as_rgba8(), pixels);
}

#[test]
fn rgb8_gains_an_opaque_alpha() {
    let png = encode(
        2,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        None,
        None,
        &[1, 2, 3, 250, 251, 252],
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!(decoded.as_rgba8(), [1, 2, 3, 255, 250, 251, 252, 255]);
}

#[test]
fn grayscale_spreads_over_the_color_channels() {
    let png = encode(
        2,
        1,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        None,
        None,
        &[0, 200],
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!(decoded.as_rgba8(), [0, 0, 0, 255, 200, 200, 200, 255]);
}

#[test]
fn a_palette_is_expanded_to_its_colors() {
    let png = encode(
        2,
        1,
        png::ColorType::Indexed,
        png::BitDepth::Eight,
        Some(vec![10, 20, 30, 40, 50, 60]),
        None,
        &[1, 0],
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!(decoded.as_rgba8(), [40, 50, 60, 255, 10, 20, 30, 255]);
}

#[test]
fn a_transparent_palette_entry_keeps_its_alpha() {
    let png = encode(
        2,
        1,
        png::ColorType::Indexed,
        png::BitDepth::Eight,
        Some(vec![10, 20, 30, 40, 50, 60]),
        Some(vec![0]),
        &[0, 1],
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!(decoded.as_rgba8(), [10, 20, 30, 0, 40, 50, 60, 255]);
}

#[test]
fn a_palette_that_is_not_whole_entries_is_reported_rather_than_panicking() {
    // Seven bytes: two entries and one byte over. The decoder expands a palette
    // three bytes at a time and indexes past the end on the leftover byte.
    let png = encode(
        2,
        1,
        png::ColorType::Indexed,
        png::BitDepth::Eight,
        Some(vec![10, 20, 30, 40, 50, 60, 70]),
        None,
        &[0, 1],
    );

    let error = decode(&png).expect_err("the palette is rejected");

    assert!(
        matches!(&error, DecodeError::Malformed { message } if message.contains("7 bytes")),
        "unexpected error: {error}"
    );
}

#[test]
fn sixteen_bit_channels_keep_their_high_byte() {
    // 0x1234 and 0xabcd per channel, big endian as PNG stores them.
    let png = encode(
        1,
        1,
        png::ColorType::Rgb,
        png::BitDepth::Sixteen,
        None,
        None,
        &[0x12, 0x34, 0xab, 0xcd, 0x00, 0xff],
    );

    let decoded = decode(&png).expect("decodes");

    assert_eq!(decoded.as_rgba8(), [0x12, 0xab, 0x00, 255]);
}

#[test]
fn a_jpeg_is_reported_as_the_format_it_is() {
    let jpeg = [0xff, 0xd8, 0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0];

    let error = decode(&jpeg).expect_err("JPEG is not supported");

    assert!(
        matches!(
            error,
            DecodeError::UnsupportedFormat {
                format: Format::Jpeg
            }
        ),
        "expected the JPEG to be recognized, got {error}"
    );
    assert!(
        error.to_string().contains("JPEG"),
        "the message should name the format: {error}"
    );
}

#[test]
fn bytes_that_are_no_image_are_reported_as_an_unknown_format() {
    let error = decode(b"not an image at all").expect_err("text is not an image");

    assert!(
        matches!(
            error,
            DecodeError::UnsupportedFormat {
                format: Format::Unknown
            }
        ),
        "expected an unknown format, got {error}"
    );
}

#[test]
fn a_truncated_png_is_reported_rather_than_panicking() {
    let png = encode(
        2,
        1,
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        None,
        None,
        &[0; 8],
    );

    let error = decode(&png[..png.len() / 2]).expect_err("half a PNG does not decode");

    assert!(
        matches!(error, DecodeError::Malformed { .. }),
        "expected a malformed PNG, got {error}"
    );
}

#[test]
fn a_file_decodes_the_same_as_its_bytes() {
    let png = encode(
        2,
        1,
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        None,
        None,
        &[1, 2, 3, 4, 5, 6, 7, 8],
    );
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("a_file.png");
    std::fs::write(&path, &png).expect("fixture is writable");

    let decoded = decode_file(&path).expect("decodes");

    assert_eq!(
        decoded.as_rgba8(),
        decode(&png).expect("decodes").as_rgba8()
    );
}

#[test]
fn a_missing_file_names_the_path_it_looked_for() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("absent.png");
    let _ = std::fs::remove_file(&path);

    let error = decode_file(&path).expect_err("the file is not there");

    assert!(
        matches!(&error, DecodeError::Read { path: p, .. } if p == &path),
        "expected a read error naming the path, got {error}"
    );
}
