// Reaches the executable the way a project's scripts do: `pnpm run` puts
// node_modules/.bin on PATH, and a script that names `pixeldelta` finds the
// launcher the root package's bin entry was linked to.
//
// Run from a project that installed the packed tarballs with a package
// manager, so the link under test is the one the install created.

import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

const script = (name) => spawnSync('pnpm', ['run', name], { cwd: here, encoding: 'utf8' });

test('a package script reaches the executable', () => {
  const result = script('version');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /pixeldelta \d+\.\d+\.\d+/);
});

test('a package script sees the exit code of a comparison', () => {
  const same = script('compare-same');
  assert.equal(same.status, 0, same.stderr);
  assert.match(same.stdout, /match/);

  // A difference exits 1, which is what a CI step reads. A launcher that
  // dropped the status would let this pass with 0.
  const differ = script('compare-differ');
  assert.equal(differ.status, 1);
  assert.match(differ.stdout, /differ: 240 pixels/);
});

test('the executable came out of the tarball runnable', () => {
  // npm records the file mode, so an executable bit lost while assembling the
  // platform package turns into EACCES on the consumer's first run.
  const { hostBinary } = require('pixeldelta/cli.js');
  const path = hostBinary();
  assert.ok(path, 'the platform package carries an executable');
  assert.ok(path.includes(join('node_modules', 'pixeldelta-')), `${path} is the installed one`);
  assert.ok(statSync(path).mode & 0o111, `${path} is executable`);
});

