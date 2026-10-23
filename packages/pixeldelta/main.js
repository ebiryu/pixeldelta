// The entry `main` names, so this is what `require('pixeldelta')` reaches.
//
// load.js finds the prebuilt addon for the host, and behind it the WebAssembly
// build in pixeldelta-wasm32-wasi. An install never brings that package in: it
// stays out of the optional dependencies, which a package manager resolves one
// entry at a time, so listing it would pull the .wasm onto every host as well.
// pixeldelta-wasm carries the same WebAssembly build under a name the consumer
// installs, and this file reaches for it before giving up.

const FALLBACK = 'pixeldelta-wasm'

// The name is written out at each call rather than passed from the constant
// above: a module reference that is a literal string is one a bundler can
// follow and a reader can check, and the constant is left to the message.
const installed = () => {
  try {
    require.resolve('pixeldelta-wasm')
    return true
  } catch {
    return false
  }
}

let binding
try {
  binding = require('./load.js')
} catch (nativeError) {
  if (!installed()) {
    const error = new Error(
      `${nativeError.message}\n` +
        `No prebuild matched this host and ${FALLBACK} is not installed either. ` +
        `Install ${FALLBACK} to run the WebAssembly build instead.`,
    )
    // The lookup lists the platforms that do ship a prebuild, which is what
    // tells a reader whether to expect one at all.
    error.cause = nativeError
    throw error
  }
  // Whatever a present package throws on the way in says more than an install
  // hint would, so it is left to travel on its own.
  binding = require('pixeldelta-wasm')
}

module.exports = binding
// Assigned one by one because that is the shape Node scans for when it reads
// named exports out of a CommonJS module for an ESM import.
module.exports.compare = binding.compare
module.exports.compareSync = binding.compareSync
