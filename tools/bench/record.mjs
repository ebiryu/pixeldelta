// Runs the criterion benchmarks and the tools/bench comparison, then inserts
// one Markdown entry at the top of BENCHMARKS.md.
//
//   node record.mjs
//
// It runs `cargo bench` and the release build of `compare_paths` itself, so
// the criterion results it reads are the ones it just produced rather than
// whatever was left in target/criterion from an earlier run.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { engineRows, endToEndRows, startupFloor, ENGINE_RUNS, PROCESS_RUNS, SIZES, THRESHOLD } from './measure.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '../..');
const criterionDir = join(root, 'target/criterion/compare');
const benchmarksFile = join(root, 'BENCHMARKS.md');

const CASES = ['identical', 'aa-off', 'aa-on', 'layout-shift'];

/** Runs a command from `root`, inheriting stdio so progress is visible. */
function runInherit(command, args) {
  execFileSync(command, args, { cwd: root, stdio: 'inherit' });
}

/** Trimmed stdout of a command, run from `root`. */
function capture(command, args) {
  return execFileSync(command, args, { cwd: root, encoding: 'utf8' }).trim();
}

/** Reads one criterion result, as milliseconds and the sample count. */
function criterionResult(caseName, size) {
  const dir = join(criterionDir, caseName, size, 'new');
  const estimatesPath = join(dir, 'estimates.json');
  const samplePath = join(dir, 'sample.json');
  if (!existsSync(estimatesPath) || !existsSync(samplePath)) {
    throw new Error(
      `missing criterion output at ${dir}; run \`cargo bench --bench compare -p pixeldelta-core\` first`,
    );
  }
  const estimates = JSON.parse(readFileSync(estimatesPath, 'utf8'));
  const sample = JSON.parse(readFileSync(samplePath, 'utf8'));
  return {
    ms: estimates.median.point_estimate / 1e6,
    samples: sample.times.length,
  };
}

/** One row per size, with a column per case, plus the sample count. */
function criterionRows() {
  const rows = [];
  const sampleCounts = new Set();
  for (const size of SIZES) {
    const row = { size };
    for (const caseName of CASES) {
      const result = criterionResult(caseName, size);
      row[caseName] = result.ms;
      sampleCounts.add(result.samples);
    }
    rows.push(row);
  }
  // One entry states one sample count over the whole table, so the results it
  // is taken from have to agree on it.
  if (sampleCounts.size !== 1) {
    throw new Error(
      `the criterion results hold different sample counts (${[...sampleCounts].join(', ')})`,
    );
  }
  return { rows, samples: [...sampleCounts][0] };
}

/** git commit and dirty state, as `<sha>` or `<sha> plus uncommitted changes`. */
function commitCondition() {
  const sha = capture('git', ['rev-parse', '--short', 'HEAD']);
  const dirty = capture('git', ['status', '--porcelain']).length > 0;
  return dirty ? `${sha} plus uncommitted changes` : sha;
}

/** The name each `os.type()` is known by, where they differ. */
const OS_NAMES = { Darwin: 'macOS', Windows_NT: 'Windows' };

/**
 * CPU model, core count and OS, as one machine description.
 *
 * The OS carries no version. This line is what tells entries taken on
 * different machines apart, and a version would move it on every update of
 * the same machine.
 */
function machineCondition() {
  const cpus = os.cpus();
  const osName = OS_NAMES[os.type()] ?? os.type();
  return `${cpus[0].model}, ${cpus.length} cores, ${osName} (${os.arch()})`;
}

/** A Markdown table: a header row, an alignment row, then one row per data row. */
function table(columns, rows) {
  const header = `| ${columns.map((c) => c.label).join(' | ')} |`;
  const align = `| ${columns.map((c) => (c.numeric ? '---:' : '---')).join(' | ')} |`;
  const body = rows.map((row) => `| ${columns.map((c) => c.value(row)).join(' | ')} |`);
  return [header, align, ...body].join('\n');
}

