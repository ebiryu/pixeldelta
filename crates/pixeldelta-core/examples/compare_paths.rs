//! Compares two PNG files, for the same-machine benchmark in `tools/bench`.
//!
//! Two modes, one per row of that benchmark:
//!
//! - default: decode both files and compare once, then print the diff pixel
//!   count. The harness times the whole process, which is the end-to-end
//!   number (startup, decode and compare).
//! - `--warm N`: decode once and compare `N` times, then print the median
//!   compare-only time in milliseconds. Decode and startup are left out, which
//!   is the engine-only number.
//!
//! ```text
//! compare_paths <base.png> <head.png> [--threshold F] [--no-aa] [--warm N]
//! ```
//!
//! This is an example rather than a benchmark because it decodes, which the
//! library does not: `png` is a dev-dependency, and only examples, tests and
//! benches can reach it.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use pixeldelta_core::{compare, CompareOptions, Image};

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().expect("base path");
    let head = args.next().expect("head path");

    let mut threshold = 0.1f32;
    let mut detect_antialiasing = true;
    let mut warm: Option<usize> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--threshold" => threshold = args.next().expect("threshold value").parse().unwrap(),
            "--no-aa" => detect_antialiasing = false,
            "--warm" => warm = Some(args.next().expect("warm count").parse().unwrap()),
            other => panic!("unknown flag {other}"),
        }
    }

    let (bw, bh, bd) = decode(&base);
    let (hw, hh, hd) = decode(&head);
    let a = Image::from_rgba8(bw, bh, &bd).expect("base wraps");
    let b = Image::from_rgba8(hw, hh, &hd).expect("head wraps");
    let opts = CompareOptions {
        threshold,
        detect_antialiasing,
        ..CompareOptions::default()
    };

    match warm {
        None => {
            let result = compare(&a, &b, &opts);
            println!("{}", result.diff_pixels);
        }
        Some(iterations) => {
            let mut samples = Vec::with_capacity(iterations);
            for _ in 0..iterations {
                let start = Instant::now();
                let result = compare(&a, &b, &opts);
                samples.push(start.elapsed().as_secs_f64() * 1e3);
                std::hint::black_box(result.diff_pixels);
            }
            samples.sort_by(|x, y| x.partial_cmp(y).unwrap());
            println!("{:.4}", samples[samples.len() / 2]);
        }
    }
}

/// Decodes a PNG file to width, height and RGBA8 bytes.
fn decode(path: &str) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(BufReader::new(
        File::open(Path::new(path)).expect("file opens"),
    ));
    let mut decoder = decoder;
    decoder.set_transformations(png::Transformations::ALPHA | png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("is a PNG");
    let mut data = vec![0; reader.output_buffer_size().expect("size fits")];
    let info = reader.next_frame(&mut data).expect("decodes");
    data.truncate(info.buffer_size());
    (info.width, info.height, data)
}
