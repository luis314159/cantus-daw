# Round 28 — integration

*The integrator's account (2026-10-14 → 2026-10-15). The ask: the owner's message of
2026-10-14 (five items, `BUILDING.md`'s top) — everything that needs neither the owner's
hardware, nor their paperwork, nor other software. The contract and the briefs:
`BUILDING.md`. One report per builder beside this file.*

## How it was run

A contract commit (`a327c53`, on master `445a44c`) declared the thirteen effect kinds and
`InstrumentKind::Chorda` as inert stubs wired into every list, schema 25 and five empty
locale areas, so the builders never shared a hot file; `BUILDING.md` (`ca5b48e`) held the
rules, the file owners and seven briefs. Never more than three builders compiled at once
(a 31 GB machine): wave 1 **voice**, **modfx**, **topbar** from the contract; wave 2
**pitch** (on voice's base), **dynfx**, **leaders**, **sound** from `r28/integration` as
slots freed. Mid-round the owner asked for a showcase song: a **demo** builder. C: was
nearly full all round: every builder's temporary files went to F: (`[env] TMP` forced in
the untracked `.cargo/config.toml`), and each builder's `target/` was deleted once merged.

Every branch was reviewed adversarially with a Workflow (five lenses — correctness, real
time, reachability, tests, interface — plus two focus lenses per branch, and an independent
refuter per finding): **20 reviews, 454 confirmed findings (21 high)**, every one fixed with
a test seen failing first where a test could show it, or answered in the builder's account
when judged wrong. Second-round reviews checked each fix round; six fix branches
(`r28/fix2` to `r28/fix7`) closed what the second rounds, the review of the whole
integrated tree and the reviews of each fix round found, until a review found nothing
above "low" (and fix7 closed those too). Under the owner's rule, every "Left" item went
back as work: what remains open below is the owner's own.

## What landed, by item of the ask

| Item | Builder | ADR | Where to read |
|---|---|---|---|
| 1. The top bar à la FL (§6e) | topbar | 0052 | `topbar.md` |
| 2. The real-time auto-tuner (§3d) and its base | voice | 0053 | `voice.md` |
| 2. The pitch editor for recordings (§3c) | pitch | 0054 | `pitch.md` |
| 3. The modulation family (§3b) | modfx | 0055 | `modfx.md` |
| 3. Dynamics and analysis (§3b) | dynfx | 0056 | `dynfx.md` |
| 4. The DRSKit acoustic kit, Chorda (§3e/§5) | sound | 0042 amended, 0059 | `sound.md` |
| 5. Modulators on any parameter, comping (§0.2) | leaders | 0057, 0058 | `leaders.md` |
| — The showcase song *With All Your Heart* | demo | — | `demo.md` |
| — The review rounds | fix2 … fix7 | — | `fix2.md` … `fix7.md` |

## Decisions taken by the integrator

- **Schema chain 25 → 32**: 25 contract (thirteen kinds, Chorda), 26 topbar (metronome
  voice and accent, pattern mode), 27 pitch (`AudioRegion::pitch`, `Track::pitch_open`),
  28 modulators and hosted automation targets, 29 comp seams and alternate comps, 30
  Exemplar's round robin in order (sound's branch had numbered it 28), 31 the pitch editor's
  `PitchNote::corrected` (fix2), 32 `Clip::comp` (fix4: a comp window is marked, not guessed). Each migration in order, each
  fixture regenerated after chaining.
- **`DSP_VERSION` 5** (leaders): an Aequalis band raised from 0 dB never glided; fixed,
  measured (a 1 kHz bell to +12 dB rendered 0 dB before), no golden hash moved.
- **Delay compensation re-planned when a latency moves under automation** (dynfx), with
  lines that keep their content and move their read point (`DelayLine::retime`); with it,
  **Flexus** reports no latency with through-zero off (a bit-exact null at its defaults)
  and the switch stays automatable.
- **Every insert hears the song position as late as its audio** (fix3/fix4,
  `EffectUnit::late`, passes cut at each late insert's own loop wrap): synced units stay on
  the beat they shape behind latency, at every block size.
- **The exported plug-ins' host clock**: `beats` is where the block ended (modfx),
  `last_beats` its last frame (Speculum's clock, dynfx) — reconciled at merge.
- **Chorda's tuning keys (A4, stretch, stiffness) are set, never automated or modulated**:
  one rule, `InstrumentKind::modulatable`, on every path; the keyboard is designed off the
  interface thread by one designer thread after a 250 ms settle — the "known UX cost" of a
  frozen window was refused.
- **Naked Drums refused**: its CC BY was added by the SFZ porters, not by Wilkinson Audio;
  DrumGizmo's DRSKit (CC BY 4.0) taken, mixed to stereo at install.
- **The VST3 casts between the SDK's untyped enums** are no-ops on Linux: allowed at module
  level, as `cantus-host` does.
- A builder committed its local `.cargo/config.toml` (machine paths): removed before the
  merge (`068aa98`); never in master.

## Lessons kept (also in the memory)

- **A fresh agent on the same branch** gets past an API safeguard false positive that stops
  a long-running one twice in a row.
- **A shared target dir** (`target-dir` in the untracked config) lets a fix branch reuse the
  integration build: one target instead of two on a nearly full disk.
- **Every fix round finds more**: the second reviews found real regressions in first-round
  fixes (a note tracker that lost note-offs on a full buffer, a constant latency that broke
  a null, a drift reintroduced by a release path). Review the fixes, not only the features.
