// Generates the compatibility fixtures and records the diff pixel counts
// pixelmatch reports for them.
//
// The images and the expected counts are committed. Run this only to add a
// fixture or to move to a new pixelmatch version, and review the diff of
// expected.txt when you do: a changed count is a change of the baseline.
//
//   pnpm install && pnpm generate

import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '../../crates/pixeldelta-core/tests/fixtures');
const thresholds = [0.0, 0.05, 0.1, 0.3];

/** Deterministic 32-bit linear congruential generator. */
function lcg(seed) {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

class Canvas {
  constructor(width, height, background = [255, 255, 255, 255]) {
    this.width = width;
    this.height = height;
    this.data = Buffer.alloc(width * height * 4);
    for (let i = 0; i < width * height; i++) {
      this.data.set(background, i * 4);
    }
  }

  set(x, y, rgba) {
    this.data.set(rgba, (y * this.width + x) * 4);
  }

  /**
   * Draws a shape by 4x4 supersampling, which produces the antialiased edges
   * the anti-aliasing detector is meant to recognize.
   *
   * @param {(x: number, y: number) => boolean} inside
   * @param {[number, number, number]} color
   */
  fill(inside, color) {
    const samples = 4;
    for (let y = 0; y < this.height; y++) {
      for (let x = 0; x < this.width; x++) {
        let hits = 0;
        for (let sy = 0; sy < samples; sy++) {
          for (let sx = 0; sx < samples; sx++) {
            const px = x + (sx + 0.5) / samples;
            const py = y + (sy + 0.5) / samples;
            if (inside(px, py)) hits++;
          }
        }
        if (hits === 0) continue;
        const coverage = hits / (samples * samples);
        const pos = (y * this.width + x) * 4;
        for (let c = 0; c < 3; c++) {
          this.data[pos + c] = Math.round(
            this.data[pos + c] * (1 - coverage) + color[c] * coverage,
          );
        }
      }
    }
  }

  toPng() {
    const png = new PNG({ width: this.width, height: this.height });
    this.data.copy(png.data);
    return PNG.sync.write(png);
  }
}

/** Signed distance test for a line segment of the given width. */
function segment(x1, y1, x2, y2, width) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lengthSquared = dx * dx + dy * dy;
  return (x, y) => {
    const t = Math.max(0, Math.min(1, ((x - x1) * dx + (y - y1) * dy) / lengthSquared));
    const px = x1 + t * dx - x;
    const py = y1 + t * dy - y;
    return px * px + py * py <= (width / 2) ** 2;
  };
}

function disc(cx, cy, radius) {
  return (x, y) => (x - cx) ** 2 + (y - cy) ** 2 <= radius * radius;
}

function rect(x0, y0, x1, y1) {
  return (x, y) => x >= x0 && x < x1 && y >= y0 && y < y1;
}

/** A solid field with one region nudged by a few levels of gray. */
function flat() {
  const make = (patch) => {
    const canvas = new Canvas(64, 64, [120, 120, 120, 255]);
    if (patch) canvas.fill(rect(10, 10, 30, 30), [124, 124, 124]);
    return canvas;
  };
  return [make(false), make(true)];
}

/** Antialiased strokes, shifted by a fraction of a pixel. */
function edges() {
  const make = (shift) => {
    const canvas = new Canvas(128, 96);
    canvas.fill(segment(10 + shift, 10, 118 + shift, 86, 2.0), [20, 20, 20]);
    canvas.fill(disc(40 + shift, 60, 18), [30, 90, 200]);
    canvas.fill(segment(70 + shift, 20, 70 + shift, 80, 1.0), [200, 40, 40]);
    return canvas;
  };
  return [make(0), make(0.35)];
}

/** Rectangles, one moved and one recolored. */
function blocks() {
  const make = (moved) => {
    const canvas = new Canvas(160, 120);
    canvas.fill(rect(10, 10, 60, 40), [40, 120, 80]);
    canvas.fill(rect(20 + (moved ? 3 : 0), 60, 70 + (moved ? 3 : 0), 100), [180, 60, 60]);
    canvas.fill(rect(90, 30, 140, 90), moved ? [60, 60, 200] : [60, 60, 190]);
    return canvas;
  };
  return [make(false), make(true)];
}

/** Semi-transparent content, which reaches the background blending path. */
function alpha() {
  const make = (shifted) => {
    const canvas = new Canvas(96, 96, [255, 255, 255, 255]);
    for (let y = 0; y < 96; y++) {
      for (let x = 0; x < 96; x++) {
        const a = Math.round((x / 95) * 255);
        const bump = shifted && y >= 30 && y < 60 ? 8 : 0;
        canvas.set(x, y, [230, 240, 250, Math.min(255, a + bump)]);
      }
    }
    return canvas;
  };
  return [make(false), make(true)];
}

