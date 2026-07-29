#![no_main]

//! Feeds arbitrary bytes to the decoder.
//!
//! `decode` is where bytes from outside the process first enter pixeldelta: the
//! comparison engine takes a decoded RGBA8 buffer and performs no I/O. A panic
//! here reaches the Node binding as an aborted process rather than an exception,
//! so the property under test is that no input panics, whatever it decodes to.
//!
//! Inputs that fail to decode are expected and are not failures.

use libfuzzer_sys::fuzz_target;

/// Bytes a PNG file starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Largest image the target decodes, in pixels.
///
/// Decoding allocates four bytes per pixel from the dimensions in the header,
/// before any of the code under test runs. A mutated header asking for hundreds
/// of megapixels makes libFuzzer report its own allocation limit, which says
/// nothing about the decoder.
const MAX_PIXELS: u64 = 1 << 22;

fuzz_target!(|data: &[u8]| {
    if declares_more_than_max_pixels(data) {
        return;
    }
    let _ = pixeldelta_io::decode(data);
});

/// Reads the dimensions a PNG header declares and compares them to the cap.
///
/// Bytes that do not start with the PNG signature are rejected on the signature
/// alone and never reach an allocation, so they are always let through.
fn declares_more_than_max_pixels(data: &[u8]) -> bool {
    if !data.starts_with(&SIGNATURE) {
        return false;
    }
    // IHDR is the first chunk: eight bytes of signature, then the chunk length
    // and type, then width and height.
    let Some(dimensions) = data.get(16..24) else {
        return false;
    };
    let width = u32::from_be_bytes([dimensions[0], dimensions[1], dimensions[2], dimensions[3]]);
    let height = u32::from_be_bytes([dimensions[4], dimensions[5], dimensions[6], dimensions[7]]);
    u64::from(width) * u64::from(height) > MAX_PIXELS
}
