// Reads and writes the version that the release carries.
//
// The version lives in three kinds of file, and every one of them has to hold
// the same string:
//
//   1. `packages/pixeldelta/package.json`, the root npm package. This one is
//      the source: `napi prepublish` copies it into the platform packages and
//      into the optionalDependencies it writes for them.
//   2. `packages/pixeldelta/npm/*/package.json`, the platform packages. What is
//      committed here is what the copy above will produce, so that the layout
//      can be read without running a release. `npm/wasm` sits among them: it is
//      published by the same job at the same version, though napi-rs does not
//      know about it.
//   3. `Cargo.toml`, which `pixeldelta --version` reports through
//      CARGO_PKG_VERSION. The crates are not published to crates.io, so the
//      version has no other reader.
//
// The loader is not among them: `packages/pixeldelta/load.js` names the
// platform packages but not their version, so a bump leaves it alone.
//
// Usage:
//
//   node tools/release/version.mjs            print the version, or the
//                                             disagreeing files and exit 1
//   node tools/release/version.mjs 0.1.0      write that version to all of them
//
// `just release` calls the second form. Nothing here commits, tags or pushes.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../..', import.meta.url))
const pkgDir = join(root, 'packages', 'pixeldelta')

// The version line of a package.json, and the one in the [workspace.package]
// table of Cargo.toml. The Cargo pattern only runs to the end of that table,
// which the lines not starting a new one delimit. Each file is edited in place
// rather than reserialized, so the formatting and the surrounding comments stay
// as they are.
//
// Each pattern captures what precedes the version and the version itself, and
// stops there: the write puts the two back together.
const JSON_VERSION = /^(\s*"version":\s*")([^"]*)/m
const CARGO_VERSION = /^(\[workspace\.package\](?:\n(?!\[).*)*?\nversion = ")([^"]*)/m

// Same shape npm accepts: three numbers, with an optional prerelease tag.
const VERSION = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/

/** Every file holding the version, each with the pattern that finds its line. */
const manifests = () => {
  const npmDir = join(pkgDir, 'npm')
  const platforms = readdirSync(npmDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(npmDir, entry.name, 'package.json'))
  return [
    { path: join(root, 'Cargo.toml'), pattern: CARGO_VERSION },
    { path: join(pkgDir, 'package.json'), pattern: JSON_VERSION },
    ...platforms.map((path) => ({ path, pattern: JSON_VERSION })),
  ]
}

/** The text of one manifest and the version in it. */
const read = ({ path, pattern }) => {
  const text = readFileSync(path, 'utf8')
  const match = text.match(pattern)
  if (!match) {
    throw new Error(`${path} has no version line this script can read`)
  }
  return { text, version: match[2] }
}

/** The version they agree on, or the disagreement and exit 1. */
const check = () => {
  const found = manifests().map((manifest) => ({
    path: manifest.path,
    version: read(manifest).version,
  }))
  if (found.some((entry) => entry.version !== found[0].version)) {
    console.error('the manifests disagree on the version:')
    for (const entry of found) {
      console.error(`  ${entry.version}  ${entry.path.slice(root.length)}`)
    }
    console.error('run `just release <version>` to set one everywhere')
    process.exit(1)
  }
  return found[0].version
}

/** Writes the version into every manifest that does not already hold it. */
const write = (version) => {
  for (const manifest of manifests()) {
    // read throws when the pattern finds nothing, so a file that drifted out of
    // the shape this script edits is reported rather than skipped.
    const { text } = read(manifest)
    const written = text.replace(manifest.pattern, `$1${version}`)
    if (written !== text) {
      writeFileSync(manifest.path, written)
    }
  }
}

const [, , argument] = process.argv
if (argument === undefined) {
  console.log(check())
} else if (VERSION.test(argument)) {
  write(argument)
  console.log(check())
} else {
  console.error(`not a version: ${argument}`)
  process.exit(1)
}
