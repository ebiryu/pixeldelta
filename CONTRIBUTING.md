# Contributing to pixeldelta

pixeldelta compares two PNGs and reports which pixels differ. Its diff pixel
counts match pixelmatch's, which makes them part of what the package promises
rather than an implementation detail. Several of the rules below exist to keep
them from moving by accident.

## Scope

Bug reports, failing fixtures, performance work and documentation fixes are all
welcome. Before writing code for a new feature, open an issue: some things are
left out on purpose, and the list below is not exhaustive.

### What pixeldelta does not do

These are left out by decision, not for want of time, so a pull request
implementing one starts from a disagreement about scope. Open an issue first if
you think the reasoning behind one no longer holds.

- **Input formats other than PNG.** The tools that take screenshots emit PNG,
  and every added format lands in the compile time and binary size that every
  consumer downloads.
- **Comparing only the overlapping region of two differently sized images.**
  Which origin to align, and how to report the area that was not compared, are
  both undecided.
- **A package with reg-cli's `compare()` surface.** What such a package could
  return is bounded by the `reg.json` schema, which has no place for clusters,
  and following another project's surface puts the maintenance scope in its
  hands. `pixeldelta ci` replaces a reg-suit setup instead, as
  [the README](README.md#coming-from-reg-suit) describes.
- **A plugin mechanism.**
- **Storage other than S3 compatible object storage and local directories, or a
  forge other than GitHub.**

## Getting set up

### Prerequisites

- **Rust**, stable, installed through [rustup](https://rustup.rs). Some recipes
  install another toolchain themselves, which a distribution package cannot do.
- **Node**, the version in [.node-version](.node-version).
- **pnpm**, the version in the `packageManager` field of
  [package.json](package.json). `corepack enable` picks it up.
- **[just](https://github.com/casey/just)**, which holds the commands below.

Three more are needed only for particular work: `cargo-deny` after adding a
dependency, `cargo-insta` to accept a changed report snapshot, and `cargo-fuzz`
to run the fuzz target.

### First build

```bash
just check
```

That is what has to pass before a commit: rustfmt, clippy with warnings as
errors, and the workspace tests. Running `just` on its own lists every recipe
with what it is for.

The Node addon is left out of `just check`. It is a cdylib against the N-API
symbols Node resolves at load time, so linking it into a test executable fails
off macOS. `just node` builds it and runs its tests against Node instead.

## Repository layout

| Path | What is in it |
| --- | --- |
| `crates/pixeldelta-core` | The comparison engine. Performs no I/O; takes decoded RGBA8. |
| `crates/pixeldelta-io` | PNG decode and encode. |
| `crates/pixeldelta-report` | HTML, JSON, JUnit and Markdown output. |
| `crates/pixeldelta-cli` | The executable. |
| `crates/pixeldelta-node` | The napi-rs binding. |
| `packages/pixeldelta` | The npm package and the per-platform packages. |
| `tools/` | Fixture generation, benchmarks, the packaging smoke test, release scripts. |
| `fuzz/` | The decoder fuzz target. A workspace of its own, since libfuzzer-sys builds only on nightly. |

Why a piece of the engine is written the way it is belongs in its rustdoc, as a
statement of what the code does now. Why it changed belongs in the commit
message.

## Running the tests

| Kind | Where | What it holds |
| --- | --- | --- |
| Unit | `#[cfg(test)] mod tests` in the same file | Can reach private items. |
| Integration | `crates/<crate>/tests/` | The public API only. |
| Compatibility | `crates/pixeldelta-core/tests/compat.rs` | Diff pixel counts against pixelmatch. |
| Property | `crates/pixeldelta-core/tests/properties.rs` | Invariants over generated images, through proptest. |
| Snapshot | `crates/pixeldelta-report/tests/` | Report output, through insta. `cargo insta review` accepts a change. |
| Benchmark | `crates/pixeldelta-core/benches/` | criterion, at 2, 8 and 18 Mpixel. |
| Fuzz | `fuzz/` | `just fuzz 600` runs the decoder target for ten minutes. |

Fixture images are committed rather than generated at test time, so a past
measurement stays comparable with a present one.

## Making a change

The steps differ by whether the change is visible to a caller, so decide that
first.

### Adding or changing behavior

1. **Write the test and run it before the implementation exists.** A test
   written afterwards can pass without checking the thing it is named for, and
   watching it fail is the only moment that separates the two. A test that
   never reads the threshold or the anti-aliasing flag looks like one that does.
2. **Write the shortest implementation that passes.** Speed is not a concern
   here. The straightforward version is what an optimized one is later checked
   against.
3. **Refactor with the tests green.**

### Changing performance

An optimization does not change what a caller sees, so there is no new test to
write for it — which means the tests that fix the behavior have to exist
already. Add them first if they do not; they pass from the start.

1. `just bench` for a baseline. criterion keeps the previous run under
   `target/criterion` and compares against it.
2. Implement the change.
3. `just check`. A diff pixel count that moved by one is a change in behavior,
   not an optimization.
4. `just bench` again, and put the numbers in the commit message together with
   the conditions they were taken under: image size, threshold, whether
   anti-aliasing detection was on, and the thread count. A number without them
   cannot be compared against a later one.

Do not commit an optimization that was not measured. Under about 5%, weigh the
gain against what the change costs to read, and drop it if it costs anything.

`just bench-record` runs criterion and the comparison against pixelmatch and
odiff, then inserts one entry at the top of [BENCHMARKS.md](BENCHMARKS.md). Two
entries are comparable only if they were taken on the same machine.

## What a change must not break

**The pixelmatch counts.** `crates/pixeldelta-core/tests/compat.rs` pins the
diff pixel count for each fixture. Those counts are what a threshold means to
someone who chose it against pixelmatch. `just fixtures` regenerates the
fixtures and their expected counts, and regenerating them so they match a change
is how the guarantee is lost. If a count should move, give the reason in the
pull request before the numbers change.

**MSRV 1.88.** Pinned by `rust-version` in the workspace manifest, checked by
`just msrv` and by CI. 1.88 is the first release where SIMD intrinsics can be
called without `unsafe`, which is what lets the engine stay free of it. Raising
the MSRV needs a reason beyond convenience.

**The absence of `unsafe`.** There is none in `crates/`. The safe calls in
`std::arch` and `portable-simd` cover what the engine needs. If a change needs
`unsafe`, the argument for its soundness goes in a comment directly above the
block; if that argument cannot be written down, the `unsafe` is not correct.

**No `panic!` or `unwrap()` in library code.** Error types are defined with
`thiserror`, split finely enough for a caller to branch on them. The binding
layer catches panics and raises them as JS exceptions, because a panic that
reaches Node takes the whole process down and leaves no error for the caller to
read.

**A small dependency graph.** `pixeldelta-core` depends on rayon and nothing
else. Adding a dependency means estimating how many lines it saves and writing
that estimate in the commit message. Run `just deny` afterwards for licenses and
advisories.

**Comments in English, describing only the current specification.** No planned
work, no note of what the code used to be. A plan written into a comment outlives
the decision it described and points the next reader at an old premise.

A SIMD path, if one is added, needs a test asserting it returns the same counts
as the scalar path, with the CPU feature in the test name. A runner without the
feature never enters the path and the test passes regardless, so the log has to
show which one ran.

## Commits and branches

[Conventional Commits](https://www.conventionalcommits.org), in English, with
the crate name as the scope.

```
perf(core): speed up comparison with an AVX2 path

criterion: 84ms -> 35ms (18Mpixel, threshold 0.1, AA off, 8 threads)
Add a test in compat.rs asserting the AVX2 path matches the scalar one.
```

One commit carries one purpose. Reformatting and a behavior change do not go in
the same one: a performance regression is found by bisecting, and a commit that
did two things gives an ambiguous answer.

Branch names start with the type of their main commit — `perf/simd-avx2`,
`fix/wasi-loader` — so a list of branches shows which is a feature and which is
a fix. `main` stays green; push work in progress to a branch.

## Pull requests

1. `just check` passes.
2. Run whichever recipe matches the work: `just deny` after adding a dependency,
   `just bench` for a performance change, `just node` for the binding or the npm
   package, `just node-wasi` for anything the WebAssembly build reaches,
   `just smoke` for the published package layout, `just msrv` if a recent
   language or library feature may have slipped in.
3. Describe what the change does and why. For a change to the public API or to
   the counts in `compat.rs`, open an issue before writing the code.

CI runs formatting, clippy, the workspace tests, a build at the MSRV, a check
that every manifest carries the same version, a one-minute fuzz run, one pass of
the benchmarks, the addon build for every published target, and an install of
the packed package into an empty project.

## Releasing

A release is built from the commit messages, so the type and scope on a commit
decide both what version comes next and what the changelog says.

A push to `main` runs `.github/workflows/release.yml`, which opens or rewrites a
pull request titled `release: v<version>`. That pull request carries the new
version in `Cargo.toml`, `Cargo.lock` and every npm manifest, together with the
changelog entry built from the commits since the last tag. `feat` raises the
minor version and anything else the patch; while the version is below 1.0.0, a
`BREAKING CHANGE` footer raises the minor rather than the major. The changelog
lists `feat`, `fix`, `perf` and `build`, and anything scoped `deps` under
Dependencies; the remaining types are left out of it and still take part in the
version bump.

Merging that pull request creates the tag `v<version>` and the GitHub release.
The tag starts the publish job in `.github/workflows/ci.yml`, which builds every
published target and publishes to npm over OIDC. Until the pull request is
merged there is no tag, and nothing reaches npm.

The GitHub release therefore appears before npm carries the version, because the
tag the publish job runs on is created along with the release. A publish that
fails leaves a release for a version npm does not have; re-running the failed
job on that tag is what resolves it.

## Reporting a bug

Include the two images or a reduction of them, the command or the call that was
made, the output of `pixeldelta --version`, and the platform. A difference in
diff pixel count against another tool is worth reporting with both counts.

For a security issue, use GitHub's private vulnerability reporting rather than a
public issue.

## License

MIT, as in [LICENSE](LICENSE). A contribution is offered under the same terms.
