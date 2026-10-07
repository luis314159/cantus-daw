# Working on Cantus — conventions for Claude (and anyone else)

Cantus is a free, open-source (GPL-3.0-or-later) DAW in Rust, FL Studio-shaped, made by
Luis Ángel Almazán with Christian values (motto: Colossians 3:23 — "whatever you do, work at
it with all your heart"). The owner writes in Spanish; the repository is written in English.
The bar is the best DAWs on the market, and excellence over speed.

## Read first

1. `docs/STATUS.md` — the handoff: the header, **"The next session"** at the top, then the
   newest round's section. It is the single source of truth for what exists and what is next.
2. `docs/BETA.md` — the road to the first public beta.
3. `docs/DEVELOPING.md` — setting up a machine, building, testing, packaging.
4. `docs/adr/` — every structural decision (Context / Decision / Consequences / what the
   tests hold). `docs/BACKLOG-excellence.md` — what stands between Cantus and excellent.
5. The newest `docs/roundNN/` — the briefs and every builder's report.

## How work is done here

- **Every mutation is a `Command`** with an exact inverse (ADR 0004); new commands get arms
  in the undo fuzz (`crates/cantus-core/tests/undo_invariants.rs`) and `cmd.*` strings.
- **Tests first.** Null tests before DSP or engine wiring: a control at its default touches no
  sample, to the bit; block-size independence at tolerance 0; offline render equals real
  time; parallel equals serial. Golden hashes change only on purpose, in one commit with a
  `DSP_VERSION` bump and the measured size of the change (`CANTUS_GOLDEN_DUMP`).
- **Real-time rules (ADR 0005):** the audio thread never allocates, locks, blocks or does
  I/O (`rt_memory.rs` counts it). `cantus-engine` is `forbid(unsafe_code)`; OS/unsafe code
  lives only in `crates/cantus-rt`. Never let a spinning thread run inside MMCSS.
- **Deterministic maths:** DSP calls `cantus_dsp::math::*`, never std's transcendentals
  (clippy `disallowed-methods` enforces it).
- **A model change is a schema bump** in `cantus-project` (`SCHEMA_VERSION`, a migration even
  if it writes nothing, `cargo run -p cantus-project --example write_fixture` for
  `fixtures/vN.cantus`); every old fixture must keep loading.
- **Every structural decision gets an ADR**; **STATUS.md is updated** at the end of a round
  (header with the test count, "The next session", a round section).
- **Strings** go in both locales (`crates/cantus-ui/locales/**/*.{en,es}.txt`, one area file
  per feature); the i18n test requires every English key in Spanish.
- **UI is the hand-drawn kit** (ADR 0024, `crates/cantus-ui/src/kit/`): no stock egui
  widgets in views, no literal colours, no `FontId::new` — lints enforce it. FL Studio is the
  visual reference; labels in clear label/value units, nothing that collides. **Look at what
  you build**: `cargo run -p cantus-ui --example shoot -- <dir> …` renders PNGs on the CPU.
- **One distributed build works with every audio driver** (WASAPI and ASIO); never ship
  feature-gated editions. Test real drivers in duplex before calling device work done, and
  never load an ASIO driver whose interface is unplugged.
- `THIRD-PARTY-NOTICES.md` is generated (`cargo run -p xtask -- notices`); a test fails when a
  new crate has no notice or an unapproved licence.
- Commit only when the owner asks; never stage or discard the owner's own uncommitted work.

## The owner's standing direction

- **FL Studio is the reference** — for the mixer (feature by feature), the workflow and the
  look — with the best ideas of Ableton, Bitwig, Logic, Studio One and FabFilter-class
  plug-ins on top. Ours should end up *better*, not a clone. Research notes live in
  `research/`, with the reference screenshots of other products (the repository is
  private; take them out of the history before it is ever made public — `docs/BETA.md`).
- **The UI must look first-rank** (the owner found an earlier one "ugly, plain"): clear
  label/value units, no loose words, nothing that overlaps, panels resizable and movable,
  every overflowing area scrollable, navigation fast. Hand-drawn vector components, with a
  measured frame budget.
- **Performance better than anything on the market** is the current priority after the
  beta blockers (`docs/BACKLOG-excellence.md` §4b, ADR 0031): take the best ideas of current
  engines and of the papers.
- **Christian values, respected choices:** a Reina-Valera verse (RVR1960 within its
  quotation allowance) greets the user at start in the interface's language — on by
  default, with an off switch for those who do not share the belief.
- **Professional recording, mastering on the master channel, free instruments and sounds**
  (sample packs download on demand, never in the repository — ADR 0030).

## Parallel builders (how the big rounds are run)

1. A **contract commit** declares the shared types, signatures, schema/ADR numbers and empty
   locale areas, plus `docs/roundNN/BUILDING.md` (rules + who owns which files) and briefs.
2. One **git worktree per builder** (`git worktree add -b rNN/<name> ../daw-rNN/<name> <base>`)
   with an untracked `.cargo/config.toml` (`[build] jobs = 4`, `debug = 0` for dev/test) so
   several builds fit on one machine. One owner per hot file (e.g. `processor*.rs`).
3. Builders commit WIP often (sessions can be cut by API errors — resume them, context intact),
   write files in pieces under ~300 lines per tool call, and finish with
   `docs/roundNN/<name>.md` and a report.
4. The integrator merges every branch into `rNN/integration`, resolves the (usually additive)
   conflicts, renumbers schema/ADR collisions, runs the whole suite and clippy, writes
   STATUS, builds the release, and only then — when the owner asks — merges to `master` and
   pushes. Delete builders' `target/` folders once merged; disk fills fast (~9 GB each).

## Machine quirks seen on the owner's PC

- The Bash tool is Git Bash: a heredoc delimited by `EOF` fails to parse — use another
  delimiter (`XEOF`) or write scripts with the Write tool.
- A corporate proxy (Zscaler) may quarantine a download for ~10 minutes and answer with an
  HTML page meanwhile: verify what arrives (a zip starts with `PK`, an exe with `MZ`), wait,
  retry. **Never bypass the proxy or TLS checks.**
- `core.autocrlf=true`: compare files ignoring CR when merging by hand.
