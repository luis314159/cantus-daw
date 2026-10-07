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
waiting launcher is one idle process of ≈ 1 MB. The installer now waits for **every**
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
