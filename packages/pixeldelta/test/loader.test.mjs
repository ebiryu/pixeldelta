// Checks the shape of the JavaScript this package publishes.
//
// The published files are the ones a supply-chain scanner reads, and what it
// reads there is all it has: a module name assembled at runtime, a path taken
// from the environment or a subprocess opened to answer a question about the
// host are the moves a compromised package makes, and a scanner reports them
// whatever the reason for them. load.js exists so that this package makes none
// of them, and these tests are what keeps that true — including across a
// regeneration of the napi-rs loader, which makes all of them.
//
// cli.js is the exception the tests below spell out: launching an executable
// that lives in another package is what it is for, and it cannot do that
// without the filesystem and a subprocess.

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
const source = (file) => readFileSync(join(pkgDir, file), 'utf8');

const published = manifest.files.filter((file) => file.endsWith('.js'));

/** Every module reference in a file, as written. */
const references = (text) =>
  [...text.matchAll(/\brequire(?:\.resolve)?\(([^)]*)\)/g)].map((match) => match[1].trim());

test('the package publishes the hand-written loader, not the generated one', () => {
  assert.ok(manifest.files.includes('load.js'), 'load.js is published');
  assert.ok(!manifest.files.includes('index.js'), 'the napi-rs loader is not published');
  assert.deepEqual(published.sort(), ['browser-entry.js', 'cli.js', 'load.js', 'main.js']);
});

test('every module a published file reaches for is named outright', () => {
  for (const file of published) {
    for (const reference of references(source(file))) {
      assert.match(
        reference,
        /^'[^']+'$/,
        `${file} builds a module name at runtime: require(${reference})`,
      );
    }
  }
});

test('nothing published reads the environment', () => {
  for (const file of published) {
    assert.doesNotMatch(source(file), /process\.env/, `${file} reads the environment`);
  }
});

test('only the launcher opens a subprocess or the filesystem', () => {
  // cli.js hands the process over to an executable in a platform package: it
  // has to find that file and run it. Nothing else here needs either.
  for (const file of published.filter((entry) => entry !== 'cli.js')) {
    const text = source(file);
    assert.doesNotMatch(text, /child_process/, `${file} opens a subprocess`);
    assert.doesNotMatch(text, /require\('(node:)?fs'\)/, `${file} reads the filesystem`);
  }
});

test('the loader and the launcher agree on the targets that have builds', () => {
  // cli.js names the packages, load.js the targets inside them. They come from
  // one build matrix, so a target added to one and not the other is a host
  // that gets a library and no executable, or the reverse.
  const { supported: packages } = require('../cli.js');
  const targets = [...source('load.js').matchAll(/require\('pixeldelta-([^']+)'\)/g)].map(
    (match) => match[1],
  );
  assert.deepEqual(
    targets.sort(),
    packages.map((name) => name.slice('pixeldelta-'.length)).sort(),
  );
});

test('every target the loader reaches for has a platform package', () => {
  const targets = [...source('load.js').matchAll(/require\('pixeldelta-([^']+)'\)/g)].map(
    (match) => match[1],
  );
  for (const target of targets) {
    const platform = require(join(pkgDir, 'npm', target, 'package.json'));
    assert.equal(platform.name, `pixeldelta-${target}`);
    // The loader tries the addon beside itself first, under the name a build
    // in this checkout leaves it under, which is the platform package's entry.
    assert.equal(platform.main, `pixeldelta.${target}.node`);
    assert.match(source('load.js'), new RegExp(`require\\('\\./pixeldelta\\.${target}\\.node'\\)`));
  }
});
