# Task runner for pixeldelta. Run `just` to list the recipes.
#
# `just check` is what has to pass before a commit. The recipes below it apply
# to particular kinds of work and are listed in docs/implementation-guide.md.

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
test:
    cargo test --workspace

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
