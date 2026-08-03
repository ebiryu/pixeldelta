# pixeldelta

Image comparison for visual regression testing, written in Rust and distributed
through npm. It decodes two PNGs and reports how many pixels differ
perceptually, using the same YIQ color metric and anti-aliasing detector as
pixelmatch, so an existing threshold keeps its meaning. Beyond a pixel count it
groups the differing pixels into clusters and estimates how far each one moved,
which separates a shifted element from a change spread across the screen.

pixeldelta ships three things.

- **A Node library.** `import { compare } from 'pixeldelta'` — two paths or two
  buffers to a verdict, a diff pixel count, and clusters. Each platform gets a
  prebuilt binary; a host that no prebuild matches runs the same engine as
  WebAssembly, in Node or in a browser. The API and the WebAssembly differences
  are in [packages/pixeldelta/README.md](packages/pixeldelta/README.md).
- **A command line.** `pixeldelta compare` for two images and `pixeldelta run`
  for two directories, with HTML, JSON and JUnit output. The exit code carries
  the verdict.
- **A CI workflow.** `pixeldelta ci` finds the baseline commit from the git
  history, fetches its snapshot from object storage, compares, publishes the
  report, and writes the result to a pull request comment or a job summary.

Input is PNG. Other formats are not decoded.

## Install

```sh
npm install pixeldelta
```

The prebuilds cover macOS and Linux on x64 and arm64, Linux on musl at x64, and
Windows on x64. The command comes with them. On a host that none of them match,
the library still runs through `npm install pixeldelta pixeldelta-wasm`; the
command does not, because it runs `git` and opens network connections, and WASI
has no sockets.

## Speed

Measured on an Apple M1 with 8 cores, macOS on arm64, rustc 1.97.1, release
build, at threshold 0.1 against the fixtures in
`crates/pixeldelta-core/benches/fixtures`. Numbers from another machine are not
comparable with these.

**Engine only** — decoded RGBA buffers to a diff pixel count, median of 50 runs,
in ms. pixeldelta and pixelmatch return the same count on every row.

| size | anti-aliasing | pixeldelta | pixelmatch |
| --- | --- | ---: | ---: |
| 2 Mpixel | off | 0.518 | 9.429 |
| 2 Mpixel | on | 1.833 | 17.435 |
| 8 Mpixel | off | 1.355 | 26.474 |
| 8 Mpixel | on | 8.865 | 72.557 |
| 18 Mpixel | off | 3.035 | 59.278 |
| 18 Mpixel | on | 14.425 | 161.955 |

**End to end** — two PNG files to a verdict, decode included, no diff image, as
the wall-clock time of the process, median of 10 runs, in ms. Decode dominates
at these sizes, so this measures the decoder as much as the engine. The startup
floor on a one-pixel pair is 2.1 ms for pixeldelta, 36.3 ms for pixelmatch and
2.5 ms for odiff; subtract it to read the comparison itself.

| size | anti-aliasing | pixeldelta | pixelmatch | odiff |
| --- | --- | ---: | ---: | ---: |
| 2 Mpixel | off | 27.5 | 151.3 | 30.4 |
| 2 Mpixel | on | 29.1 | 177.1 | 41.3 |
| 8 Mpixel | off | 105.8 | 471.9 | 117.5 |
| 8 Mpixel | on | 108.1 | 506.1 | 151.6 |
| 18 Mpixel | off | 222.7 | 939.7 | 247.4 |
| 18 Mpixel | on | 236.7 | 1059.5 | 337.3 |

odiff matches the same counts with detection off and differs by under half a
percent with it on, from its own anti-aliasing detector.

Every recorded run, with its machine and toolchain, is in
[BENCHMARKS.md](BENCHMARKS.md); the newest entry is the source of the tables
above. To take the measurement yourself, see
[tools/bench/README.md](tools/bench/README.md) for the comparison against
pixelmatch and odiff, and `crates/pixeldelta-core/benches` for the engine on its
own.

## Compare two images

```sh
pixeldelta compare base.png head.png --output diff.png
```

```text
differ: 190034 pixels (9.1644%)
```

`--threshold T` sets the color delta a pixel must exceed to count (default
`0.1`), `--no-antialiasing` counts anti-aliasing differences instead of
excluding them, and `--ignore-region X,Y,W,H` leaves a rectangle out of the
comparison and its ratio. No diff image is drawn or encoded unless `--output`
asks for one.

## Compare two directories

```sh
pixeldelta run ./expected ./actual --report ./report --json result.json
```

```text
fail: 1 changed, 0 added, 0 removed, 0 size mismatch, 0 tolerated, 1 matched
```

Both trees are walked recursively and their `.png` files are paired by relative
path. Each pair becomes one of six categories: matched, tolerated, changed,
sizeMismatch, added, removed.

`--report DIR` writes `DIR/index.html` with the images it references under
`DIR/images/`, so the directory opens as it is once carried elsewhere. `--json`
and `--junit` write a file each. Nothing is written unless asked for.

`--tolerance-ratio R` moves an entry whose differing pixels are at most `R` of
the image into tolerated, which keeps it out of the verdict while leaving its
diff pixel count, clusters and diff image in the report. The default is `0`, so
a single differing pixel is a change. `--max-clusters N` bounds how many
clusters one entry lists, largest first (default 100, `0` for all); the rest are
reported as a count.

## Exit codes

A CI step fails on a difference without reading the output.

| Code | `compare` | `run` and `ci` |
| --- | --- | --- |
| `0` | the images match | every entry matched or was tolerated |
| `1` | the images differ | an entry changed, was added, removed, or changed size |
| `2` | the sizes differ | not used |
| `3` | a runtime error | a runtime error |

