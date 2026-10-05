# Round 19 — demos: six songs, the startup project, the demo picker

Builder: **demos** (`r19/demos`). The owner asked for example tracks and for the program to
open on something that shows what Cantus can do.

## What is in the tree

- `crates/cantus-ui/src/demos/` — the demo songs as code, and what they are written with:
  - `theory.rs` — note names (`"F#4"`), chord symbols (`"A/C#"`, `"Em9"`, `"B7#9"`,
    `"C7sus4"` …), **voice leading** (each chord voiced as the smallest total movement from
    the one before, inside a range, drifting back towards a centre so a long progression
    cannot walk off the keyboard), melodies in text (`"E5:1.5 D5:.5 r:1 >F#5:2"`, `>` accents,
    `_` softens), drum grids in text (`X` accent, `x` hit, `o` soft, `g` ghost) and a seeded
    generator, so every demo is the same song on every machine.
  - `parts.rs` — what a band plays over a chord chart: held chords, comping on a rhythm,
    a bass line that follows each chord's bass and steps into the next (root, octave,
    fifth, approach, any interval), arpeggios with the bass under them, drum bars,
    crescendos.
  - `song.rs` — the builder: a part is a track with its instrument (a factory preset with a
    few values moved), its colour and icon, on a named, coloured strip; buses; returns
    docked on the right; post-fader sends; inserts from factory presets plus values;
    sidechain keys; a gate keyed by a part's notes; the master chain; clips with a
    player's **feel** (timing a few milliseconds off the grid, velocities that move,
    swing on a chosen grid); the channel trim (FL's channel volume) and the fader. A
    section longer than eight bars is laid out as eight-bar phrase clips.
  - `ballad.rs`, `house.rs`, `funk.rs`, `lofi.rs`, `ambient.rs`, `trap.rs` — the songs.
  - `mod.rs` — the list the picker shows (`ALL`), `startup_project()`.
- `crates/cantus-ui/src/views/demos.rs` — File → **Demo songs…** (Ctrl+Shift+N, the
  `demo-songs` action): a kit-drawn list (the song's colour at the row's edge, the name,
  *style · tempo · length*), arrows and Enter, double-click opens. Opening replaces the
  project as *New* does; when the open project has unsaved changes the picker asks first:
  *Save first* (Save, or Save As for an untitled project; the demo opens only if the save
  happened), *Don't save*, *Cancel*.
- `crates/cantus-ui/tests/demos.rs` — the proofs (below).
- `locales/areas/demos.{en,es}.txt` — every string, English and Spanish.

## The songs

| Song | Style | Key, tempo | Bars | Instruments | On the desk |
|---|---|---|---|---|---|
| **Neon Tide** | House | A minor, 124 | 64 (intro 8 · build 8 · drop 16 · break 8 · drop 16 · outro 8) | Pulsus II 909 kit (kick, clap+snare, hats, rim+crash), Gravis saw+sub bass, Calor unison-saw chords, Unda pluck hook, Unda hyper-saw lead, Exemplar choir, Unda noise riser | drum bus (Comprimo glue → Multiplex low end → Terminus), **Comprimo pump keyed from the kick** on chords, bass and choir, Sibilus on the hats, Aequalis everywhere, Porta on the riser, Spatium hall + delay returns |
| **Morning Light** | Pop / worship ballad | D major, 74 | 36 (intro 4 · verse 8 · chorus 8 · bridge 4 · chorus 8 · outro 4) | Pulsus II studio kit (kick, rim+snare, cymbals, toms), Gravis fingered bass, Lamina ballad tines, Unda warp pad, Exemplar nylon guitar and bells, Modulus FM flute (the melody) | drum bus (Comprimo → Terminus), Porta on the toms, Sibilus + Comprimo on the melody, Spatium concert hall, dotted-eighth delay |
| **Sunday Strut** | Funk and soul | E minor, 102 | 40 (intro 4 · verse 8 · chorus 8 · solo 8 · chorus 8 · outro 4) | studio kit with ghost notes and a sixteenth swing, Gravis slap bass (a written riff), Organum soul registration through the rotary, Lamina growling reed piano, Modulus brass section, Calor sync lead (the solo) | parallel drum bus, Porta on snare and toms, Multiplex de-harsh on the reed piano, Spatium room |
| **Paper Lanterns** | Lo-fi hip-hop | F major, 80 | 32 (intro 4 · A 8 · hook 8 · A 8 · outro 4) | boom-bap kit swung hard and late, Lamina worn tines (ninth chords, rolled), Gravis fretless, Exemplar nylon (the tune), Modulus marimba, Calor velvet pad | squashed drum bus with a low-pass, Aequalis taking the top off keys and master, small dark room, long delay |
| **Aurora** | Cinematic | D minor, 66 | 32 (swell 8 · rise 8 · climax 8 · outro 8) | Unda glass choir, Calor string machine and glass-pluck ostinato, Modulus bells (the theme), Exemplar choir, Gravis sine sub, Pulsus II low toms and drums | Multiplex on the strings, Sibilus on the choir, Porta on the drums, Spatium cathedral |
| **Midnight Static** | Trap | F minor, 140 half-time | 56 (intro 4 · hook 16 · verse 16 · hook 16 · outro 4) | trap kit (short kick, clap+snare on three, hats with 1/32, 1/64 and triplet rolls), **Gravis 808 with slides** (overlapping notes glide), Unda digital bells, Modulus pluck, Exemplar pad **chopped by a Porta gate opened by the hats' notes** | Terminus on the drum bus, Multiplex low end on the 808, Sibilus on the hats |

Every native instrument plays in at least two songs; every native effect (Aequalis,
Comprimo, Terminus, Spatium, Multiplex, Sibilus, Porta, and the delay) is on a strip with
settings a mixer would choose. Every song has a reverb return and a delay return docked
on the right, sends from the parts that want space, and a master chain: Aequalis (a sub
cut, a little air), Multiplex glue or Comprimo's master style, and **Terminus**
(streaming preset, ceiling −1 dBTP). Culmen can take Terminus's place once it lands.

## The startup project

`demos::startup_project()` is **Neon Tide's second drop** (the choir has joined), sixteen
bars moved to the start and looping: nine tracks — four drum tracks into the drum bus, the
pumped chords, the bass, the hook, the lead, the choir — every strip named and coloured,
inserts on eleven of them, the reverb and delay returns docked right, the master chain.
`Song::excerpt` leaves out a track with nothing in the window (the riser) with its strip,
so the desk closes up. The splash opens it when no project was asked for (`boot.rs`);
`--stress` still opens the stress project. The shoot example renders it: `cargo run -p
cantus-ui --example shoot -- <dir> 1 "" startup` writes the mixer and the whole window
(arrangement, mixer, inspector) in both themes.

`welcome_project()` stays as it was: it is what *New* opens (a light groove to start from)
and the fixture twenty tests and examples are built on — replacing it would have rewritten
other builders' tests. `starter_project()` is untouched.

## How it was proved without ears

`crates/cantus-ui/tests/demos.rs` (the demos render side by side on threads):

- `every_demo_renders_clean_and_at_a_sane_loudness` — the whole song through
  `cantus_engine::render`: every sample finite, peak under 0 dBFS and over −12, integrated
  loudness (BS.1770: K-weighting, 400 ms blocks, the two gates) in −20…−9 LUFS, length
  55–150 s.
- `every_track_of_every_demo_is_heard` — each track soloed for two bars where it plays
  (in the song's reference section when it plays there): RMS over −50 dBFS.
- `nothing_lies_outside_the_song` — clips inside the song, notes inside their clips, no
  empty clip, the loop region inside the song.
- `every_native_instrument_plays_in_some_demo` — all seven native kinds, and Pulsus II.
- `every_demo_survives_save_and_open` — saved and opened again, the project is equal.
- `levels` (ignored) — the numbers the mix was set by: each part soloed in the song's
  reference section (peak, RMS, LUFS), the section's mix, and with `FULL=1` the whole
  song. `DEMO=<id>` picks one.

## Measured (the whole song, today's engine, no master chain)

| Song | Integrated | Peak | Render (this machine, loaded) |
|---|---|---|---|
| Neon Tide | −17.3 LUFS | −2.7 dBFS | 288 s for 124 s |
| Morning Light | −18.6 LUFS | −1.2 dBFS | 296 s for 117 s |
| Sunday Strut | −17.4 LUFS | −2.1 dBFS | 82 s for 94 s |
| Paper Lanterns | −19.2 LUFS | −1.4 dBFS | 79 s for 96 s |
| Aurora | −18.7 LUFS | −1.2 dBFS | 148 s for 116 s |
| Midnight Static | −18.5 LUFS | −0.6 dBFS (kick then lowered 1 dB) | 124 s for 96 s |

The parts were placed by measuring each one soloed in the song's fullest section (the
`levels` report) against targets a mix engineer would recognise: kick and bass around
−22 LUFS, the lead and the main harmony −23…−24, snare and clap −25, pads and secondary
lines −27…−30, hats −32, percussion −35 — then the mix checked as a whole.

## Notes for the integrator

- **Gain staging and the master chain.** Today's engine does not run `Master::effects`
  (the master builder adds it). The demos are mixed so the song is already clean without
  the chain (peaks under −1 dBFS, around −18 LUFS from the strips and the drum-bus
  limiters); the master's Terminus adds +3…4 dB of gain into a −1 dBTP ceiling, which puts
  them near −14 LUFS once the chain plays. The loudness test's range (−20…−9) holds both
  before and after the master chain lands.
- **Instruments are quiet at unity**: a single part of a native instrument at velocity
  ~90 measures about −30 LUFS. The demos bring each part up with the track's channel
  trim (`Track::gain_db`, FL's channel volume) so the faders sit around 0 dB as in a real
  session; that is a fact about the instruments' output staging worth a look by whoever
  owns them.
- **Render time**: on this machine, shared by nine builders, a two-minute demo renders in
  several minutes; the tests render the six songs in parallel.
- Shared files touched (small, local hunks): `app.rs` (the `demos` field, `replace_project`
  — *New* now goes through it — the frame call, the action), `boot.rs` (the startup
  project), `keymap.rs` (`DemoSongs`, Ctrl+Shift+N), `views/transport.rs` (the menu entry),
  `views/interop.rs` (`form` made `pub(crate)` to reuse its buttons), `views/mod.rs`,
  `lib.rs`, `examples/shoot.rs` (the mixer shot on the startup project, and a `startup` case
  that renders the whole window).
