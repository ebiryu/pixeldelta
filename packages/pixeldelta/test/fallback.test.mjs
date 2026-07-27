// Checks main.js, the entry `main` names: index.js when a prebuild matched,
// and the pixeldelta-wasm package when none did.
//
// The fallback runs against stubs in a temporary directory. A checkout able to
// build the addon is exactly the case where the fallback does not run, so
// nothing here can reach it by loading the package as it stands.

import test from 'node:test';
import assert from 'node:assert/strict';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = join(here, '..');

/**
 * A directory holding main.js, an index.js that throws the way the generated
 * loader does on a host with no prebuild, and pixeldelta-wasm beside it or not.
 */
const withoutPrebuild = ({ fallbackInstalled, fallbackBody }) => {
  const dir = mkdtempSync(join(tmpdir(), 'pixeldelta-main-'));
  copyFileSync(join(pkgDir, 'main.js'), join(dir, 'main.js'));
  writeFileSync(join(dir, 'index.js'), "throw new Error('Failed to load native binding')\n");
  if (fallbackInstalled) {
    const installed = join(dir, 'node_modules', 'pixeldelta-wasm');
    mkdirSync(installed, { recursive: true });
    writeFileSync(join(installed, 'package.json'), '{"name":"pixeldelta-wasm","main":"index.js"}');
    writeFileSync(
      join(installed, 'index.js'),
      fallbackBody ?? "module.exports = { compare: 'the wasm build' }\n",
    );
  }
  return dir;
};

test('a host with no prebuild is served by the WebAssembly package', (t) => {
  const dir = withoutPrebuild({ fallbackInstalled: true });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.equal(require(join(dir, 'main.js')).compare, 'the wasm build');
});

test('a host with neither is told which package to install', (t) => {
  const dir = withoutPrebuild({ fallbackInstalled: false });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.throws(
    () => require(join(dir, 'main.js')),
    (error) => {
      assert.match(error.message, /pixeldelta-wasm/);
      // The generated loader names the platforms that do ship a prebuild, so
      // its report has to survive rather than be replaced by this one.
      assert.match(error.cause.message, /Failed to load native binding/);
      return true;
    },
  );
});

test('a WebAssembly package that fails to load reports its own failure', (t) => {
  // An install hint here would send a reader after a package they already
  // have. What went wrong inside it is the only thing that names the cause,
  // which for a missing transitive dependency is that dependency.
  const dir = withoutPrebuild({
    fallbackInstalled: true,
    fallbackBody: "require('@emnapi/wasi-threads')\n",
  });
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.throws(() => require(join(dir, 'main.js')), /@emnapi\/wasi-threads/);
});

test('the entry hands back what the binding exports', () => {
  const entry = require('../main.js');
  const generated = require('../index.js');
  assert.equal(entry.compare, generated.compare);
  assert.equal(entry.compareSync, generated.compareSync);
});

test('the entry carries named exports into an ESM import', async () => {
  // Node reads named exports out of a CommonJS module by scanning it for
  // `module.exports.<name> =`, so assigning the binding alone would leave
  // `import { compare } from 'pixeldelta'` undefined.
  const entry = await import('../main.js');
  assert.equal(typeof entry.compare, 'function');
  assert.equal(typeof entry.compareSync, 'function');
});
