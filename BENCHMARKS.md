# Benchmarks

One entry per run of `just bench-record`, newest first. Past entries are
never rewritten.

Entries taken on different machines are not comparable: each entry names its
machine, and only entries with the same machine line can be read against each
other.

To reproduce the numbers: `tools/bench/README.md` for the comparison against
pixelmatch and odiff, `crates/pixeldelta-core/benches` for the engine on its
own.

## 0.2.3 — 2026-08-02

- commit: 467ea36 plus uncommitted changes
- machine: Apple M1, 8 cores, macOS (arm64)
- toolchain: rustc 1.97.1 (8bab26f4f 2026-07-14), release build
- fixtures: crates/pixeldelta-core/benches/fixtures, threshold 0.1

Engine, decoded buffers, median of 20 criterion samples, in ms:

| size | identical | aa-off | aa-on | layout-shift |
| --- | ---: | ---: | ---: | ---: |
| 2mpx | 0.103 | 0.399 | 1.984 | 20.757 |
| 8mpx | 0.691 | 1.616 | 7.707 | 49.759 |
| 18mpx | 1.527 | 3.638 | 17.703 | 92.936 |

Against pixelmatch, decoded buffers, median of 50 runs, in ms:

| size | aa | pixeldelta | pixelmatch | pixeldelta diff | pixelmatch diff |
| --- | --- | ---: | ---: | ---: | ---: |
| 2mpx | off | 0.515 | 9.461 | 236698 | 236698 |
| 2mpx | on | 1.842 | 21.553 | 190034 | 190034 |
| 8mpx | off | 1.539 | 26.892 | 969741 | 969741 |
| 8mpx | on | 7.047 | 72.366 | 768167 | 768167 |
| 18mpx | off | 3.144 | 59.891 | 2159024 | 2159024 |
| 18mpx | on | 15.695 | 161.804 | 1715159 | 1715159 |

End to end, two files to a verdict, decode included, no diff image, median of 10 process runs, in ms:

| size | aa | pixeldelta | pixelmatch | odiff | odiff diff |
| --- | --- | ---: | ---: | ---: | ---: |
| 2mpx | off | 29.2 | 155.6 | 31.6 | 236698 |
| 2mpx | on | 30.2 | 168.9 | 41.4 | 189268 |
| 8mpx | off | 104.0 | 454.8 | 115.3 | 969741 |
| 8mpx | on | 110.5 | 505.6 | 154.4 | 764600 |
| 18mpx | off | 228.9 | 1016.9 | 255.4 | 2159024 |
| 18mpx | on | 241.6 | 1058.5 | 341.1 | 1707610 |

Startup floor on a 1-pixel pair, in ms: pixeldelta 2.8, pixelmatch 38.7, odiff 4.1.
