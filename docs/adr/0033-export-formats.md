# ADR 0033 — Export formats: MP3, FLAC, Ogg Vorbis, loudness and the export dialog

**Status:** Accepted · **Date:** 2026-10-06 · **Round:** 22 (export) · **Extends:** ADR 0001 (licence), ADR 0021 §3 (stems), ADR 0024 (the kit), ADR 0026 (the master chain)

> Numbering: written as 0033 on `r22/export`; if another round-22 branch takes the number
> first, this one moves up at the merge.

## Context

Cantus exported 32-bit float WAV only — the song (`render_to_wav`) and stems
(`render_stems_to_folder`). The first thing a beta user asks for is an MP3
(`docs/BETA.md`, *Strongly recommended*), then FLAC and Ogg; with them come the choices
every DAW's export dialog has: bit depth, sample rate, bitrate, dither, tags, loudness
for the streaming services, presets, stems in the same formats, progress and a stop.

The render belongs to the engine (and to the pool builder this round): what is written
must be what the engine plays (ADR 0021 §1). So export is a stage *after* the render, in
a crate of its own.

## Decisions

### 1. `cantus-export`, GPL-3.0-or-later

A new crate between the engine and the interface: `cantus-ui → cantus-export →
cantus-engine`. It takes what `render_with_instruments` and `render_stems_to_folder`
produce and writes files; it never renders differently. It is GPL because it links LAME
(LGPL) and libvorbis (BSD-3-Clause), and dependency direction is licence direction
(ADR 0001): the engine stays MIT OR Apache-2.0, untouched by either C library. LGPL in a
GPL program whose whole source is published meets LGPL's relinking terms by itself.

### 2. The encoders

| Format | Encoder | Licence | Build |
|---|---|---|---|
| FLAC | `flacenc` 0.5 (pure Rust) | Apache-2.0 | nothing |
| MP3 | LAME 3.100 via `mp3lame-encoder` 0.2 / `mp3lame-sys` | LGPL | C, compiled from the vendored source by `cc` |
| Ogg Vorbis | libvorbis 1.3.7 + aoTuV/Lancer via `vorbis_rs` 0.5 | BSD-3-Clause | C, compiled from the vendored source by `cc` |

Both C libraries come as source inside their crates and build with the MSVC compiler the
workspace already needs for `asio-sys`: no download beyond `cargo fetch`, no manual step,
the same in CI. (On this machine, from a cold cache: LAME and libvorbis add about a
minute to a full build, most of it libvorbis.)

**MP3 — LAME, measured.** Three encoders were compared on 16 s of music-like material
(a saw-pad chord, a bass, kick, snare and hats, then a quiet decaying-tone passage,
44.1 kHz stereo), decoded by symphonia and scored by a Johnston-style noise-to-mask
ratio (25 critical bands, spreading, tonality from spectral flatness, the absolute
threshold; lower is better; *over* is the share of band-frames whose coding noise rises
above the mask, i.e. audible):

| Encoder | 128 kbps CBR | 192 kbps CBR | 320 kbps CBR | VBR best (avg kbps) | Speed (×RT, 320) |
|---|---|---|---|---|---|
| **LAME 3.100** (q2) | −10.2 dB, 11.7 % over | −15.2 dB, 0.9 % | −26.1 dB, 0 % | V0: −21.7 dB, 0 % (228) | 13× (q2), 4× (q0) |
| rusty_mp3 1.0 (pure Rust) | −9.6 dB, 23.3 % over | −13.9 dB, 1.1 % | −21.6 dB, 0 % | q0: −17.9 dB, 0 % (245) | 19× |
| shine-rs 0.1 (pure Rust) | −2.8 dB, 41.7 % over | −6.7 dB, 19.3 % | −11.1 dB, 0 % | — (CBR only) | 16× |

LAME is better at every rate — at 128 kbps twice as few band-frames have audible noise as
the best pure-Rust encoder, at 320 kbps 4.5 dB more margin — and it spends fewer bits in
VBR for more margin. Shine (fixed-point, no real psychoacoustics) is not in the race.
rusty_mp3 is new (1.0) and promising; it can be measured again in a later round, and the
writer is one function to swap. LAME's quality setting is **q2** (`-h`, LAME's own
recommendation): the same NMR as q0 to 0.1 dB at three times the speed. (Raw SNR — which
favours the pure-Rust encoders, since they do not low-pass — says nothing about what is
heard; the method and the program are in `docs/round22/mp3-eval/`.)

