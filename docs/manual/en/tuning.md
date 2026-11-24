# The song's tuning

*Cantus 0.1.0-beta.18 · [Leer en español](../es/tuning.md) · back to the [manual](README.md)*

Every song starts in equal temperament with A4 at 440 Hz, every song's default. When a piece
wants another tuning — a baroque pitch, a historical temperament, just intonation, a Scala
scale of 31 notes, or the tuning another program leads — you set it once for the song, and
**every pitched instrument of Cantus plays it**: Chorda, Exemplar and the sampled instruments,
the synthesisers, Fides, Gravis, Organum; the pitched effects (Tonus, Saltus, Nitor's tonic,
Loquax) follow it; hosted plug-ins hear it as a per-note tuning where they can. A track can
follow it, ignore it, or have its own.

**Open it** with *Options ▸ Tuning…* or **Ctrl+Shift+T** (⌘⇧T on a Mac; the command palette
finds it as "tuning", "temperament", "Scala" or "MTS-ESP"). A song in a tuning of its own says
so on the top bar, after its length: *9 tracks · 64 bars · 2:03 · Werckmeister III in Eb ·
A415* (when the bar is short of room, "· A415" goes first, then the temperament, each whole).
A song in the default says nothing.

The window stays in front of the song: while it is open the keys do not reach the project
behind it, but for **Ctrl+Z** / **Ctrl+Y** (every change in it is one step of the history) and
its own shortcut, which closes it; **Esc** closes it too. Opening another song closes it. On a
narrow screen (or at 200 % on a small one) the circle and the degrees move under the controls.

## The system

- **Equal** — twelve equal steps to the octave.
- **Temperament** — a historical or just tuning, chosen from the list: Pythagorean,
  quarter- and sixth-comma meantone, Werckmeister III and IV, Kirnberger II and III, Vallotti,
  Young II (1799), Kellner, Neidhardt III "große Stadt" (1724), Bach (Lehman), just intonation
  major and minor, and a 7-limit "barbershop" tuning. **Key** rotates it so its purest key sits
  where you play (the ‹ › steps it a semitone). **Follow the song's key** rotates it to the
  song's key whenever that changes (the key is then greyed: the song sets it) — set the key in
  the top bar first. It can always be unticked.
- **Scala** — a scale read from a `.scl` file (and its `.kbm` keyboard map, if it has one),
  any number of notes to any period: 31-EDO, Bohlen–Pierce, a gamelan's slendro. The
  **Keyboard map** row opens a list: no map (degree 0 on C4), or import a `.kbm`.
- **MTS-ESP** — the tuning another program leads (see below).

A keyboard map belongs to a scale: choosing *Equal* or *Temperament* removes it.

## The pitch

**A4** is the note every other one is tuned from: it sounds at this frequency whatever the
temperament, and the others move with it. The **presets** are the pitches musicians ask for:
415 (baroque), 430 (classical), 432, 440 (concert), 442 and 443 (many orchestras). The ‹ ›
step it 1 Hz; a click on the value types another (442.5, say) and Enter sets it.

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
second and its fifth in cents from pure; the notes are spelt by interval (E's third is G#, not
Ab). The middle names the key the tuning is rotated to, and its A4.

Under it, **the degrees**: each note's cents above C, its cents **from equal temperament** (at
the chosen A4: A reads 0), and its fifth's and third's beat rates (blank for a key a map leaves
unmapped). A scale that is not twelve to the octave lists its degrees as its file wrote them (a
ratio as a ratio: "2/1"), in cents, and the nearest key of a twelve-note keyboard with its
distance ("C +38.71": degree 0 is C).

## Scala files

**Import .scl / .kbm…** reads a scale, and its keyboard map — picked with it, or beside it
with the same name. The song keeps the **scale**, not the file: send the song to someone and
the tuning goes with it. A file the Scala format does not allow is refused with the reason, in
your language ("Not read: it says 17 pitches but holds 16."), and nothing changes. A `.kbm`
alone maps what already sounds: equal temperament or a temperament become their twelve notes
as a Scala scale, with that map. **Export…** writes the tuning as a `.scl` (a temperament or
equal temperament as its twelve notes in cents) and, when it has a keyboard map, the `.kbm`
beside it — for Surge XT, Pianoteq, a hardware synth's editor, or Scala itself.

