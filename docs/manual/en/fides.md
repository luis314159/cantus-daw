# Fides, the resonator

*Cantus 0.1.0-beta.17 · [Leer en español](../es/fides.md) · back to the [manual](README.md)*

Fides makes sound the way acoustic instruments do: something **excites** an object — a finger
plucks it, a pick snaps it, a bow drags it, a mallet strikes it, noise blows on it — and the
object **resonates**: a string, a bar, a tine, a plate, a membrane, a tube, a bell, a glass.
A **body** radiates what it rings with. Nothing is sampled: every note is computed from the
physics as you play, so it answers to how hard, where and how long you play it, and it never
repeats a recording. Marimba, vibraphone, kalimba, steel pan, harp, nylon guitar, a bowed
cello and glass are its first presets; the family goes further (bells, gongs, timpani, a
hammered dulcimer, a pan flute, bowed pads, rain on glass).

Its name is the Latin for the strings of a lyre — a stringed instrument — and for faith.

**Add it** from the device picker (the **+** above the track headers, its *Keys* shelf, and
*Other instruments*). A new Fides plays a plucked nylon string in a guitar's body.

## The Play view

**The picture** (left) is the instrument as it is set: the resonator drawn as a plan — a
string between its bridge (left) and its nut, a bar with the nodes of its first mode dashed,
a tine from its clamp, a plate, a drumhead seen from above with the radius it is struck along,
a tube (closed at the right when it is one), a bell, a glass, a pan — the exciter drawn where
it plays it, and the microphone's ring where a modal object is heard from. **Drag across it**
to move where it is played: the partials below follow.

Under it, **the partials**: each a line at its frequency over the fundamental (×1, ×2, ×4 … on
a log axis), as tall as the strike excites it where it is played. A string's are its
harmonics with the gaps the position cuts (play it at a fifth of the length and every fifth
harmonic is gone); a bar's are its stretched modes (×2.76, ×5.40 …); a marimba bar's are
tuned to two octaves and three octaves and a third. While a note sounds they light with what
the newest voice rings.

**The exciter** (right): its six kinds, the knobs that kind has, and where it plays.

