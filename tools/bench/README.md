# Same-machine comparison

Measures pixeldelta against pixelmatch and odiff on the benchmark fixtures
under `crates/pixeldelta-core/benches/fixtures`. What is held equal across the
three tools is described under "What it reports" below.

## Run

```bash
cd tools/bench
pnpm install
cargo build --release --example compare_paths   # from the repo root
node bench.mjs
```

## What it reports

Two tables, each with anti-aliasing detection off and on, at threshold 0.1.

- **Engine only**: the compare of decoded RGBA buffers, in milliseconds.
  pixeldelta and pixelmatch only; odiff decodes internally and has no buffer
  entry point. Decoder speed is left out, so this is the engine on its own.
- **End to end**: two PNG files to a verdict, decode included, no diff image,
  as the wall-clock time of the process. A one-pixel run gives each tool's
  startup floor to subtract. Decode dominates at these sizes, so this measures
  the decoder as much as the engine.

Each row prints the diff pixel counts as well. pixeldelta and pixelmatch agree
exactly; odiff agrees with detection off and differs by under half a percent
with it on, from its own anti-aliasing detector. A speed comparison holds only
because the counts are this close.

The numbers depend on the machine, the core count and the build, so read them
as ratios measured together rather than as absolute figures.
