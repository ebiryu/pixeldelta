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
