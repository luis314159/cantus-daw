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

What the tests measured on the written files: FLAC and integer WAV decode to **exactly**
the integers the pipeline made (16/24 bit, dither off, TPDF, shaped); MP3 (CBR 128/320,
VBR V0/V4) and Ogg (q3/6/10) come back at **exactly** their length (LAME's info tag,
Vorbis's granule position), within 0.25 dB of level (0.6 dB at 128 kbps and V4, which
low-pass above 16–17 kHz) and 1 dB per octave band 63 Hz–8 kHz; a 48 → 44.1 kHz WAV
within 0.05 dB of level; loudness-normalised exports land within ±0.5 LU of −14, −9 and
−16 (mono, dual-mono measure), the true peak under the −1 dBTP ceiling when limiting.

## Tests

36 new tests, all green; clippy clean on `cantus-export` and `cantus-ui` (tests and
examples).

- `crates/cantus-export` (28): `round_trip.rs` (7: FLAC bit-exact ×4 settings, WAV exact
  in 16/24/32f, MP3 length/level/spectrum ×4 modes, mono 48 kHz MP3 and its bitrate, Ogg
  ×3 qualities, rate conversion, MP3 rate fallback), `tags.rs` (5: MP3, WAV, FLAC, Ogg read
  back by symphonia/claxon/lewton, empty tags left out with the credit kept),
  `loudness.rs` (7: quiet mix raised, loud target limited, limiter off stops at the
  ceiling, Streaming preset as MP3, Podcast preset mono, silence untouched, stems never
  normalised), `jobs.rs` (5: the mix equals the engine's render to the bit, stems in all
  four formats with FLAC stems equal to the engine's to the bit, a stop at 30 % in every
  format leaves nothing, a stems export stopped rendering or encoding leaves nothing, an
  old file is replaced only by a whole one), unit tests (4: ID3 size, RIFF padding, the
  Vorbis block, stem names).
- `crates/cantus-ui/tests/export_dialog.rs` (8): opens with the last choices and the
  song's title (and on stems from the menu; a missing selection falls back to the song),
  the choices persist in `settings.json` and read back, an export runs off the interface
  thread and the status says so, `export_to` takes the format from the extension, a stop
  leaves no file, stems in the chosen format, the dialog renders in both themes × both
  languages × every preset, every key has words in both languages.
- Unchanged and still green: `app_flows` (its WAV export now goes through the new path),
  `interop_flows`, `kit_lint`, the settings unit tests.
- Shots: `cargo run -p cantus-ui --example shoot_export -- <dir> 1 [es]` — Streaming,
  stems for mastering, podcast, a custom FLAC with tags and an export under way, light
  and dark, EN and ES (20 images); looked at in both themes and languages.

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
- **Notices**: `mp3lame-encoder`/`mp3lame-sys` declare `LGPL-3.0`; per the integrator's
  ruling the `xtask notices` allow-list needs the LGPL ids (exact lines and every crate
  this branch adds, with licences, in `docs/round22/requests.md`), then
  `cargo run -p xtask -- notices`. The generator is on `r22/beta-docs` only, so it is not
  changed here. symphonia (MPL-2.0) is a test dependency only.
- **Manual and release notes** (beta-docs' chapter 8 and the "WAV only" known issue): the
  formats are WAV (16-bit, 24-bit, 32-bit float), FLAC (16, 24-bit), MP3 (CBR 128, 160,
  192, 224, 256, 320 kbps; VBR V0, V2, V4, V6; 32/44.1/48 kHz; mono or stereo) and Ogg
  Vorbis (quality 3, 4, 5, 6, 8, 10); sample rate: the session's, 44.1, 48, 88.2, 96 or
  192 kHz (MP3 falls back to 44.1/48); dither off, TPDF or shaped (16/24-bit only);
  loudness off or −9, −11, −14, −16, −19, −23 LUFS with a −0.1, −0.3, −1 or −2 dBTP
  ceiling and an optional true-peak limiter; tags title, artist, album, year, comment;
  song, time selection or stems (strips or tracks), all formats. Presets: **Streaming**
  (MP3 320 CBR, 44.1 kHz, −14 LUFS, −1 dBTP), **CD** (WAV 16-bit, 44.1 kHz, shaped
  dither), **Stems for mastering** (every strip, WAV 24-bit, session rate, no
  normalisation), **Podcast** (mono MP3 128 CBR, 44.1 kHz, −16 LUFS, −1 dBTP). Menu:
  *File ▸ Export audio…* (Ctrl+E) and *File ▸ Export stems…*.
- **math2** changes Terminus's `Ditherer` to counter-based TPDF: `process::quantise` calls
  `Ditherer::new(seed)` and `process(x, bits, shaping)`. If the signature changes, that
  one function follows; the FLAC/WAV round-trip tests compare against `quantise` itself,
  so they keep holding whatever the dither's noise.
- Settings: other builders adding fields to `Settings` merge beside `export` trivially.
- First build: LAME and libvorbis compile from C (MSVC, via `cc`), ≈1 min on a quiet
  machine; nothing to download beyond crates.
