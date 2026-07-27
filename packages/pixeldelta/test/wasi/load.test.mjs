// Checks that a run forced onto the WebAssembly (WASI) fallback really loaded
// it. Without this the fallback could be absent or broken and the comparison
// tests beside it would still pass against the native addon.
//
// It sits in a subdirectory so the `test` script's glob leaves it out: it only
// holds on a run that sets NAPI_RS_FORCE_WASI, which `pnpm run test:wasi` does.

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

test('the run is forced onto the WASI binding', () => {
  assert.equal(
    process.env.NAPI_RS_FORCE_WASI,
    'error',
    'run this through `pnpm run test:wasi`, which sets NAPI_RS_FORCE_WASI',
  );
});

test('the package exports the WASI binding, not the native addon', () => {
  // index.js loads the native addon first and replaces it when the flag is
  // set, so the question is which one came out, not which one was opened.
  const pkg = require('../../index.js');
  const wasi = require('../../pixeldelta.wasi.cjs');
  assert.equal(pkg.compare, wasi.compare);
  assert.equal(pkg.compareSync, wasi.compareSync);
});
