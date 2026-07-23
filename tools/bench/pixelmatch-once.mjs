// Decodes two PNG files and runs pixelmatch once, for the end-to-end row of
// the benchmark. The orchestrator times this whole process.
//
//   node pixelmatch-once.mjs <base.png> <head.png> <threshold> <on|off>
//
// The last argument says whether anti-aliasing detection runs; pixelmatch
// spells that as the inverse `includeAA`.

import { readFileSync } from 'node:fs';
import { PNG } from 'pngjs';
import pixelmatch from 'pixelmatch';

const [base, head, threshold, aa] = process.argv.slice(2);
const a = PNG.sync.read(readFileSync(base));
const b = PNG.sync.read(readFileSync(head));

const diff = pixelmatch(a.data, b.data, null, a.width, a.height, {
  threshold: Number(threshold),
  includeAA: aa !== 'on',
});
process.stdout.write(String(diff));
