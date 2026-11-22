# Round 41 — the integrator's report (r41/integration)

Integrator: session cantus-bc, 2026-11-20 to 2026-11-22, beside session cantus-6f (the site,
the README, the cards). The owner approved Fides, Nitor, Fragor's pedals and Chorda's retune
by ear on 2026-11-21 ("visto bueno, suena bien"), and on 2026-11-22 left every open choice to
the integrator by the excellence bar ("de todo lo pendiente decide lo que busque la
excelencia"); those choices are listed under *Decided*.

## What is in

| Branch | What it brings |
|---|---|
| r41/contract | schema 78, ADRs 0150–0153, the stubs every builder filled |
| r41/fides | Fides, the resonator instrument (ADR 0150): six exciters, twelve resonators, two at once, a body, MPE, 25 presets |
| r41/nitor | Nitor, the one-knob enhancer (ADR 0151), CLAP/VST3 bit-identical; the song's key reaches effects (ADR 0153) |
| r41/pedals | Fragor's five circuits as board-only Tabula pedals (ADR 0152), "Stacked drives" board |
| r41/chorda-tune | every Chorda string of all 88 keys in tune (`DSP_VERSION` 11, ADR 0098's fifth amendment) |
| r41/theme-follows-os | a fresh install follows the system's light or dark (ADR 0111 §12) |
| r41/fix-fides, r41/fix-nitor | the integrated review's 18 findings (below) |
| r41/no-elision | `r41_whole`: every label of Nitor, the pedals and Fides whole in both languages; Nitor's corona at 4.5:1 in every theme |
| r41/fides-all | Fides in `InstrumentKind::ALL` and the picker; its words whole; three DSP faults the instrument contract's twins found |
| r41/status-line | a dialog's ground no longer spells its words on the status line (a sweep of every dialog) |
| r41/thumb-store | the picker's pictures never run out past the 28th (least recently used goes; a page more only when shown at once) |
| r41/value-words | a value of words painted whole in one ink (Nitor's "The song's"); a number and its unit as before |
| r41/fides-input | Fides's input is a sidechain from another strip (ADR 0154, schema 79); Inharmonicity acts over its whole range |
| r41/key-wiring | the instrument's key everywhere a key is: the agents' `set_track`, the desk's cable, the matrix, the inspector |
| r41/take-tempo, r41/fuzz-two, r41/refs-ir | the undo fuzz's findings (below) |
| r41/cards (cantus-6f) | Fides's and Nitor's unit cards, pictures and excerpts; the cards' pitches out of the wasm |
| r41/appimage-name (cantus-6f) | the stable AppImage name on the site and README; the download page's lines name the saved file |
| r41/video | the launch video's whole pipeline in `tools/video/` (the owner: every script used, kept) |
| b16/* | beta.16's fixes, also on master: bounce names, timing turns, Comparatio's plug-in test |

## The integrated review (three finders, two fixers)

Found and fixed, each with a test that failed first:

- **Fides:** a slow position never re-placed a modal object; the noise band and level froze at
  note start; a body or output stage taken out mid-ring never came to rest; resonator B
  switched on mid-note sounded as a stale object; the checkpoint estimate was the contract's
  stub (1 400 KB now; and Tympanum's, 2 → 7 200 KB).
- **Nitor and the song's key:** a freeze ignored the key Nitor follows; stems through the
  master and the mastering assistant's trials tuned Nitor to C; a Nitor colouring nothing never
  rested after a Mix change; the agents were told it "passes the sound through".
- **Seams:** Tabula's 16th board missing from the README and site counts; the key names written
  with ♯/♭ the fonts lack; Fides's gold under 4.5:1 on paper themes; two tests that walked
  every effect kind and met the board-only pedals.

## The undo fuzz (ADR 0004)

Changing one arm of the main fuzz moved its draws onto new ground and it found real bugs; the
fuzz no longer depends on one arm (`takes_through_tempo_edits_…`, side streams checked after
each command, `CANTUS_FUZZ_SEEDS=a..=b`):

- a tempo change could join two takes through a linked window between them (`tempo/keep.rs`);
- a comp seam move that would leave no seam was accepted, and its undo refused (`comp.rs`);
- a frozen strip's own pan law was missing from its freeze key (`freeze.rs`);
- a group's VCA taken away came back at the end of the list (`InsertGroupVca`, inverse-only);
- a Comparatio's compared file could be one its list did not hold (`SetEffectIr`, repair on
  load).

Main fuzz 1..=600 (normal draws and with the Fides key arm), takes-through-tempo 1..=1000: green.

## Decided (the integrator, with the owner's mandate of 2026-11-22)

- Fides's live input = a sidechain from another strip (option C of three; ADR 0154).
- Fides's face accent copper (no two faces share a material and an accent within 20°).
- Fides's default patch level kept: a pluck's crest limits it, as Chorda's (instrument_reference
  PENDING, documented).
- Nylon guitar and Harp trimmed to their role's target (0.0 LU).
- The video: the pickers scroll to show Fides and Nitor; the remade lines 2 and 4 kept; loop
  0:31–0:49, vertical first 30 s, poster the title card; published on the site and on YouTube
  (the owner uploads from his account).

## Verified only in emulation

- Fides's MPE and its sidechain from a real microphone (docs/beta-checklist.md, Ronda 41).
- Nitor as a plug-in in another DAW.
- The Linux suite of this tree: WSL could not start (its disk was held by an open diskpart
  window); the release run of beta.16 tested master on all three systems.

## Left

- The WSL Linux check of this tree (needs the WSL disk free).
- The picker's pictures on a first opening: made a few a frame, so tiles further down show a
  card for a moment; warming them during idle after start-up is BACKLOG §(UI).
- The bowed string's tuning, a few cents (as the physics gives; fides.md *Measured*).
