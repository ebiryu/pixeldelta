// Compares pixeldelta against pixelmatch and odiff on the committed benchmark
// fixtures, on one machine. See docs/design.md section 9.1 for what is and is
// not held equal.
//
//   pnpm install
//   cargo build --release --example compare_paths   # from the repo root
//   node bench.mjs
//
// Two views, each a table:
//
// - engine only: the compare of decoded RGBA buffers, for pixeldelta and
//   pixelmatch. odiff has no buffer entry point and is left out.
// - end to end: two PNG files to a verdict, decode included, no diff image,
//   as the wall-clock time of the process. A one-pixel run gives the startup
//   floor to subtract.
//
// Both are measured with anti-aliasing detection off and on, since the three
// tools disagree on the default.

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';
import { findBinary } from 'odiff-bin/binary.js';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '../..');
const fixtures = join(root, 'crates/pixeldelta-core/benches/fixtures');
const pixeldelta = join(root, 'target/release/examples/compare_paths');
const odiff = findBinary();

const SIZES = ['2mpx', '8mpx', '18mpx'];
const THRESHOLD = 0.1;
// Process runs for an end-to-end number, and in-process repeats for an
// engine-only one. The engine loop is longer because each pass is cheaper.
const PROCESS_RUNS = 10;
const ENGINE_RUNS = 50;

/** Median of an array of numbers. */
function median(values) {
  const sorted = [...values].sort((x, y) => x - y);
  return sorted[Math.floor(sorted.length / 2)];
}

/** Median wall-clock milliseconds of running `fn` `runs` times. */
function timeProcess(fn, runs) {
  const samples = [];
  for (let i = 0; i < runs; i++) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  return median(samples);
}

/** Reads a fixture pair as decoded RGBA buffers. */
function decode(size) {
  const dir = join(fixtures, size);
  const a = PNG.sync.read(readFileSync(join(dir, 'base.png')));
  const b = PNG.sync.read(readFileSync(join(dir, 'head.png')));
  return { a, b };
}

/** Engine-only median milliseconds for pixelmatch on decoded buffers. */
function pixelmatchEngine({ a, b }, aa) {
  const samples = [];
  for (let i = 0; i < ENGINE_RUNS; i++) {
    const start = performance.now();
    const diff = pixelmatch(a.data, b.data, null, a.width, a.height, {
      threshold: THRESHOLD,
      includeAA: aa !== 'on',
    });
    samples.push(performance.now() - start);
    if (i === 0) pixelmatchEngine.diff = diff;
  }
  return median(samples);
}

/** Engine-only median milliseconds for pixeldelta, which times itself. */
function pixeldeltaEngine(size, aa) {
  const dir = join(fixtures, size);
  const out = run(pixeldelta, [
    join(dir, 'base.png'),
    join(dir, 'head.png'),
    '--threshold',
    String(THRESHOLD),
    ...(aa === 'on' ? [] : ['--no-aa']),
    '--warm',
    String(ENGINE_RUNS),
  ]);
  return Number(out);
}

/** Runs a binary and returns its trimmed stdout. */
function run(bin, args) {
  return execFileSync(bin, args, { encoding: 'utf8' }).trim();
}

/** Runs odiff on two paths without writing a diff, returning the diff count. */
function runOdiff(base, head, aa) {
  const args = ['--parsable-stdout', `--threshold=${THRESHOLD}`];
  if (aa === 'on') args.push('--antialiasing');
  args.push(base, head);
  let stdout = '';
  try {
    stdout = execFileSync(odiff, args, { encoding: 'utf8' });
  } catch (error) {
    // odiff exits 22 when it finds differences, which is not a failure here.
    if (error.status !== 22) throw error;
    stdout = error.stdout;
  }
  return Number(stdout.split(';')[0]);
}

/** A one-pixel PNG pair, to measure each tool's startup floor. */
function onePixelPair() {
  const dir = mkdtempSync(join(tmpdir(), 'pixeldelta-floor-'));
  const make = (rgba) => {
    const png = new PNG({ width: 1, height: 1 });
    png.data.set(rgba, 0);
    const path = join(dir, `${rgba.join('-')}.png`);
    writeFileSync(path, PNG.sync.write(png));
    return path;
  };
  return { base: make([0, 0, 0, 255]), head: make([255, 255, 255, 255]) };
}

function engineTable() {
  console.log('\nEngine only: compare of decoded buffers, median ms');
  console.log('size\taa\tpixeldelta\tpixelmatch\tpd_diff\tpm_diff');
  for (const size of SIZES) {
    const buffers = decode(size);
    for (const aa of ['off', 'on']) {
      const pd = pixeldeltaEngine(size, aa);
      const pm = pixelmatchEngine(buffers, aa);
      const pdDiff = pixeldeltaDiff(size, aa);
      console.log(
        `${size}\t${aa}\t${pd.toFixed(3)}\t\t${pm.toFixed(3)}\t\t${pdDiff}\t${pixelmatchEngine.diff}`,
      );
    }
  }
}

/** pixeldelta diff count from a single end-to-end run. */
function pixeldeltaDiff(size, aa) {
  const dir = join(fixtures, size);
  return Number(
    run(pixeldelta, [
      join(dir, 'base.png'),
      join(dir, 'head.png'),
      '--threshold',
      String(THRESHOLD),
      ...(aa === 'on' ? [] : ['--no-aa']),
    ]),
  );
}

function endToEndTable(floor) {
  console.log('\nEnd to end: two files to a verdict, decode included, median ms');
  console.log(`startup floor (1px): pixeldelta ${floor.pd.toFixed(1)}  pixelmatch ${floor.pm.toFixed(1)}  odiff ${floor.od.toFixed(1)}`);
  console.log('size\taa\tpixeldelta\tpixelmatch\todiff\tod_diff');
  for (const size of SIZES) {
    const dir = join(fixtures, size);
    const base = join(dir, 'base.png');
    const head = join(dir, 'head.png');
    for (const aa of ['off', 'on']) {
      const pdArgs = [base, head, '--threshold', String(THRESHOLD), ...(aa === 'on' ? [] : ['--no-aa'])];
      const pmArgs = [join(here, 'pixelmatch-once.mjs'), base, head, String(THRESHOLD), aa];
      const pd = timeProcess(() => run(pixeldelta, pdArgs), PROCESS_RUNS);
      const pm = timeProcess(() => run(process.execPath, pmArgs), PROCESS_RUNS);
      let odDiff = 0;
      const od = timeProcess(() => { odDiff = runOdiff(base, head, aa); }, PROCESS_RUNS);
      console.log(
        `${size}\t${aa}\t${pd.toFixed(1)}\t\t${pm.toFixed(1)}\t\t${od.toFixed(1)}\t${odDiff}`,
      );
    }
  }
}

function startupFloor() {
  const { base, head } = onePixelPair();
  const pd = timeProcess(
    () => run(pixeldelta, [base, head, '--threshold', String(THRESHOLD)]),
    PROCESS_RUNS,
  );
  const pm = timeProcess(
    () => run(process.execPath, [join(here, 'pixelmatch-once.mjs'), base, head, String(THRESHOLD), 'on']),
    PROCESS_RUNS,
  );
  const od = timeProcess(() => runOdiff(base, head, 'off'), PROCESS_RUNS);
  return { pd, pm, od };
}

engineTable();
endToEndTable(startupFloor());
