// Checks pixeldelta-wasm32-wasi, the package napi-rs assembles for the
// WebAssembly target, and the loaders it is assembled from.
//
// `napi artifacts` copies the loaders committed beside this package into
// npm/wasm32-wasi, and `napi prepublish` refuses to publish that package unless
// its manifest names them. Both run in the release job alone, so a missing file
// or a stale entry would otherwise surface only once a tag is pushed.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = join(here, '..');

const manifest = require('../package.json');
const wasi = require('../npm/wasm32-wasi/package.json');
const binaryName = manifest.napi.binaryName;

const read = (file) => readFileSync(join(pkgDir, file), 'utf8');

test('the committed loaders carry the type declaration napi copies', () => {
  // `napi build` writes this beside index.d.ts from the same type definitions,
  // so the two hold the same text. Without it `napi artifacts` stops before it
  // moves anything, and takes the loaders beside this package with it.
  assert.equal(read(`${binaryName}.wasi.d.cts`), read('index.d.ts'));
});

test('the package entries name what the loaders are', () => {
  assert.equal(wasi.name, `${manifest.napi.packageName}-wasm32-wasi`);
  assert.equal(wasi.type, 'module');
  assert.equal(wasi.main, `${binaryName}.wasi.cjs`);
  assert.equal(wasi.types, `${binaryName}.wasi.d.cts`);
  assert.equal(wasi.browser, `${binaryName}.wasi-browser.js`);
});

test('the package declares no platform fields', () => {
  // WebAssembly runs on any host, and `napi prepublish` fails on a WASI package
  // that narrows itself with one of these.
  assert.equal(wasi.cpu, undefined);
  assert.equal(wasi.os, undefined);
  assert.equal(wasi.libc, undefined);
});

test('the package ships every file napi assembles into it', () => {
  const required = [
    `${binaryName}.wasm32-wasi.wasm`,
    `${binaryName}.wasi.cjs`,
    `${binaryName}.wasi.d.cts`,
    `${binaryName}.wasi-browser.js`,
    'wasi-worker.mjs',
    'wasi-worker-browser.mjs',
  ];
  for (const file of required) {
    assert.ok(wasi.files.includes(file), `${file} is assembled into the package but is not in files`);
  }
});
