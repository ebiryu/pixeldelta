# pixeldelta

Image comparison for Node. Decodes two PNGs and reports how many pixels differ
perceptually, using the same YIQ color metric and anti-aliasing detector as
pixelmatch. Beyond a pixel count, it can group the differences into clusters and
estimate how far each moved, so a caller can tell one shifted element from a
change spread across the screen.

The comparison engine is written in Rust. Each platform gets a prebuilt binary
through an optional dependency; the install downloads no source and runs no
build. Environments without a matching prebuild fall back to a WebAssembly
(WASI) build.

## Install

```sh
npm install pixeldelta
```

## Usage

```js
import { compare } from 'pixeldelta';

const result = await compare('base.png', 'head.png', {
  threshold: 0.1,
  antialiasing: true,
  layoutShift: true,
});

console.log(result.verdict); // 'match' | 'differ' | 'sizeMismatch'
console.log(result.diffPixels, result.diffRatio);
for (const c of result.clusters) {
  console.log(c.x, c.y, c.width, c.height, c.displacement, c.ssim);
}
```

Both arguments accept a file path or a `Buffer` of PNG bytes. `compare` runs the
decode and comparison off the event loop; `compareSync` runs them on the calling
thread.

### Options

| Option | Default | Meaning |
| --- | --- | --- |
| `threshold` | `0.1` | Matching threshold in `[0, 1]`; smaller is more sensitive. |
| `antialiasing` | `true` | Exclude pixels that differ only by anti-aliasing. |
| `ignoreRegions` | `[]` | Rectangles left out of the comparison and its ratio. |
| `failFast` | none | `{ maxDiffPixels }`: stop once more than this many pixels differ. |
| `cluster` | `false` | Group differing pixels into `clusters`. |
| `layoutShift` | `false` | Also search each cluster for the offset it moved by. |

When `failFast` stops the scan, `stoppedEarly` is `true` and `diffPixels` is a
lower bound.

## Command line

The package installs a `pixeldelta` command, so a script in `package.json`
reaches it without a global install:

```json
{
  "scripts": {
    "visual-diff": "pixeldelta run ./expected ./actual --report ./report"
  }
}
```

```sh
npm run visual-diff
pnpm run visual-diff
npx pixeldelta compare base.png head.png --output diff.png
```

`compare` takes two images; `run` takes two directories and writes an HTML,
JSON or JUnit report. The exit code carries the verdict, so a CI step fails on
a difference without reading the output.

| Code | `compare` | `run` |
| --- | --- | --- |
| `0` | the images match | every file matched |
| `1` | the images differ | a file differed, was added, was removed, or changed size |
| `2` | the sizes differ | not used |
| `3` | a file could not be read | a file could not be read |

The command comes with the platform-specific prebuild. The WebAssembly fallback
does not carry it, because it runs `git` and opens network connections and WASI
has no sockets. On a platform with no prebuild the library still works through
that fallback, and the command reports which platforms ship it.

## The WebAssembly fallback

A host that no prebuild matches loads `pixeldelta-wasm32-wasi` instead. The API
is the same and so are the results: every comparison timed below returns the
same diff pixel count either way. Three things behave differently.

**Threads.** The native build sizes its thread pool from the core count. That
count is not available under WASI, so the pool holds one thread. Setting
`RAYON_NUM_THREADS` gives it back:

| Pixels | Native | WASI | WASI, `RAYON_NUM_THREADS=8` |
| --- | --- | --- | --- |
| 2 Mpixel | 25 ms | 41 ms | 33 ms |
| 8 Mpixel | 100 ms | 185 ms | 131 ms |
| 18 Mpixel | 221 ms | 353 ms | 287 ms |

Held to one thread the native build takes 33, 130 and 269 ms, which is where the
WASI column without the variable sits.

Concurrent calls run on a pool of four workers rather than libuv's threads.
Four 8-Mpixel comparisons started at once take 134 ms natively and 402 ms
through WASI, on a single run of each.

**Files.** The module reads through a WASI preopen of the root of the working
directory, so a path outside that root — on Windows, a path on another drive —
does not resolve. Paths cost no more than buffers either way: 354 ms against
348 ms at 18 Mpixel.

**Memory.** Everything lives in one `WebAssembly.Memory` capped at 4 GiB, which
both decoded images and the encoded input share; the native build is bounded by
the host's memory alone. A 20000×20000 pair (400 Mpixel) still completes.

Measured on an 8-core Apple M1 with Node 24.14.0, at threshold 0.1 with
anti-aliasing detection on, as the median of five comparisons of two file paths
to a verdict, decode included.