## MTS-ESP: following another program, and leading them

MTS-ESP (by ODDSound) is how tuning travels between programs and plug-ins on one computer.
Cantus uses it when it is installed — ODDSound's free **MTS-ESP Mini**, **Surge XT** or
**Pianoteq** install it; Cantus does not ship it. Without it the MTS-ESP choice is greyed and
the window says why.

- **Follow**: choose *MTS-ESP* as the system and every instrument of the song plays the table
  the master program publishes, live (a render takes the table it finds when it starts). With
  no master, it plays equal temperament at A4. If Cantus was the master, choosing it resigns:
  it cannot follow and lead at once.
- **Lead**: **Be the MTS-ESP master** publishes the song's tuning, and publishes it again each
  time you change it: Surge XT, Vital, Pianoteq and every other MTS-ESP plug-in — inside Cantus
  or in another program — play in the song's tuning. Only one program may lead at a time:
  while another does, Cantus says so and leaves it alone. If that program closed or crashed and
  left its place taken, **Reinitialise MTS-ESP** clears it (it also disconnects a master still
  running). Once on, it can always be switched off.

## A track's own tuning

In a track's inspector, **Tuning** says what it plays and opens a list:

- **Follows the project** — the default.
- **Equal temperament** — this track ignores the song's tuning: a drum bus, a sampled loop, a
  bass you want plain.
- **Its own…** — opens the same window for this track: everything above, for it alone. *Follow
  the project* in its footer gives it back.

## The audition

**Play a cadence in …** plays I–IV–V–I (i–iv–V–i in a minor song) in the tuning's key: on the
track the window edits; for the project's tuning, on the selected track when it follows the
project, else on the first instrument track that does (a track with a tuning of its own would
play its own; when none follows, the window says so). So you hear the temperament's colour
before the song. A scale that is not twelve notes to the octave has no cadence: play it from
the keyboard.

## Chorda: a tuning of its own, from inside Chorda

Chorda can play in a tuning of its own while the rest of the song stays as it is. Its *Edit*
view has a **Temperament** card:

- **Plays in**: *The project's tuning* (the default) or *Its own*.
- **Tuning**: the presets, in groups — *Equal* (Concert, octaves stretched; Studio tight;
  Bright, wide stretch), *Historical* and *Just intonation* (each in the **Key** below:
  Werckmeister III in E, meantone in D …), *Character* (Honky-tonk, Old parlour upright, Saloon,
  Slightly out, Baroque A415, Verdi A432) and *Your tunings*. Choosing one gives Chorda its own
  tuning and sets the stretch and the unisons it needs, in one step of the history (**Ctrl+Z**
  puts all of it back).
- **Key**: where a historical or just tuning sits (‹ › a semitone); each Chorda track keeps its
  own. Following a temperament of the project's, it shows the key that sounds, greyed: the
  project sets it.
- *Save as mine* keeps the current tuning as a Scala file of yours (in the data
  folder's `Tuning`), listed under *Your tunings*; *Import .scl* and *Export .scl* read and write
  `.scl` / `.kbm`.
- **Cadence**: hear it before playing.
- The **circle of fifths** and the **purest and roughest major thirds** (the roughest first)
  say which keys sound calm and which rough; in equal temperament all twelve are the same size,
  and the card says so.

**A4** follows one rule, so nothing old changes: while the track plays no tuning, Chorda's own
A4 knob is the pitch, as in every older song; when it plays the project's tuning, the knob
shows the project's A4, greyed ("the project's"); with a tuning of its own, it sets that
tuning's A4 (‹ ›, or a click to type it). The line under the stretch says which tuning the
piano plays; a click on it opens *Options ▸ Tuning…*. Chorda's **stretch**, **stiffness** and
**unisons** stay its own: a piano technician stretches the octaves of any temperament, and
Chorda does the same on top of it.

## Good to know

- A keyboard map may leave keys unmapped: they sound nothing, as in Scala.
- Organum (one wheel serves several keys) and Lamina (keys folded by octaves) follow the pitch
  class of a scale that is not twelve to the octave.
- Pulsus and Tympanum are unpitched: the tuning leaves them alone.
