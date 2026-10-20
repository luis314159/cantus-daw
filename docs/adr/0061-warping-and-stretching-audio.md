# ADR 0061 — Warping and stretching audio: our own stretcher, warp markers, analysis at import

**Status:** Accepted (round 29, *warp*; parts A and B — the stretcher and playback under
the tempo map, after the tempo builder's merge; amended by its review, §10) · **Date:** 2026-10-16 · **Round:** 29 (warp) · **Builds on:** ADR
0053 (the pitch base: the tracker, the epochs, TD-PSOLA), ADR 0054 (the pitch render path: a
worker, a disk cache, renders attached to the snapshot) · **Extends:** ADR 0004 (commands),
ADR 0023 (analysis stored), ADR 0031 (offline = real time), ADR 0047 (licences) ·
**Replaces:** ADR 0025 §1's *"warping is a later decision"* for warped clips · **Serves:**
BACKLOG §3 *Time* and *Editing*

> Measurements and tests: `docs/round29/warp.md`. The manual: §5, *Warping and stretching
> audio*.

## 1. Licences (reviewed before any code was written)

The stretcher, the shifter, the onset detector and the tempo/beat tracker are **our own
implementation, in `cantus-dsp` (MPL-2.0)**, from papers. No third-party code is copied,
ported or linked; no new crate is added; `THIRD-PARTY-NOTICES.md` does not change.

| Reference | Licence | How it was used |
|---|---|---|
| Rubber Band (R2/R3) | GPL-2.0-or-later or commercial | **Not read for code, not ported.** A listening reference only. `research/engine/dsp-quality.md` §2 and `README.md` say it is "usable since Cantus is GPL" — that predates ADR 0047: `cantus-dsp` and `cantus-engine` are MPL-2.0, so GPL code can never enter them, and a GPL crate (the UI) could not feed the engine's render path. This ADR corrects those notes. |
| SoundTouch | LGPL-2.1 | Not used, not ported (WSOLA is textbook; we do not use it). |
| zplane élastique | commercial | A listening reference only (in Ableton Live, Cubase, FL Studio). |
| Signalsmith Stretch | MIT | Studied as a *design* (G. Luff, ADC 2022, "Four ways to write a pitch-shifter"): pitch-shifting by **mapping STFT bins** with the spectral envelope corrected, inside the time-stretcher, instead of stretch-then-resample. The idea is used; no code was copied (if it ever is, it is carried with its MIT notice). |
| Bungee | MPL-2.0 | Studied as a *design*: a fixed synthesis hop with analysis frames placed anywhere by a time map, and the instantaneous frequency taken from **two analysis frames one synthesis hop apart** (so the analysis hop may vary freely). The idea is textbook (Flanagan and Golden 1966; Dolson 1986) and is ours; no code was copied. |
| aubio, Loris | GPL | Not read. |
| Papers | — | J. Laroche, M. Dolson, *Improved phase vocoder time-scale modification of audio*, IEEE TSAP 7(3), 1999 (identity phase locking around spectral peaks); J. Driedger, M. Müller, *A review of time-scale modification of music signals*, Appl. Sci. 6(2):57, 2016, and Driedger, Müller, Ewert, *Improving TSM using harmonic-percussive separation*, IEEE SPL 21(1), 2014; Z. Průša, N. Holighaus, *Phase vocoder done right*, EUSIPCO 2017 (PGHI); A. Röbel, *A new approach to transient processing in the phase vocoder*, DAFx 2003 (phase reset at onsets); J. Bonada, *Automatic technique in frequency domain for near-lossless time-scale modification of audio*, ICMC 2000; A. Röbel, X. Rodet, *Efficient spectral envelope estimation and its application to pitch shifting and envelope preservation*, DAFx 2005 (true envelope); D. Ellis, *Beat tracking by dynamic programming*, JNMR 36(1), 2007; S. Böck, G. Widmer, *Maximum filter vibrato suppression for onset detection* (SuperFlux), DAFx 2013; S. Dixon, *Onset detection revisited*, DAFx 2006. |

The licence rule stands for later work: a GPL/LGPL stretcher may be *heard* as a reference;
MIT/BSD/Apache/MPL code may be used with its notice, recorded here first.

## 2. The stretcher (`cantus_dsp::stretch`, offline, deterministic)

One render reads the source through a **time map** (output frame → source frame,
piecewise linear; `stretch::Map`) with a transposition, in the clip's **mode**:

- **Complex** (the default, mixes and anything polyphonic). The input is split into its
  harmonic and percussive parts by median filtering of a 21 ms spectrogram (Driedger,
  Müller, Ewert 2014) — a bin counts as percussive only where the frequency median stands
  clearly over the time median (a separation factor of 2 with a soft edge), so a dense
  chord or a held noise stays whole with the vocoder (with a plain Wiener mask a held
  chord fluttered by 3–5 dB through the grains; now ≤ 0.7 dB). The two parts always sum
  back to the input. The **harmonic part** goes through a phase vocoder: 170 ms Hann frames
  (8 192 at 44.1/48 kHz; 85 ms until the review, §10), a synthesis hop of N/8, each synthesis frame reading the source
  where the map says and a second frame one hop earlier, so every bin's instantaneous
  frequency comes from two frames exactly a hop apart whatever the local speed;
  zero-phase frames; **identity phase locking** around peaks (Laroche–Dolson), channel by
  channel, with the right channel's phase locked to the left's analysed difference
  wherever both hold the same partial (to a twenty-fifth of a bin, §10) — the image is the
  source's, and a chorus's partials that differ left and right each keep their own phase.
  Around a transient the map is held to a pure shift for a whole window before it and
  30 ms after (`Map::hold`: taken from the room on either side, at most 45 %, never across
  a warp marker), and there every frame resets the peaks whose energy rose (their regions take
  the analysed phase; a sustained partial goes on), so a hit read through a stretch is the
  source's hit; transposed, the phases reset once at the transient (Röbel 2003).
  **Transposition** moves each peak's whole region by its true frequency times the ratio
  less one — fractionally, the magnitudes read between bins by a windowed sinc — so a
  partial keeps its lobe and its level (§10);
  **formants** are kept by moving each bin back under the source's **true envelope**
  (Röbel–Rodet; 16 rounds, a cepstral order of 1.25 ms, on the spectrum pooled to a
  512-point grid: the same envelope at an eighth of the work). The **percussive part** is
  re-laid by 5 ms Hann grains a quarter apart through the map held around transients
  (4 ms before, 30 ms after), read at the transposition's rate.
- **Rhythm** (drums, percussive loops; Ableton's *Beats*). Slices start at every
  transient and every warp marker; each plays at its own speed (resampled by the
  transposition) from the frame where the map puts its transient — the slice is the source
  sample for sample there — and is cut with a 3 ms fade where the next begins (none when
  the two join seamlessly), or, when it runs out first (the clip slowed), ends by its
  **tail**: a fade, a forward loop of its last 60 ms with a 5 ms cross-fade, or back and
  forth.
- **Melodic** (a voice, a solo line; Ableton's *Tones*). TD-PSOLA on the pitch base's
  epochs (ADR 0053) with the same time map, the ratio constant, the formants kept by the
  grains (moved with the pitch when not kept); where the line is unvoiced or loosely
  periodic (YIN aperiodicity over 0.3: more than one voice) the Complex render is
  cross-faded in over 10 ms.
- **Re-pitch** (tape). The source read at the map's speed through Exemplar's 32-tap
  windowed-sinc kernel on the octave mip level where the local speed is at most 1 (no
  alias); it does not transpose: its pitch is its speed.

**Identity:** a map that reads the source sample for sample (slope 1 at a whole-frame
offset, `Map::plain_over`) and no transposition return the source **to the bit**, in every
mode. Every render is exactly the requested length.

## 3. Analysis at import (`cantus_dsp::onset`, `cantus_dsp::beat`, `cantus_engine::warp`)

- **Onsets:** SuperFlux — a log-magnitude spectrogram (46 ms Hann, 200 frames a second)
  on a 24-band-per-octave triangular filterbank from 30 Hz to 16 kHz; the positive
  difference to the frame a quarter window back after a 3-band maximum filter (a vibrato is
  not a note); weighed by the share of power the frame kept (an end's spectral splatter is
  not an onset); picked as the largest within ±30 ms over the mean of −100…+70 ms by 6 % of
  the function's top, 30 ms apart; kept only where the next 20 ms are 3 dB louder than the
  20 ms before (as they are, or in their first difference: a hi-hat over a kick's tail);
  each refined (within a millisecond, early rather than late: never more than half a millisecond late in the tests) — the 0.3 ms block whose level jumps most over the 10 ms
  before it within the frame's reach, walked back to a tenth of what it reaches.
- **Tempo:** the autocorrelation of the normalised onset function, combed over its first
  four multiples and weighted by a log-Gaussian an octave and a half wide around 120 BPM,
  its peak refined by a parabola; the octave put inside **70–180 BPM**, then between a
  tempo and its double the faster when the slower's off-beats carry at least 55 % of its
  beats' strength, the slower when the faster's off-beats are weaker; a confidence from the
  peak's height over the median.
- **Beats:** Ellis's dynamic programme (tightness 100), each beat moved onto the onset
  within 35 ms, a beat with none placed from its found neighbours; the tempo refitted by
  least squares through the beats. **Downbeat:** of the ways to count bars, the one whose
  first beats carry the most low (< 200 Hz) onsets, the earliest when two are within 15 %.
  **Loop:** a file that starts on its downbeat (within 30 ms) and lasts a whole number of
  bars within 2 % of a bar — its tempo then made exact from its length. The WAV **`acid`**
  chunk's tempo, when the file has one, wins.
- **Where:** a sibling of the pitch service, `WarpService` (not one generalised service:
  the pitch path is round 28's, reviewed; the two keep their own worker and cache, and the
  sample cache counts both in `loading()`) — one worker for analyses and full renders
  (renders first, a newer render for a clip replacing a waiting one) and one for bridges;
  analyses in memory and on disk in `<data>/cache/warp/<key>.onsets` (FNV-1a over the
  file's name, size, time, rate and the analyser's version), the folder set in
  `CantusApp::assemble` (the boot path; `tests/warp_boot.rs`). Never on the interface
  thread.

**The import rule** (every import path — *File ▸ Import*, a drop from the system or from
the browser, all through `import_audio`): the import is one undo step kept open; when the
file's analysis lands, a **loop** (no longer than 60 s, a whole number of bars or an `acid`
tempo, a confidence of 0.5 or more) is warped at once — markers on every bar of its grid,
Rhythm when it holds a transient a beat or more, Complex otherwise — and its window set to
its bars at the project's tempo, **in the import's own undo step** (`exec_merging` with the
import's key; if the user did something else first, it is its own step). A long file plays
at its natural speed until the user warps it (a recovered take too: §10). A **recording** is analysed as it lands and
never warped by itself: it plays as it was played. A **paste** keeps what it copied
(warped or not); its analysis is asked when the editor, the inspector or *Quantise audio*
needs it.

## 4. The model (schema 34)

`AudioRegion::warp: Option<Box<Warp>>` — `rate` (the source's), `mode`, `bpm` (the
source's tempo outside the markers), `markers` (each a **source frame** with a **content
tick**, both strictly increasing), `cents`, `formants`, `tail` (Rhythm), `follow` (*Follow
the project tempo*), `detected` (tempo, confidence, downbeat, bars: what the analysis
said). Defaults are not written: a clip never warped saves the same bytes. The map runs
through the markers and beyond them at the slope of the segment beside them (one marker or
none: the source's tempo). Content ticks and source frames, never timeline ticks: split,
trim, slip, glue (glued only with the same warp), loops (the pieces share the content),
links (the warp is content, ADR 0037), the clipboard and comping keep every marker valid.
**Comp seams:** a warped window takes no seam (`comp::seam_window`): its render would need
the seam in the warp map. **Reverse:** the render is of the window forwards and the engine
reads it backwards, as before. **The pitch edit chains:** pitch in source frames first (its
render), then the warp over it.

`Command::SetClipWarp { track, clip, warp, op }` replaces the whole warp; the inverse is the
warp as it was; `op` (`WarpOp`) names it for the undo menu (`cmd.warp.*`) and merges a drag
(marker, content, transposition, tempo). Refused on a clip that is not audio and for a warp
that is not valid. The edits are pure functions (`warp_edit.rs`): add a marker (on a tick
or on a transient's frame), move it (clamped between its neighbours), slide the content
under it, remove it, quantise (to lines the interface counts through the metres), stretch
(an Alt-drag of the right edge), set the tempo, from the analysis's grid, reset. Undo fuzz
arm 44 with its counter (set, cleared, marker edits; one in nine refused).

## 5. The time contract (replaces ADR 0025 §1 for warped clips; extends ADR 0060)

A warped clip that follows the tempo hears, at timeline tick `T`, content tick
`T − (start − offset)` — wherever the tempo map puts `T` in time — and the source frame its
warp pairs with that content tick. One function, `cantus_core::audio_time::frame_through`
(`clip_frame_at` / `clip_tick_at_frame` for a region, `tick_through` the inverse), serves
the engine's render plan and every interface site (the waveform through the warp, the pitch
editor's `ClipTime`, the transients and markers drawn, the window on the file); a test
holds the engine and the interface to the same bits under a ramp. A warp that does not
follow the tempo plays at natural speed (ADR 0060's rule), transposed. A warp that maps
exactly as the unwarped clip under one tempo, untransposed, **is no warp**: the snapshot
drops it and the clip plays the same bits as before round 29.

## 6. Playback: the render, its key, the bridge

The engine renders each warped window (`warp_render::Plan`): output frame `k` is song time
`seconds(start) + k / rate`, so the window's start is the render's frame 0 — the sample
cache sets the clip's `slip` to `−offset_seconds`, and `Transport::clip_zero` puts frame 0
on the window's first sample: `add_clips` plays it as any source, unchanged. The map's
breakpoints are the window's ends, every marker inside it and, where the tempo moves under
the window, one every 256 frames. **The key** holds the file, the input (the source, or the
pitch render's key once that render is what the clip plays), the warp, the window's content
and length, the tempo map's `span_key` over the window (not its place: a clip moved along a
held tempo keeps its render; a loop's pieces under one tempo share one) and the rates.
**While a render is on its way** the clip plays its **bridge** — the same map read at
varispeed, or, transposed, a WSOLA sketch at the transposition (§10), made in milliseconds
on its own worker — or the newest render or bridge its piece had; never its source at the
wrong speed or pitch; a clip warped for the first time is silent until its bridge arrives
(milliseconds). Kept: what plays, and the renders and bridges most recently played or made
within 512 MB, whether or not their clip is in the snapshot now (§10), freed only where the
cache is the last holder (never on the audio thread). `compile_for_render` waits for the pitch renders, compiles again and waits for the
warp renders: an export, stems, freezing and a bounce play the full renders, never a
bridge. Offline = real time and block-size independence hold (the render is attached data;
tested at 1, 37, 64, 512 and mixed sizes). Nothing new runs on the audio thread.

## 7. The interface

The arrangement draws a warped clip's waveform through its warp (a run of columns per
marker segment), its transients as ticks along a top band and its markers as gold flags.
Double-click the band to add a marker (on a transient: on it), drag a marker (snapped
through `views/bars.rs` unless Ctrl) — **Alt** slides the content under it instead — drag
in the band to add one and move it, double-click a marker, select it and press Del, or use its menu to
remove it; **Alt-drag of the right edge stretches the clip** (drawn stretched while
dragged; Alt because Shift already slips). The clip editor has two tabs, **Pitch** and
**Warp** (ADR 0054's window; ADR 0051 §4 stands — a double-click opens the clip editor in
the tab it was last in): the warp editor's bar ruler, the waveform large, the same
gestures, and a bar with the mode, the transposition, the formants, *Follow*, the clip's
tempo (½, ×2), *Detect tempo*, *Quantise…* and *Reset warp*. The inspector's **Warp**
section and the clip menu's own list offer the same and *Warp from here*, *Set the project
tempo from this clip*, *Edit warp* and *Bounce warped audio* (the window rendered exactly as
the engine plays it, written beside the project's audio on a thread of its own, the clip
then playing that file unwarped — one undo step). *Quantise audio…*: a grid, a strength and
which transients (a share of the strongest), one undo step for every clip chosen.

## 8. Interchange

DAWproject carries audio clips (ADR 0021 amended): `<Clip>` with its fades, `<Warps
contentTimeUnit="seconds">` over `<Audio algorithm channels sampleRate duration>` with its
`<File>` in the archive (`audio/…`; referenced where it is when it cannot be read), the warp
points as `<Warp time contentTime>` (content beats against file seconds; a clip at natural
speed is its natural line). Read back from any application: points on the natural line
come back unwarped with their fine offset; the files are written under the data folder and
the sources pointed at them. What DAWproject has no words for is reported: a transposition,
pitch edits, a reversed clip.

## 9. Measurements (`cantus-bench --full --only warp`; `docs/round29/warp.md`)

After the review (§10); one core, stereo, 48 kHz.

| What | Target | Measured |
|---|---|---|
| Identity (every mode; also through the render path) | the source to the bit | to the bit (tests) |
| Output length | exact | exact (every mode × 0.25–4 × transpositions) |
| A hit's place through markers (every mode; the source's own rise subtracted) | ≤ 12 samples | 0 (bench); ≤ 12 every mode (test) |
| A marker's frame on its tick (engine; every mode; 44.1/48/96 kHz files at 44.1/48/96 kHz; under a ramp) | ±1 sample | ±1 |
| Attack (10–90 %), ×0.5 … ×2; under a tempo ramp | ≤ the source + 1 ms | the source's (92 frames median); 0 added under a ramp |
| Pre-echo before a hit (Complex): ×0.5 / 0.8 / 1.25 / 2; under a ramp | ≤ −30 dB | −31 / −35 / −36 / −35 dB median; −36 … −37 dB |
| Pre-echo before a lone drum hit, detected onsets (the source's own −30 dB) | the source's | −29.6 dB |
| A held tone's level before each hit (×0.8, ×1.25) | ≤ 1 dB | 0.03 dB |
| A transposed render's level (−12/−5/+7/+12 st, tone and dense noise) | ≤ 0.5 dB | 0.03 dB, 0.02 dB |
| Pitch of a shifted tone (−12 … +12 st, ×0.8 … ×1.25) | ≤ 3 cents | ≤ 0.002 cents |
| A held chord's voices, each, both channels: added level wander | ≤ 1 dB | 0.44 dB (×0.8), 0.52 dB (×1.25) |
| A voice +5 st, formants kept (envelope distance) | Complex ≤ 4 dB, Melodic ≤ 3 dB | 2.7 dB, 0.9 dB (not kept: 9.2, 9.2) |
| A delayed stereo pair through ×0.8 / ×1.5, +4 st | ±0.05 rad | within |
| Aliasing reading faster (Rhythm, grains, Re-pitch; up to +24 st) | ≤ −60 dB | below (test) |
| Tempo within ±1 BPM, metronomic (clicks, drums, swing; 70–180 BPM) | ≥ 97 % | 100 % (69/69); the 18 outside 70–180 all their octave inside it |
| Tempo within ±1 BPM, played (jitter, drift, a pad, a pick-up, 3/4, 6/8) | ≥ 90 % | 90.6 % (87/96) in the test; every miss an octave; 92.8 % in the bench's variant |
| CPU per second of audio (×1.25 −3 st) | — | Complex ≈ 0.11 s, Melodic ≈ 0.13 s, Re-pitch ≈ 5 ms, Rhythm ≈ 1 ms |
| CPU per second of a 2 s window of a 1-minute file (Complex) | the window's | ≈ 0.09 s |

The listening set — five materials (drums, a full mix from the startup demo, a sung phrase,
a solo line, a chordal pad) × four modes × ratios 0.5, 0.8, 1.25, 2, the transpositions and
the brief's case (×1.25, −3 st) — is written by `cantus-bench listen <dir>` (in
`F:\projects\daw-r29\shots\warp\listen\` with a README) for the owner's ears.

## 10. Round 29's review (`reviews/warp-1.json`, 76 findings): what it changed

Every finding is fixed with a test seen failing first (`docs/round29/warp.md` lists each).
The decisions among them:

- **Soft and hard map points.** The stretcher's map carries which breakpoints pin the
  audio (a window's ends, a warp marker: *hard*) and which only sample a smooth curve (the
  tempo map's ramp every 256 frames, a tempo step's corner: *soft*). Rhythm cuts only at
  hard points and transients; a transient's hold is bounded only by hard points, and the
  soft points beside it give way (dropped over the room that takes up the hold's
  stretch). Under a tempo change a Rhythm clip no longer chops and Complex's holds keep
  their size. Every tempo point inside a window is a breakpoint of its render.
- **Transposition keeps the level.** Each peak's region moves whole (its lobe intact), by
  its true frequency, the magnitudes read between bins by a 8-tap windowed sinc; and the
  render's level is matched, frame by frame over ±8 frames, to the source's power where it
  read, moved by what the transposition did to the harmonic frame (a band moved beyond
  Nyquist, the formants put back). A transposed tone or a dense noise keeps its level
  within 0.1 dB from −24 to +24 st.
- **Resets only where a transient is.** At a hold, only the regions whose peak rose 3.5 dB
  over the frame a hop before take the analysed phase; a held bass goes on. DC, its lobe
  and the Nyquist bin keep their analysed phase.
- **The frame is 170 ms.** Measured per voice of a detuned saw chord in both channels (a
  measure that resolves voices 30 Hz apart), 85 ms frames let two close voices share a
  lobe and beat (5–6 dB); 170 ms holds every voice within 0.6 dB, at the same cost (the
  hop grows with the frame). Dense hits pay about a decibel of pre-echo (still under the
  −30 dB target). A two-band vocoder was tried and refused: the two renders' phases
  disagree where the bands cross (−2 dB on a partial there). The image lock now needs the
  channels' phase difference to hold still over a hop (a twenty-fifth of a bin).
- **Reading faster without aliasing.** Above speed 1, Re-pitch, Rhythm's slices and the
  percussive grains read through octave mip levels and a 64-tap windowed sinc whose
  cut-off follows the local speed (seventeen kernels an octave) — no aliasing at +24 st,
  and a speed just over 1 keeps its top octave.
- **A window of a long file costs the window.** Complex and Melodic render only the span
  the window reads plus the context their analyses reach, on the split's frame grid (the
  interior is the whole file's render); the engine takes only that span out of the input;
  Melodic's pitch analysis is made once per input and kept.
- **The render store.** Renders and bridges are kept by recency within 512 MB whether or
  not their clip is in the snapshot (mute, a pattern audition, a comp merge's foreign
  compile never throw one away); each clip *piece* (a loop's repetition) remembers its
  newest render or bridge and plays it while its own is on its way; a final is cancelled
  only when nothing plays its key, and one still being made for such a key is not waited
  for. A transposed clip's bridge is a WSOLA sketch at the transposition (grains placed by
  waveform similarity, anchored on markers and transients), so an edit never drops it to
  its source's pitch. `compile_for_render` compiles until nothing new is asked and counts
  a render made before everything was in (`renders_late`, logged); the modulator bake
  waits beside the window while renders are on their way.
- **The first edit.** An unwarped clip's first warp is `Warp::natural_of` — markers on the
  unwarped rule through the tempo map (exact where the tempo holds; inside a ramp sampled
  every sixteenth, within half a millisecond) — not following for a transposition, the
  formants, the slice ends or the mode, following for a marker, a stretch, *Quantise*, the
  clip's tempo, *Detect*, *Warp from here* and *Follow*. A warp laid again keeps the
  window's first frame (an offset written beside it). A warp that does not follow and does
  not transpose (`Warp::natural`) is no warp under any tempo map — the snapshot drops it and
  a comp seam keeps its crossfade.
- **The model's integrity.** `InsertClip` and `SetClipAudio` refuse an invalid warp as
  `SetClipWarp` does; a file's invalid warp is dropped on load and said; the undo fuzz
  checks every warp after every command. A paste keeps a following warp's ticks (tempo's
  relocation leaves it too). Quantise pins its range's ends (and a guard beyond), so only
  the hits inside move. An Alt-stretch or a bounce of a linked copy makes it unique first.
- **The import rule.** A loop's length is four beats a bar (as the analysis counts); every
  file of one import joins its undo step; a recovered take is a recording (never warped
  by itself). Recordings are analysed as they land (`analyse_file_soon`).
- **The interface.** The band takes a press only on a clip that follows the tempo (and in
  the editor); handles are offered by nearness to the pointer; a reversed clip's band is
  read mirrored; the editor has its own snap grid, Ctrl+wheel zoom and scrolling; a
  selected marker takes Del only while its clip is what is edited; the pitch editor's keys
  are off in the Warp tab (Q opens *Quantise audio*); *Warp from here* starts where the
  clip was right-clicked; *Quantise audio* takes the selection; *Set the project tempo*
  sets the tempo in force where the clip starts; controls that do nothing in a mode are
  drawn disabled; the detector's certainty has a row of its own.
- **Interchange.** Only the playing lane is written (the takes said); a track of audio is
  `contentType="audio"` (its input said, and given back on import); a clip at natural
  speed is a bare `<Audio>` placed in seconds; a warp on its own tempo's line comes back a
  warp under a tempo map; audio is stored (not deflated), read once per file, streamed to
  the destination on a worker, with ZIP64 past 4 GiB (written and read); a file referenced
  outside the archive is found next to it; an import's audio goes to a folder of its own;
  clip gain, fade shapes, slice ends, formants off and inert markers are reported.

## Consequences

- A loop dropped into a project plays in time at once and stays in time through every tempo
  change; a recording keeps the time it was played in; a project that does not warp plays
  exactly as before (no golden hash moved, `DSP_VERSION` unchanged).
- A warped clip costs a render per window and tempo stretch (about nine times real time in
  Complex on one core, the cost following the window, not the file) held in memory;
  renders and bridges are kept within a 512 MB budget beyond what plays.
- Complex's transient handling depends on the transients found: a window rendered before
  its file's analysis is in renders without holds, and is rendered again when it lands only
  if the clip changes (the analysis is fast next to the render, and asked at import).
- Whether it sounds as good as élastique is the owner's to hear (the listening set).
