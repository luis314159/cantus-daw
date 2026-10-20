# Isolated plug-ins: what they cost (round 29, ADR 0063)

`cargo build --release -p cantus-sandbox -p cantus-bench`, then
`target/release/cantus-bench --full --only isolation --json <file>`. The group builds the
test gain (`cantus-test-clap`) and Cantus's own plug-ins as CLAP (`cantus-plugins`) in
release into `target/bench-plugins`, and starts this build's `cantus-sandbox` (or
`CANTUS_SANDBOX_EXE`). It is never part of the quick gate.

Machine: the i7-12700H the round was built on (6 P + 8 E cores), Windows 11, shared with
two other builders' compilations — absolute times are noisy; the ratios and the order of
magnitude are the result. One run per figure.

## Run 1 — one helper thread per instance, a 300 µs spin after each job

| Metric | 64 | 128 | 256 |
|---|---|---|---|
| Together, hot round trip (µs, median / p99) | 0.5 / 0.8 | 0.7 / 0.9 | 1.1 / 1.2 |
| Together, paced round trip | 15.4 / 73.0 | 24.5 / 54.5 | 24.5 / 424 |
| Individually, paced round trip | 13.6 / 41.4 | 24.4 / 55.6 | 24.0 / 410 |
| Together, helpers' CPU per instance (% of a core) | 23.7 | 13.4 | 6.7 |
| Individually, helpers' CPU per instance | 22.0 | 10.9 | 5.0 |

32 × Comprimo CLAP at 128 frames: native 525 µs, within 527, Together 924 (1.75×),
Individually 992 (1.88×).

## Run 2 — the lobby: a thread pool per helper, a 50 µs spin per burst (shipped)

| Metric | 64 | 128 | 256 |
|---|---|---|---|
| Within, hot round trip (µs, median / p99) | 0.3 / 0.3 | 0.5 / 0.6 | 0.8 / 1.0 |
| Together, hot round trip | 0.6 / 1.1 | 0.7 / 1.2 | 1.2 / 1.7 |
| Individually, hot round trip | 0.6 / 1.1 | 0.7 / 0.8 | 1.2 / 1.7 |
| Within, paced round trip | 1.6 / 6.5 | 3.1 / 10.3 | 5.5 / 13.6 |
| Together, paced round trip | 11.7 / 54.8 | 36.5 / 52.3 | 34.0 / 276 |
| Individually, paced round trip | 14.2 / 44.2 | 26.0 / 262 | 41.6 / 61.8 |
| Together, helpers' CPU per instance (% of a core) | 0.55 | 0.35 | 0.20 |
| Individually, helpers' CPU per instance | 5.8 | 2.4 | 1.5 |

32 × Comprimo CLAP at 128 frames (`isolation.dsp32.*`): native 563 µs, within 593
(1.05× native), **Together 773 (1.30× within)**, Individually 1 125 (1.90×).

Reading: *hot* is a further isolated plug-in or note segment in the same block; *paced*
is the first isolated job of a block, after the helper parked. ADR 0031's phase-6 budget
(a round trip ≤ 5 % of a 64-sample period per group, 67 µs) holds for *Together* (11.7 µs
median, 54.8 µs p99). Its 1.2× target for 32 instances is not met in the serial engine
(1.30×): about 5.6 µs per instance per block of handing audio between cores.