LAME is told the output rate (never left to resample), writes its **info tag** (Xing/LAME:
the length, the encoder delay and padding) over the empty first frame once the stream
is complete, so players that honour it — most do, symphonia included — play the file
gaplessly at its exact length; an ID3v2.3 tag goes before it (ID3v2.3, not 2.4: Windows
and older players still read 2.4 badly; Latin-1 when the text fits, else UTF-16).

**Ogg**: quality VBR, `-q 0…10`; the stream serial is derived from the title and the
length, so the same export makes the same bytes. **FLAC**: flacenc's default (block 4096,
LPC search), 16 or 24 bits, MD5 in STREAMINFO, a VORBIS_COMMENT block added.

### 3. The pipeline after the render

Fixed order: **channels → sample rate → loudness → integers with dither → encoder**.

- **Channels.** Stereo to mono is (L + R)/2: a centred sound keeps its level.
- **Rate.** The session renders at its own rate; another rate goes through the engine's
  converter (`samples::resample`: Kaiser-windowed sinc, β = 10, ≈100 dB of rejection,
  32 zero crossings, band edge at 95 % of the narrower Nyquist, DC exact). Rendering at
  the target rate instead would make a file the engine never played (ADR 0021 §1). MP3
  carries 32, 44.1 and 48 kHz; asked for more, it takes the same family's rate below
  (88.2 → 44.1, 96 → 48), and the dialog says so.
