#!/usr/bin/env node
// Assembles pixeldelta-wasm, the package that carries the WebAssembly build for
// hosts with no prebuild and for browsers.
//
//   node scripts/place-wasm.mjs
//     Takes the .wasm from beside this package, where `pnpm build --target
//     wasm32-wasip1-threads` leaves it.
//
//   node scripts/place-wasm.mjs --artifacts <dir>
//     Takes it from <dir>, where the release job gathers the build matrix.
//
// The loaders are copied verbatim from the package root. Each one reaches the
// .wasm and its worker relative to itself, so a copy beside them resolves with
// nothing rewritten.
//
// The run fails unless every file npm/wasm/package.json lists has arrived,
// because the alternative is publishing a package that installs and then throws
// on the first require.

import { copyFileSync, existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), '..')
const wasmDir = join(pkgDir, 'npm', 'wasm')
const manifest = JSON.parse(readFileSync(join(wasmDir, 'package.json'), 'utf8'))

const [flag, dir] = process.argv.slice(2)
if (flag && flag !== '--artifacts') {
  throw new Error(`unknown argument: ${flag}`)
}
const binaryDir = flag === '--artifacts' ? resolve(pkgDir, dir ?? '.') : pkgDir

for (const file of manifest.files) {
  const source = join(file.endsWith('.wasm') ? binaryDir : pkgDir, file)
  if (!existsSync(source)) {
    continue
  }
  copyFileSync(source, join(wasmDir, file))
  console.log(`${source} -> ${join(wasmDir, file)}`)
}

const missing = manifest.files.filter((file) => !existsSync(join(wasmDir, file)))
if (missing.length > 0) {
  process.stderr.write(
    `place-wasm: ${manifest.name} is missing ${missing.join(', ')}.\n` +
      `Build the WebAssembly target first, or pass --artifacts <dir>.\n`,
  )
  process.exit(1)
}
