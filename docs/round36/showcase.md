# Round 36 — showcase: the site and the README sell what Cantus is now

Builder **showcase**, branch `r36/showcase` (from `r36/integration` 8d676d02, with r36/record's
final merge 311c96c3). The owner (2026-11-03): sell the recording mode, the instruments and
effects (click for details and a large picture), and say Linux and the macOS preview plainly.
Nothing deployed, nothing pushed.

## Done

**Site (`web/`).**
- **The recording mode** (`src/studio.rs`, `#record`, right after the tour): a dark stage, the
  bar on *Neon Tide* in the page's theme and language (tabs: in the mode / before it starts),
  five stat tiles with r36/record's measured figures and their method (machine, load, the
  live runs on a shared machine), five points, and *What it does not do* (the interface's
  round trip; the lowest buffer is the user's to find). A *New* chip in the hero links to it.
- **Instruments and effects** (`src/units.rs`, `src/units_data.rs`, `#units`): a card per
  native unit (9 instruments, 23 effects: picture, name, kind, one-line pitch; *Listen* badge
  on the 7 with A/B excerpts). A click opens the unit's page — a modal dialog
  (`role="dialog"`, `aria-modal`, focus to *Close*, Tab trapped, Esc closes and gives the focus
  back to the card, ←/→ to the previous/next unit, the page behind does not scroll): the window
  large (dark or light, following the site's theme, switchable; click for full size), what it
  does, 4–6 standout features, presets, *also a CLAP and VST3 plug-in* for the effects, and the
  with/without player where an excerpt exists. EN/ES; phone layout checked.
- **Hear what they do** (`src/fx.rs`): the A/B section now reads its list, pitch and pictures
  from `units_data.rs`; the player is a component (`AbPlayer`) the unit pages reuse. Its stale
  "seven of the twelve" line is gone.
- **The professional studio** (`#studio`): VCA faders + snapshots + cues, the audio editor,
  outboard gear with the ping, Mackie Control/HUI ("tested byte for byte against a built-in
  emulation … real hardware still to come"), ARA 2 ("hosts ARA 2 plug-ins on audio clips …
  runs inside Cantus's own process"; no plug-in named, no logo).
- **Platforms**: hero line *Windows · Linux · macOS (preview)*; the download card gains a row
  of three systems (each a direct download once the latest release holds its file — the
  existing `shelf.rs` check — "Coming soon" otherwise); the macOS preview's note everywhere it is
  offered (universal, built and tested on GitHub's macOS runners, not yet opened on a real Mac,
  unsigned, *System Settings → Privacy & Security → Open Anyway*, report problems → issues).
  FAQ and roadmap rewritten from the current state (the roadmap's "next" listed round 29's
  features as coming).
- Nav: *Recording* and *Instruments* added; the links no longer wrap (lower-priority ones give
  way at 100rem / 70rem). Removed `src/faces.rs`, `assets/faces/`, `assets/fx/*.png`.

**README.** Platforms under the title; a **Download** section (Windows, Linux, macOS preview
with its note); highlights for the recording mode and the studio; tour sections *The
recording mode* (picture, points, the measured table, what it does not do) and *The
professional studio*; **Instruments** and **Effects**: a thumbnail grid (each links to the
full-size picture) and a `<details>` per unit with the large picture, what it does,
features, presets, a light-theme link; *Everything it does* gains the five round-36 areas.

**Pictures.** `shoot_skins` (all 32 units, Slate/Paper, 100/200 %) → `web/assets/units/`
(thumb 640 px, large ≤ 1800 px WebP, 7.6 MB) and `docs/images/units/` (PNG at 100 % +
480 px JPEG thumbs, oxipng). New example **`shoot_showcase`** (the recording mode on *Neon
Tide* with the demo interface — no "no audio device" line — and the desk's VCAs/snapshots,
1920 × 1080, Night/Paper, EN/ES). `web/tools/units_pictures.py` regenerates the unit pictures;
`web/README.md` says how. `shoot_readme` no longer re-shoots `fx-*`/`inst-*` unless named (the
old `docs/images/fx-*.png`, `inst-*.png` are deleted; nothing else referenced them).

## Checked

`trunk build --release` (Trunk from `%LOCALAPPDATA%\cantus-build-tools\trunk`); `cargo test`
in `web/` (3 pass); `cargo clippy --target wasm32-unknown-unknown` in `web/`: only the two
warnings that were there before (`audio.rs`, `platform.rs`). Pages rendered with headless
Chrome over the DevTools protocol at 1440 px, 1000 px and 390 px, EN/ES, dark/light, the unit
page open, the macOS card with a simulated release that holds the `.dmg`. Shots:
`docs/round36/showcase/`.

## Honesty notes

- Recording-mode numbers are r36/record's (`docs/round36/record.md`), with the machine named
  and the live runs' shared machine said. The CPU figures *inside* the bar's picture come from
  `stage_for_shot` (staged); the caption says so.
- Unit copy was written from the design notes: only what is built (Gravis's ducking, Pulsus's
  per-pad outputs and Unda's tempo sync, built but not wired, left out); file formats named
  (Serum/Surge tables, DX7 SysEx, SFZ, Decent Sampler, SF2), no product comparisons; Chorda's copy
  is about its model, not its sound. "808"/"909" kept as the product's own model names.

## Integrator's fix round

- **Stat tiles**: auto-fit columns (min 12rem) and each figure sized by its own tile
  (container units, `clamp(1.2rem, 11.5cqi, 2.2rem)`, never wrapping); checked at 1440, 1200,
  1000 and 390 px (`site-record-stats-*`, `phone-record-stats-es-dark`).
- **Spanish term**: the site now says *pre-renderizado / pre-renderizar / renderiza* instead
  of *horneado*. The app's own Spanish (`crates/cantus-ui/locales/areas/record.es.txt`, r36/record's)
  still says *horneado/hornear* in about 15 strings (the bar's "7 horneados", "Master horneado", the
  question, the offer, notices) — and so does the bar inside the ES recording-mode pictures.
  Proposed for the app: *pre-renderizado* (adj.), *pre-renderizar* (verb), keeping *renderizar /
  render* for the act. **Done**: r36/record changed the app (a43a4145); the ES recording-mode
  pictures were re-shot and read "7 pre-renderizados … 2 siguen en vivo … Salir", nothing
  colliding (`site-record-picture-es-light.webp`). The re-shoot also found that the *question*
  pictures (EN and ES) carried a status line "Dropout: the engine took 1.5 ms of 1.3 ms" — the
  live song run by the shot on a busy machine; `shoot_showcase` no longer runs the engine for
  the question, and the site's and README's `record-ask` pictures are clean now. After r36/record shortened the
  master rows ("Baked: the whole mix in one render", "Pre-renderizado: toda la mezcla en un render"),
  the question pictures were re-shot once more: every row whole in EN/ES, Night/Paper.
- **Spanish pictures**: `shoot_skins --es` (new flag) draws every editor in Spanish; the ES page
  shows `assets/units/*-es.webp` (cards, pages, the A/B section), the desk in Spanish
  (`shoot_showcase` now shoots `mix-snap` in EN and ES), and the editor's, external gear's and
  the surfaces' Spanish shots. ARA's clip picture has almost no text and is shared.
  `assets/units/` is now 16 MB (only the clicked page's large picture loads).

## Needs the owner

- Eyes on the new sections (`docs/round36/showcase/`), and on the unit copy (`web/src/units_data.rs`).
- `VERSION` in `web/src/top.rs` is still beta.10 (the integrator's at release); the macOS row
  shows "Coming soon" until the release holds `Cantus-macos-universal.dmg`.
- Done: the external-gear and surfaces pictures (site EN/ES, README `docs/images/external-gear.png`)
  were retaken from r36/external's re-shot settings page ("Hardware Synth", "HUI port"). No picture
  of this branch names a product brand; the protocol names (Mackie Control, HUI) and the driver
  name ASIO4ALL (in the demo interface's top-bar chip of the recording-mode and desk pictures)
  are the only third-party names, used as names of what they are.