| Exciter | What it is | Its knobs |
|---|---|---|
| **Pluck** | a fingertip letting go: the partials fall as 1/n from the first | Contact (ms: a fingertip 0.6–1.5, a thumb's flesh 2–3), Velocity tone, Strike noise (a nail), Tone, Level |
| **Pick** | a plectrum: a quarter of the contact, with its scrape | the same |
| **Bow** | a bow drawn across, by a friction curve: the string sticks and slips (Helmholtz motion) | Pressure, Bow speed, Attack, Lift, Level |
| **Mallet** | a power-law spring's force pulse (the Hunt–Crossley contact, p = 2.5): harder is shorter and brighter | Contact (a yarn mallet 1.5–3 ms, rubber 0.5–1, brass 0.2), Velocity tone, Strike noise (felt), Tone, Level |
| **Noise** | band-passed noise through its own envelope — breath, wind, rain (Density below 100 % makes it grains) | Colour, Width, Density, Attack, Release |
| **Input** | another strip's audio into the resonator, while a note is held: chosen under *Input from* in its card, as a compressor's key is (any strip after its fader, a sidechain — a drum bus ringing a marimba, a voice driving a string); with none chosen it is silent and says so | Level, Attack, Lift, Tone |

**Position** is where it is played, as a fraction of the length from the end the picture
draws at the left: a string's bridge, a bar's end, a tine's tip, a membrane's rim, a bell's
lip. Near the end is thin and bright; the middle is round. **Noise** lays the noise generator
under any exciter (a bow's rosin, a mallet's felt).

**The resonator** (below): twelve kinds.

| Resonator | What it is |
|---|---|
| **String** | a waveguide string (Gravis's tuning maths): its loss lumped at the bridge, its stiffness as allpasses — *Inharmonicity* stretches its partials as a piano's |
| **Bar** | a free–free bar (a xylophone's untuned, a glockenspiel's): modes at 1, 2.76, 5.40, 8.93 … |
| **Marimba bar** | a bar undercut to tune its partials to 1 : 4 : 9.2 |
| **Tine** | a bar clamped at one end (a kalimba's, a music box's, a celesta's): 1, 6.27, 17.5 |
| **Plate** | a rectangular plate's modes (a gong's, a thunder sheet's) |
| **Membrane** | an ideal drumhead: the Bessel zeros (1, 1.59, 2.14, 2.30 …) |
| **Drumhead** | a kettledrum's head, tuned by the air under it (1, 1.5, 2, 2.44 …) |
| **Tube** | a tube closed at one end (a pan flute's): odd harmonics |
| **Pipe** | a pipe open at both ends: every harmonic, its lows radiated less |
| **Bell** | a tuned church bell's partials (hum, prime, tierce, quint, nominal …) |
| **Glass** | a wine glass's ring modes |
| **Pan** | a steel-pan note: fundamental, octave and twelfth tuned |

**Decay** is how long the fundamental rings (T60, the time to fall 60 dB). **Material** is how
the highs die: wood's die at once, glass's and metal's ring as long as the fundamental.
**Inharmonicity** stretches (or squeezes) the partials: on a string, at 0 they are harmonic, above it a stiff string's (a piano's at 100 %), below it drawn in as far — every step moves them. **Listen** is where a modal object is
heard from (the microphone's ring).

**The body**: None, Guitar (its air mode near 98 Hz and its top's modes), Violin (its
signature modes and the bridge hill near 2.5 kHz), Cello, Harp (its soundbox and soundboard),
Box (a kalimba's), Shell (a drum's or a pan's skirt) — with its **mix**, **size** (a bigger body
lowers its modes) and **ring**.

**Output**: Release (how fast the resonators are damped when you let go: 60 s lets them ring as
a marimba's bars do), Width (voices spread across the stereo by key) and Gain.

## The Edit view

Every part as a card: the picture and the exciter; **Resonator A** and **Resonator B** (B's lamp
turns it on), each with every knob — *Key decay* (how much faster a note an octave up dies),
*Partials* (how many modes a modal object plays: fewer is lighter on the CPU), *Tune* and *Fine*
(B against A), *Key track* (0 % holds the resonator's pitch whatever the key: a drum); the
**coupling** — *Parallel* (the exciter into both) or *A into B* (A drives B, as a marimba's bar
drives the tube under it) and what is heard of each; the **bow and MPE**; the **noise** with its
envelope; the **body**; **vibrato** (depth in cents, rate, a delay before it starts) and
**tremolo** (its phase the song's clock, as a vibraphone's motor); the **output** with its low
and high cuts; the **voice** — Poly, Mono or Legato, *Strike again* (on: a key struck while it
rings strikes the same resonator again, as a real bar or string; off: a new voice), Voices,
Glide, Bend range, Octave, Transpose, Fine, A4, Velocity, Release and the Seed of its noise.

## Playing it

- **Velocity** sets how hard the strike is: louder and, by *Velocity tone*, a shorter contact
  (brighter), as a faster mallet's is.
- **MPE**: a note's **pressure** presses the bow (*MPE pressure*: how much; with Noise or Input
  it raises the level), its **slide** moves where it is played (*MPE slide*), its **pitch bend**
  bends it. A non-MPE controller's aftertouch, wheel and expression are modulation sources.
- **Pitch** is exact: the waveguide's loops and filters are solved for the fundamental, and the
  modes are set by their angles — measured within 0.002 cent at every bend.
- **The sustain pedal** holds released notes, as a vibraphone's damper bar.

## The presets

Marimba · Vibraphone · Kalimba · Steel pan · Harp · Nylon guitar · Bowed cello · Struck glass ·
Glass harmonica · Xylophone · Glockenspiel · Celesta · Music box · Bell · Timpani · Frame drum ·
Hammered dulcimer · Banjo · Pizzicato · Bowed strings pad · Bowed vibraphone · Pan flute · Rain on
glass · Gong · Crystal pluck. Each has four macros: **1** how hard (the contact, the bow's
pressure, the noise's colour), **2** the decay, **3** the body, **4** where it is played.

## What it costs

A plucked or bowed string is a few dozen operations a sample; a modal object one complex
multiplication a partial (48 at most). Its voices split across the engine's threads with the
bits of one (ADR 0038). The *Partials* knob is the price of a modal voice.
