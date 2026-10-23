// Checks the layout that lets a bundler reach the WebAssembly build from a
// browser target: the root package's `browser` field, the entry it names, and
// the files pixeldelta-wasm has to carry for that entry to run. The rest of
// that package is checked in wasm-package.test.mjs.
//
// It reads the manifests instead of loading the entry. Loading it needs a
// bundler to resolve the bare import inside it, and a cross-origin-isolated
// page for the shared memory it allocates, so neither Node nor this test
// runner can stand in for that.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = join(here, '..');

const manifest = require('../package.json');
const wasm = require('../npm/wasm/package.json');

test('the root package sends a browser target to the WebAssembly entry', () => {
  assert.equal(manifest.browser, 'browser-entry.js');
  assert.ok(
    manifest.files.includes(manifest.browser),
    `${manifest.browser} is the browser field but is not in files, so it would not be published`,
  );
});

test('the browser entry re-exports the WebAssembly package', () => {
  const source = readFileSync(join(pkgDir, manifest.browser), 'utf8');
  assert.match(source, new RegExp(`from '${wasm.name}'`));
});

test('the WebAssembly package ships every file its browser entry loads', () => {
  const entry = readFileSync(join(pkgDir, wasm.browser), 'utf8');
  const loaded = [...entry.matchAll(/new URL\('([^']+)'/g)].map((match) => basename(match[1]));
  assert.deepEqual(loaded.sort(), ['pixeldelta.wasm32-wasi.wasm', 'wasi-worker-browser.mjs']);
  for (const file of loaded) {
    assert.ok(wasm.files.includes(file), `${file} is loaded by the entry but is not in files`);
  }
});
