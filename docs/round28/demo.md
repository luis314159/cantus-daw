# Round 28 — demo: *With All Your Heart*, the round's showcase song

Builder: **demo** (branch `r28/demo`, from `r28/integration` at `30f05f0`). The owner asked
(2026-10-15): *"generate a demo song using the new instruments and effects, to have something
to show that sounds super professional."* No schema change, no ADR (a demo song is content;
the one model addition — Chorda's `pedal` parameter — is a parameter, not a schema change).

## The song

**With All Your Heart** (Spanish: *De todo corazón*) — the title is the project's motto
(Colossians 3:23). **Cinematic pop**, F minor / A♭ major, **116 BPM**, 80 bars, **2 min 46 s**
(2 min 48 s with its tail). In the app: *File ▸ Demo songs…* (`Ctrl+Shift+N`), the first row.
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
| 73–80 | Outro | The intro's arpeggio over the verse's chords, the tape wow drifting back in, the heartbeat fading; it ends on A♭add9 — the relative major — held on the pedal |

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
