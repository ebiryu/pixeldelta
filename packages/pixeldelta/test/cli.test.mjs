// Checks the layout that lets an installed package reach the executable:
// the root package's bin entry, the launcher's platform lookup, and the
// executable each platform package ships.
//
// The tests that run the executable read it from npm/<platform>/, where
// `pnpm run build:cli` places the host build.

import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = join(here, '..');

const manifest = require('../package.json');
const { platformPackage, binaryName, supported, hostBinary } = require('../cli.js');

const fixtures = join(pkgDir, '..', '..', 'crates', 'pixeldelta-core', 'tests', 'fixtures');
const fixture = (name, side) => join(fixtures, name, `${side}.png`);

test('the root package exposes the launcher as a bin', () => {
  assert.equal(manifest.bin.pixeldelta, 'cli.js');
  assert.ok(manifest.files.includes('cli.js'), 'cli.js is published');
});

test('each supported host names one platform package', () => {
  const cases = [
    [['darwin', 'arm64', false], 'pixeldelta-darwin-arm64'],
    [['darwin', 'x64', false], 'pixeldelta-darwin-x64'],
    [['linux', 'x64', false], 'pixeldelta-linux-x64-gnu'],
    [['linux', 'x64', true], 'pixeldelta-linux-x64-musl'],
    [['linux', 'arm64', false], 'pixeldelta-linux-arm64-gnu'],
    [['win32', 'x64', false], 'pixeldelta-win32-x64-msvc'],
  ];
  for (const [[platform, arch, musl], expected] of cases) {
    assert.equal(platformPackage(platform, arch, musl), expected);
  }
});

test('a host with no build resolves to nothing', () => {
  // The matrix carries no arm64 musl and no 32-bit target, and the WebAssembly
  // package holds the library alone.
  assert.equal(platformPackage('linux', 'arm64', true), null);
  assert.equal(platformPackage('win32', 'arm64', false), null);
  assert.equal(platformPackage('freebsd', 'x64', false), null);
});

test('the executable carries the Windows extension only on Windows', () => {
  assert.equal(binaryName('linux'), 'pixeldelta');
  assert.equal(binaryName('darwin'), 'pixeldelta');
  assert.equal(binaryName('win32'), 'pixeldelta.exe');
});

// Windows has no executable bit: a file mode there reports writability, and
// npm reaches a bin through a generated .cmd shim rather than the shebang.
const posixOnly = { skip: process.platform === 'win32' && 'POSIX file modes only' };

test('the launcher starts with a shebang', () => {
  assert.ok(readFileSync(join(pkgDir, 'cli.js'), 'utf8').startsWith('#!/usr/bin/env node\n'));
});

test('the launcher is executable', posixOnly, () => {
  // npm links a bin as a symlink on POSIX, so the target itself has to run.
  assert.ok(statSync(join(pkgDir, 'cli.js')).mode & 0o111, 'cli.js has the executable bit');
});

test('the platform packages are published with npm', () => {
  // `napi prepublish` runs `<npmClient> publish` for each platform package and
  // defaults to npm. The client is not interchangeable here: `pnpm pack`
  // writes every file as 0644, so the executable would arrive unrunnable.
  assert.equal(manifest.napi.npmClient, undefined);
});

test('every platform package that has a build ships the executable', () => {
  for (const name of supported) {
    const dir = join(pkgDir, 'npm', name.replace(/^pixeldelta-/, ''));
    const files = require(join(dir, 'package.json')).files;
    const expected = name.startsWith('pixeldelta-win32') ? 'pixeldelta.exe' : 'pixeldelta';
    assert.ok(files.includes(expected), `${name} publishes ${expected}`);
  }
});

test('the launcher names the supported packages when none is installed', () => {
  const work = mkdtempSync(join(tmpdir(), 'pixeldelta-cli-'));
  try {
    // A tree holding the launcher alone, so the lookup finds no platform
    // package however the host is built.
    const installed = join(work, 'node_modules', 'pixeldelta');
    mkdirSync(installed, { recursive: true });
    writeFileSync(join(installed, 'cli.js'), readFileSync(join(pkgDir, 'cli.js')));
    writeFileSync(join(installed, 'package.json'), '{"name":"pixeldelta","version":"0.0.0"}');

    const result = spawnSync(process.execPath, [join(installed, 'cli.js'), '--version'], {
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    for (const name of supported) {
      assert.ok(result.stderr.includes(name), `the error names ${name}`);
    }
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
});

test('the launcher runs the executable and forwards arguments', () => {
  const result = spawnSync(process.execPath, [join(pkgDir, 'cli.js'), '--version'], {
    encoding: 'utf8',
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^pixeldelta \d+\.\d+\.\d+/);
});

test('the launcher forwards the exit code of a comparison', () => {
  const run = (base, head) =>
    spawnSync(process.execPath, [join(pkgDir, 'cli.js'), 'compare', base, head], {
      encoding: 'utf8',
    });

  const same = run(fixture('blocks', 'base'), fixture('blocks', 'base'));
  assert.equal(same.status, 0, same.stderr);
  assert.match(same.stdout, /match/);

  // A comparison that finds differences exits 1, which is what makes the
  // executable usable from a CI script.
  const differ = run(fixture('blocks', 'base'), fixture('blocks', 'head'));
  assert.equal(differ.status, 1, differ.stderr);
  assert.match(differ.stdout, /differ/);

  // Codes above 1 reach the caller as themselves, not folded into a single
  // failure.
  const sizes = run(fixture('blocks', 'base'), fixture('flat', 'base'));
  assert.equal(sizes.status, 2, sizes.stderr);
  assert.match(sizes.stdout, /size mismatch/);
});

test('the host build sits where the platform package publishes it', () => {
  const path = hostBinary();
  assert.ok(path, 'the host has a platform package holding a build');
  assert.match(path, /npm[\\/][^\\/]+[\\/]pixeldelta(\.exe)?$/);
});

test('the host build carries the executable bit', posixOnly, () => {
  assert.ok(statSync(hostBinary()).mode & 0o111, 'the executable bit is set');
});
