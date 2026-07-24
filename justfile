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
node:
    pnpm install
    pnpm --filter pixeldelta run build
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
