# The song's tuning

*Cantus 0.1.0-beta.18 · [Leer en español](../es/tuning.md) · back to the [manual](README.md)*

Every song starts in equal temperament with A4 at 440 Hz, as every song before this one.
When a piece wants another tuning — a baroque pitch, a historical temperament, just
intonation, a Scala scale of 31 notes, or the tuning another program leads — you set it
once for the song, and **every pitched instrument of Cantus plays it**: Chorda, Exemplar and
the sampled instruments, the synthesisers, Fides, Gravis, Organum; the pitched effects
(Tonus, Saltus, Nitor's tonic, Loquax) follow it; hosted plug-ins hear it as a per-note
tuning where they can. A track can follow it, ignore it, or have its own.

**Open it** with *Options ▸ Tuning…* or **Ctrl+Alt+T** (the command palette finds it as
"tuning", "temperament", "Scala" or "MTS-ESP"). A song in a tuning of its own says so on the
top bar, after its length: *9 tracks · 64 bars · 2:03 · Werckmeister III in Eb · A415*. A song
in the default says nothing.

Every change in the window is one step of the history: **Ctrl+Z** puts the tuning back.

## The system

- **Equal** — twelve equal steps to the octave.
- **Temperament** — a historical or just tuning, chosen from the list: Pythagorean,
  quarter- and sixth-comma meantone, Werckmeister III and IV, Kirnberger II and III, Vallotti,
  Young (1799), Kellner, Neidhardt, Bach (Lehman), just intonation major and minor, and a
  7-limit "barbershop" tuning. **Key** rotates it so its purest key sits where you play
  (the ‹ › steps it a semitone). **Follow the song's key** rotates it to the song's key
  whenever that changes — set the key in the top bar first.
- **Scala** — a scale read from a `.scl` file (and its `.kbm` keyboard map, if it has one),
  any number of notes to any period: 31-EDO, Bohlen–Pierce, a gamelan's slendro.
- **MTS-ESP** — the tuning another program leads (see below).

## The pitch

**A4** is the note every other one is tuned from: it sounds at this frequency whatever the
temperament, and the others move with it. The **presets** are the pitches musicians ask for:
415 (baroque), 430 (classical), 432, 440 (concert), 442 and 443 (many orchestras). The ‹ ›
step it 1 Hz.

A **keyboard map** names its own reference — "key 69 is 442 Hz" — so with one the row is
called **Diapason**: 440 plays the map as written, and any other pitch raises or lowers the
whole map.

## The circle of fifths and the degrees

The right side shows what you will hear before you hear it. The **circle of fifths** goes
round C, G, D, A … F: each arc of the **outer ring** is the fifth from its note to the next,
each arc of the **inner ring** the major third on its note, coloured by **how fast it beats**
at middle C — a pure interval does not beat (the meters' low colour), a fast-beating one is
their clip colour. That is how tuners compare temperaments: in equal temperament every fifth
beats a little under once a second and every third about ten times; in Werckmeister III the
keys near C have calm thirds and the far ones sharper ones; in meantone the thirds are pure and
one fifth howls (the wolf). Rest the pointer on a note to read its fifth and third in beats a
second and its fifth in cents from pure. The middle names the key the tuning is rotated to, and
the pitch of A4.

Under it, **the degrees**: each note's cents above C, its cents **from equal temperament** (at
the chosen A4: A reads 0), and its fifth's and third's beat rates. A scale that is not twelve
to the octave lists its degrees as its file wrote them, in cents, and the nearest key of a
twelve-note keyboard with its distance ("C +38.71": degree 0 is C).

## Scala files

**Import .scl / .kbm…** reads a scale, and its keyboard map — picked with it, or beside it
with the same name. The song keeps the **scale**, not the file: send the song to someone and
the tuning goes with it. A file the Scala format does not allow is refused with the reason
("Not read: 16 pitches of 17"), and nothing changes. **Export…** writes the tuning as a `.scl`
(a temperament or equal temperament as its twelve notes in cents) and, when it has a keyboard
map, the `.kbm` beside it — for Surge XT, Pianoteq, a hardware synth's editor, or Scala itself.

## MTS-ESP: following another program, and leading them

MTS-ESP (by ODDSound) is how tuning travels between programs and plug-ins on one computer.
Cantus uses it when it is installed — ODDSound's free **MTS-ESP Mini**, **Surge XT** or
**Pianoteq** install it; Cantus does not ship it. Without it the MTS-ESP choice is greyed and
the window says why.

- **Follow**: choose *MTS-ESP* as the system and every instrument of the song plays the table
  the master program publishes, live (a render takes the table it finds when it starts). With
  no master, it plays equal temperament at A4.
- **Lead**: **Be the MTS-ESP master** publishes the song's tuning, and publishes it again each
  time you change it: Surge XT, Vital, Pianoteq and every other MTS-ESP plug-in — inside Cantus
  or in another program — play in the song's tuning. Only one program may lead at a time:
  while another does, Cantus says so and leaves it alone. If that program closed or crashed and
  left its place taken, **Reinitialise MTS-ESP** clears it (it also disconnects a master still
  running). A song that follows MTS-ESP cannot also lead it.

## A track's own tuning

In a track's inspector, **Tuning** says what it plays and opens a list:

- **Follows the project** — the default.
- **Equal temperament** — this track ignores the song's tuning: a drum bus, a sampled loop, a
  bass you want plain.
- **Its own…** — opens the same window for this track: everything above, for it alone. *Follow
  the project* in its footer gives it back.

## The audition

**Play a cadence in …** plays I–IV–V–I (i–iv–V–i in a minor song) in the tuning's key on the
track the window edits, the selected track, or the first instrument track — so you hear the
temperament's colour before the song.

## Chorda: a tuning of its own, from inside Chorda

Chorda can play in a tuning of its own while the rest of the song stays as it is. Its *Edit*
view has a **Temperament** card:

- **Plays in**: *The project's tuning* (the default) or *Its own*.
- **Tuning**: the presets, in groups — *Equal* (Concert, stretched as tuned; Studio tight;
  Bright, wide stretch), *Historical* and *Just intonation* (each in the **Key** below:
  Werckmeister III in E, meantone in D …), *Character* (Honky-tonk, Old parlour upright, Saloon,
  Slightly out, Baroque A415, Verdi A432) and *Yours*. Choosing one gives Chorda its own tuning
  and sets the stretch and the unisons it needs, in one step of the history (**Ctrl+Z** puts
  all of it back).
- **Key**: where a historical or just tuning sits (‹ › a semitone).
- **Yours**: *Save as mine* keeps the current tuning as a Scala file of yours (in the data
  folder's `Tuning`), listed under *Yours*; *Import…* and *Export…* read and write `.scl` / `.kbm`.
- **Cadence**: hear it before playing.
- The **circle of fifths** and the **purest and roughest major thirds** say which keys sound
  calm and which rough.

**A4** follows one rule, so nothing old changes: while the track plays no tuning, Chorda's own
A4 knob is the pitch, as in every older song; when it plays the project's tuning, the knob
shows the project's A4, greyed ("the project's"); with a tuning of its own, it sets that
tuning's A4. The line under the stretch says which tuning the piano plays; a click on it opens
*Project ▸ Tuning*. Chorda's **stretch**, **stiffness** and **unisons** stay its own: a piano
technician stretches the octaves of any temperament, and Chorda does the same on top of it.

## Good to know

- A keyboard map may leave keys unmapped: they sound nothing, as in Scala.
- Organum (one wheel serves several keys) and Lamina (keys folded by octaves) follow the pitch
  class of a scale that is not twelve to the octave.
- Pulsus and Tympanum are unpitched: the tuning leaves them alone.
