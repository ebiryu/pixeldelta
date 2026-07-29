# Fuzzing the decoder

`decode` is where bytes from outside the process enter pixeldelta: the
comparison engine takes a decoded RGBA8 buffer and performs no I/O. A panic
there reaches the Node binding as an aborted process rather than an exception,
and reaches the WebAssembly build as a trap, so the property under test is that
no input panics. Failing to decode is the expected outcome for most inputs and
is not a failure.

    just fuzz        # 60 seconds
    just fuzz 600

This needs a nightly toolchain and `cargo install --locked cargo-fuzz`.
`libfuzzer-sys` builds only on nightly, which is why this package declares a
workspace of its own and stays out of `cargo check`, `cargo test` and the MSRV
build at the repository root.

## Where the inputs live

`seeds/decode` is committed. It holds seeds covering the color types the decoder
normalizes and the shapes of a file that fails to decode, plus the input behind
every crash that has been fixed.

`corpus/decode` is not committed. It is where libFuzzer writes what it finds,
and it reaches thousands of files within minutes of running. Deleting it costs
nothing but the coverage a past run accumulated.

`../crates/pixeldelta-io/tests/seeds.rs` replays `seeds/decode` through `decode`
on stable, so the seeds stay a regression test on `cargo test` with neither
nightly nor cargo-fuzz in the way.

## After a crash

libFuzzer writes the input to `artifacts/decode`.

1. Shrink it: `cargo +nightly fuzz tmin decode artifacts/decode/<input>`.
2. Fix the cause, and add a test naming the condition beside the decoder's own
   tests.
3. Rebuild the chunk CRCs of the shrunk input, and put it in `seeds/decode`
   under a name saying what it is, not the hash it arrived with.

The CRCs matter because the fuzz build sets `--cfg fuzzing`, which turns off the
decoder's CRC checks so that a mutated file reaches the code behind them. An
input whose CRCs no longer match stops at the CRC on the stable path, and the
regression test then exercises nothing.

## The size cap

The target skips PNGs whose header declares more than 2^22 pixels. Decoding
allocates four bytes per pixel from those dimensions before any of the code
under test runs, so a mutated header asking for hundreds of megapixels reports
libFuzzer's allocation limit instead of anything about the decoder.
