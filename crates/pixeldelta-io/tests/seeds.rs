//! Replays the fuzzing seeds through the decoder.
//!
//! `fuzz/seeds/decode` holds what is worth keeping of the fuzz target's input:
//! seeds covering the color types the decoder normalizes and the shapes of a
//! file that fails to decode, plus the input behind every crash that has been
//! fixed. The fuzz target itself builds only on nightly, so running the seeds
//! from here is what puts them on `cargo test`, and what keeps a fixed crash
//! from coming back unnoticed.

use std::path::PathBuf;

use pixeldelta_io::decode;

/// Locates the seeds, which sit beside the workspace rather than in a crate.
fn seeds_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/seeds/decode")
}

#[test]
fn no_seed_panics() {
    let dir = seeds_dir();
    let entries = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} could not be read: {error}", dir.display()));

    let mut replayed = 0;
    for entry in entries {
        let path = entry.expect("the directory entry is readable").path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).expect("the seed is readable");

        // Failing to decode is the expected outcome for several of the seeds.
        // What is checked is that the call returns at all, and that a buffer
        // handed back holds what its dimensions say it does: the engine indexes
        // it without a bounds check.
        match std::panic::catch_unwind(|| decode(&bytes)) {
            Ok(Ok(decoded)) => assert_eq!(
                decoded.as_rgba8().len(),
                decoded.width() as usize * decoded.height() as usize * 4,
                "{} decoded to a buffer its dimensions do not account for",
                path.display()
            ),
            Ok(Err(_)) => {}
            Err(_) => panic!("{} made the decoder panic", path.display()),
        }
        replayed += 1;
    }

    // An empty directory would leave this test green while checking nothing.
    assert!(replayed > 0, "{} holds no seeds", dir.display());
}
