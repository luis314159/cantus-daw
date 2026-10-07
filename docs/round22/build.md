# Round 22 — build: the x86-64-v3 release, its v2 fallback, and the launcher

Builder: **build**, branch `r22/build` (from `c8bc9f2`). ADR 0031 §5.3 (amended below with
what was measured), ADR 0032 §1–§2 amended (the launcher and the programs in the one
Windows edition). `docs/perf/plan.md` Phase 3, row *build*.

## What changed, in one paragraph

A Windows release now carries Cantus **twice** — `cantus-x86-64-v3.exe` (AVX, AVX2, FMA,
BMI1/2, F16C, LZCNT, MOVBE: Intel since Haswell 2013, AMD since Excavator 2015) and
`cantus-x86-64-v2.exe` (SSE4.2, POPCNT, SSSE3) — and `cantus.exe` is a **launcher**: a
small program built for the plain x86-64 baseline that asks the processor what it has and
starts the highest level whose program is beside it, with the same arguments, returning
its exit code. `xtask package --v4` adds `cantus-x86-64-v4.exe` (AVX-512). Everything that
starts Cantus (Start menu, desktop, `.cantus` association, "Open with", the finish page,
the updater's `/RELAUNCH`) still names `cantus.exe`; the installer and the portable zip keep
the asio builder's single edition and file names, so the updater is untouched. A CI job
builds the null corpus at v2, v3 and the plain baseline and requires the same bits.

## 1. The launcher (`crates/cantus-launch`)

| Piece | What |
|---|---|
| `src/lib.rs` | `Level` (V2, V3, V4): name (`x86-64-v3`), program (`cantus-x86-64-v3.exe`), `rustflags()` (`-C target-cpu=x86-64-v3`), `features()` — **exactly** what `rustc --print cfg -C target-cpu=<level>` enables (a test runs rustc and compares, so a future rustc that adds a feature to a level fails the test instead of shipping a program the launcher would start on a processor without it); `of_this_build()` (the level a crate was compiled for, from `cfg!(target_feature)`); `DETECT`, one `is_x86_feature_detected!` per feature (the OS's AVX/AVX-512 state included); `choose(dir, has, exists, forced)` → `Choice` or `Refusal::{CpuTooOld, NoProgram}`; the refusal texts in English then Spanish; `info()` for `--launcher-info`. `#![deny(unsafe_code)]`. |
| `src/main.rs` | Windows subsystem in release (no console flashes); `Command::new(program).args(args).status()` then `process::exit(code)` — all 32 bits of the code; on a refusal or a failed start, the reason on stderr **and** in a `MessageBoxW` (the one `unsafe`, with its `SAFETY` line). `--launcher-info` is the launcher's own. |
| `src/appid.rs` | The AppUserModelID `Cantus.Cantus` (§3). |
| `build.rs` | Cantus's icon and version resource (FileDescription "Cantus"), from the app's icon code, now shared: `crates/cantus-app/icon.rs`, included by both build scripts. |

**Why wait for the program instead of starting it and leaving.** The installer's
"wait until Cantus is closed" (an update starts as the app quits) checked `cantus.exe`; a
script piping `cantus --audio-drivers` (the package task does) reads the program's output
through the launcher's handles; and the exit code reaches whoever started Cantus. The
waiting launcher is one idle process (a 335 KB program, 10 MB working set measured). The installer now waits for **every**
program of the folder anyway (`CANTUS_PROGRAMS_BUSY`, generated from the staged list), so
an update cannot replace `cantus-x86-64-v3.exe` while it is still closing.

**`CANTUS_CPU=v2`** starts a lower level (to compare builds, or to step around a fault
in one); a higher one than the processor runs is refused with a note — it would stop on the
first instruction the processor lacks. **`cantus.exe --launcher-info`** prints the
processor's level, what it lacks for the next one, the programs present and the one that
would start: a line for the hardware-report issue template (beta-docs).

On this laptop (i7-12700H): `processor: x86-64-v3`, `lacks for x86-64-v4: avx512bw,
avx512cd, avx512dq, avx512f, avx512vl`.

## 2. The release (`xtask package`)

`levels.rs` builds each level in a target folder of its own (`target/cpu-x86-64-v3/`) with
`RUSTFLAGS=-C target-cpu=<level>` **and an explicit `--target <host triple>`**: with
`--target`, cargo gives `RUSTFLAGS` to the program only, not to build scripts and
proc-macros, which run on the build machine — so a v4 program can be built on this laptop,
which has no AVX-512 (without `--target`, every build script of the v4 build would die on
an illegal instruction). The launcher builds in the ordinary target with no `target-cpu`.

The install folder (and the portable zip's top folder):

| File | What |
|---|---|
| `cantus.exe` | the launcher (built for x86-64) |
| `cantus-x86-64-v2.exe` | the program, x86-64-v2 |
| `cantus-x86-64-v3.exe` | the program, x86-64-v3 |
| `cantus-x86-64-v4.exe` | only with `package --v4` |
| `cantus-scan.exe` | the plug-in scanner (below: no release had it) |
| `LICENSE.txt`, `COPYRIGHT.txt`, `README.txt` | as before; the readme now says what the programs are (EN/ES) |

Checks before anything is written: each program's `--version` must name its level (the app
now prints `cantus 0.1.0 (x86-64-v3)` — proof the flags reached the build) and its
`--audio-drivers` must list ASIO; a level this machine cannot run is skipped with a note
(v4 here). Then the launcher, laid out beside the programs as released, must start the
right one (`--version` through the launcher names this machine's level) and list ASIO.
The plug-ins zip carries no `.exe`. `SHA256SUMS.txt` covers the same three files. The
file names come from `cantus_update::release` as before.

**Found on the way: the plug-in scanner was never shipped.** `cantus_host::Scanner` starts
`cantus-scan.exe` *beside the running program* ("where Cargo and the installer put it"), but
`cargo build --release -p cantus` does not build another package's binary, and the staging
list had only `cantus.exe`: every installed or portable Cantus of rounds 20–21 would have
reported each third-party plug-in as "cannot run the scanner". It is now built (with the
v2 flags) and staged.

**The real release, made here** (`cargo run -p xtask -- package`, the ASIO SDK from the
cache, NSIS 3.11, unsigned): both programs said `cantus 0.1.0 (x86-64-v2)` / `(x86-64-v3)`
and `audio drivers: WASAPI, ASIO`; the launcher laid out beside them started the v3 one and
listed ASIO; out came the same three files and the sums —
`Cantus-0.1.0-windows-x64-setup.exe` 35.2 MB (round 21: 25.7), `…-portable.zip` 78.1 MB
(64.4), `Cantus-0.1.0-plugins-windows-x64.zip` 52.9 MB. The second program costs ≈ 10 MB in
the installer and ≈ 14 MB in the zip (each program is 33.5 MB unpacked; the launcher 335 KB,
the scanner 497 KB). 7-Zip lists the installer's payload: `cantus.exe`,
`cantus-x86-64-v2.exe`, `cantus-x86-64-v3.exe`, `cantus-scan.exe`, the three texts. The
portable zip, unpacked into a temporary folder and run through its `cantus.exe`:
`--launcher-info` → x86-64-v3, both programs found; `--version` → `(x86-64-v3)`;
`CANTUS_CPU=v2` → `(x86-64-v2)`; `CANTUS_CPU=v4` → the note *"this processor lacks avx512bw,
… for x86-64-v4"* and v3; `--audio-drivers` → WASAPI, ASIO, the three installed drivers;
started plainly, the process tree was `cantus.exe` (10 MB) → `cantus-x86-64-v3.exe` with
the window "Cantus", and the portable `data\` folder appeared beside them. The installer
itself was not run (it would change the owner's Start menu and registry on a shared
machine); its script compiles, and its wait, shortcuts and IDs are tested (§6).

## 3. One taskbar button: the AppUserModelID

With a launcher, the Start-menu shortcut points to `cantus.exe` but the window belongs to
`cantus-x86-64-v3.exe`. Windows groups taskbar buttons and resolves pins by
**AppUserModelID**, which by default it derives from each executable's path — so a Cantus
pinned from the Start menu would show a second button while running, and pinning the
running window would pin the program past the launcher. Fixed both ends:

- the program calls `SetCurrentProcessExplicitAppUserModelID("Cantus.Cantus")` first thing
  in `main`, before any window (`cantus_launch::set_app_id`; test: set and read back);
- the installer gives the Start-menu and desktop shortcuts the same ID
  (`xtask/installer/appid.nsh`: IShellLinkW → IPersistFile::Load → IPropertyStore::SetValue
  (`PKEY_AppUserModel_ID`) → Commit → Save, with NSIS's System plug-in only — no third-party
  plug-in). The ID is written into `config.nsh` from `cantus_launch::APP_ID`, so the two
  cannot drift.

Proved here: a test installer built from `appid.nsh` gave a shortcut in a temporary folder
the ID, and the shell read it back (`System.AppUserModel.ID = Cantus.Cantus`, target
intact). Not proved on the real Start menu: installing would have changed the owner's
profile on a shared machine. *To check by hand after installing:* pin Cantus from Start,
start it, and see one button.

## 4. v2 = v3, to the bit (`xtask null-hashes`, the CI job)

`cantus-bench hashes` renders the **null corpus** and prints one SHA-256 per item (of the
samples' bits, f32 little-endian): every `cantus_dsp::math` function in f32 and f64 over
4 096 points (39 items), every native effect alone at its defaults and at the bench's heavy
settings on two seconds of the busy signal in 128-frame blocks (24), every instrument
holding eight voices (9), each demo song (6) and every saved-project fixture of
`cantus-project` (19) — 97 items. Effects, instruments and demos must be audible (a peak
above −80 dBFS), or the corpus panics: two builds agreeing on silence would prove nothing.
By default a demo is its eight bars where the whole band plays; `--whole` renders each song
whole (ten minutes of audio in all).

`cargo run -p xtask -- null-hashes [--baseline] [--levels v2,v3,v4] [--whole]` builds
`cantus-bench` once per level in the level's own target, runs each, and fails naming every
item whose bits differ. A level the machine cannot run is skipped (v4 here).

**Result on this machine** (i7-12700H, Windows 11, rustc 1.97.1, the corpus with the demo
songs **whole**): the plain x86-64 build (the one the tests' goldens come from), the
x86-64-v2 build and the x86-64-v3 build give **the same 97 hashes**, every one. Nothing to
explain: none of Cantus's code uses an FMA or a reduction LLVM could reorder, every
transcendental goes through the pinned `libm` (round 21), whose only target-feature
switches are for x86 without SSE2 (`fma.rs`, `sqrt.rs`, `pow.rs`, …), and there is no FFT
crate whose run-time dispatch could pick a different algorithm. The lists are kept in this
worktree's `target/null-hashes/` until it is cleaned (§8); e.g. `demo.house` (whole) is
`c36cbca4…6799f6` in all three.

**CI** (`.github/workflows/ci.yml`): job `null-hashes` on `windows-latest` and
`ubuntu-latest` runs `xtask null-hashes --baseline` (v4 joins on a runner with AVX-512) and
uploads the lists; job `null-hashes-across` then requires the Windows and Linux v3 lists
to be equal — R9 (4)'s second machine class, another C runtime. An aarch64 runner is still
to add (R9 (4)); it would compare its plain build's list with these.

## 5. v3 against v2, measured

**The machine was shared and loaded** — five other builders compiling and testing on the
same i7-12700H all night (cargo's package-cache lock queued builds for up to 25 minutes) —
so every number here is **paired**: `cargo run -p xtask -- perf --ab v2,v3` builds the bench
at both levels and runs them alternately (v2 v3, v3 v2, …) on **one pinned P-core**
(logical CPU 6, `start /AFFINITY 40`) at **High priority**, and reports per metric each
side's median of runs and the **median of the per-pair ratios v3/v2**, with their range and
in how many pairs v3 was faster. The first attempt (6 pairs of the whole engine and fx
groups at normal priority) put 14–28 minutes between the two runs of a pair and its ratios
spanned 0.3–3.4×: useless, and the reason the tool got `--sizes` (only some engine
sessions, `CANTUS_BENCH_SIZES`) and High priority. Even so a single pair's ratio spans
0.35–2.6× on the kernels: only the median over many pairs and the count say anything.

*The heavy kernels* — `perf --ab v2,v3 --full --only fx --runs 5 --pairs 20 --pin 6`
(each run five 2-second kernel runs per setting; ns per stereo frame, lower is better):

| Kernel (heavy) | v2 | v3 | v3/v2, median of 20 pairs | v3 faster in |
|---|---:|---:|---:|---:|
| Aequalis (8 bands, 2 dynamic, linear phase) | 48.4 | 51.0 | 1.033× | 6 of 20 |
| Comprimo (2× oversampling) | 323.8 | 322.7 | 1.021× | 9 of 20 |
| Terminus (true peak, 5 ms look-ahead, dither) | 473.8 | 424.6 | 0.952× | 12 of 20 |
| Spatium (8 s decay) | 577.3 | 502.1 | 0.967× | 12 of 20 |
| Multiplex (4 bands, linear phase, 48 dB) | 738.1 | 625.6 | 0.939× | 13 of 20 |
| Sibilus (split, linear phase) | 423.4 | 404.7 | 0.987× | 11 of 20 |
| Porta (style 5) | 99.2 | 93.8 | 1.005× | 9 of 20 |
| Fervor (8× oversampling) | 380.9 | 362.1 | 0.996× | 10 of 20 |
| Latitudo | 28.9 | 25.0 | 0.969× | 11 of 20 |
| Culmen (3 bands, true peak) | 677.7 | 667.2 | 1.015× | 9 of 20 |
| Delay (1.5 s, feedback 0.9) | 19.1 | 21.9 | **1.195×** | **2 of 20** |
| Reverb (size 1) | 37.9 | 35.2 | 0.978× | 12 of 20 |

At default settings the same: every heavy-ish kernel between 0.95× and 1.05×, and three
light ones **slower** at v3 in most pairs — Delay 1.175× (v3 faster in 2 of 20), Fervor at
1× 1.388× (3 of 20), Multiplex with its bands off 1.25× (3 of 20; 4 ns a frame, near the
timer's resolution). Report and per-pair JSON: `target/perf/ab/` (kept in the scratch copy
named in §8).

*The 64-track session* — `perf --ab v2,v3 --full --only engine --sizes 64 --runs 1
--pairs 12 --pin 6` (`sessions::scaled(64)`, one second of warm-up then four of audio per
run; µs per block):

| Pair | s64.b128 v2 | v3 | v3/v2 | s64.b64 v2 | v3 | v3/v2 | core share v2 / v3 |
|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | 14 456 | 31 091 | 2.15 | 15 272 | 8 158 | 0.53 | 98 / 96 % |
| 1 | 21 275 | 16 275 | 0.77 | 6 896 | 11 489 | 1.67 | 79 / 95 % |
| 2 | 41 525 | 11 939 | 0.29 | 14 304 | 12 043 | 0.84 | 49 / 65 % |
| 3 | 6 606 | 21 114 | 3.20 | 3 628 | 10 830 | 2.99 | 94 / 86 % |
| 4 | 7 915 | 22 449 | 2.84 | 10 389 | 12 971 | 1.25 | 77 / 84 % |
| 5 | 20 802 | 11 233 | 0.54 | 11 624 | 3 713 | 0.32 | 80 / 71 % |
| 6 | 16 308 | 6 530 | 0.40 | 4 215 | 4 478 | 1.06 | 65 / 86 % |
| 7 | 22 128 | 6 983 | 0.32 | 4 809 | 10 019 | 2.08 | 52 / 75 % |
| **8** | **6 602** | **6 506** | **0.985** | 3 591 | 6 026 | 1.68 | 94 / 94 % |
| 9 | 8 325 | 15 120 | 1.82 | 3 993 | 9 201 | 2.30 | 89 / 86 % |
| **10** | **6 452** | **6 476** | **1.004** | 3 519 | 4 365 | 1.24 | 96 / 96 % |
| 11 | 6 707 | 21 193 | 3.16 | 3 838 | 9 957 | 2.59 | 94 / 70 % |
| **median of ratios** | | | **0.995×** (v3 faster in 6 of 12) | | | 1.457× (3 of 12) | |
| **best run** | **6 452** | **6 476** | **1.004×** | 3 519 | 3 713 | 1.055× | |

The session's time is **bimodal**: ≈ 6.5 ms per block when the pinned core is free, 15–40
ms when something else lands on it or its hyperthread sibling (High priority and the
affinity mask do not keep other processes off a core; the "core share" column, the bench
thread's CPU time over wall time, drops to 49–80 % in the slow runs). The ratio of a pair
is therefore mostly *which of its two runs was unlucky*. The two pairs in which both runs
were quiet (8 and 10) give **0.985× and 1.004×**, and the best runs of the twelve give
**1.004×** at 128 frames and 1.055× at 64: v3 equals v2 on `engine.s64.b128` to within a
few per cent, and nothing here says the 64-frame case differs beyond the noise.

**Reading.** On today's code the x86-64-v3 build is **not measurably faster** than the v2
build on this machine: the heavy kernels sit within ±6 % of 1.0× (the noise of the median
here is about that), and the 64-track session likewise (above). That is what the code
predicts. Rust does not fuse `a * b + c` into an FMA on its own (which is also why the bits
are the same), so v3's FMA units are idle; the DSP is mostly recursive filters, smoothers and
per-sample state that no compiler vectorises; the transcendentals are pinned `libm` code,
scalar either way; and LLVM already vectorises the plain loops (gain, sums, copies) four
f32 wide with v2's SSE — AVX2's eight wide helps only those, and they are a small share of
a block. The STATUS figure of 15–25 % (round 17, `xtask bundle --v3`, the exported
plug-ins' fat-LTO profile, method not recorded) predates round 21's pinned `libm`, which
replaced the CRT's transcendentals; it does not reproduce here. **The plan's target, v3 ≥
1.15× on `engine.s64.b128`, is not met**, and no build flag will meet it: the gain comes
with the hand kernels of Phase 3 (SIMD across channels and voices with `wide`, compiled
for the binary's level — which is what this round's two programs make possible) and with
any FMA used **on purpose**, as a reviewed `DSP_VERSION` bump. The slower light kernels
(Delay, Fervor at 1×, Multiplex off) are the opposite of what a wider ISA should do and are
likely code layout (alignment of hot loops) rather than instructions; worth a look with
`cargo asm` by whoever vectorises them, not a reason to ship v2 — the launcher can be told
`CANTUS_CPU=v2` and `xtask perf --ab` re-measures in one command.

## 6. Tests

| Where | What it holds |
|---|---|
| `cantus-launch/tests/choose.rs` (6) | the highest level the processor runs **and** the folder holds is started (v3 on v3, v2 on v2, v3 on an AVX-512 machine without the v4 program, v4 with it, the next one down when a program is missing); `CANTUS_CPU` steps down, never up, nonsense and a missing program noted; a processor below v2 refused with what it lacks, an empty folder refused naming both programs, the texts in English and Spanish; names, parse, flags; **the feature lists equal `rustc --print cfg -C target-cpu=<level>`** and each has a run-time test; this machine is at least v2 |
| `cantus-launch/tests/launch.rs` (2, Windows) | the real launcher, copied in as `cantus.exe` beside copies of `cmd.exe` as `cantus-x86-64-v2.exe`/`-v3.exe`: the right one starts (its command line echoed), arguments pass untouched, exit codes 7 and 300 come back, `CANTUS_CPU=v2` steps down, `--launcher-info` reports; with no program, the report says nothing starts |
| `cantus-launch/tests/appid.rs` (1) | the process takes `Cantus.Cantus`, read back from Windows |
| `xtask` `package_tests` (6, 2 new) | the dist list unchanged (one edition, same names); the portable zip holds `cantus.exe`, both programs, the scanner, the marker, the plug-ins; the plug-ins zip no `.exe`; the staged launcher and programs are the right files; the uninstaller's `File`/`Delete` symmetry with 7 app files; `CANTUS_PROGRAMS_BUSY` names every program and `WAIT_FOR_CANTUS` uses it; every shortcut, association command and the finish page name `$INSTDIR\cantus.exe`, never a level's program; each `CreateShortcut` is followed by `CANTUS_SET_APP_ID`; `config.nsh` defines `APP_ID` from `cantus_launch::APP_ID`; **a real installer compiles with NSIS 3.11** from the stand-ins (the new macros and `appid.nsh` included) |
| `cantus-bench/tests/bench.rs` (1 new) | the null corpus: the math sweeps the same twice in one process, 39 + 24 items named once each, every effect audible at 2 × 96 000 samples, the report's format (64-hex hash per line, header) |
| `xtask` `levels_tests` (3) | the null-hash comparison (headers ignored; each differing or missing item named); the paired table (median of per-pair ratios, range, "better in k of n", higher-is-better metrics) |

`cargo test -p cantus-launch -p xtask`: 9 + 11 pass; `cargo test -p cantus-bench --test
bench`: 6 pass. `xtask null-hashes --no-build` with the default corpus (demos as eight
bars): v2 = v3, 97 items, 117 s and 161 s of hashing on the loaded machine (demos 107 s of
it). `cargo clippy --tests` clean on
`cantus-launch`, `xtask`, `cantus-bench`, `cantus` (the two `cantus-plugins` dead-code
warnings under `--no-default-features` are round 20's).

## 7. Files outside my area

- `crates/cantus-bench/` (round 20's bench, no owner this round): `src/hashes.rs` (new),
  `src/main.rs` (the `hashes` subcommand), `src/lib.rs` (one line), `src/sessions.rs`
  (`full_band_bar` made `pub`), `Cargo.toml` (`cantus-dsp`, `sha2`).
- `crates/cantus-app/`: `build.rs` split into `icon.rs` (shared with the launcher's build
  script), `src/main.rs` (the AppUserModelID first thing, the level in `--version` and the
  start-up log line), `Cargo.toml` (`cantus-launch`). *beta-docs* bumps the version and may
  touch `main.rs`'s `--version`: keep the `({level})` suffix, `xtask package` checks it.
- Workspace `Cargo.toml` (member, default member, workspace dependency), `Cargo.lock`.
- `docs/adr/0032-…` (§1 and §2 amended), `.github/workflows/ci.yml` (two jobs),
  `release.yml` (a step's name).

## 8. Left

- **The engine target** (v3 ≥ 1.15× on `engine.s64.b128`) is not met and cannot be by flags
  (§5). It moves to the Phase 3 kernel builders: SIMD across channels and voices with
  `wide`, and FMA on purpose behind a `DSP_VERSION` bump. Re-measure each with
  `xtask perf --ab v2,v3 --only fx` / `--only engine --sizes 64`.
- **The slower light kernels at v3** (Delay, Fervor at 1×, Multiplex with its bands off):
  look at the hot loops with `cargo asm` before vectorising them.
- **An aarch64 runner** for the cross-class null hashes (R9 (4)); Windows = Linux is in CI.
- **v4**: built with `package --v4`, never run here (no AVX-512 on this laptop; a CI runner
  with it runs the v4 hashes automatically). Ship it only if a Zen 4/5 measurement pays.
- **The installer on a real Start menu**: install, pin from Start, start Cantus, see one
  taskbar button; update over a round-21 install (its `cantus.exe` is the old program; the
  wait covers it by name).
- **The plug-ins** stay one baseline x86-64 build: a host loads one DLL, so a level choice
  would need a stub DLL forwarding to a v2/v3 one. Not worth it while v3 does not pay.
- **CI cost**: `null-hashes` builds the bench three times per OS (plain, v2, v3). If that is
  too long for every pull request, run it on pushes to `master` only, or drop
  `--baseline` (the goldens' tests already run the plain build).
- **Clean-up done**: this worktree's whole `target/` (the plain, v2, v3, plug-in and debug
  builds, ≈ 8 GB) was deleted at the end except `target/package/dist/` — the release made
  in §2 (installer, portable zip, plug-ins zip, sums; 166 MB), kept for the owner to try. The
  hash lists and the A/B reports quoted here were copied out first (session scratch).

## 9. Notes for the integrator

- **Merge order**: anything after this that edits `crates/cantus-app/src/main.rs` keeps
  `set_app_id()` first and the `({level})` in `--version` (the package task requires it).
  *beta-docs*' version bump changes nothing here (the names come from the workspace version).
- **`cargo run`** at the root is unchanged in kind: `cantus-launch` is a default member, but
  `cantus-host`'s `cantus-scan` already made the root ambiguous; use `cargo run -p cantus`.
- **Build-machine note**: cargo's package-cache lock serialised every builder's builds for
  up to 25 minutes at a time tonight. This builder worked around it with a private
  `CARGO_HOME` (junctions to the shared `registry` and `git`, `CARGO_NET_OFFLINE=true`, so
  nothing was fetched and the proxy never came into it) — at the cost of one full rebuild,
  since the source paths change. Worth a line in BUILDING.md for the next round.
- `xtask perf --ab` is general: any two levels, any groups; future rounds can A/B a change
  the same way by building twice.
