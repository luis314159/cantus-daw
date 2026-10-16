# Round 28 — demo: *With All Your Heart*, the round's showcase song

Builder: **demo** (branch `r28/demo`, from `r28/integration` at `30f05f0`). The owner asked
(2026-10-15): *"generate a demo song using the new instruments and effects, to have something
to show that sounds super professional."* No schema change, no ADR (a demo song is content;
the one model addition — Chorda's `pedal` parameter — is a parameter, not a schema change).

## The song

**With All Your Heart** (Spanish: *De todo corazón*) — the title is the project's motto
(Colossians 3:23). **Cinematic pop**, F minor / A♭ major, **116 BPM**, 80 bars and two more
of the last chord ringing on the pedal: **2 min 50 s**. In the app: *File ▸ Demo songs…*
(`Ctrl+Shift+N`), the first row (the picker reaches it through `cantus_ui::run` → boot →
`CantusApp::assemble` like every demo; `demos::ALL[0]`).
Code: `crates/cantus-ui/src/demos/heart/` (`mod.rs` the band, `desk.rs` the mix, `music.rs`
the charts and the parts, `drums.rs` the grooves, `moves.rs` automation and modulators).

| Bars | Section | What happens |
|---|---|---|
| 1–8 | Intro | Chorda alone, a pedalled arpeggio through Vibrans's tape wow (fading out by bar 8); the pad, a shaker and a heartbeat kick (*lub-dub*) join at bar 5; a short swell into the verse |
| 9–24 | Verse | Fm9 · D♭maj7 · A♭add9 · E♭ E♭/G. First half: piano, pad, the voice, half-time drums; second half: bass, Lamina's keys panned by Tremor, the pluck arpeggio, the backbeat |
| 25–32 | Pre-chorus | B♭m7 · A♭/C · D♭maj7 · E♭7sus4 E♭. Piano in eighths swelling, the bass in eighths; the kick drops out for a snare roll, the music bus's high-pass climbs, Translatio's riser spins up; the last beat is silent |
| 33–48 | Chorus | D♭maj7 · E♭ · Fm7 · A♭/C, then D♭maj7 · E♭ · Cm7 · Fm7 E♭/G. Four on the floor, Fluctus pumping pad and keys, the sixteenth arpeggio, the nylon guitar through Phasis, the hook; a reverb throw on its last word |
| 49–56 | Breakdown | Chorda's own tune in octaves over D♭maj9 · A♭/C · B♭m7 · E♭7sus4, the choir, Modulus bells answering through Anulus (growing, then gone); the heartbeat returns, a roll and a riser into… |
| 57–72 | Last chorus | …the chorus again with the choir under it and Calor's sync lead doubling the hook an octave up through Flexus |
| 73–82 | Outro | The intro's arpeggio over the verse's chords, the tape wow drifting back in, the heartbeat fading; it ends on A♭add9 — the relative major — rung two bars on the pedal, then the pedal lifts and the hall closes it |

**The hook** (the voice, bars 33–36): *A♭ C D♭ – C A♭ | B♭ C E♭ – D♭ C | C A♭ F – E♭ C | E♭ – D♭ C*,
every note in F minor; the answer climbs to G and A♭ before resolving on F.

## Which unit does what, where

