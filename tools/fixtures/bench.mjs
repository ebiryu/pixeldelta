// Generates the benchmark fixtures at 2, 8 and 18 Mpixel.
//
// The images are committed, so a change here moves the baseline every past
// measurement was taken against. Run it only to add a size or to change what
// the content is meant to represent, and say so in the commit message.
//
//   pnpm install && pnpm bench-fixtures

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '../../crates/pixeldelta-core/benches/fixtures');

// 1080p, 4K and 6K. The pixel counts are what the benchmark reports
// throughput against.
const sizes = [
  { name: '2mpx', width: 1920, height: 1080 },
  { name: '8mpx', width: 3840, height: 2160 },
  { name: '18mpx', width: 5760, height: 3240 },
];

// The tile the page content is built from. Tiling keeps the committed PNGs
// small; the comparison works on decoded buffers, so how well the file
// compresses does not reach the measurement.
const TILE = 240;

/** Deterministic 32-bit linear congruential generator. */
function lcg(seed) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

/**
 * One tile of screenshot-like content: flat panels, antialiased strokes and a
 * band of text-sized glyphs.
 *
 * `shift` moves the strokes and glyphs by a fraction of a pixel, which is what
 * a font renderer of a different version does and what the anti-aliasing
 * detector is meant to absorb.
 */
function tile(shift) {
  const data = Buffer.alloc(TILE * TILE * 4);
  const random = lcg(0x9e3779b9);
  const glyphs = [];
  for (let i = 0; i < 96; i++) {
    glyphs.push({
      x: 12 + random() * 200,
      y: 150 + Math.floor(random() * 5) * 16,
      width: 2 + random() * 8,
      height: 6 + random() * 4,
    });
  }

  for (let y = 0; y < TILE; y++) {
    for (let x = 0; x < TILE; x++) {
      // Panels: a header, a sidebar and a body, each a flat fill.
      let color = [250, 250, 252];
      if (y < 40) color = [32, 36, 44];
      else if (x < 64) color = [240, 241, 245];

      // A diagonal stroke crossing the body, antialiased by its distance.
      const sx = x + shift;
      const distance = Math.abs(sx - y) / Math.SQRT2;
      if (y >= 40 && distance < 2.5) {
        const coverage = Math.min(1, Math.max(0, 2.0 - distance));
        color = blend(color, [70, 130, 220], coverage);
      }

      // Glyph-sized boxes with antialiased edges, standing in for text.
      for (const glyph of glyphs) {
        const gx = x - (glyph.x + shift);
        const gy = y - glyph.y;
        if (gx < -1 || gy < -1 || gx > glyph.width || gy > glyph.height) continue;
        const coverage =
          Math.min(1, Math.max(0, gx + 1)) *
          Math.min(1, Math.max(0, glyph.width - gx)) *
          Math.min(1, Math.max(0, gy + 1)) *
          Math.min(1, Math.max(0, glyph.height - gy));
        color = blend(color, [40, 44, 52], coverage);
      }

      data.set([...color.map(Math.round), 255], (y * TILE + x) * 4);
    }
  }
  return data;
}

function blend(under, over, coverage) {
  return under.map((c, i) => c * (1 - coverage) + over[i] * coverage);
}

/** Fills a `width` x `height` buffer by repeating `source`. */
function spread(source, width, height) {
  const data = Buffer.alloc(width * height * 4);
  for (let y = 0; y < height; y++) {
    const row = (y % TILE) * TILE * 4;
    for (let x = 0; x < width; x += TILE) {
      const span = Math.min(TILE, width - x) * 4;
      source.copy(data, (y * width + x) * 4, row, row + span);
    }
  }
  return data;
}

/**
 * Recolors three rectangles, which is the difference a benchmark run has to
 * find. They cover about 2% of the image, close to what a real regression of
 * a single component covers.
 */
function repaint(data, width, height) {
  const patches = [
    { x: 0.05, y: 0.1, w: 0.12, h: 0.35, color: [200, 70, 60] },
    { x: 0.4, y: 0.55, w: 0.2, h: 0.06, color: [60, 160, 110] },
    { x: 0.75, y: 0.2, w: 0.08, h: 0.5, color: [120, 90, 200] },
  ];
  for (const patch of patches) {
    const x0 = Math.round(patch.x * width);
    const y0 = Math.round(patch.y * height);
    const x1 = x0 + Math.round(patch.w * width);
    const y1 = y0 + Math.round(patch.h * height);
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        data.set(patch.color, (y * width + x) * 4);
      }
    }
  }
}

function write(path, width, height, data) {
  const png = new PNG({ width, height });
  data.copy(png.data);
  // Paeth filtering costs nothing to decode and takes a quarter off the
  // committed files, which at 18 Mpixel is measured in megabytes.
  writeFileSync(path, PNG.sync.write(png, { deflateLevel: 9, filterType: 4 }));
}

const base = tile(0);
const head = tile(0.35);

for (const size of sizes) {
  const dir = join(outDir, size.name);
  mkdirSync(dir, { recursive: true });

  const baseData = spread(base, size.width, size.height);
  const headData = spread(head, size.width, size.height);
  repaint(headData, size.width, size.height);

  write(join(dir, 'base.png'), size.width, size.height, baseData);
  write(join(dir, 'head.png'), size.width, size.height, headData);
  console.log(`wrote ${size.name} (${size.width}x${size.height})`);
}
