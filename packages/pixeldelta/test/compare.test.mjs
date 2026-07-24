// Exercises the binding against the fixtures the core crate is checked on, so
// a diff pixel count here can be read against tests/fixtures/expected.txt.

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { compare, compareSync } from '../index.js';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', '..', '..', 'crates', 'pixeldelta-core', 'tests', 'fixtures');
const fixture = (name, side) => join(fixtures, name, `${side}.png`);

test('an image matches itself', async () => {
  const result = await compare(fixture('blocks', 'base'), fixture('blocks', 'base'));
  assert.equal(result.verdict, 'match');
  assert.equal(result.diffPixels, 0);
  assert.equal(result.diffRatio, 0);
});

test('the diff count matches the fixture baseline', async () => {
  // blocks at threshold 0.1 with anti-aliasing on counts 240 in expected.txt.
  const result = await compare(fixture('blocks', 'base'), fixture('blocks', 'head'), {
    threshold: 0.1,
    antialiasing: true,
  });
  assert.equal(result.verdict, 'differ');
  assert.equal(result.diffPixels, 240);
  assert.ok(result.diffRatio > 0);
});

test('compareSync returns the same as compare', async () => {
  const asyncResult = await compare(fixture('blocks', 'base'), fixture('blocks', 'head'));
  const syncResult = compareSync(fixture('blocks', 'base'), fixture('blocks', 'head'));
  assert.deepEqual(syncResult, asyncResult);
});

test('a buffer compares the same as its path', async () => {
  const base = readFileSync(fixture('blocks', 'base'));
  const head = readFileSync(fixture('blocks', 'head'));
  const fromBuffers = await compare(base, head);
  const fromPaths = await compare(fixture('blocks', 'base'), fixture('blocks', 'head'));
  assert.equal(fromBuffers.diffPixels, fromPaths.diffPixels);
});

test('clustering reports where the differences sit', async () => {
  const result = await compare(fixture('blocks', 'base'), fixture('blocks', 'head'), {
    cluster: true,
  });
  assert.ok(result.clusters.length > 0);
  for (const cluster of result.clusters) {
    assert.ok(cluster.width > 0 && cluster.height > 0);
    assert.ok(cluster.diffPixels > 0);
    assert.ok(typeof cluster.ssim === 'number');
  }
});

test('images of different sizes are not compared', async () => {
  const result = await compare(fixture('blocks', 'base'), fixture('flat', 'base'));
  assert.equal(result.verdict, 'sizeMismatch');
  assert.equal(result.diffPixels, 0);
});

test('fail-fast stops early and flags the count as a lower bound', async () => {
  const result = await compare(fixture('blocks', 'base'), fixture('blocks', 'head'), {
    failFast: { maxDiffPixels: 10 },
  });
  assert.equal(result.stoppedEarly, true);
  assert.ok(result.diffPixels > 10);
});

test('bytes that are not an image reject with a message', async () => {
  await assert.rejects(
    () => compare(Buffer.from('not an image'), Buffer.from('not an image')),
    /unrecognized|not supported/i,
  );
});

test('a missing file rejects naming the path', async () => {
  await assert.rejects(
    () => compare('does-not-exist.png', fixture('blocks', 'base')),
    /does-not-exist\.png/,
  );
});