| Unit | Where | What it does |
|---|---|---|
| **Chorda** (Pop Forward, width 0.8) | Piano, the whole song | the featured instrument; **pedalled from an automation lane** on its new `pedal` parameter: up exactly on each chord change, down a sixteenth later (legato pedalling), so each chord rings with its strings' sympathy and never the last chord's |
| **Concentus** | Pad (*string ensemble*), keys (*Juno II*), voice (*vocal double*, 25 %) | width and motion |
| **Flexus** | Last chorus's lead (*synced sweep*, 40 %) | through-zero sweep on the doubled hook |
| **Phasis** | Guitar (*Phase 90 script*, 0.35 Hz) | the classic phased nylon figure |
| **Tremor** | Keys (*wide pan*, 65 %) | auto-pan, a bar per sweep |
| **Vibrans** | Piano (*tape wow*, spread 0) | depth automated: 16 ct in the intro → 0 by the verse; 0 → 22 ct across the outro |
| **Anulus** | Bells (*stereo shimmer*) | mix automated across the breakdown (0 → 20 % → 65 %, gone before the last chorus) |
| **Translatio** | Riser (*barber pole up*) | mix and shift automated (2 → 40 Hz) through the four bars before each chorus |
| **Fluctus** | Pad and keys (*pump quarters*) | depth automated: 0.75 in both choruses, 0 elsewhere (the pump lands on the kick's quarters by the song position) |
| **Impetus** | Kick (*kick*), snare (*snap*) | attack shaped at the source |
| **Discrimen** | Drum bus (*snappy*, attack +3 dB, body −2 dB) | transients against body |
| **Tonsor** | Drum bus (*drum bus*, drive 4 dB), master (*soft master*, drive 1.5 dB, true-peak guard) | peaks shaved before the limiters |
| **Speculum** | Master, last (*master*, target −11 LUFS) | a pass-through, there to look at |
| **Tonus** | Voice (*hard tune with vibrato*, F minor, throat +1.5 st, created vibrato 22 ct) | snaps the voice's 70 ms glides onto the scale: the hard-tune staircase |
| Gravis, Calor, Unda, Lamina, Exemplar, Modulus, Pulsus II | bass, pad and sync lead, pluck/voice/riser/sub drop, keys, nylon guitar and choir, bells, drums (electro kick, studio snare + 909 clap, 909 hats, shaker, rim, studio toms and crash) | — |

**Modulators** (ADR 0057): an **LFO** (sine, four bars, synced) on the pad's filter cutoff
(±12 % of its travel); a **follower** on the kick's strip ducking the bass's fader (attack
2 ms, release 140 ms, −12 % of travel: a sidechain made of a modulator); a **sixteen-step
sequence** on the pluck's filter (1/16, 15 % glide); a **smooth random** walk of the pluck's
pan (1/8, seed 116).

**Automation** (ADR 0027), 26 clips: the pedal (one per section), Vibrans's depth, the
music bus's high-pass (20 Hz → 650 / 900 Hz into each chorus), Translatio's mix and shift,
Fluctus's depth on two strips, Anulus's mix, the voice's reverb send (−11 → −2 dB on the last
word of each chorus).

**The desk**: kick, snare, hats, percussion and toms into a drum bus (Discrimen → Comprimo
glue → Tonsor); keys, pad, pluck and guitar into a music bus (the sweeping high-pass, light
glue); a concert-hall reverb and a dotted-eighth delay (388 ms) docked right, with sends from
eleven strips. The master: Aequalis (24 Hz cut, +2.5 dB at 140 Hz, −1 dB at 3.5 kHz, −1.5 dB
shelf from 8 kHz) → Multiplex glue → Latitudo master air → Tonsor → Culmen *streaming*
(+8.8 dB into −1 dBTP) → Speculum.

**Drums**: Pulsus II's factory kits only — DRSKit (round 28's acoustic pack) is a download
and a demo must play the same on every machine without it, so the song does not use it.

## Chorda's sustain pedal from the arrangement (a gap closed)

Making the song showed that **Chorda could not be pedalled from the arrangement**: its
sustain pedal was controller 64 only, which clips do not carry (a clip holds notes, not
controllers), so a sequenced part could only fake the pedal with long notes — without the
sympathetic ringing lifted dampers give (−16 dB against −35 dB, ADR 0059). Fixed in the
instrument, not worked around in the song:

- `crates/cantus-core/src/inst/chorda.rs`: a **`pedal`** parameter (0 up … 1 down, default 0,
  continuous: half-pedalling works), automatable and modulatable like every other.
