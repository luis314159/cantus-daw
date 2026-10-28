# Round 31 — integration: the daily gaps an FL or Ableton user hits first

**The ask** (the owner, 2026-10-25, `docs/next-session.md` of round 30; BACKLOG §0.4 item 3):
what round 30 left first (the flaky sandbox tests' real causes; the Mac session's results
and beta.6), then automation write/touch/latch and the piano roll's CC, pitch-bend,
velocity and release lanes; arpeggiator, chords and scale as note effects, scale
highlighting and snap, groove templates; crossfades between overlapping clips and clip gain
envelopes; a command palette, an undo history panel, project and track templates; effects
that sound on insert. Run as round 30: a contract, a worktree per builder, one reviewer per
branch and one on the integrated tree.

## How

- Contract `697550fa` on `r31/integration`: `docs/round31/BUILDING.md` (the seams — the
  note path, the model's additive fields, the piano roll, the arrangement — and who owns
  which files; schema 42–45, keymap 11–14, ADRs 0068–0072; empty locale areas).
- Builders in `F:\projects\daw-r31\{automation,midi,workflow,editing}`, never more than
  three compiling. editing started when the first branch merged and took its target (F:
  fell to 2.7 GB at one point; the owner freed space). Fix rounds shared the integration
  target until there was room again.
- **The Mac session had not happened**, so beta.6 is Windows and Linux; `release.yml`'s
  Mac job stays gated by `CANTUS_RELEASE_MACOS`.
