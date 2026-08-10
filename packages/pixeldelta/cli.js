#!/usr/bin/env node
// Runs the pixeldelta executable that the host's platform package ships.
//
// This package cannot name the executable in its own bin entry: the executable
// lives in a platform package, and which package that is follows from the host
// the install ran on. The launcher resolves that package and hands the process
// over, forwarding the arguments, the standard streams and the exit status.

'use strict'

const { spawnSync } = require('node:child_process')
const { existsSync } = require('node:fs')
const { constants } = require('node:os')
const { dirname, join } = require('node:path')

// The targets the build matrix produces an executable for, each paired with
// the lookup that finds it once installed. The WebAssembly package is absent
// on purpose: the executable runs git and opens TLS connections, and
// wasm32-wasip1 has no sockets, so a build there would carry subcommands that
// cannot work.
//
// Each lookup names its package outright rather than assembling the name from
// the host. A module reference built at runtime is one a bundler cannot follow
// and a reader cannot check against the packages under npm/, and it is the
// shape a supply-chain scanner has to treat as a package loading something it
// declined to name.
const manifests = {
  'pixeldelta-darwin-arm64': () => require.resolve('pixeldelta-darwin-arm64/package.json'),
  'pixeldelta-darwin-x64': () => require.resolve('pixeldelta-darwin-x64/package.json'),
  'pixeldelta-linux-arm64-gnu': () => require.resolve('pixeldelta-linux-arm64-gnu/package.json'),
  'pixeldelta-linux-x64-gnu': () => require.resolve('pixeldelta-linux-x64-gnu/package.json'),
  'pixeldelta-linux-x64-musl': () => require.resolve('pixeldelta-linux-x64-musl/package.json'),
  'pixeldelta-win32-x64-msvc': () => require.resolve('pixeldelta-win32-x64-msvc/package.json'),
}

const supported = Object.keys(manifests)

const binaryName = (platform) => (platform === 'win32' ? 'pixeldelta.exe' : 'pixeldelta')

/** Names the platform package for a host, or null when none carries a build. */
const platformPackage = (platform, arch, musl) => {
  let name
  if (platform === 'darwin') {
    name = `pixeldelta-darwin-${arch}`
  } else if (platform === 'win32') {
    name = `pixeldelta-win32-${arch}-msvc`
  } else if (platform === 'linux') {
    name = `pixeldelta-linux-${arch}-${musl ? 'musl' : 'gnu'}`
  } else {
    return null
  }
  return supported.includes(name) ? name : null
}

// A glibc runtime version in the process report is what separates the two
// Linux ABIs. A musl build reports none.
const hostIsMusl = () =>
  process.platform === 'linux' && !process.report.getReport().header.glibcVersionRuntime

/** Names the platform package for the host, or null when none carries one. */
const hostPackage = () => platformPackage(process.platform, process.arch, hostIsMusl())

/** Locates the executable for this host, or returns null when it is absent. */
const hostBinary = () => {
  const name = hostPackage()
  if (!name) {
    return null
  }

  const file = binaryName(process.platform)
  const candidates = []
  try {
    candidates.push(join(dirname(manifests[name]()), file))
  } catch {
    // The platform package is optional, so a host outside the matrix and a
    // partial install both land here.
  }
  // The repository layout, where `pnpm run build:cli` places the host build.
  candidates.push(join(__dirname, 'npm', name.slice('pixeldelta-'.length), file))

  return candidates.find(existsSync) ?? null
}

const main = () => {
  const binary = hostBinary()
  if (!binary) {
    process.stderr.write(
      `pixeldelta: no executable for ${process.platform} ${process.arch}.\n` +
        `It ships with one of these packages, installed as an optional dependency:\n` +
        supported.map((name) => `  ${name}\n`).join('') +
        `Reinstall to let the package manager pick one for this host.\n`,
    )
    process.exit(1)
  }

  const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' })
  if (result.error) {
    process.stderr.write(`pixeldelta: cannot run ${binary}: ${result.error.message}\n`)
    process.exit(1)
  }
  if (result.signal) {
    // Report a killed run the way a shell does, so it is not read as a clean
    // exit by whatever called this.
    const number = constants.signals[result.signal]
    process.exit(number ? 128 + number : 1)
  }
  process.exit(result.status ?? 1)
}

if (require.main === module) {
  main()
}

module.exports = { binaryName, hostBinary, hostPackage, platformPackage, supported }
