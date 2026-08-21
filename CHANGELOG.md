# Changelog

## [0.2.7](https://github.com/ebiryu/pixeldelta/compare/v0.2.6...v0.2.7) (2026-08-22)

### Bug Fixes

* **npm:** meet the WASI package rules of @napi-rs/cli 3.8 ([#8](https://github.com/ebiryu/pixeldelta/issues/8)) ([e2241c9](https://github.com/ebiryu/pixeldelta/commit/e2241c93a8712b2b6718ac630b22d5547f627491))

## [0.2.6](https://github.com/ebiryu/pixeldelta/compare/v0.2.5...v0.2.6) (2026-08-19)

### Bug Fixes

* **deps:** update npm dependencies ([#7](https://github.com/ebiryu/pixeldelta/issues/7)) ([a4b15ec](https://github.com/ebiryu/pixeldelta/commit/a4b15ec0930f515def384a985460b8508be2452c))
* **report:** align the viewer's sides and lift the captions off the images ([#6](https://github.com/ebiryu/pixeldelta/issues/6)) ([dea8934](https://github.com/ebiryu/pixeldelta/commit/dea89344cc10f21e287dc7cb319b05d35c34261f))

## [0.2.5](https://github.com/ebiryu/pixeldelta/compare/v0.2.4...v0.2.5) (2026-08-10)

### Build System

* **npm:** publish a hand-written loader instead of the generated one ([#4](https://github.com/ebiryu/pixeldelta/issues/4)) ([81de588](https://github.com/ebiryu/pixeldelta/commit/81de58850947ad8f795ac20939eb6b8e8a5ad6f7))

## [0.2.4](https://github.com/ebiryu/pixeldelta/compare/v0.2.3...v0.2.4) (2026-08-03)

### Features

* **cli:** take ignore regions from a flag and a config file ([5851e49](https://github.com/ebiryu/pixeldelta/commit/5851e4996e2b36b3bbe226f38a64dafb5aa0b5e5))

### Build System

* **cli:** drop the exact version pin on clap ([2f7738c](https://github.com/ebiryu/pixeldelta/commit/2f7738cefc66e2ee95a01524b3230b6a637fa7fc))

## [0.2.3](https://github.com/ebiryu/pixeldelta/compare/v0.2.2...v0.2.3) (2026-08-01)

### Features

* **report:** enlarge the viewer and give it zoom levels ([fdd67c9](https://github.com/ebiryu/pixeldelta/commit/fdd67c9047b387a841cd5960ded9835c879cb201))
* **report:** cap the clusters one entry reports ([fc23fcf](https://github.com/ebiryu/pixeldelta/commit/fc23fcfa7bf393ca55e86fd4d87dacf59d19beb8))
* **core:** leave a rectangle below 16 pixels out of the layout-shift search ([1aeb08d](https://github.com/ebiryu/pixeldelta/commit/1aeb08d21452ae83cb8293f0824984fd19c2183e))

### Bug Fixes

* **cli:** size the connection pool to the parallel object requests ([e9864b6](https://github.com/ebiryu/pixeldelta/commit/e9864b6910b6ce96b1d583367206734ce3a2cde2))

### Performance Improvements

* **cli:** decide a pair matches before building what a difference needs ([df0451a](https://github.com/ebiryu/pixeldelta/commit/df0451a25494c0a2ba27e0b091cec52eef1a44d1))
* **core:** hold the layout-shift offset table instead of rebuilding it ([d4edf13](https://github.com/ebiryu/pixeldelta/commit/d4edf1394d861bd9b1bf745e177f9885a8ef7a12))

## [0.2.2](https://github.com/ebiryu/pixeldelta/compare/v0.2.1...v0.2.2) (2026-07-31)

### Features

* **cli:** name the file a decode failure came from ([0e2ebf4](https://github.com/ebiryu/pixeldelta/commit/0e2ebf4608ac4eb74780bd412c7d97c6a6b986c8))

### Performance Improvements

* **cli:** write an entry's images as soon as it is compared ([a06992f](https://github.com/ebiryu/pixeldelta/commit/a06992f4238eeb3d78c1d96c6887cacc9b7b5b22))
* **cli:** send the object requests of a snapshot in parallel ([4cbddcb](https://github.com/ebiryu/pixeldelta/commit/4cbddcb2d2ef3b7162a2df34506ed69a067c2af2))
* **cli:** compare the files of a run in parallel ([4181980](https://github.com/ebiryu/pixeldelta/commit/4181980de8aab0baf10b54fb5aa27ff76c04c3b9))
* **core:** narrow the layout-shift search over a sample ladder ([1a7d61e](https://github.com/ebiryu/pixeldelta/commit/1a7d61eea0464f4ced89c47cbe1da2305eebd467))

## [0.2.1](https://github.com/ebiryu/pixeldelta/compare/v0.2.0...v0.2.1) (2026-07-30)

### Features

* **cli:** add an HTTP timeout and retries ([dfdb2d3](https://github.com/ebiryu/pixeldelta/commit/dfdb2d31bc33b87562ef7c3bb200e8d66b815b23))

### Bug Fixes

* **cli:** percent-encode the object key in the request URL ([0034792](https://github.com/ebiryu/pixeldelta/commit/0034792b9f0fad6fac8c1e43c6012641997b9fda))
* **io:** reject a PLTE chunk that is not whole entries ([8212c62](https://github.com/ebiryu/pixeldelta/commit/8212c6257624957f8542b749dda19f81b818a75f))

## [0.2.0](https://github.com/ebiryu/pixeldelta/compare/v0.1.2...v0.2.0) (2026-07-29)

### Features

* **report:** reference report images by URL instead of embedding them ([1b15963](https://github.com/ebiryu/pixeldelta/commit/1b1596302cda0f50c4d7a918a51f02eb8413cf9a))
* **cli:** put the report URL in the notification body ([c89fc86](https://github.com/ebiryu/pixeldelta/commit/c89fc866a9eba0fa4e6e53659597d03d72adec81))
* **cli:** allow a per-image difference ratio to pass ([f56a6ae](https://github.com/ebiryu/pixeldelta/commit/f56a6ae73e364ef7d166f42baaddd61e786358aa))

### Bug Fixes

* **cli:** set a Content-Type on every stored object ([383176a](https://github.com/ebiryu/pixeldelta/commit/383176a11fa5d213cb012f073984d4af93cdb114))
* **release:** keep the generated loader at the released version ([31e81b1](https://github.com/ebiryu/pixeldelta/commit/31e81b1ee1774ee753a4cd1ca53cb4cc8c1ba358))
* **smoke:** take the tarball names from npm pack ([ed52608](https://github.com/ebiryu/pixeldelta/commit/ed526082a8defa67843c1ee35de3dbf50273ed29))

## [0.1.2](https://github.com/ebiryu/pixeldelta/compare/v0.1.1...v0.1.2) (2026-07-27)

### Features

* **node:** publish the WebAssembly build as pixeldelta-wasm ([fa86ba5](https://github.com/ebiryu/pixeldelta/commit/fa86ba57eca9caa4bc2f458b0694e84e859fa1c7))
* **node:** send a browser target to the WebAssembly build ([d38c181](https://github.com/ebiryu/pixeldelta/commit/d38c1816cef2511dcfb9b0a99ff8ac25c3cc4b79))

## [0.1.1](https://github.com/ebiryu/pixeldelta/compare/v0.1.0...v0.1.1) (2026-07-27)

## 0.1.0 (2026-07-27)

### Features

* **cli:** ship the executable through the npm package ([83d4ef2](https://github.com/ebiryu/pixeldelta/commit/83d4ef2151fee42ed6634e70b88176ac338295c5))
* **cli:** post the notification body as a pull request comment ([d766880](https://github.com/ebiryu/pixeldelta/commit/d766880e501b891d950857553d3963e6b1b72730))
* **cli:** store snapshots on an S3-compatible service ([27820d9](https://github.com/ebiryu/pixeldelta/commit/27820d9774044a64b8db97c1041e9a454ce834c7))
* **cli:** write the notification body from the ci subcommand ([351afcf](https://github.com/ebiryu/pixeldelta/commit/351afcf2035a4fba32943646b211f0704cc09ae5))
* **report:** render the Markdown body for a notification ([b5f8661](https://github.com/ebiryu/pixeldelta/commit/b5f8661b548c7b711f990be0b18cc771fdb608ab))
* **cli:** add the ci subcommand ([5c9892f](https://github.com/ebiryu/pixeldelta/commit/5c9892f6f5e45bd4eacbfe29f80b985a92ed0648))
* **cli:** resolve the baseline commit from git history ([d60c9b5](https://github.com/ebiryu/pixeldelta/commit/d60c9b5a7979c912102aa90de7006db2dad926c0))
* **cli:** add snapshot storage with a local directory backend ([5856646](https://github.com/ebiryu/pixeldelta/commit/58566468fd2734d13d630d34e2f72cb1017ded6f))
* **cli:** add the run subcommand for directory comparison ([0cdbd4b](https://github.com/ebiryu/pixeldelta/commit/0cdbd4be0b079da02312544fcde34177c1951bd1))
* **report:** render a self-contained HTML report ([9170e4e](https://github.com/ebiryu/pixeldelta/commit/9170e4ef6511daa7c5f063d6ccdad722e6e58770))
* **report:** add JUnit XML output ([2bc3632](https://github.com/ebiryu/pixeldelta/commit/2bc3632dc7b428fa98abcd6072446df644492d8c))
* **report:** add the report crate with JSON output ([96ea736](https://github.com/ebiryu/pixeldelta/commit/96ea7361ffec25a168d2c9f28a5db740be52bca8))
* **node:** return the diff image from a comparison ([3234d33](https://github.com/ebiryu/pixeldelta/commit/3234d3395d4de6562cf570c4ce5b5607e5617883))
* **cli:** add the compare subcommand ([f14ce3b](https://github.com/ebiryu/pixeldelta/commit/f14ce3bc8eb45a3ae08857df90bde84b8ac908d5))
* **core:** render a diff image from a comparison ([7ff1d57](https://github.com/ebiryu/pixeldelta/commit/7ff1d57b6dd066a55bfe91d29ec4e90e864452d2))
* **io:** encode RGBA8 buffers to PNG ([36ffa2e](https://github.com/ebiryu/pixeldelta/commit/36ffa2e3d0aee753ded159b8c430fc10ed97a504))
* **node:** add the napi-rs binding and its npm package ([0e07819](https://github.com/ebiryu/pixeldelta/commit/0e07819225b0ff00677ca62cc6f76378fc3b7122))
* **io:** decode PNG files and buffers to RGBA8 ([99f7346](https://github.com/ebiryu/pixeldelta/commit/99f7346eb9f3a28aad0586cc5e9f3cbd4974efce))
* **core:** score each cluster with structural similarity ([526b007](https://github.com/ebiryu/pixeldelta/commit/526b007a8185d28016792f93eb9b06b317b718d5))
* **core:** estimate how far each cluster moved ([de750c9](https://github.com/ebiryu/pixeldelta/commit/de750c92c0c379602e0f9781981be0034431d861))
* **core:** group differing pixels into clusters ([9838f0b](https://github.com/ebiryu/pixeldelta/commit/9838f0b97eca7ce5fb12528aa9c7ed1de26e8289))
* **core:** stop the scan at a fail-fast limit ([bab8a02](https://github.com/ebiryu/pixeldelta/commit/bab8a029d8213e55626467c0066bd06f191a504c))
* **core:** leave ignored regions out of the comparison ([dc6cbec](https://github.com/ebiryu/pixeldelta/commit/dc6cbecc9958392cdce3e356a113a122ea69a989))
* **core:** exclude pixels that differ only by anti-aliasing ([3ab984d](https://github.com/ebiryu/pixeldelta/commit/3ab984d118a6d4501dc64c3a2e5e4280466f8c02))
* **core:** blend semi-transparent pixels on a checkerboard background ([dbc2378](https://github.com/ebiryu/pixeldelta/commit/dbc2378e330dafae83110e96c421bef13385a469))
* **core:** compare RGBA8 images with a scalar YIQ metric ([0b9fa42](https://github.com/ebiryu/pixeldelta/commit/0b9fa42586a183393bf6b99c7716e238a9a7d722))

### Bug Fixes

* **ci:** stop the publish job from creating a GitHub release ([b3a81e7](https://github.com/ebiryu/pixeldelta/commit/b3a81e73fbc36629408db2b4f51c1ce680c751a1))
* **ci:** declare the wasm runtime packages for the WASI build ([6fbf003](https://github.com/ebiryu/pixeldelta/commit/6fbf00372e70e2a5c30ec8644dd63891c0b68353))
* **ci:** build the Linux targets without Docker ([e12c836](https://github.com/ebiryu/pixeldelta/commit/e12c8362984f0c25381427b9498d347a64f5e45a))
* **ci:** drop the removed --skip-gh-release flag from prepublish ([8285cfb](https://github.com/ebiryu/pixeldelta/commit/8285cfbe3fb77f34bedd0eb2299667954e6fccad))
* **core:** treat a threshold of NaN as the most sensitive setting ([314b842](https://github.com/ebiryu/pixeldelta/commit/314b8429d56a37a0c848b799b1c5dc593bb7623c))

### Performance Improvements

* **core:** skip chunks of eight pixels that are byte for byte equal ([984f47c](https://github.com/ebiryu/pixeldelta/commit/984f47c5849ebe2981b0115c02353f1682ad1b17))
* **core:** scan row blocks in parallel with rayon ([2fe151f](https://github.com/ebiryu/pixeldelta/commit/2fe151fb63fc1843118430f284905d6240013157))