- Reviews (one agent each, read-only, a refuter's discipline inside): midi (10), workflow
  (11), automation (13), editing (9), the integrated tree's seams (7), the fix rounds' delta
  (7) — **57 confirmed findings, all fixed with tests** (each failing before its fix but
  one timing bound and one atomic-write check, said in the builders' reports).
- Merges: midi, then workflow (its track templates had to learn midi's note effects — the
  first cross-branch seam), then automation (nine conflicts with midi's note path,
  resolved by its builder: the release velocity rides through the note effects), then
  editing (schema 44 after 43). The owner's requests during the round were added as they
  came (below).

## What each part brought

| Part | ADR | Schema / keymap | What |
|---|---|---|---|
| automation | 0068 | 42 / 12 | Read/Write/Touch/Latch per track and master, a global arm, a touch glide, one writer (round 28's recording is its Latch case), one undo step per pass; `Note::release`, `Clip::lanes` (bend, pressure, CC 0–119) fired sample-accurately, chased and rested; live recording, MIDI, DAWproject, clipboard, split, glue, links carry them |
| midi | 0069 | 43 / — | Note effects before the instrument (arpeggiator, chords, scale) on clips and live input, automatable and modulatable; *Bake to notes*; the song's key (light, fold, snap in key); grooves (pool, extract from MIDI or audio, commit, pattern groove); MIDI export as played or as written |
| workflow | 0072 | — / 11 | The three flaky sandbox tests' causes; the command palette; the undo history panel (`CommandBus` step ids, the saved mark); the start page, project templates (four starters) and track templates; hermetic hosted-plug-in tests; the start page at launch; the cores as engines |
| editing | 0070, 0071 | 44 / — | Crossfades between overlapping clips on the comp seams' engine path; clip gain envelopes; starting presets on insert (`cantus_core::starting`, *Neutral settings*), also in the exported CLAP/VST3; tests choose their language per thread |
| vehicles | — | — | The cores' ten vehicles as SVG illustrations of our own, in theme colours, two levels of detail; the PNG writer compresses |

## Causes found, not waits

- `session_faults5` (Windows): the test support's own scan counted as destroyed instances.
- `session_faults5` late state (macOS VM): a real race — a value's echo after a save asked
  for the state left the late answer stale; value moves now carry the first request served
  after them.
- `bridge_faults4` pause (macOS VM): liveness judged from a heartbeat timer; now from a
  progress word; the Mac helper opts out of App Nap.
- `bridge` abort and hang (Windows under load, also round 30's `late_blocks == 1`): a
  helper answered a load before its new processing thread was serving, so the first block
  of an isolated plug-in on a busy machine passed dry; it now waits (≤ 1 s) for the thread.
- `tempo_review6_ui`: tests wrote the process-wide language; they now set their thread's.
- 15–17 hosted-plug-in UI tests failed on any machine with real plug-ins installed: an app
  built from parts read the user's plug-in database.

## The owner's requests during the round

- BACKLOG §6h (2026-10-26): a look of its own for each of our plug-ins; hosted plug-ins in
  their native editor or Cantus's view, chosen per plug-in (part 2 to start after this round).
- The start page opens at launch, with *Show at start* on the page and in Settings.
- The audio cores as engines and real vehicles named by model only (no makers), drawn again
  from reference photos when the first silhouettes fell short.
- BACKLOG §7a (2026-10-27): a singing voice that plays like an instrument, with the owner's
  research in `research/ai/singing-voice.md` — to evaluate and build later.

## Results

On `59bc8282` (0.1.0-beta.6): **Windows** `cargo test --workspace` 4 707 passed, 0 failed,
56 ignored; the engine fuzz (`CANTUS_WORKERS=3`) 895 passed; `cantus-rt`'s fork fuzz 5;
clippy clean. **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`, which now prints the
total): 4 689 passed, 0 failed, 56 ignored; clippy clean, with and without the plug-ins'
editor. **macOS**: the runner's `macos.yml` at publish.

## After the release (same session)

- **beta.6 released** 2026-10-27 (`v0.1.0-beta.6`, Windows and Linux; every cantus-daw run
  green). Its macOS run passed with one test FLAKY on retry: `cantus-sandbox --test session`
  `a_crash_mid_song…`, 20 478 samples wrong before the crash and 0 late blocks. **Cause**:
  the rig's `settle` waited a fixed 64 blocks while an isolated plug-in's restart (asked for
  by its new latency) was still out — a wire passes dry, on time. Test code only; a test
  forcing a slow restart failed 3/3 before the fix (`mac-flaky.md`).
- **BACKLOG §6h point 2** (ADR 0073, `hosted-view.md`): hosted plug-ins in their own editor or
  Cantus's view, remembered per plug-in. One review (8 findings) and one on its fix round
  (8 findings), a second on its fix round (4 more: an identity lost on a VST3 restart, a second press cancelling a window on its way, a waiting plug-in taking back a frame, the header measured at text size 1.0), and a third on that fix round: nothing left.
- Round 32's prompt reordered by the owner: §6h point 1 and notation first.
- Suites on `r31/close` (195e1061): Windows 4 737 passed / 0 failed (57 ignored), clippy clean; the engine fuzz 895 at `CANTUS_WORKERS=3`; Linux (WSL) clippy clean (with and without the plug-in editors) and 4 719 passed / 0 failed (57 ignored).
- On `cantus-daw` (public 60ad4281) three of the new hosted-view tests failed on the runners and passed here: two took for granted that a plug-in's own window opens (a runner has no display, and the fallback rightly keeps Cantus's view for that load — reproduced in WSL with `DISPLAY` unset), and the busy-helper tests loaded a state equal to the one synced last, never sent, so the helper was never busy (the Windows runner). Fixed in the tests (583f1886: the fallback accounted for, a tagged state of its own, the helper's being busy asserted on Windows); the next runs (public 7a36baff) were all green, macOS with nothing retried.

## What only the owner can do

- **Listen**: the arpeggiator's feel, the swings against an MPC, the chord voicings; each
  starting preset; the crossfade curves on real material; a Touch and a Latch pass; a
  keyboard's bend, mod wheel, sustain and release on Lamina and a hosted synth.
- **The model names** of the cores' vehicles may be registered marks: a legal look when the
  review of Chorda and the CLA happens (they are one table, `engines.rs`).
- Everything round 30 left: the Mac session and the Apple account, a real Linux desktop,
  Flathub, the UMC in duplex, code signing, the legal reviews, the RVR1960 permission, the
  trademark filing, compacting the WSL disk image.
