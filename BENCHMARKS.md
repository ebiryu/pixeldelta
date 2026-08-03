# Benchmarks

One entry per run of `just bench-record`, newest first. Past entries are
never rewritten.

Entries taken on different machines are not comparable: each entry names its
machine, and only entries with the same machine line can be read against each
other.

To reproduce the numbers: `tools/bench/README.md` for the comparison against
pixelmatch and odiff, `crates/pixeldelta-core/benches` for the engine on its
own.

## 0.2.3 — 2026-08-03

- commit: 8d35141
- machine: Apple M1, 8 cores, macOS (arm64)
- toolchain: rustc 1.97.1 (8bab26f4f 2026-07-14), release build
- fixtures: crates/pixeldelta-core/benches/fixtures, threshold 0.1

Engine, decoded buffers, median of 20 criterion samples, in ms:

| size | identical | aa-off | aa-on | layout-shift |
| --- | ---: | ---: | ---: | ---: |
| 2mpx | 0.105 | 0.420 | 2.012 | 21.002 |
| 8mpx | 0.624 | 1.482 | 8.958 | 52.137 |
| 18mpx | 1.451 | 3.226 | 17.179 | 91.635 |

Against pixelmatch, decoded buffers, median of 50 runs, in ms:

| size | aa | pixeldelta | pixelmatch | pixeldelta diff | pixelmatch diff |
| --- | --- | ---: | ---: | ---: | ---: |
| 2mpx | off | 0.518 | 9.429 | 236698 | 236698 |
| 2mpx | on | 1.833 | 17.435 | 190034 | 190034 |
| 8mpx | off | 1.355 | 26.474 | 969741 | 969741 |
| 8mpx | on | 8.865 | 72.557 | 768167 | 768167 |
| 18mpx | off | 3.035 | 59.278 | 2159024 | 2159024 |
| 18mpx | on | 14.425 | 161.955 | 1715159 | 1715159 |

End to end, two files to a verdict, decode included, no diff image, median of 10 process runs, in ms:

| size | aa | pixeldelta | pixelmatch | odiff | odiff diff |
| --- | --- | ---: | ---: | ---: | ---: |
| 2mpx | off | 27.5 | 151.3 | 30.4 | 236698 |
| 2mpx | on | 29.1 | 177.1 | 41.3 | 189268 |
| 8mpx | off | 105.8 | 471.9 | 117.5 | 969741 |
| 8mpx | on | 108.1 | 506.1 | 151.6 | 764600 |
| 18mpx | off | 222.7 | 939.7 | 247.4 | 2159024 |
| 18mpx | on | 236.7 | 1059.5 | 337.3 | 1707610 |

Startup floor on a 1-pixel pair, in ms: pixeldelta 2.1, pixelmatch 36.3, odiff 2.5.
