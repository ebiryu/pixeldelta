// The binding the comparison tests run against.
//
// A plain run takes the package entry, which reaches whichever prebuild the
// checkout has. `pnpm run test:wasi` sets PIXELDELTA_TEST_WASI, and the same
// tests then run against the WebAssembly build by name.
//
// Naming it here rather than asking the loader for it is what makes the WASI
// run mean something: the loader prefers a native addon, so a run that let it
// choose would pass against the addon on any host that has one. The switch
// lives in the test tree, which is not published.

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

const binding = process.env.PIXELDELTA_TEST_WASI
  ? require('../pixeldelta.wasi.cjs')
  : require('../main.js');

export const { compare, compareSync } = binding;
export default binding;
