#!/usr/bin/env bash
# Verifies that the published package layout installs and loads in a project
# that has nothing else.
#
# It builds the addon for the host, packs the root package and the host's
# platform package into tarballs as they would be published, extracts them into
# a flat node_modules the way npm installs them, and runs a smoke test that
# loads the package by name. A pass means the root package resolves its
# platform package and the binary runs.
#
# The tarballs are laid out by hand rather than installed with a package
# manager so the check needs no network and no registry: it exercises the
# tarball contents and the runtime resolution in index.js, which fail the same
# way on every platform. npm's own os/cpu selection of the platform package is
# covered by the build matrix in CI and by an `npm i` against the registry
# after a release.
#
# Only the host platform is exercised here.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
pkg="$root/packages/pixeldelta"
smoke="$root/tools/smoke"

echo "Building the addon for the host..."
pnpm --filter pixeldelta build

node_file=$(ls "$pkg"/pixeldelta.*.node | head -1)
tag=$(basename "$node_file" | sed -E 's/^pixeldelta\.(.*)\.node$/\1/')
echo "Host platform package: pixeldelta-$tag"

# The platform package ships the binary; place the built one beside its
# package.json so it lands in the tarball.
cp "$node_file" "$pkg/npm/$tag/"

work=$(mktemp -d "${TMPDIR:-/tmp}/pixeldelta-smoke.XXXXXX")
trap 'rm -rf "$work"' EXIT

echo "Packing the tarballs..."
pnpm --filter pixeldelta pack --pack-destination "$work" >/dev/null
( cd "$pkg/npm/$tag" && pnpm pack --pack-destination "$work" >/dev/null )

# Extract into a flat node_modules, as npm would install them. Each tarball
# holds its files under a top-level `package/` directory.
modules="$work/app/node_modules"
mkdir -p "$modules/pixeldelta" "$modules/pixeldelta-$tag"
tar -xzf "$work"/pixeldelta-0.0.0.tgz -C "$modules/pixeldelta" --strip-components=1
tar -xzf "$work/pixeldelta-$tag-0.0.0.tgz" -C "$modules/pixeldelta-$tag" --strip-components=1

app="$work/app"
cp "$smoke/smoke.test.mjs" "$app/"
# A fixture pair from the core baseline, so the count is known.
cp "$root/crates/pixeldelta-core/tests/fixtures/blocks/base.png" "$app/"
cp "$root/crates/pixeldelta-core/tests/fixtures/blocks/head.png" "$app/"

echo "Running the smoke test against the installed layout..."
cd "$app"
node --test "smoke.test.mjs"
echo "Smoke test passed for pixeldelta-$tag."
