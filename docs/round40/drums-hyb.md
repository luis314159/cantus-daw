# Round 40 — `drums-hyb` (Tympanum's hybrid voice)

**Status: built and tested; both kits' packs fitted (jazz from Virtuosity, CC0; studio from DRSKit, CC BY 4.0), measured and listened to (see
below); to upload (owner).** Branch `r40/drums-hyb` (from `r40/tymp-engine` 6c4dd2d3),
worktree `F:\projects\daw-r40\drums-hyb`. The owner (2026-11-13): *"un híbrido como opción,
pero el generado debe mejorar mucho más."* ADR 0124's third amendment says the design and
the licence; this is what was built and what it measures.

## What it is

*Hybrid* plays, under model 5, what a recorded kit holds in each hit's first 30–200 ms
that the model does not: a **spectral residual** (the recording under the mask
`√(1 − E|model|²/|rec|²)`, so the powers add up whatever seed the model's hit drew),
**deconvolved to each piece's dry source** by the engine's own measured source → mix
response, so the virtual mics, the bleed cells, the room and the outputs apply to it as to
the model. *Generated* (the default) is the model alone, to the bit.

## Files

| Where | What |
|---|---|
| `cantus-dsp/src/tympanum_residual{,_tests}.rs` | `CRS1` decode/encode, `Player` (48 voices, one draw per hit, layer crossfade, hat states, any rate) |
| `cantus-dsp/src/tympanum_model{,_paths}.rs` (drums-gen's) | the hook: a `hybrid` field; `strike` in `note_on`, `render` after Pulsus's buses, `reset`, `at_rest` |
| `cantus-dsp/src/pulsus.rs` | `hat_openness(pitch)`, read-only |
| `cantus-engine/src/inst/tympanum_hybrid{,_tests}.rs` | the residual cache (lock-free, per session), its source, `prepare`, `configure` |
| `cantus-engine/src/inst/tympanum_unit.rs` (fix-e's) | one call after `configure_modelled` |
| `cantus-core/src/inst/tympanum_voice.rs` | `voice` key, `Voice`, `hybrid_pack`, `set_voice` |
| `cantus-project` | schema 71 (70 was `tymp-ui`'s), `migrate_70_to_71`, `fixtures/v71.cantus` (the jazz kit, hybrid) |
| `cantus-ui/src/views/inst/tympanum_voice.rs`, `tympanum_kits.rs` | *Generated · Hybrid* under the lit kit; *Get · size*, progress, the licence |
| `cantus-ui/src/tympanum_load.rs` | the residual's source (`<content>/<pack>/residual.crs`) |
| `cantus-ui/examples/tympanum_hybrid.rs` | the fit's renders (`takes`, `probe`) and the listening (`listen`) |
| `tools/tympanum-fit/residual_{fit,pack,zip}.py`, `refit_hybrid.sh` | the fit, the format's independent writer, the pack and its manifest entry; one command |
| `cantus-content/manifest/content.json` | `tympanum-hybrid-jazz` (CC0), `tympanum-hybrid-studio` (CC BY 4.0) |

## Tests

- `cantus-dsp` `tympanum_residual`: round trip to the byte, bad files refused, a file the
  Python writer made (its bytes in the test), no data touches nothing, chunking 1/37/64/
  256/333/512 the same bits, a take played exactly, never the same take twice running,
  velocity law, hat states, another rate, the blob's fingerprint.
- `cantus-engine` `tympanum_hybrid`: through the unit, hybrid without a residual = generated
  to the bit (and never silent); with one, heard; blocks 1/64/333/512 the same bits.
  `rt_memory::tympanum_hybrid_never_allocates`: playing, and the voice switched both ways
  while it plays. `hybrid_cost` (ignored): the full kit's cost.
- `cantus-core`: the voice key; `set_voice` one command; the undo fuzz arm.
- `cantus-project`: `schema_71_keeps_a_tympanums_voice`.
- `cantus-ui` `tympanum_hybrid_e2e` (through `CantusApp::assemble`): hybrid before its pack
  = generated to the bit; the manifest's own entry installed from `file://` by the content
  manager; then the residual heard; blocks 1/64/333 = 512; live = offline; *Generated*
  through the command = the model to the bit; its undo = the hybrid again.

**Green after merging `r40/integration` 305df85d** (Windows): the tests above, `cantus-project`,
`cantus-content`, the i18n test, `tympanum_e2e`, and clippy `-D warnings` on dsp, engine, core,
project, content and ui (all targets); `cargo fmt` per crate. The Linux check in WSL was not
run (the integrator's).

## The fit (one command: `tools/tympanum-fit/refit_hybrid.sh <jazz|studio> <content> <work>`)

15 articulations (kick, snare, rimshot, sidestick, three toms, closed hat, key 46 at 0.1,
0.5 and 0.95, the pedal, crash, ride, bell) × 5 layers (30, 55, 80, 105, 127) × 4 takes
(the next ordinal, the velocity 0/−3/+3/−6 off), recorded kit and model rendered through
the engine, 2.5 s a hit. Every articulation's residual stays above −10 dB of the recording
to the 200-ms cap, so every piece keeps 200 ms (faded over its last 80). How much of the
recording's first 200 ms the model already explains (`1 − E_res/E_rec`, the fit's report,
refitted on the shipping engine after merging `r40/integration` 305df85d):

| | jazz (Virtuosity) | studio (DRSKit) |
|---|---|---|
| kick, snare | 83, 76 % | 79, 87 % |
| toms | 73–81 % | 74–81 % |
| hats (closed, half, open, pedal) | 65, 73, 69, 50 % (key 46 nearly closed: 36 %) | 68, 72, 74, 55 % (52 %) |
| crash, ride, bell | 79, 50, 57 % | 73, 60, 72 % |
| pack | 5.74 MB residual, 5.04 MB zip | 5.74 MB, 5.11 MB zip |

The model's power *above* the recording's in the same 200 ms (what a residual cannot take
away) is 30–110 % of the recording's on the drums — mostly the kick's and snare's highs
(`NEXT-drums.md`'s gap 2). That is the model's to fix (`drums-gen`).

## Measured (`hybrid_measure.py`; tympanum_measure's hits and the listening grooves)

Distance from the recording, mean |dB| over third octaves 60 Hz–16 kHz (levels aligned),
all 13 articulations × 4 velocities:

| | 0–50 ms generated → hybrid | 50–200 ms | groove 6–16 kHz flatness | components / kHz |
|---|---|---|---|---|
| jazz vs Virtuosity | 4.78 → 3.96 dB | 4.39 → 3.56 dB | 0.233 → 0.262 (rec 0.280) | 22.5 → 27.0 (rec 28.0) |
| studio vs DRSKit | 4.78 → 3.79 dB | 4.16 → 3.23 dB | 0.250 → 0.280 (rec 0.363) | 15.0 → 20.2 (rec 30.9) |

(Before the merge, on tymp-engine's seeds, the same: 4.67 → 3.93 and 4.41 → 3.50 jazz,
4.81 → 3.87 and 4.22 → 3.26 studio. The detail per hit: `F:\projects\daw-r40\tmp\hyb\measure-{jazz,studio}-2.txt`.)

The cymbals' texture measures over 0.15–0.65 s (flatness, top three, stability) do not move
— the residual ends at 200 ms; the closed and pedal hats' (0.01–0.12 s) move toward the
recording (jazz pedal 4–8 kHz flatness 0.24–0.34 → 0.27–0.42, recording 0.42–0.50; studio
closed hat 0.25–0.32 → 0.31–0.36, recording 0.26–0.34).

**Cost.** The player alone, a busy groove (`player_cost`, release, best of five): **0.03 %
of a core**. The full jazz kit with every mic on (`hybrid_cost`, best of five), on a machine
loaded by three other builds, two runs: generated 18.1 / hybrid 14.0 % and generated 13.6 /
hybrid 17.2 % of a core (fix-e's `full_kit_with_mics_cost` 18.7 % in the first; 12.0 % idle
in tymp-engine's report) — the load's noise, far above the player's 0.03 %. **Memory**: the
residual is `i16` in one `Arc` per kit for the session — 5.74 MB for either real pack (the
synthetic test pack 3.4 MB); nothing per instance but the 48-voice pool (~2 KB).

## The listen

`F:\projects\daw-r40\compartir\tympanum-hibrido-1\` (`NOTA.md` in Spanish): `jazz/` and
`estudio/`, the groove and seven pieces at 50/90/127, generated · hybrid · recording, each
three at one RMS and one peak.

## Left / for the owner

- **Upload** `tympanum-hybrid-jazz-1.zip` and `tympanum-hybrid-studio-1.zip`
  (`F:\projects\daw-r40\tmp\hyb\{jazz,studio}-2\pack\`) as release assets `content-r40` of the
  public repository; the manifest entries carry their sizes and SHA-256 — until then *Get*
  fails as for any missing asset (and the kit plays generated). Re-run the fit and replace
  both entries whenever `drums-gen` changes the blob (the engine logs a stale residual).
  *(Version 1 was published; `drums-gen`'s blob then changed and both packs were refitted
  as version 2 — `hyb-refit.md`.)*
- The residual is mono (the cymbals' side stays the model's); after 200 ms it is the model.
- Verified only by measure and the e2e here; the ear is the owner's (`docs/beta-checklist.md`
  has no device step for this).
