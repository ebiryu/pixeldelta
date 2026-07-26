# smoke

Checks that the published npm layout installs and loads in a project that has
nothing else. This is the M4 completion condition from `docs/design.md` 8: an
empty project runs `npm i` and the package works.

## Local check

```bash
just smoke
```

`run.sh` builds the addon and the command-line executable for the host, packs
the root package (`packages/pixeldelta`) and the host's platform package
(`npm/<host>`) into tarballs as they would be published, and checks both halves
of what the package offers.

1. `smoke.test.mjs` loads the package by name and compares a fixture pair. The
   tarballs are extracted into a flat `node_modules` by hand, as npm lays them
   out, so this stage needs no package manager at all. It exercises the tarball
   contents and the runtime resolution in `index.js` (root package to platform
   package to binary).
2. `cli.test.mjs` runs the executable through `pnpm run`, so the `bin` link
   under test is the one an install created. It checks that a package script
   reaches the launcher, that the exit code of a comparison survives, and that
   the executable arrived runnable.

Both stages read local tarballs, so neither needs a network or a registry.

The tarballs are packed with `npm`, matching what a release runs. The client is
not interchangeable: `pnpm pack` writes every file as `0644`, which would leave
the executable unrunnable after install.

It does not exercise npm's own `os`/`cpu` selection of the platform package.

Only the host platform is covered locally. The other targets are built by the
matrix in `.github/workflows/ci.yml`.

## After a release

The local check skips two things a real install does: npm's platform selection,
and the `optionalDependencies` that `napi prepublish` writes into the published
root package. Confirm both against the registry once a version is published:

```bash
mkdir /tmp/pixeldelta-install && cd /tmp/pixeldelta-install
npm init -y
npm install pixeldelta
node -e "const {compareSync} = require('pixeldelta'); \
  console.log(compareSync(process.argv[1], process.argv[1]).verdict)" some.png
npx pixeldelta --version
```

A working install pulls exactly one `pixeldelta-<platform>` package for the
current OS and CPU. Run it on each OS the matrix builds for.
