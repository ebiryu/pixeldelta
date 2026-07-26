# Task runner for pixeldelta. Run `just` to list the recipes.
#
# `just check` is what has to pass before a commit. The recipes below it apply
# to particular kinds of work.

_default:
    @just --list

# Everything that has to pass before a commit.
check: fmt-check lint test

# Reject formatting that differs from rustfmt's output.
fmt-check:
    cargo fmt --all -- --check

# Apply rustfmt.
fmt:
    cargo fmt --all

# Lint, treating warnings as errors.
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run the unit, integration, compatibility and property tests.
#
# The Node addon is left out: it is a cdylib against the N-API symbols Node
# resolves at load time, so linking it into a test executable fails off macOS.
# `just node` builds and tests it against Node instead.
test:
    cargo test --workspace --exclude pixeldelta-node

# Build the Node addon and run its tests against Node. The crate lives in
# crates/pixeldelta-node; the npm package that wraps it is packages/pixeldelta.
#
# The command-line executable is built too, and placed into the host's platform
# package: the tests cover the launcher that a consumer's `pnpm run` reaches,
# and it runs the executable rather than standing in for it.
node:
    pnpm install
    pnpm --filter pixeldelta run build
    pnpm --filter pixeldelta run build:cli
    pnpm --filter pixeldelta test

# Pack the package and load it from an empty project, to check the published
# layout resolves on the host. See tools/smoke/README.md.
smoke:
    bash tools/smoke/run.sh

# Check dependency licenses and security advisories. Run after adding one.
deny:
    cargo deny check

# Build at the MSRV, which the workspace pins to 1.88.
msrv:
    rustup toolchain install 1.88 --profile minimal
    cargo +1.88 check --workspace --all-targets

# Measure performance. Compare against target/criterion from before a change.
bench:
    cargo bench

# Regenerate the compatibility fixtures and the pixelmatch counts they are
# checked against. A changed count in expected.txt moves the baseline, so read
# the diff before committing it.
[doc("Regenerate the compatibility fixtures and their pixelmatch counts.")]
fixtures:
    cd tools/fixtures && pnpm install && node generate.mjs

# Regenerate the benchmark fixtures. Every past measurement was taken against
# the committed images, so running this discards the basis for comparison.
bench-fixtures:
    cd tools/fixtures && pnpm install && node bench.mjs

# Print the version, or the manifests that disagree on it.
version:
    node tools/release/version.mjs

# Set the version everywhere, then commit and tag it. Takes the new version:
#
#     just release 0.1.0
#
# Pushing the tag is what releases: the publish job in .github/workflows/ci.yml
# runs on a tag beginning with v, and on nothing else. So this recipe stops at
# the tag, and the push stays a separate decision:
#
#     git push origin main --follow-tags
#
# The version this writes is the one `npm i pixeldelta` resolves and the one
# `pixeldelta --version` reports. tools/release/version.mjs lists the manifests
# it lands in.
[doc("Set the version everywhere, then commit and tag it.")]
release new_version:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "$(git status --porcelain)" ]; then
        echo "the working tree has uncommitted changes" >&2
        exit 1
    fi
    if git rev-parse -q --verify "refs/tags/v{{new_version}}" >/dev/null; then
        echo "the tag v{{new_version}} already exists" >&2
        exit 1
    fi
    node tools/release/version.mjs {{new_version}}
    # Workspace members carry their version into the lock file too.
    cargo update --workspace --offline
    git commit --all --message "release: v{{new_version}}"
    git tag "v{{new_version}}"
    echo "tagged v{{new_version}}; push it to publish:"
    echo "    git push origin main --follow-tags"
