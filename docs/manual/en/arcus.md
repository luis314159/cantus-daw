# Arcus, the bowed string

*Cantus 0.1.0-beta.18 · [Leer en español](../es/arcus.md) · back to the [manual](README.md)*

Arcus is a cello and a double bass that are **played, not replayed**: nothing is sampled. A
string with its real length, tension and mass is drawn by a bow with a force in newtons, at a
speed and at a distance from the bridge; the rosin's friction holds the string and lets it go
hundreds of times a second (the stick and slip of Helmholtz's motion), and the instrument's
body radiates what the string sings. So a note answers to how you play it — softly or hard,
slurred or bowed again, near the bridge or over the fingerboard — and no two notes are the
same recording.

Its name is the Latin for the bow.

**Add it** from the device picker (the **+** above the track headers, *Other instruments*).
A new Arcus is a cello that slurs the notes you overlap.

## Playing it from a keyboard

Arcus **never falls silent** for want of a controller: the velocity alone plays it.

- **Velocity** sets the note's dynamic (from pianissimo to fortissimo: how fast the bow moves,
  how hard it presses and how near the bridge it plays, together, as players do) and its
  attack (a harder note accelerates the bow sooner and presses more — a soft one speaks a
  little later, as a real string does, a low string later than a high one).
- **Overlap two notes** and the second is **slurred**: the bow goes on, the finger moves —
  at once, with no delay added (the *Finger* time is the finger's own move, 3 ms). **Leave a
  gap** and it is **bowed again** in the other direction (détaché), the string still ringing.
  The **Play mode** says which: *Legato* (overlaps slur — the default), *Mono* (every note
  bowed again), *Poly* (each note its own string: double stops).
- **CC1** (the mod wheel) moves the dynamic continuously once you move it — swell a held
  note and the bow speeds up and presses as it grows. **CC11**, **breath** (CC2) and
  **aftertouch** work too (*Dynamic from*: *Auto* takes whichever of CC1, breath and
  aftertouch you moved last, with CC11 scaling it; or name one; or *Velocity* alone).
- **CC21** sets the vibrato's depth (or CC2, or aftertouch: *Vibrato from*). Until it moves,
  the vibrato comes by itself, a little after the note starts.
- **CC65** switches portamento on (a slide between slurred notes); otherwise there is none —
  only when you ask for it (*Portamento*, in ms).
- **MPE**: a note's pressure is its own dynamic; its slide moves where the bow plays, towards
  the bridge or the fingerboard; its pitch bends it.

A note's own **articulation** (the piano roll's articulation lane) is heard as it says:
*sustain* re-bows it, *legato* slurs it, *staccato* stops the bow on the string, *spiccato*
bounces it, *pizzicato* plucks it, *tremolo* shakes the bow; its **bow** (down or up) chooses
the direction. Without one, the bow alternates by itself (*Bow*: *Alternate*, or always down,
or always up), and starts down after a rest.

## The Play view

**The picture** (left) is the instrument as it is played: the sounding string lit, the
finger where the note stops it, **the bow** across it at its contact point, pressing and
moving as it does (drag across the picture to move the contact and the force).

On the Edit view, **the Schelleng diagram**: the bow's force (up) against its distance from the
bridge (across), both on log scales. The amber region is where the string keeps its
Helmholtz motion — the clean, singing tone. Above it the bow presses too hard for its speed
(a raucous crunch); under it too lightly (a breathy, whistling surface sound); the region
narrows towards the bridge, as Schelleng drew it in 1973. The garnet point is the bow now.
**Drag the point** to move the contact (across) and the force (up and down).

**The instrument's card** (right): *Cello* or *Double bass*; *Arco*, *Pizzicato* or
*Tremolo*; and the four numbers a player turns — **Dynamic** (the dynamic with no
controller and no velocity's share), **Contact** (× the dynamic's own distance from the
bridge: ×0.7 nearer the bridge, ×1.4 towards the fingerboard, sul tasto), **Force** (× the
dynamic's own, ×0.5 … ×2), **Bow speed** (× its own). The read-out under them says the same
numbers and what they make: the note, the string, the dynamic's marking, β, the force in
newtons with the region it must stay in, the bow's speed in cm/s.

Under them: **Vibrato** (depth, rate, delay), **Body and string** (how much of the body is
heard, its size, how long it rings) and **Output** (*Ring*: how long a note rings once the bow
has lifted and the key is up; the gain).

## The Edit view

Every part, each with its card: the **dynamic** and what moves it; the **bow** (its contact,
force, speed, attack, lift, the bow change's time, the hair's noise, MPE's slide, and the
friction law: *Hyperbolic* — the classic curve of rosin, the default — or *Thermal*, a model in
which the rosin's grip depends on how warm the contact is, an option to compare); the bow's
**direction**, the **tremolo**'s rate and the **pizzicato**'s point, finger and decay; the
**vibrato** (its delay and onset, its life — how much its rate and depth wander from cycle to
cycle — and its controller); the **body and the string** (the string's decay, the ring); the
**output** (low and high cuts); the **voice** (play mode, voices, finger and portamento
times, bend range, octave, transpose, fine, the seed of its small variations).

## What it is made of

- **The strings** — a cello's C2 G2 D3 A3 over 690 mm, a double bass's E1 A1 D2 G2 over
  1 060 mm (the orchestra's tuning), each with its tension (the manufacturers' ranges), so
  each has its own impedance under the bow; a stopped string rings shorter than an open one.
  A key plays on the highest string whose open pitch is at or under it.
- **The bow** — the friction junction of McIntyre, Schumacher and Woodhouse (1983), solved
  exactly every sample, the rosin's curve that of the literature; the string's **twist**
  under the bow (its torsion) is part of the contact, which is what lets the Helmholtz motion
  start cleanly; the **hair's noise** is a layer of its own (*Bow noise*).
- **The body** — Fides's cello body (its signature modes); the double bass's is the cello's
  **scaled to its size** (no measured double-bass body was found: said plainly).
- **The pitch** — a bowed string does not sound its free length's pitch (the friction moves
  it); Arcus corrects it from a table measured on the model itself, so a bowed note is within
  a cent of its key; a plucked one within a tenth.
- **The vibrato** moves the string's length; the partials' loudness moves as they sweep
  through the body's resonances — which is how a real vibrato is heard (Mellody & Wakefield,
  2000).
- **The tuning** of the project (Project ▸ Tuning) reaches it like every pitched instrument.

## Presets

Twenty-four, the cello's and the double bass's: the plain and cantabile lines, sul tasto and
near the bridge, without vibrato, détaché and marcato, portamento, double stops, pizzicato
orchestral and ringing (and the bass's jazz pizzicato), tremolo, a slow bow, MPE. Their four
macros, a new Arcus's too: Expression (the dynamic), Contact, Body, Vibrato.

The violin and the viola are in the same engine but not yet offered: they wait for the same
measured gate the cello and the bass passed.
