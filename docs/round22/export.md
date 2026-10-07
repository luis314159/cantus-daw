# Round 22 — export: MP3, FLAC, Ogg Vorbis, loudness and the export dialog

Builder: **export** (`r22/export`, base c8bc9f2). Decision record: ADR 0033.

## Done

**A crate, `cantus-export` (GPL-3.0-or-later)**, after the engine's render, never instead
of it:

- `settings` — `ExportSettings` (scope, format, depth, MP3 CBR/VBR, Ogg quality, rate,
  channels, dither, loudness, tail, stem choices, tags, folder; serde defaults
  everywhere), and the four presets (`Preset::{Streaming, Cd, MasteringStems, Podcast}`:
  `apply`, `matches`, and `ExportSettings::preset()` for the lit card).
- `process` — channels (L+R)/2, the engine's windowed-sinc converter
  (`cantus_engine::samples::resample`), BS.1770-4 loudness and true peak with Terminus's
  meter, normalisation with Terminus's limiter (true-peak, its latency taken back,
  iterated to 0.05 LU), the cut to 16/24-bit integers with Terminus's `Ditherer` (TPDF
  or shaped), digital silence kept exact.
- `encode` — WAV (our writer: PCM 16/24, IEEE float 32 with `fact`, a `LIST/INFO`
  chunk), FLAC (`flacenc`, pure Rust, VORBIS_COMMENT added, progress and stop through its
  `Source`), MP3 (LAME 3.100 through `mp3lame-encoder`: q2, CBR 128–320 or VBR V0–V6,
  the output rate pinned, ID3v2.3 before the stream, the LAME/Xing info frame written over
  the first frame at the end so the file is gapless), Ogg Vorbis (`vorbis_rs`, libvorbis
  with aoTuV, quality VBR, comments, a deterministic serial).
- `tags` — ID3v2.3 (Latin-1 or UTF-16), Vorbis comments, RIFF INFO; the credit
  `Cantus <version>` in every file (TSSE / ENCODER / ISFT).
- `job` — `export_mix` (the render, then the pipeline), `export_rendered` (the pipeline
  alone), `Control` (phase, fraction, stop; `stop_at` for exact tests), `Done` (files,
  seconds, rate, peak, loudness report, silent stems), `Part` (`name.ext.part`, synced,
  renamed; removed when dropped unfinished).
- `stems` — `export_stems`: the engine's one-pass stems into a hidden scratch folder as
  float WAV, then each converted and encoded into place (`NN Name.ext`), never
  normalised; a stop or a failure removes every stem already written and the folder.

**MP3 encoder chosen by measurement** (ADR 0033 §2): LAME beats the pure-Rust rusty_mp3
and shine-rs on a noise-to-mask measure at every bitrate (table below), builds from
source with the compiler the workspace already needs, and is LGPL — fine in a GPL app.

**The dialog (`views::export`, `export_dialog`, `export_rows`)**, all kit: a floating
panel over a dimmed room (as the first-run setup), the presets as cards with *Custom*,
banded rows (What · Format · Loudness · Tags), segmented controls, menu fields opening kit
menus, lamps, text wells, a one-line summary of what will be written, a footer with the
progress (phase and percentage, or a gliding block while the render cannot say) and
Cancel/Export… or Stop. *File ▸ Export audio…* (Ctrl+E) and *File ▸ Export stems…* open
it; *Export…* asks for a file (or a folder for stems) and runs on the `cantus-export`
thread. The last choices (all but the title, which is the song's) are kept in
`settings.json` (`Settings::export`) when the dialog closes or an export starts.
`CantusApp::export_to(path)` keeps working and takes the format from the extension.
Both languages (`locales/areas/export_audio.{en,es}.txt`).

## Measurements

MP3 encoders (16 s of music-like material, 44.1 kHz stereo; symphonia decodes; a
Johnston NMR over 25 critical bands — lower is better, *over* = band-frames whose noise
is above the mask; machine shared with five builders, so speeds are indicative):

| | 128 CBR | 192 CBR | 320 CBR | VBR best | ×RT @320 |
|---|---|---|---|---|---|
| LAME 3.100 q2 | −10.2 dB / 11.7 % | −15.2 / 0.9 % | −26.1 / 0 % | V0 −21.7 (228 kbps) | 13 |
| rusty_mp3 1.0 | −9.6 / 23.3 % | −13.9 / 1.1 % | −21.6 / 0 % | q0 −17.9 (245 kbps) | 19 |
| shine-rs 0.1 | −2.8 / 41.7 % | −6.7 / 19.3 % | −11.1 / 0 % | — | 16 |

LAME q0 vs q2 at 320: −26.2 vs −26.1 dB, 4× vs 13× real time → q2. The program is in
`docs/round22/mp3-eval/` (standalone; `cargo run --release --manifest-path …`).

RESULTS-PLACEHOLDER

## Tests

TESTS-PLACEHOLDER

## Files outside my area

- `Cargo.toml` (workspace member, default member, `cantus-export` path dependency),
  `Cargo.lock`.
- `crates/cantus-ui/Cargo.toml` (+ `cantus-export`), `src/app.rs` (the `export` field is
  now `views::export::ExportUi`; `export_dialog`, `export_to`, `export_in_progress`,
  `poll_export` are thin calls into `views::export`; `export_dialog::show` after
  `interop::frame`; two imports gone), `src/settings.rs` (`Settings::export`),
  `src/i18n.rs` (two lines), `src/views/mod.rs` (three modules), `src/views/interop.rs`
  (*Export stems…* opens the new dialog), `locales/{en,es}.txt` (*Export WAV…* →
  *Export audio…*, two lines each).
- Nothing in the engine: `render.rs` and `stems*.rs` are untouched (pool's this round).

## Left

- **Progress and stop inside the mix render** — asked of pool in
  `docs/round22/requests.md`; until then the bar pulses while the song renders and a stop
  takes effect when the render returns (nothing is written either way).
- Hosted plug-ins in stems: the stems render builds wires for them (as before this round).
- WAV over 4 GB (RF64), AAC/Opus, album art, a loudness report window (the numbers are in
  `Done::loudness` and the status line).
- The old stems dialog in `views::interop` is no longer reachable from the menu; it stays
  for `interop_flows.rs` and `shoot_interop`, and can go when those move to the new one.

## Notes for the integrator

- ADR number **0033**: rename if another branch took it.
- `THIRD-PARTY-NOTICES` (beta-docs): LAME (LGPL-2.0-or-later, via `mp3lame-sys`),
  libvorbis/libogg (BSD-3-Clause, with the aoTuV/Lancer patches), flacenc (Apache-2.0);
  symphonia (MPL-2.0) is a test dependency only.
- **math2** changes Terminus's `Ditherer` to counter-based TPDF: `process::quantise` calls
  `Ditherer::new(seed)` and `process(x, bits, shaping)`. If the signature changes, that
  one function follows; the FLAC/WAV round-trip tests compare against `quantise` itself,
  so they keep holding whatever the dither's noise.
- Settings: other builders adding fields to `Settings` merge beside `export` trivially.
- First build: LAME and libvorbis compile from C (MSVC, via `cc`), ≈1 min on a quiet
  machine; nothing to download beyond crates.
