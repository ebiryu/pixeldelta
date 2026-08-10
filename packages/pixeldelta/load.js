// Finds the prebuilt addon for the host and loads it.
//
// napi-rs generates a loader of its own, and this file is published in its
// place. The generated one covers every target napi-rs knows, and reaches them
// through a module name it builds at runtime, a path it reads out of the
// environment, and `ldd` run through a shell. None of that is needed to find
// one of six prebuilds, and all of it is what a package does when it is
// loading something it would rather not name: a supply-chain scanner reads the
// generated loader as dynamic requires, environment access and shell access,
// and it is not wrong about what the code says, only about why.
//
// So every module here is named outright, the one thing the lookup has to
// decide is answered from the process report, and nothing is read from the
// environment. `napi build` still writes the generated loader, which stays out
// of git and out of `files`; index.d.ts, from the same build, is what the
// package ships for types.

'use strict'

// The targets `napi.targets` in package.json builds, under the names npm gives
// their packages, each paired with the two places its addon can sit: beside
// this file, where a build in the checkout leaves it, and in the platform
// package, where an install brings it. They are tried in that order, so a
// local build wins over an installed one. cli.js holds the same targets for
// the executable, which the WebAssembly target has no build of.
//
// The lookup is a table of thunks rather than a name passed to `require`, so
// that each call site stays a literal string. That is what lets a bundler
// follow this file, and what lets a reader check it against the platform
// packages under npm/ without running it.
const prebuilds = {
  'darwin-arm64': [
    () => require('./pixeldelta.darwin-arm64.node'),
    () => require('pixeldelta-darwin-arm64'),
  ],
  'darwin-x64': [
    () => require('./pixeldelta.darwin-x64.node'),
    () => require('pixeldelta-darwin-x64'),
  ],
  'linux-arm64-gnu': [
    () => require('./pixeldelta.linux-arm64-gnu.node'),
    () => require('pixeldelta-linux-arm64-gnu'),
  ],
  'linux-x64-gnu': [
    () => require('./pixeldelta.linux-x64-gnu.node'),
    () => require('pixeldelta-linux-x64-gnu'),
  ],
  'linux-x64-musl': [
    () => require('./pixeldelta.linux-x64-musl.node'),
    () => require('pixeldelta-linux-x64-musl'),
  ],
  'win32-x64-msvc': [
    () => require('./pixeldelta.win32-x64-msvc.node'),
    () => require('pixeldelta-win32-x64-msvc'),
  ],
}

const supported = Object.keys(prebuilds)

// A glibc runtime version in the process report is what separates the two
// Linux ABIs. A musl build reports none.
const hostIsMusl = () =>
  process.platform === 'linux' && !process.report.getReport().header.glibcVersionRuntime

/** Names the target for a host, or null when the matrix carries no build. */
const target = (platform, arch, musl) => {
  let name
  if (platform === 'darwin') {
    name = `darwin-${arch}`
  } else if (platform === 'win32') {
    name = `win32-${arch}-msvc`
  } else if (platform === 'linux') {
    name = `linux-${arch}-${musl ? 'musl' : 'gnu'}`
  } else {
    return null
  }
  return supported.includes(name) ? name : null
}

/** Names the target for this host, or null when the matrix carries no build. */
const hostTarget = () => target(process.platform, process.arch, hostIsMusl())

/** What went wrong, with the first attempt at the end of the cause chain. */
const chain = (errors) =>
  errors.reduce((previous, current) => {
    current.cause = previous
    return current
  })

const load = () => {
  const name = hostTarget()
  const errors = []

  for (const open of name ? prebuilds[name] : []) {
    try {
      return open()
    } catch (error) {
      errors.push(error)
    }
  }

  // The WebAssembly build, when one sits beside this file. An install never
  // has it: `files` leaves it out, and main.js reaches for pixeldelta-wasm on
  // a host with no prebuild. This serves a checkout built for
  // wasm32-wasip1-threads and nothing else.
  try {
    return require('./pixeldelta.wasi.cjs')
  } catch (error) {
    errors.push(error)
  }

  const error = new Error(
    name
      ? `No pixeldelta addon for ${process.platform} ${process.arch}.\n` +
          `It ships in pixeldelta-${name}, installed as an optional dependency. ` +
          `A package manager that skipped it leaves the addon missing: remove the ` +
          `lockfile and node_modules and install again.\n`
      : `No pixeldelta addon for ${process.platform} ${process.arch}.\n` +
          `The build matrix carries these:\n` +
          supported.map((entry) => `  pixeldelta-${entry}\n`).join(''),
  )
  // assign instead of the `new Error(message, { cause })` options form, which
  // Node < 16.9 silently ignores
  error.cause = chain(errors)
  throw error
}

// The binding alone. main.js is what names the exports one by one, which is
// what an ESM import of this package reads.
module.exports = load()
