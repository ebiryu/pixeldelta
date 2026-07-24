# smoke

Checks that the published npm layout installs and loads in a project that has
nothing else. This is the M4 completion condition from `docs/design.md` 8: an
empty project runs `npm i` and the package works.

## Local check

```bash
just smoke
```

`run.sh` builds the addon for the host, packs the root package
(`packages/pixeldelta`) and the host's platform package (`npm/<host>`) into
tarballs as they would be published, extracts them into a flat `node_modules`
the way npm installs them, and runs `smoke.test.mjs`, which loads the package
by name and compares a fixture pair.

It lays the tarballs out by hand rather than through a package manager, so the
check needs no network and no registry. It exercises the tarball contents and
the runtime resolution in `index.js` (root package to platform package to
binary), which fail the same way on every platform. It does not exercise npm's
own `os`/`cpu` selection of the platform package.

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
```

A working install pulls exactly one `pixeldelta-<platform>` package for the
current OS and CPU. Run it on each OS the matrix builds for.
