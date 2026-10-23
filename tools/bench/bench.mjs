// Compares pixeldelta against pixelmatch and odiff on the committed benchmark
// fixtures, on one machine.
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
// tools disagree on the default. The measurements themselves live in
// measure.mjs; this file only formats them.

import { engineRows, endToEndRows, startupFloor } from './measure.mjs';

function printEngineTable(rows) {
  console.log('\nEngine only: compare of decoded buffers, median ms');
  console.log('size\taa\tpixeldelta\tpixelmatch\tpd_diff\tpm_diff');
  for (const row of rows) {
    console.log(
      `${row.size}\t${row.aa}\t${row.pixeldelta.toFixed(3)}\t\t${row.pixelmatch.toFixed(3)}\t\t${row.pixeldeltaDiff}\t${row.pixelmatchDiff}`,
    );
  }
}

function printEndToEndTable(floor, rows) {
  console.log('\nEnd to end: two files to a verdict, decode included, median ms');
  console.log(`startup floor (1px): pixeldelta ${floor.pd.toFixed(1)}  pixelmatch ${floor.pm.toFixed(1)}  odiff ${floor.od.toFixed(1)}`);
  console.log('size\taa\tpixeldelta\tpixelmatch\todiff\tod_diff');
  for (const row of rows) {
    console.log(
      `${row.size}\t${row.aa}\t${row.pixeldelta.toFixed(1)}\t\t${row.pixelmatch.toFixed(1)}\t\t${row.odiff.toFixed(1)}\t${row.odiffDiff}`,
    );
  }
}

printEngineTable(engineRows());
printEndToEndTable(startupFloor(), endToEndRows());