/** Random speckle, with a fraction of the pixels perturbed. */
function noise() {
  const random = lcg(0x5eed);
  const base = new Canvas(100, 100);
  const head = new Canvas(100, 100);
  for (let i = 0; i < 100 * 100; i++) {
    const rgba = [
      Math.floor(random() * 256),
      Math.floor(random() * 256),
      Math.floor(random() * 256),
      255,
    ];
    base.data.set(rgba, i * 4);
    if (random() < 0.02) {
      head.data.set([rgba[0], Math.min(255, rgba[1] + 30), rgba[2], 255], i * 4);
    } else {
      head.data.set(rgba, i * 4);
    }
  }
  return [base, head];
}

/**
 * Pairs whose color deltas are spread densely around every threshold.
 *
 * The other fixtures differ far from the thresholds, so their counts stay put
 * even if the metric moves by a few percent. Here the perturbation magnitude
 * sweeps a range, which places pixels just above and just below each cutoff and
 * makes the recorded counts sensitive to the weights of the metric.
 */
function nearThreshold() {
  const random = lcg(0xc0ffee);
  const base = new Canvas(128, 128);
  const head = new Canvas(128, 128);
  for (let i = 0; i < 128 * 128; i++) {
    const rgb = [
      Math.floor(random() * 256),
      Math.floor(random() * 256),
      Math.floor(random() * 256),
    ];
    // A direction on the unit sphere, scaled to a magnitude that sweeps the
    // range where the thresholds sit.
    const theta = random() * Math.PI * 2;
    const z = random() * 2 - 1;
    const radius = Math.sqrt(1 - z * z);
    const magnitude = random() * 200;
    const direction = [radius * Math.cos(theta), radius * Math.sin(theta), z];
    const moved = rgb.map((c, axis) =>
      Math.max(0, Math.min(255, Math.round(c + direction[axis] * magnitude))),
    );
    base.data.set([...rgb, 255], i * 4);
    head.data.set([...moved, 255], i * 4);
  }
  return [base, head];
}

/**
 * The same idea as `nearThreshold`, with alpha varying as well.
 *
 * This is what pins the background semi-transparent pixels are composited
 * onto: the counts move if the background does.
 */
function alphaNearThreshold() {
  const random = lcg(0xa1fa);
  const base = new Canvas(128, 128);
  const head = new Canvas(128, 128);
  for (let i = 0; i < 128 * 128; i++) {
    const rgba = [
      Math.floor(random() * 256),
      Math.floor(random() * 256),
      Math.floor(random() * 256),
      Math.floor(random() * 256),
    ];
    const moved = rgba.map((c) =>
      Math.max(0, Math.min(255, Math.round(c + (random() * 2 - 1) * 60))),
    );
    base.data.set(rgba, i * 4);
    head.data.set(moved, i * 4);
  }
  return [base, head];
}

const fixtures = {
  flat,
  edges,
  blocks,
  alpha,
  noise,
  'near-threshold': nearThreshold,
  'alpha-near-threshold': alphaNearThreshold,
};
const rows = [];

for (const [name, build] of Object.entries(fixtures)) {
  const [base, head] = build();
  const dir = join(outDir, name);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'base.png'), base.toPng());
  writeFileSync(join(dir, 'head.png'), head.toPng());

  // Measure on the encoded files, so the recorded counts describe what is
  // committed rather than what was held in memory.
  const a = PNG.sync.read(readFileSync(join(dir, 'base.png')));
  const b = PNG.sync.read(readFileSync(join(dir, 'head.png')));
  for (const threshold of thresholds) {
    // `includeAA` skips the detector, so the row is labelled by whether the
    // detector runs rather than by the option name.
    for (const detectAA of [false, true]) {
      const diff = pixelmatch(a.data, b.data, null, a.width, a.height, {
        threshold,
        includeAA: !detectAA,
      });
      rows.push(`${name}\t${threshold.toFixed(2)}\t${detectAA ? 'on' : 'off'}\t${diff}`);
    }
  }
}

const version = JSON.parse(
  readFileSync(join(here, 'node_modules/pixelmatch/package.json'), 'utf8'),
).version;

writeFileSync(
  join(outDir, 'expected.txt'),
  [
    `# Diff pixel counts reported by pixelmatch ${version}.`,
    '# Generated by tools/fixtures/generate.mjs.',
    '# The aa column says whether anti-aliasing detection ran.',
    '# fixture\tthreshold\taa\tdiff_pixels',
    ...rows,
    '',
  ].join('\n'),
);

console.log(`wrote ${rows.length} rows for ${Object.keys(fixtures).length} fixtures`);
