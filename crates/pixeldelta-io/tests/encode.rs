//! Encoding an RGBA8 buffer to PNG and reading it back.

use pixeldelta_io::{decode, encode_png, EncodeError};

#[test]
fn an_encoded_buffer_decodes_to_the_same_pixels() {
    let width = 3;
    let height = 2;
    let rgba: Vec<u8> = (0..width * height * 4)
        .map(|i| (i * 7 % 256) as u8)
        .collect();

    let png = encode_png(width, height, &rgba).expect("the buffer encodes");
    let decoded = decode(&png).expect("the PNG decodes");

    assert_eq!(decoded.width(), width);
    assert_eq!(decoded.height(), height);
    assert_eq!(decoded.as_rgba8(), rgba.as_slice());
}

#[test]
fn a_buffer_of_the_wrong_length_is_rejected() {
    let error = encode_png(2, 2, &[0; 8]).expect_err("four pixels need sixteen bytes");
    assert!(matches!(
        error,
        EncodeError::WrongLength {
            expected: 16,
            actual: 8,
        }
    ));
}
