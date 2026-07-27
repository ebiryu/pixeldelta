#!/usr/bin/env node
// Runs the library tests against the WebAssembly (WASI) build.
//
// NAPI_RS_FORCE_WASI=error makes index.js hand back the WASI binding and throw
// when none is there, so a run without the .wasm beside it fails instead of
// falling back to the native addon and passing. Setting it here rather than in
// the npm script keeps the command the same on Windows, where a shell does not
// take VAR=value in front of a command.
//
// The command-line tests are left out: the WebAssembly package carries no
// executable, for the reason cli.js states.

import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), '..');

const result = spawnSync(
  process.execPath,
  ['--test', 'test/compare.test.mjs', 'test/wasi/load.test.mjs'],
  {
    cwd: pkgDir,
    stdio: 'inherit',
    env: { ...process.env, NAPI_RS_FORCE_WASI: 'error' },
  },
);

if (result.error) {
  process.stderr.write(`test-wasi: cannot run the test runner: ${result.error.message}\n`);
}
process.exit(result.status ?? 1);
