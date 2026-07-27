// Checks the layout that lets a bundler reach the WebAssembly build from a
// browser target: the root package's `browser` field, the entry it names, and
// what the platform package behind it has to carry for that entry to run.
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
const wasiManifest = require('../npm/wasm32-wasi/package.json');
const wasiPackage = `${manifest.napi.packageName}-wasm32-wasi`;

test('the root package sends a browser target to the WebAssembly entry', () => {
  assert.equal(manifest.browser, 'browser.js');
  assert.ok(
    manifest.files.includes(manifest.browser),
    `${manifest.browser} is the browser field but is not in files, so it would not be published`,
  );
});

test('the browser entry re-exports the WebAssembly platform package', () => {
  const source = readFileSync(join(pkgDir, 'browser.js'), 'utf8');
  assert.match(source, new RegExp(`from '${wasiPackage}'`));
});

test('the platform package sends a browser target to its own entry', () => {
  assert.equal(wasiManifest.name, wasiPackage);
  assert.equal(wasiManifest.browser, `${manifest.napi.binaryName}.wasi-browser.js`);
  assert.ok(wasiManifest.files.includes(wasiManifest.browser));
});

test('the platform package ships every file the browser entry loads', () => {
  const entry = readFileSync(join(pkgDir, wasiManifest.browser), 'utf8');
  // The copy read here is the one `napi artifacts` places in the platform
  // package during a release, after rewriting the worker URL from a relative
  // path to a specifier of that package. Both forms end in the same file name,
  // which is what the package lists.
  const loaded = [...entry.matchAll(/new URL\('([^']+)'/g)].map((match) => basename(match[1]));
  assert.deepEqual(loaded.sort(), ['pixeldelta.wasm32-wasi.wasm', 'wasi-worker-browser.mjs']);
  for (const file of loaded) {
    assert.ok(wasiManifest.files.includes(file), `${file} is loaded by the entry but is not in files`);
  }
});

test('the platform package declares what the browser entry imports', () => {
  const entry = readFileSync(join(pkgDir, wasiManifest.browser), 'utf8');
  const imported = [...entry.matchAll(/from '([^'.][^']*)'/g)].map((match) => match[1]);
  assert.deepEqual([...new Set(imported)], ['@napi-rs/wasm-runtime']);
  for (const dependency of imported) {
    assert.ok(
      wasiManifest.dependencies?.[dependency],
      `${dependency} is imported by the entry but is not a dependency of ${wasiPackage}`,
    );
  }
});