## In CI

```sh
pixeldelta ci ./actual --storage s3://bucket/prefix --report ./report
```

`ci` runs the same comparison as `run`, against a baseline it works out itself:

1. `git merge-base HEAD <base>` gives the merge base. `<base>` comes from
   `--base-branch` and defaults to `main`; on a runner, pass a name that
   resolves there, such as `origin/main`.
2. It walks back from there, up to `--history-limit` commits (default 50), and
   takes the newest commit that has a snapshot stored.
3. It fetches that snapshot, compares `ACTUAL` against it, writes whichever
   reports were asked for, and stores `ACTUAL` under the current commit's SHA.

With no baseline it stores the snapshot and exits `0`; nothing to compare
against is not a regression, so the first run of a new setup is not a failure.

`--storage` selects where snapshots live: a string without a scheme is a local
directory, and `s3://bucket/prefix` goes to the S3 API, which also covers R2 and
MinIO. Credentials come from the environment — `AWS_ACCESS_KEY_ID`,
`AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` when the credentials are temporary,
`AWS_REGION` (default `us-east-1`), and `AWS_ENDPOINT_URL` to point at a service
other than AWS.

`--markdown FILE` appends the result as Markdown, and `--comment` posts the same
body as a pull request comment, replacing the one the previous run left rather
than adding to the thread. Both carry the category counts, the comparison
conditions, a link to the report when its URL is known, and a per-entry list of
what changed, including how the clusters split between moved and altered and the
lowest SSIM. Commenting reads `GITHUB_TOKEN`, `GITHUB_REPOSITORY` and
`GITHUB_API_URL`, and takes the pull request number from the workflow event
unless `--pr` gives it. A token from a forked pull request cannot comment; that
prints a warning and does not fail the run.

The report URL comes from the storage when it can serve one. When the workflow
publishes the report somewhere else, `--report-url` supplies the address that
goes into the comment.

```yaml
permissions:
  contents: read
  pull-requests: write

steps:
  - uses: actions/checkout@v7
    with:
      fetch-depth: 0 # the baseline search walks the history
  - run: npm ci
  - run: npm run screenshots # writes ./actual
  - run: >
      npx pixeldelta ci ./actual
      --storage s3://my-bucket/pixeldelta
      --base-branch origin/main
      --report ./report
      --markdown "$GITHUB_STEP_SUMMARY"
      --comment
    env:
      AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
      AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
      AWS_REGION: us-east-1
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Coming from reg-suit

What replaces a reg-suit setup is the `ci` subcommand. Its three plugin
boundaries — the baseline key, the storage, the notification — are all covered
by that one command.

- `reg-keygen-git-hash-plugin` → the baseline search above, keyed by commit SHA.
- `reg-publish-s3-plugin` → `--storage s3://bucket/prefix`, with
  `AWS_ENDPOINT_URL` for R2 and MinIO.
- `reg-notify-github-plugin` → `--comment` and `--markdown`, on a `GITHUB_TOKEN`
  with `pull-requests: write`. There is no GitHub App to install.
- `regconfig.json` thresholds → the flags above and
  [`pixeldelta.config.json`](#configuration-file).

Snapshots go to S3 compatible storage or to a local directory, and results are
reported to GitHub. No other storage or forge is supported.

## Configuration file

`run` and `ci` read a config file when the same threshold or the same excluded
region does not suit every screenshot. `--ignore-region` on the command line
applies to every image; the file is what changes a setting for some paths and
not others. `compare` does not read it, since a path pattern means nothing for a
single pair.

```json
{
  "threshold": 0.1,
  "toleranceRatio": 0,
  "ignoreRegions": [{ "x": 0, "y": 0, "width": 1280, "height": 64 }],
  "overrides": [
    {
      "paths": ["dashboard/**", "**/clock-*.png"],
      "toleranceRatio": 0.001,
      "ignoreRegions": [{ "x": 980, "y": 120, "width": 220, "height": 48 }]
    }
  ]
}
```

Those four keys are all the file accepts, and an unknown key is an error rather
than a setting that silently does nothing. `--config PATH` names the file;
without it, `pixeldelta.config.json` in the working directory is read when it is
there. A path given with `--config` that does not exist is an error. Parent
directories are not searched.

**Patterns.** `paths` is matched against the relative path used to pair the two
directories, with separators normalized to `/` so one pattern works on Windows
too. The syntax is three constructs, and every other character is a literal:

- `*`: zero or more characters, not crossing a `/`.
- `**`: zero or more whole path segments, when written as an entire segment
  (`dashboard/**`, `a/**/b.png`). Written inside a segment, as in `a**b`, it
  behaves as `*`.
- `?`: one character other than `/`.

There is no `{a,b}` or `[a-z]`.

**Precedence.** `threshold` and `toleranceRatio` are overwritten in the order:
default, top level of the file, command-line flag, matching `overrides` entry.
The narrowest scope wins, so an override beats a flag; among several matching
overrides, the last one written wins. `ignoreRegions` is unioned instead — the
flag, the top level and every matching override are all excluded — so a status
bar common to every screen and a region specific to one screen can be given
together.

## The `--json` output

The JSON report carries no schema version field. Its compatibility is the
version of pixeldelta itself:

- Adding a field is a compatible change and ships in a minor release. A reader
  is expected to ignore fields it does not know.
- Removing a field, changing its type, or changing what an existing name means
  happens only in a major release.

## License

MIT.
