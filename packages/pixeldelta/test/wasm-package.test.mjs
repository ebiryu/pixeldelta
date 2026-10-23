// Checks pixeldelta-wasm, the package a host with no prebuild installs.
//
// It carries the same WebAssembly build as pixeldelta-wasm32-wasi, under a name
// the consumer installs rather than one napi-rs manages. What the loaders inside
// it read has to be listed in `files`, or the package installs and then throws
// when something requires it.
//
// scripts/place-wasm.mjs copies those loaders in from beside this package and
// checks that each listed file arrived. The checks here are the other half:
// that the list itself names what the loaders reach for.

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
const wasm = require('../npm/wasm/package.json');

/** The loader as committed here, which is the copy the package publishes. */
const source = (file) => readFileSync(join(pkgDir, file), 'utf8');

test('the package carries no platform fields', () => {
  assert.equal(wasm.name, 'pixeldelta-wasm');
  // Declaring nothing is what makes this one installable everywhere, and is the
  // reason it exists: nothing else in the tree reaches a host with no prebuild.
  assert.equal(wasm.cpu, undefined);
  assert.equal(wasm.os, undefined);
  assert.equal(wasm.libc, undefined);
});

test('the package entries name files it ships', () => {
  assert.equal(wasm.main, `${manifest.napi.binaryName}.wasi.cjs`);
  assert.equal(wasm.browser, `${manifest.napi.binaryName}.wasi-browser.js`);
  for (const entry of [wasm.main, wasm.browser]) {
    assert.ok(wasm.files.includes(entry), `${entry} is an entry but is not in files`);
  }
});

test('the package ships every file the Node loader reads', () => {
  const read = [...source(wasm.main).matchAll(/join\(__dirname, '([^']+)'\)/g)].map((match) => match[1]);
  // The debug build is left out on purpose: the loader prefers it wherever it
  // finds it, and it is not published.
  const shipped = read.filter((file) => !file.endsWith('.debug.wasm'));
  assert.deepEqual(shipped.sort(), ['pixeldelta.wasm32-wasi.wasm', 'wasi-worker.mjs']);
  for (const file of shipped) {
    assert.ok(wasm.files.includes(file), `${file} is read by the loader but is not in files`);
  }
});

test('the package declares what its loaders import', () => {
  const imports = [...source(wasm.main).matchAll(/require\('([^'.][^']*)'\)/g)].map((match) => match[1]);
  const browserImports = [...source(wasm.browser).matchAll(/from '([^'.][^']*)'/g)].map((match) => match[1]);
  const external = [...new Set([...imports, ...browserImports])].filter((name) => !name.startsWith('node:'));
  assert.deepEqual(external, ['@napi-rs/wasm-runtime']);
  for (const dependency of external) {
    assert.ok(wasm.dependencies?.[dependency], `${dependency} is imported but is not a dependency`);
  }
});