- `crates/cantus-engine/src/inst/chorda.rs`: the pedal the strings hear is **the higher of the
  parameter and controller 64**, told to the DSP only when it moves; at its default nothing
  changes (the instrument's every-parameter null test covers the new key).
- The editor's PEDALS card (Play and Edit) has its knob, *Sustain* / *Derecho* (the editor's
  own Spanish line already says *Pedal derecho*), with its automation and *Modulate…* menus
  through the shared cell path; `docs/instruments/chorda.md` lists it.
- Tests (`crates/cantus-engine/tests/inst_chorda.rs`), written before the code (the key did
  not exist, so both failed at their first line): `the_pedal_parameter_is_the_sustain_pedal`
  (held by the parameter, a released chord rings bit for bit as with controller 64; the
  higher of the two plays) and `a_lane_on_the_pedal_holds_and_releases_the_strings` (a lane
  down holds released notes more than 12 dB over the dry release; lifted, they fall more
  than 20 dB).

For a later round (not this builder's area): MIDI import could turn a file's controller 64
into a lane on this parameter.

## Measured (the whole song, offline, latency compensated, today's engine)

From `cargo run -p cantus-ui --example demo_master -- <dir> heart` (Speculum's own meter,
EBU Tech 3341/3342):

| What | Measured | Target |
|---|---|---|
| Integrated loudness | **−11.3 LUFS** | −12 … −10 (a current pop single on streaming) |
| True peak | **−1.14 dBTP** (sample peak −1.14 dBFS) | ≤ −1 dBTP |
| Loudness range | 5.8 LU | pop: 4–8 |
| Loudest short-term | −9.7 LUFS (the last chorus) | — |
| Sections (integrated) | intro −17.3 · verse −12.6 / −11.2 · pre −10.4 · chorus −10.2 / −10.3 · breakdown −12.1 · last chorus −10.0 / −9.9 · outro −14.3 | an arc: the choruses lift 2–3 dB over the verse |
| Spectrum, octaves against 1 kHz | 32 Hz +0.7 · 63 +5.7 · 125 +1.6 · 250 +0.2 · 500 +1.1 · 1k 0 · 2k −1.1 · 4k −2.2 · 8k −6.7 · 16k −14.3 | a pop master's tilt (+1, +5, +4, +2, +1, 0, −2.5, −5, −8, −16): within **2.8 dB** everywhere above 40 Hz |
| Stereo correlation (400 ms windows) | mean **0.77**, lowest −0.08 (4 of 427 windows just under 0: reverb tails) | healthy: mean 0.5–0.9, nothing deeply negative |
| Clicks at section changes | the sharpest second difference within 30 ms of each change: 0.08–1.72, against 0.99 median / 1.69 largest on the downbeats inside sections | no change sharper than the music around it |
| Live against offline (bars 28–36, irregular blocks) | **identical to the bit**, every track | — |
| Render time (dev profile, this machine) | 157–217 s for 170 s | — |

The mix was set by numbers: each part soloed in the first chorus with the master chain out
(`DEMO=heart DRY=1 cargo test -p cantus-ui --test demos -- --ignored --nocapture levels`),
then the whole song through the master against the targets above, in three passes: the
verse thinned so the chorus lifts; Vibrans's spread to 0 when the intro measured slightly
negative correlation; the master EQ's low-mid warmth and an eased top when the spectrum sat
4 dB under the reference at 125 Hz.

## The render

`F:\projects\daw-r28\shots\demo\` (never in the repository):

- `heart.wav` — the whole song, 48 kHz, 24-bit, 2 min 53 s with a 3 s tail;
- `heart.mp3` — the same, 320 kbps, 44.1 kHz;
- `heart-short.mp3`, `heart-short.ogg` — the pre-chorus into the first chorus (bars 25–48,
  52 s, faded over the last two seconds), for a website player;
- `master-final.txt` — the measurements above;
- `png\` — 44 shots from `cargo run -p cantus-ui --example shoot_demo -- <dir>`: the song
  opened through the picker, the whole window (arrangement with the pedal and tape-wow lanes,
  the desk), the mixer, the hook in the piano roll, and the editors of Chorda (with its pedal
  knob), Tonus, Fluctus, Concentus, Tremor and Speculum on the master, in Cantus Night and
  Cantus Paper, English and Spanish, and the window and Chorda at 75 % and 200 %. Looked at:
  nothing overlaps; the Spanish pedal label became *Derecho* after the first pass.

## Tests and what each holds

`crates/cantus-ui/tests/demos.rs` (every demo test now runs over seven songs):

- `every_demo_renders_clean_and_at_a_sane_loudness` — for the showcase: −12 … −10 LUFS
  integrated, true peak ≤ −1.0 dBTP, no sample over full scale or not a number, 55–180 s.
- `every_track_of_every_demo_is_heard`, `nothing_lies_outside_the_song` (now also: an
  automation clip is valid, has points, none past its end), `no_note_is_written_twice`
  (it caught a doubled D♭4 in the breakdown's octaves; fixed), `every_demo_survives_save_and_open`,
  `every_native_instrument_plays_in_some_demo`, the picker and the startup project — as
  before, now over the showcase too.
- `the_showcase_plays_round_28` — Chorda plays; all thirteen round-28 effect kinds are on a
  strip or the master; Speculum ends the master chain; Tonus hard-tunes to F minor; LFO,
  follower, step and random modulators drive something; the pedal is played from a lane;
  Vibrans, Translatio, Fluctus and Anulus have lanes.
- `the_showcase_renders_the_same_offline_and_in_real_time` — bars 28–36 (riser, filter
  build, the first chorus: automation, modulators, the follower, every insert, Tonus's and
  Flexus's latency compensated) played through a processor at irregular blocks equal the
  offline render to the bit. The harness sends the automation runtime as the backend does
  (without it the lanes on effect and instrument parameters did not play — a harness
  mistake, found with the ignored `live_against_offline`, which compares every track).
- `the_topline_is_hard_tuned` — Tonus changes the voice by more than −20 dB of its energy
  against the same render with its amount at 0 (it really hears and moves the synth voice).
- `levels` (ignored) takes `DRY=1`: the balance before mastering.

`crates/cantus-engine/tests/inst_chorda.rs`: the two pedal tests above; the whole file passes.

**The full runs** (Windows): `cargo test -p cantus-core -p cantus-project -p cantus-engine
--no-fail-fast` — 977 passed after the contract fix; `cargo test -p cantus-ui
--no-fail-fast` — 1 256 passed, 1 failed once under load and passed alone twice
(`hosted_instruments::opening_its_editor_from_the_track_opens_the_plugin_window`, a hosted
plug-in's OS window, nothing this round touched); `cargo clippy --workspace --all-targets
-- -D warnings` clean; `cargo fmt --all -- --check` clean. No `cfg(windows)` code was added.

## Files outside my area (all small, additive)

- `crates/cantus-ui/src/demos/song.rs` — `strips` (more middle strips than a project opens
  with), `automate` (a lane from values in the parameter's own units), `modulate`,
  `track_id`, `pads_from` (pads from several kits).
- `crates/cantus-ui/src/demos/mod.rs` — the showcase first in `ALL`.
- `crates/cantus-core/src/inst/chorda.rs`, `crates/cantus-engine/src/inst/chorda.rs`,
  `crates/cantus-ui/src/views/inst/chorda.rs`, `locales/inst/chorda.{en,es}.txt`,
  `docs/instruments/chorda.md` — the pedal parameter.
- `crates/cantus-engine/tests/support/heavy.rs` and `tests/inst_contract.rs` — Chorda's
  `pedal` is drawn from a stream of its own (every other draw stays as it was), and the rest
  contract's mid-run change lets it up (review 1, below).
- `locales/areas/demos.{en,es}.txt` — the song, its style, the pre-chorus, *Vox*, *Sub drop*,
  *Music bus*, the automation clips' and modulators' names.
- `crates/cantus-ui/examples/render_demos.rs` — the showcase's window for the website excerpt.
- New examples: `demo_master.rs` (render, measure, write WAV/MP3/OGG), `shoot_demo.rs`.
- `docs/manual/{en,es}/README.md` — the demo list (seven songs).

## Left

Only the owner's: **listening**. What to listen for:

- the voice: Unda's formant wavetable through Tonus's hard tune is meant as a stylised,
  vocoder-like topline (its glides step through F minor); if it reads as too synthetic,
  Tonus's `speed` at 20–40 ms softens it;
- Chorda's pedalling (every chord change) and the breakdown, where the piano is alone with
  the choir and the bells;
- the pump: Fluctus on pad and keys and the follower on the bass — if it breathes too much,
  Fluctus's depth (0.75) and the follower's depth (−0.12) are the two numbers;
- the master: Culmen at +8.8 dB into −1 dBTP after Tonsor's soft clip; the choruses sit at
  −10 LUFS.

## Review 1

Six confirmed findings (one medium); each fixed, the testable ones with a test seen failing
first.

1. **The chorus piano held Fm7 under E♭/G** (and the intro/outro arpeggio carried a note of
   each chord into the next): a hit late in a chord outlasted the pedal's re-press a
   sixteenth after the change, so A♭ rang against G in the last bar of every chorus cycle
   (bars 40, 48, 64, 72). Fixed where every demo writes its parts: `parts::comp`, `bass`
   and `arp` stop each note at the next chord change (`Laid::left`). Test
   `parts::tests::no_note_rings_into_the_next_chord` (failed: *a note at 0 lasting 3 rings
   past the change at 2*). **Other demos**: Morning Light's notes lose 1.0 % of their total
   length and Aurora's 0.3 % (their renders change slightly, musically for the better:
   nothing rings under the next chord); Neon Tide, Sunday Strut, Paper Lanterns and
   Midnight Static are unchanged note for note. Every demo still passes its loudness test
   (now −14.1 … −14.8 LUFS, −1.14 … −1.18 dBTP).
2. **Hats and percussion trims at +15.5 / +15 dB**, beyond the +12 dB a command can set:
   now +12 with the rest on their faders (+3.5 / +3 dB). `Song::trim` and `Song::mix`
   assert the command range, so no demo can do it again. Test
   `every_level_is_one_a_command_can_set` (failed: *heart: Hats's trim 15.5 dB*).
3. **The manual's Chorda section** (EN and ES) now documents the Sustain / Derecho knob, the
   lane (*Create automation clip*), the modulator, half-pedalling and the higher-of rule.
4. & 5. **The pedal left out of every random contract**: the skip is gone from
   `random_instrument`; the pedal is drawn from a stream of its own (so no existing draw
   moves), and only the rest contract's mid-run change lets it up (`released`). Split
   invariance (blocks 1/7/4 096, random splits), voice split and the pool's parallel =
   serial now run with it over its whole range: `inst_contract` 31 and `pool` 6 pass.
6. **The picker had seven fixed rows and no scroll area**: the rows now scroll in an area
   bounded by the screen (the title, the band, the hint and the buttons always stay on it),
   the arrow keys scroll the selection into view, and the window has a stable id. Test
   `the_picker_fits_a_small_screen_at_200_percent` (Spanish, 683 × 384 points; failed: the
   window ran from −61 to 445). Shots: `png\heart-picker-{night,paper}-{en,es}-1366-z200.png`
   (1366 × 768 at 200 %), looked at — three rows and the scroll bar, the hint and *Abrir* /
   *Cancelar* on screen.

**Re-measured** after the fixes (`master-final.txt`; the WAV, the MP3 and the short MP3/OGG
rendered again): −11.3 LUFS, LRA 5.8 LU, −1.14 dBTP, the spectrum within 2.8 dB of the
reference, correlation 0.77 (lowest −0.08), no clicks at section changes — the same as
before to a tenth, the clash gone.
