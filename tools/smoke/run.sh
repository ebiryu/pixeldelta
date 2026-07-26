#!/usr/bin/env bash
# Verifies that the published package layout installs, loads and runs in a
# project that has nothing else.
#
# It builds the addon and the executable for the host, packs the root package
# and the host's platform package into tarballs as they would be published, and
# checks both halves of what the package offers:
#
#   1. `require('pixeldelta')` resolving through to the addon. The tarballs are
#      extracted into a flat node_modules by hand, as npm lays them out.
#   2. `pnpm run` reaching the executable through the bin entry. Here a package
#      manager does the installing, because the link under test is the one it
#      creates.
#
# Neither stage needs a network or a registry: the tarballs are local files.
# Two things a published install does are still not covered, and belong to the
# after-release check in README.md: npm's own os/cpu selection of the platform
# package, and the optionalDependencies that `napi prepublish` writes into the
# root package. Stage 2 stands in for the second by naming both tarballs as
# direct dependencies.
#
# Only the host platform is exercised here.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
pkg="$root/packages/pixeldelta"
smoke="$root/tools/smoke"
fixtures="$root/crates/pixeldelta-core/tests/fixtures/blocks"

echo "Building the addon and the executable for the host..."
pnpm --filter pixeldelta build
pnpm --filter pixeldelta build:cli

node_file=$(ls "$pkg"/pixeldelta.*.node | head -1)
tag=$(basename "$node_file" | sed -E 's/^pixeldelta\.(.*)\.node$/\1/')
echo "Host platform package: pixeldelta-$tag"

# The platform package ships the binary; place the built one beside its
# package.json so it lands in the tarball. `build:cli` has already placed the
# executable there.
cp "$node_file" "$pkg/npm/$tag/"

work=$(mktemp -d "${TMPDIR:-/tmp}/pixeldelta-smoke.XXXXXX")
trap 'rm -rf "$work"' EXIT

# Packed with npm because that is what a release runs: the publish job calls
# `npm publish` for the root package, and `napi prepublish` calls it for each
# platform package. The client matters here. `pnpm pack` writes every file as
# 0644, which drops the executable bit the command-line binary needs.
echo "Packing the tarballs..."
( cd "$pkg" && npm pack --loglevel=warn --pack-destination "$work" >/dev/null )
( cd "$pkg/npm/$tag" && npm pack --loglevel=warn --pack-destination "$work" >/dev/null )

# Extract into a flat node_modules, as npm would install them. Each tarball
# holds its files under a top-level `package/` directory.
modules="$work/app/node_modules"
mkdir -p "$modules/pixeldelta" "$modules/pixeldelta-$tag"
tar -xzf "$work"/pixeldelta-0.0.0.tgz -C "$modules/pixeldelta" --strip-components=1
tar -xzf "$work/pixeldelta-$tag-0.0.0.tgz" -C "$modules/pixeldelta-$tag" --strip-components=1

app="$work/app"
cp "$smoke/smoke.test.mjs" "$app/"
# A fixture pair from the core baseline, so the count is known.
cp "$fixtures/base.png" "$fixtures/head.png" "$app/"

echo "Loading the package from the extracted layout..."
( cd "$app" && node --test "smoke.test.mjs" )

# Stage 2. A package manager installs the same tarballs, so node_modules/.bin
# holds the link it made from the root package's bin entry, and `pnpm run`
# resolves the executable the way a consumer's script does.
cli="$work/cli"
mkdir -p "$cli"
cp "$smoke/cli.test.mjs" "$cli/"
cp "$fixtures/base.png" "$fixtures/head.png" "$cli/"
cat > "$cli/package.json" <<EOF
{
  "name": "pixeldelta-cli-smoke",
  "private": true,
  "scripts": {
    "version": "pixeldelta --version",
    "compare-same": "pixeldelta compare base.png base.png",
    "compare-differ": "pixeldelta compare base.png head.png"
  },
  "dependencies": {
    "pixeldelta": "file:$work/pixeldelta-0.0.0.tgz",
    "pixeldelta-$tag": "file:$work/pixeldelta-$tag-0.0.0.tgz"
  }
}
EOF

echo "Installing the tarballs and running the executable through pnpm run..."
( cd "$cli" && pnpm install --ignore-workspace --silent && node --test "cli.test.mjs" )

echo "Smoke test passed for pixeldelta-$tag."
