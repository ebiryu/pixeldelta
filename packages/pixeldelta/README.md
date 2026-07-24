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
