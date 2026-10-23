// Checks that a run pointed at the WebAssembly (WASI) build really loaded it.
// Without this the build could be absent or broken and the comparison tests
// beside it would still pass against the native addon.
//
// It sits in a subdirectory so the `test` script's glob leaves it out: it only
// holds on a run that sets PIXELDELTA_TEST_WASI, which `pnpm run test:wasi`
// does.

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

test('the run is pointed at the WASI build', () => {
  assert.equal(
    process.env.PIXELDELTA_TEST_WASI,
    '1',
    'run this through `pnpm run test:wasi`, which sets PIXELDELTA_TEST_WASI',
  );
});

test('the tests run against the WASI binding, not the native addon', async () => {
  // test/binding.mjs is what the comparison tests import, so this asks the
  // question they answer with: which build did their functions come from.
  const { default: binding } = await import('../binding.mjs');
  const wasi = require('../../pixeldelta.wasi.cjs');
  assert.equal(binding.compare, wasi.compare);
  assert.equal(binding.compareSync, wasi.compareSync);
});
