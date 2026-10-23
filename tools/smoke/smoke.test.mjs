// Loads pixeldelta the way an installed consumer does: `require('pixeldelta')`
// resolving to the root package, which in turn loads the platform package's
// binary. Run from an empty project that installed the packed tarballs, so a
// pass means the published layout resolves and the addon runs.

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

// Resolved from the app's node_modules, not the workspace.
const { compareSync } = require('pixeldelta');

test('the installed package loads and compares', () => {
  const base = join(here, 'base.png');
  const head = join(here, 'head.png');

  const same = compareSync(base, base);
  assert.equal(same.verdict, 'match');
  assert.equal(same.diffPixels, 0);

  // blocks at threshold 0.1 counts 240 in the core fixture baseline.
  const differ = compareSync(base, head, { threshold: 0.1 });
  assert.equal(differ.verdict, 'differ');
  assert.equal(differ.diffPixels, 240);
});