function buildEntry() {
  console.log('Running cargo bench --bench compare -p pixeldelta-core ...');
  runInherit('cargo', ['bench', '--bench', 'compare', '-p', 'pixeldelta-core']);

  console.log('Running cargo build --release --example compare_paths -p pixeldelta-core ...');
  runInherit('cargo', ['build', '--release', '--example', 'compare_paths', '-p', 'pixeldelta-core']);

  console.log('Running the tools/bench comparison ...');
  const { rows: criterionData, samples: criterionSamples } = criterionRows();
  const engine = engineRows();
  const endToEnd = endToEndRows();
  const floor = startupFloor();

  const version = capture('node', [join(root, 'tools/release/version.mjs')]);
  const commit = commitCondition();
  const date = new Date().toISOString().slice(0, 10);
  const machine = machineCondition();
  const toolchain = `${capture('rustc', ['--version'])}, release build`;

  const criterionTable = table(
    [
      { label: 'size', value: (r) => r.size },
      ...CASES.map((caseName) => ({
        label: caseName,
        numeric: true,
        value: (r) => r[caseName].toFixed(3),
      })),
    ],
    criterionData,
  );

  const engineTable = table(
    [
      { label: 'size', value: (r) => r.size },
      { label: 'aa', value: (r) => r.aa },
      { label: 'pixeldelta', numeric: true, value: (r) => r.pixeldelta.toFixed(3) },
      { label: 'pixelmatch', numeric: true, value: (r) => r.pixelmatch.toFixed(3) },
      { label: 'pixeldelta diff', numeric: true, value: (r) => String(r.pixeldeltaDiff) },
      { label: 'pixelmatch diff', numeric: true, value: (r) => String(r.pixelmatchDiff) },
    ],
    engine,
  );

  const endToEndTable = table(
    [
      { label: 'size', value: (r) => r.size },
      { label: 'aa', value: (r) => r.aa },
      { label: 'pixeldelta', numeric: true, value: (r) => r.pixeldelta.toFixed(1) },
      { label: 'pixelmatch', numeric: true, value: (r) => r.pixelmatch.toFixed(1) },
      { label: 'odiff', numeric: true, value: (r) => r.odiff.toFixed(1) },
      { label: 'odiff diff', numeric: true, value: (r) => String(r.odiffDiff) },
    ],
    endToEnd,
  );

  return `## ${version} — ${date}

- commit: ${commit}
- machine: ${machine}
- toolchain: ${toolchain}
- fixtures: crates/pixeldelta-core/benches/fixtures, threshold ${THRESHOLD}

Engine, decoded buffers, median of ${criterionSamples} criterion samples, in ms:

${criterionTable}

Against pixelmatch, decoded buffers, median of ${ENGINE_RUNS} runs, in ms:

${engineTable}

End to end, two files to a verdict, decode included, no diff image, median of ${PROCESS_RUNS} process runs, in ms:

${endToEndTable}

Startup floor on a 1-pixel pair, in ms: pixeldelta ${floor.pd.toFixed(1)}, pixelmatch ${floor.pm.toFixed(1)}, odiff ${floor.od.toFixed(1)}.
`;
}

/** Inserts `entry` immediately before the first `## ` line, or at the end. */
function insertEntry(entry) {
  if (!existsSync(benchmarksFile)) {
    throw new Error(`${benchmarksFile} does not exist; create it with its header first`);
  }
  const text = readFileSync(benchmarksFile, 'utf8');
  const lines = text.split('\n');
  const firstEntry = lines.findIndex((line) => line.startsWith('## '));
  if (firstEntry === -1) {
    const separator = text.endsWith('\n') ? '' : '\n';
    writeFileSync(benchmarksFile, `${text}${separator}\n${entry}`);
    return;
  }
  const before = lines.slice(0, firstEntry).join('\n');
  const after = lines.slice(firstEntry).join('\n');
  writeFileSync(benchmarksFile, `${before}\n${entry}\n${after}`);
}

const entry = buildEntry();
insertEntry(entry);
console.log(`Recorded a new entry at the top of ${benchmarksFile.slice(root.length + 1)}.`);