- **Integers.** 16 and 24 bits through Terminus's `Ditherer` — one per channel, seeded
  the same every time so an export is repeatable; *TPDF* (flat) or *Shaped* (Lipshitz's
  five taps, Terminus's *Optimized*), or off. Digital silence stays exact zeros. MP3 and
  Ogg take the float samples (their encoders' own input), so they are never dithered;
  32-bit float WAV is the engine's samples untouched.

### 4. Loudness

- Measured to **ITU-R BS.1770-4 with EBU R128 gating** by Terminus's meter (the same code
  the limiter's editor shows, conformance-tested in `terminus_loudness.rs`): integrated
  loudness and true peak (4× oversampled).
- **The gain** is target − measured, at most +30 dB. If it would put a true peak over the
  ceiling: with *Hold the peaks* on (the default), Terminus limits at the ceiling (clean
  style, true-peak detection, 5 ms look-ahead, its latency taken back so nothing moves);
  limiting costs a little loudness, so the gain is corrected and the pass repeated until
  the result is within 0.05 LU (two or three passes; six at most, the best kept). Off,
  the gain stops where the peak meets the ceiling and the file comes out quieter than
  asked — the status line says by how much.
- **Mono is measured as dual mono** — the one channel heard from both speakers — so
  −16 LUFS means the same loudness for a mono podcast as for a stereo song. (BS.1770's
  single channel would read 3 dB lower; AES TD1004's "−19 LUFS mono" is this same level.)
- **Never for stems**: normalised one by one they would no longer add up to the mix.
- After encoding, an MP3's decoded true peak can rise a few tenths of a dB over the
  ceiling (the codec's ringing) — which is why streaming targets leave −1 dBTP.

### 5. Tags and the credit

Title (the song's name by default), artist, album, year, comment — as ID3v2.3 frames
(TIT2, TPE1, TALB, TYER, COMM), Vorbis comments (TITLE, ARTIST, ALBUM, DATE, COMMENT) in
FLAC and Ogg, and a RIFF `LIST/INFO` chunk in WAV (INAM, IART, IPRD, ICRD, ICMT), placed
before `data`. Every file also says what made it — `Cantus <version>` as TSSE, ENCODER or
ISFT. Empty tags are left out. A stem's title is `Song — Stem`.

### 6. Nothing half-written, progress and stop

A file is written as `name.ext.part` beside its place, flushed and synced, then renamed
over the destination; stopped or failed, the part is removed — an old file of the same
name is replaced only by a whole new one. Stems render into a hidden scratch folder
(`.cantus-stems-<pid>`) as float WAV, are encoded one by one into place, and on a stop
or a failure every stem already written is removed with the folder. `Control` carries the
phase and the fraction out (render, convert, loudness, encode) and the stop in. The mix
render cannot report or stop midway yet — the bar pulses through it; the hook is asked of
the render's owner in `docs/round22/requests.md`.

### 7. Presets

| Preset | Format | Rate | Loudness | Other |
|---|---|---|---|---|
| Streaming | MP3 320 kbps CBR | 44.1 kHz | −14 LUFS, −1 dBTP, limiter | stereo |
| CD | WAV 16-bit | 44.1 kHz | as mastered | shaped dither |
| Stems for mastering | WAV 24-bit | the session's | never | every strip, not through the master chain |
| Podcast | MP3 128 kbps CBR, mono | 44.1 kHz | −16 LUFS (dual mono), −1 dBTP | |

A preset sets every choice it names and keeps the tags, the folder and the tail; the
dialog lights the preset the choices still match, or *Custom*.

### 8. The dialog

*File ▸ Export audio…* (Ctrl+E, keymap id unchanged) and *Export stems…* open one dialog,
drawn with the kit over a dimmed room: presets as cards down the left; on the right,
banded rows — *What* (Song | Selection | Stems, the tail; for stems strips or tracks, the
master mix, silent stems, through the master chain), *Format* (WAV | FLAC | MP3 | Ogg,
then bit depth, CBR/VBR and bitrate, or Ogg's quality, sample rate, channels, dither),
*Loudness* (target, ceiling, hold the peaks), *Tags* — and a one-line summary of what
will be written. The footer has the progress (phase and percentage, or a pulse) and the
buttons. *Export…* asks where (a file, or a folder for stems) and starts the export on a
thread of its own. The last choices — all but the title — are kept in `settings.json`
(`Settings::export`) when the dialog closes or an export starts.

## What the tests hold

`crates/cantus-export/tests/`: FLAC at 16 and 24 bits, with and without dither, decodes
(claxon) to **exactly** the integers the pipeline made; integer WAV likewise and 32-bit
float WAV to the engine's samples; MP3 (CBR 128/320, VBR V0/V4) and Ogg (q3/6/10) decode
(symphonia, lewton) to **the exact length** and to within 0.25 dB of level and 1 dB per
octave band from 63 Hz to 8 kHz; a mono 48 kHz MP3 at its bitrate; a 48 → 44.1 kHz WAV
within 0.05 dB/0.2 dB; tags read back by symphonia (MP3, WAV), claxon (FLAC) and lewton
(Ogg), the credit in every file; loudness lands within ±0.5 LU (quiet mix raised, loud
target limited with the true peak under the ceiling, the Streaming preset as an MP3, the
Podcast preset as mono), silence untouched, the limiter off stops at the ceiling; the
mix export is the engine's render to the bit (FLAC 24); stems in every format and FLAC
stems equal to the engine's stems to the bit; a stop at 30 % in every format, and a
stems export stopped while rendering or while encoding, leaves nothing; an existing file
is replaced only by a whole one. `crates/cantus-ui/tests/export_dialog.rs`: the dialog
opens with the last choices and the song's title, the choices persist in the settings
file and read back, an export runs off the interface thread, `export_to` takes the format
from the extension, a stop leaves no file, stems come out in the chosen format, the
dialog renders in both themes, both languages and every preset, and every key has words
in both languages. Shots: `cargo run -p cantus-ui --example shoot_export -- <dir> 1 [es]`.

## Consequences

- New: `crates/cantus-export` (`settings`, `tags`, `process`, `encode`, `job`, `stems`),
  `views::{export, export_dialog, export_rows}`, `Settings::export`,
  `locales/areas/export_audio.*`. The File menu's *Export WAV…* is *Export audio…*; its
  *Export stems…* opens the new dialog (the old stems dialog in `views::interop` stays for
  its tests and is no longer reachable from the menu).
- Dependencies: `flacenc` (Apache-2.0), `mp3lame-encoder`/`mp3lame-sys` (LGPL),
  `vorbis_rs`/`aotuv_lancer_vorbis_sys`/`ogg_next_sys` (BSD-3-Clause); for tests
  `symphonia` (MPL-2.0). THIRD-PARTY-NOTICES must name LAME and libvorbis.
- Left: progress and stop inside the mix render (asked of pool); hosted plug-ins in stems
  (the stems render builds wires for them, as before); RF64 for WAV over 4 GB; AAC/Opus;
  album art; a loudness report window (the measurements are in `Done::loudness`).
