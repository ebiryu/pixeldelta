// Places the command-line executable into the platform package that publishes
// it. `napi artifacts` does the same for the addon, but it only moves files
// ending in .node and .wasm, so the executable needs its own step.
//
//   node scripts/place-cli.mjs
//     Takes the host build from target/release and places it.
//
//   node scripts/place-cli.mjs --artifacts <dir>
//     Takes every pixeldelta-cli.<platform> in <dir> and places each one. This
//     is the release job, gathering what the build matrix left.
//
// The executable bit is set here. It has to survive `npm pack`, or an install
// succeeds and the first run fails with EACCES.

import { chmodSync, copyFileSync, existsSync, readdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const { binaryName, hostPackage } = require('../cli.js')

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), '..')

/** Copies one built executable into npm/<platform>/, executable bit set. */
const place = (source, tag, file) => {
  const target = join(pkgDir, 'npm', tag, file)
  copyFileSync(source, target)
  chmodSync(target, 0o755)
  console.log(`${source} -> ${target}`)
}

const placeHostBuild = () => {
  const name = hostPackage()
  if (!name) {
    throw new Error(`no platform package for ${process.platform} ${process.arch}`)
  }
  const file = binaryName(process.platform)
  const source = join(pkgDir, '..', '..', 'target', 'release', file)
  if (!existsSync(source)) {
    throw new Error(`${source} is missing; build the pixeldelta-cli crate first`)
  }
  place(source, name.slice('pixeldelta-'.length), file)
}

const placeArtifacts = (dir) => {
  // The build jobs name each one after the platform package it belongs to, so
  // the gathered directory can hold every target at once.
  const built = readdirSync(dir).filter((file) => file.startsWith('pixeldelta-cli.'))
  if (built.length === 0) {
    throw new Error(`${dir} holds no pixeldelta-cli.<platform> executable`)
  }
  for (const file of built) {
    const windows = file.endsWith('.exe')
    const tag = file.slice('pixeldelta-cli.'.length).replace(/\.exe$/, '')
    place(join(dir, file), tag, windows ? 'pixeldelta.exe' : 'pixeldelta')
  }
}

const [flag, dir] = process.argv.slice(2)
if (flag === '--artifacts') {
  placeArtifacts(dir ?? '.')
} else if (flag) {
  throw new Error(`unknown argument: ${flag}`)
} else {
  placeHostBuild()
}
