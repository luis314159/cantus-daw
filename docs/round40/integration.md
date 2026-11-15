# Round 40 — integration: Chorda toward the Salamander, the owner's answers, recorded pianos, Tympanum and the instruments' pictures

**The ask** (the owner, 2026-11-11, `docs/next-session.md`): Chorda *"tan bien como el
Salamander"* — his ABX at beta.14 told Chorda 2 from the Salamander 16 of 16 and preferred the
Salamander 7 of 7; insert and delete time with the section drag that moves the song (6 of 8
leaders have it); memory, the scorecard's one lost row; and his twenty-two answers to round 39's
decisions (`docs/round39/integration.md`, «The owner's answers»), built as features. Before
anything was built he looked at round 39 on his own screen: *"no encontré nada malo nuevo"* —
**round 39's look is accepted** (2026-11-11).

On 2026-11-12 he widened the round (phase 3, `BUILDING-p3.md`): **Chorda takes recorded piano
packs** — free or bought, *following the standard* (SFZ, Decent Sampler, SF2, the
Salamander/bitKlavier naming) — while the modelled piano stays the goal; **an internal mixer with
outputs to mixer strips** for Chorda and for **a Superior Drummer-class acoustic drum
instrument** (modelled and recorded kits, the kit in its room, a mic mixer with bleed, each mic
to its own strip). He named it **Tympanum** the same day and asked its picture to be *"muy muy
bonito"*. On 2026-11-13 he asked for a hybrid drum voice as an option — *"pero el generado debe
mejorar mucho más para que sea algo muy bueno y deseable de usar"* — and for microtuning (a
design note for round 41).

**Where it stands (2026-11-14).** Everything below is merged into `r40/integration` — the
last two builders too: `r40/drums-gen` at `4e622739` (drums-gen 3: Tympanum's cost under budget,
71 of the 162 measured cues, the eleventh listen) and `r40/chorda` at `927fd92a` (candidates
13–16). At the close the owner asked for the ship (2026-11-14): **candidate 16 is Chorda's
default** (`DSP_VERSION` 7, ADR 0098's third amendment), **Tympanum is in the device picker**
(ADR 0124's fifth amendment), both blind evaluations stay in the BACKLOG, and the round is
squashed onto master; no beta.15 is cut — the owner's ear comes first, and he cannot listen for
some days.

## How

Three phases from master `dd4ad68b`, each branch merged into `r40/integration`
(`F:\projects\daw-r40\integration`), one worktree per agent, at most three cargo builds at once
through `F:\projects\daw-r40\slot.sh` (31 GB of RAM), no WSL by builders (C: was short).

1. **Phase 1 (2026-11-11)** — contract `f00ed550` (`BUILDING.md`, `briefs.md`): `chorda` (the
   voicing toward the Salamander), `ripple`, `memory`, `look` (the owner's look answers) and
   `dancer` (design only: the Slate cast).
2. **Phase 2 (2026-11-12)** — `pads` (a pad's own mixer track, *Hear the key*, *Record: before
   FX · after FX*; owner of `processor*.rs`), `delay` (tempo sync, Delay as CLAP/VST3), and
   `cast` (the twelve dancers the owner approved). **The mid-round review** (three read-only
   reviewers on `91339a2b`) → `fix-a` (engine) and `fix-b` (model, keys, interface, words);
   **review 2** of the merged fixes (on `1357827a`) → `fix-c` and `fix-d`.
3. **Phase 3 (2026-11-12)** — contract `45f9fa7a` (`BUILDING-p3.md`, `briefs-p3.md`; schema
   66–68, ADRs 0123–0126, manifest format 5): `scene`, `scene-px`, `tymp-engine`, `tymp-ui`,
   `chorda-src`. **The mid-phase integrated review** of `40a27bb7` (7 043 tests green; three
   agents) → `fix-e` (engine) and `fix-f` (UI, model, content). Then, from the owner's listens
   and looks: `drums-gen` (the generated sound, twice), `drums-hyb` (the hybrid voice), `ladder`
   (the representation study before a model 6), `abx5` (a listening kit for strangers),
   `visual` (the pictures up to the reference, eight checkpoints) and `hyb-refit` (the hybrid's
   packs version 2).

| Branch | Merged at | What | Report |
|---|---|---|---|
| `r40/look` | `ff9815b5` | The owner's look answers: dock at the edge, maximise keeps the inspector, top-bar extras, value bars, Mute in colour, Paper's sheets, Lapis C, softer Bordeaux and Pewter (ADRs 0110, 0111 amended) | `look.md` |
| `r40/memory` | `545c5168` | The private bytes outside the heap mapped; four levers, −15 to −17 MB mapped (ADR 0120) | `memory.md` |
| `r40/ripple` | `41d47e7b` | Insert and delete time, the section drag, the ripple mode (ADR 0116, keymap 17) | `ripple.md` |
| `r40/delay` | `4cd32006` | Delay synced to the tempo; *Cantus Delay* as a CLAP/VST3 plug-in (ADR 0119) | `delay.md` |
| `r40/pads` | `dc11879a` | A pad's own mixer track chosen per pad; *Hear the key*; *Record: before FX · after FX*; the engine starts with no compensation line (ADRs 0117, 0118, 0120 §7; schema 62) | `pads.md` |
| `r40/cast` | `9300d0c2` | The Slate cast: twelve dancers, Compás the default (ADR 0122, schema 65) | `cast.md` |
| `r40/fix-a`, `r40/fix-b` | `06877400`, `bccdc85d` | The mid-round review's findings | `fix-a.md`, `fix-b.md` |
| `r40/fix-c`, `r40/fix-d` | `76240ece`, `0dd7956e` | Review 2's findings; Slate Paper "less blue" compared and kept | `fix-c.md`, `fix-d.md` |
| `r40/scene` | `cc75a06c` | The instruments' scenes as projected vectors; the *Drawn · Photographic* setting and its backend (ADR 0126) | `scene.md` |
| `r40/scene-px` | `3404391e` (through `visual`) | The photographic renderer (ADR 0104 grown up) | `scene-px.md` |
| `r40/chorda-src` | `4d14aff8` | Chorda's recorded source, its mixer, the piano packs, *Install from a file…* (ADR 0125, schema 68) | `chorda-src.md` |
| `r40/tymp-engine` | `6c4dd2d3` | Tympanum's engine, named outputs, the drum packs, models 4 and 5 (ADRs 0123, 0124; schemas 66, 67) | `tymp-engine.md` |
| `r40/tymp-ui` | `cd88086e` | Tympanum's views, the shared instrument mixer, bands on the desk (ADR 0016, 0123 amended; schema 70) | `tymp-ui.md` |
| `r40/fix-e`, `r40/fix-f` | `f319e653`, `24ab662d` | The mid-phase review's findings | `fix-e.md`, `fix-f.md` |
| `r40/drums-gen` | `b33054e2` | The generated drums after listens 8 and 9; the fifteen cues; the "iuuu" swell fixed | `tymp-engine.md` (top sections) |
| `r40/drums-hyb` | `c50bb62a` | Tympanum's hybrid voice (ADR 0124 third amendment, schema 71) | `drums-hyb.md` |
| `r40/ladder` | `c4967df5` | The representation ladder and the owner's MUSHRA | `ladder.md` |
| `r40/abx5` | `22d4ec97` | Chorda's listening kit 5, for other listeners | `abx5.md` |
| `r40/visual` | `87914c23` | The pictures up to the photographic reference, checkpoints 1–8 | `visual.md` |
| `r40/chorda` | `faa71b75` (candidate 12) | The voicing, candidates 1–12 | `chorda.md` |
| `r40/hyb-refit` | `e714654a` | The hybrid's packs refitted as version 2 on the merged model | `hyb-refit.md` |
| `r40/drums-gen` (close-out) | `4e622739` | drums-gen 3: the cost 18.7 → 7.3 % of a core, accept_b10 57 → 71 of 162, the hybrid packs version 3 (uploaded with the owner's OK); the eleventh listen | `tymp-engine.md` («drums-gen 3») |
| `r40/chorda` (close-out) | `927fd92a` | Candidates 13–16 and their two knobs (`draw.<key>`, `hf_key.<key>`, neutral by default; no sample moves) | `chorda.md` |

**What the round taught about running it** (in the memory, `docs/claude-memory/round-40.md`):
an owner interrupt stops *every* background agent for good — each was relaunched with a takeover
brief; two worktrees sharing one `target/` linked each other's crates (cargo hashes a workspace
crate without its path) — `delay` and `pads` each saw "impossible" errors until one built with
its own profile; F: reached 0 bytes once (git commits failed) and was freed by deleting merged
builders' targets — check `df -h /f` before every launch.

## What each part brought

### Phase 1 — the look, the ripple, the memory

**The owner's look answers (`look`; ADR 0110 and 0111 amended).** Each is configurable with his
answer as the default (*Settings ▸ Appearance ▸ Layout and controls*, settings of this machine,
never of the song): the mixer's right dock **at the window's edge** with the next empty strips
numbered before it (or *After the last strip*); **maximise keeps the inspector** beside the area
(`I` hides it alone; or *Nothing*); the top bar's **extras** — *None* (default) · *Automation
arm* · *Typing keyboard* · *Both*; **value bars** in the inspector (or *Slim bars*); **Mute lit
in colour** (or *Grey*); Paper's menus and dialogs with a crisper edge, a contact shadow and a
lighter scrim; Slate Lapis's room **C, the bluest** (the owner: *"me gusta más la C, los más
azules"* — saturation × 4.2 Night, × 5.5 Paper, every contrast held at Slate's luminance);
Bordeaux and Pewter softer. Asked for Slate Paper "menos azul", he compared today's, half and
neutral side by side and **kept today's** (fix-d, R2-B11). The manual tells FL users how to
group (a bus or a VCA group).

**Insert and delete time, the section drag, the ripple mode (`ripple`; ADR 0116, keymap 17).**
Researched in seven leaders' manuals first. *Insert time* (`Insert`) and *Delete time*
(`Ctrl+Del`) over a time selection act on the whole song: clips split at the edges, automation,
tempo and metre changes, markers, sections, loop, punch and the mixer's cues move (fix-b B-1),
and **delete time heals the cut an insert left** (none of the leaders does); the property
*insert then delete = identity to the tick* holds on 400 random songs. **A section dragged in
its lane moves its bars on every track**: *Settings ▸ Editing ▸ Section drag* — **Insert** (the
default, as Logic and Studio One) or *Replace*; `Shift` does the other, `Ctrl` copies. The
ripple mode is REAPER's three-way switch (*Off · This track · Every track*, `Alt+Shift+P`).
Cues keep the scene each piece of music plays under (fix-d R2-B2, 3 000 random songs); the
time keys act on the arrangement wherever it is — docked, floating or maximised — and say why
when they do not (fix-b C-3, fix-d R2-B1).

**Memory (`memory`; ADR 0120, 0107 amended).** The ~140 MB of an empty song mapped byte by byte
from outside the process (`tools/scorecard/memmap.ps1 -Detail`, a heap census build): Cantus's
own live heap is 25 MB; **a bare eframe window with one label holds 105–109 MB** on the
scorecard laptop's AMD OpenGL driver. Four levers: the kit's layer atlas starts at 512² and
grows once if needed (−8.2 MB); Exemplar's built-in sounds are computed when an instrument may
be chosen, not at every start (−7.8); a language's strings parsed when first read (−3.3); egui
told of a texture side no wider than 4 096 (−2.2). The final branch mapped **136.5 → 121.2 MB**
(empty) and **140.4 → 123.6 MB** (*Beat*) on a busy machine — a map, not the scorecard. `pads`
made the fifth lever (the application's engine starts with no compensation line, −2.25 MB,
ADR 0120 §7). A CPU-presented window (11 MB, but 37 ms a full 1080p redraw on one core) went to
BACKLOG §4b. **The scorecard was not measured this round** (it needs a quiet machine with every
agent stopped).

**Chorda's voicing (`chorda`; ADR 0098)** — the whole trail is its own section below.

### Phase 2 — pads, Delay, the cast

**A pad's own mixer track, chosen per pad (`pads`; ADR 0117).** The model existed since round
37 (`DrumPad::output`); what was missing was the way in. Every pad row in the rack has the mixer
track's well (the kit's number until one is chosen); a click opens *The kit's strip (n)*,
*First free strip (n)* and every strip — one undo step, a free strip taking the pad's name.
*Merge back* undoes *Split to strips* and only that (fix-b B-2/B-3, fix-d R2-B3).

**Hear the key, Record before or after FX (`pads`; ADR 0118, schema 62).** *Hear the key* under
a keyed insert's *From* row plays what the insert's detector hears (a compressor's *Listen:
Key* to the bit); never saved, never in a render, and never in an *After FX* take (fix-a A-M1:
a strip writing an After FX take hears no key). *Take from: Before FX · After FX* (Spanish *Se
graba: Sin FX · Con FX*) under *Monitor*: an after-FX take is what was heard, stereo, landed
where it was played.

**Delay synced to the tempo, and Cantus Delay as a plug-in (`delay`; ADR 0119).** *Free · Sync*
in Delay's window (*At the tempo* became the switch); note values 1/64 … 4/1, straight, dotted
or triplet; inside a tempo ramp the repeats slide as tape, a jump crossfades over 50 ms; the
line grows with the note (up to 48 s) on ADR 0022's path, built off the audio thread. The sync
off is round 39's unit instruction for instruction (`DSP_VERSION` 6, no golden moved). **Cantus
Delay** is the 25th exported plug-in — CLAP `com.github.lalmazan.cantus.delay`, VST3
`625C4F90AB5A380458F1F1D3D1DCA44C` (ids forever; **the owner's confirmation is pending**).
fix-a made the automated note ask for its line in advance (offline = real time), moved a long
line in bounded stretches per callback (a 48 s `supply` had taken 22.9 ms in one call) and took
the sync's target from the told tempo; fix-c stopped a second reset bringing the old line back,
made a listen equal the export after a mid-song memory answer, and reserved only the line a
modulator reaches.

**The Slate cast (`cast`; ADR 0122, schema 65).** The designer's Python rig ported number for
number; twelve characters the owner approved, each with its own groove, step and signature:
**Compás** (the new default — *approved as is*), Tono, Síncopa (*"noir y paper se ven cool"*),
Bajo, Onda (the oscilloscope), Voz, Glissando (neon, an original genre-inspired design), Eco,
Vinilo, Cinta, Metrónomo, Espectro (which hears the song's spectrum, live and in an exported
video). Songs saved without a character keep Fuga; the dancer stays off at start. Side-by-side
GIFs against the approved designs in `docs/round40/cast/`. fix-c stopped the analyser running
at rest and let Espectro hear the song in the companion window; fix-d gave the picker the kit's
scrollbar and Compás his own *Cueing* mood (for the owner's eyes).

### Phase 3 — recorded pianos, Tympanum, the mixer's bands, the pictures

**Named outputs (ADR 0123; schema 66).** An instrument may have named outputs (`OutputSlot`:
id, name, family, strip): Chorda and Tympanum. Each goes to *Main*, *its own strip* or a
*chosen strip* in one click (`Command::SetInstrumentOutput`, an exact inverse); the plans *All
to Main*, *Pieces to strips* (one strip per family) and *Each mic to its own strip* are one
undo step each, and **the split plays the unsplit instrument to the bit** at 512 and 333
frames, offline and live. Each output has its mix (level, balance, width, polarity, mute) and a
**delay** (0–50 ms, to time-align a mic): fix-e made the line glide instead of clearing on
every change and exist whenever the delay is set *or automated*.

**Chorda's recorded source and its mixer (`chorda-src`; ADR 0125, schema 68).** *Source* at the
left of *Play*: *Modelled* (Chorda 2; *Previous models* ▸ Chorda 1) · *Recorded* (the Salamander
first and the default recorded piano until the owner's ABX of bitKlavier; Accurate-Salamander,
bitKlavier's Steinway D in nine microphone images, Upright Piano KW, Old Piano FB; *Piano in 162*
and *Helsinki 1967 Pro* by their authors' pages) · *Your own pianos* · ***Install from a
file…*** (an SFZ, a `.dspreset`, an SF2, a DrumGizmo kit, an archive — off the window's thread
with progress and cancel, fix-f A). A source switch is one undo step and keeps notes,
automation, strips and routes. The recorded piano plays through Exemplar's engine, its zone
table sized to the instrument (every bitKlavier image at once, ≈ 6 000 zones, fits); packs
are kept as FLAC at install, decoded again and compared sample for sample (bitKlavier's nine
images 13.8 GB instead of 34.2). The **Mixer** view (the frame's fourth view) shows the piano in
its room, the selected output's card with its credit, *All to Main* · *Each mic to its own
strip*, then *Piano*, the outputs by family and *Room* — where **Add our room** (Small room ·
Studio · Hall) puts Tympanum's room on a recorded piano. The model's five outputs (*Hammers,
Player, Audience, Room, Mechanics*) still play together on Main until the voicing's renderer
plays them apart (`MODEL_RENDERS_OUTPUTS` is false). fix-f kept a recorded piano through a
preset, A/B or *Mix* (they had switched it back to the model). Measured, bitKlavier streamed:
one image 6.0 % of a core, two 12.6 %, four 25.3 % — and at four the 16 GB stream cache at its
limit (a 317 ms offline block; BACKLOG: a larger or FLAC-backed cache).

**Tympanum, the acoustic drummer (`tymp-engine`, `tymp-ui`; ADR 0124, schema 67).** A new
instrument over Pulsus's engine (Pulsus stays the pad machine). **Modelled kits** — *Studio
Maple* (12 pieces, 12 movable virtual mics), *Jazz Birch* (measured off Virtuosity Drums, CC0),
and the designs' others — and **recorded multi-mic kits** (Gogodze Phu II, DRSKit 13 mics,
CrocellKit 15, MuldjordKit 13, Virtuosity 7, Big Rusty 6; each licence read on its page, each
download's hash checked), a user's own SFZ, DrumGizmo kit or WAV folder. One draw per hit shared
by every mic; hi-hat openness on CC4 with the foot's chick; the *Feel* card (timing only ever
late, humanize, vary); the snare's flam, drag, buzz and held roll, the hat's foot splash, the
kick's beater; a kit change keeps the old kit playing until the new one's files are in; a
recorded kit plays its own room by default, our Booth, Studio or Hall (Aula's responses) as an
option — **Room is picture and sound in one undo step**. The **Kit view** draws the kit through
the scene module (a press plays and selects a piece, Alt-click walks its articulations, where it
is struck sets `p{n}.pos`); the **Mixer view** has the band of mics (pattern, distance, aim,
bleed), *What it hears*, *Every mic* (the bleed matrix) and the strips with each mic's meter.
**Voice: Generated · Hybrid** (`drums-hyb`; ADR 0124 third amendment, schema 71): *Generated*
(default) is the model alone, to the bit; *Hybrid* plays under it the first 30–200 ms of a
recorded kit's hits that the model does not explain — a spectral residual deconvolved to each
piece's dry source, so the virtual mics, bleed, room and outputs apply to it as to the model.
Its packs (jazz from Virtuosity CC0, studio from DRSKit CC BY 4.0; 5.2–5.3 MB each) were
uploaded to the public release `content-r40` with the owner's yes — version 1 on 2026-11-13,
**version 2** (refitted when the merged model's tables moved, hash `0xED1998F7`) on 2026-11-14
(*"Súbelo"*), the public download verified. The mid-phase review's fixes (below) routed every
kit change through `change_kit`, made a user's DrumGizmo kit play on its own mics and freed the
scenes nothing shows.
**Tympanum joined the device picker at the ship** (*The ship*, below): until then
`tympanum::READY` was false and the kind out of `InstrumentKind::ALL`.

**Bands on the desk (`tymp-ui`; ADR 0016 and 0123 amended, schema 70).** The owner's groups
sheets: a user's group is a band over its strips — a gap and a seam either side, its count, a
chevron that folds it into one strip with its members named and their meters side by side,
never hidden; its right edge dragged takes in the next strip or gives the last back (one undo
step, `Esc` leaves it). An instrument's family reads apart (a solid bar, its glyph, *Drums ·
Tympanum · 3 mics*). One band per strip: a group sits inside one family or takes whole
families; *Each mic to its own strip* makes a sub-group per output family in the same undo
step. A strip fed only by a family's outputs (a bus, parallel compression) can be **added to
the family's band** and leaves it, with the status line's reason, when its routing changes. A
band's head stays readable while the desk scrolls. The shared instrument mixer gained each
output's *Delay* cell.

**The instruments' pictures (`scene`, `scene-px`, `visual`; ADR 0126, ADR 0104).** The owner
chose the designer's projected-vector render over ADR 0104's baked scene (*"vamos frescos con
el diseño mejor"*; *"tiene que quedar incluso mejor"*), then saw the per-pixel renderer polished
and made the pictures **configurable**: *Settings ▸ Appearance ▸ Instrument pictures:* **Drawn**
(the default — light, one cached texture per state, nothing at rest) · **Photographic** (traced
reflections and soft shadows, baked off the interface's thread on a below-normal-priority pool
that lets go 30 s after its last bake; a quick preview while a control glides; the last three
pictures kept). He called the photographic picture *"casi excelente"*, then corrected the
course: **Drawn stays the default and looks as close as it can to Photographic**; one constant
(`DEFAULT_PICTURES`) flips it. `visual` took both to the reference (the concert grand in black
lacquer on the stage, Night, Photographic) over eight checkpoints: a grand's lid in two leaves
that never meets the music desk at any angle (`scene_lid_desk.rs`), the prop stick in its cup,
a modern studio upright, light finishes that keep their colour, the striped chrome stands found
(three causes) and drawn as tube strokes, cymbals with a real bell and lathe, coated heads,
110 strings with the wound bass in copper, the ivory keyboard. The owner on checkpoint 5:
*"¡Van increíbles los pianos, excelente!!!"*. Drawn costs 1.05× its time before the pass over
33 cases (busy machine). The old baked renderer's geometry files are unused but mounted.

## The reviews and their fixes

Three integrated reviews, each with at most three read-only reviewers, each finding reproduced
by a failing test before its fix (CLAUDE.md: the cross-feature bugs are the worst ones).

| Review | On | Fixers | What they found and fixed (detail in each report) |
|---|---|---|---|
| Mid-round | `91339a2b` (look, memory, ripple, delay, pads) | `fix-a` (engine), `fix-b` (model, keys, words) | Hear the key recorded into After FX takes; Delay's automated note never asked for its line, and `supply`/`reset` did O(line) work in one callback; the sync's stale tempo; the mixer's cues ignored by the time edits; *Merge back* removed a strip the user chose; the time keys acted over the mixer; Spanish agreement in the ripple's lines; the record tap's Spanish words; the inspector's value line through its figures. Refuted, for the owner: the ghost strips' numbering (C-1, FL's own) |
| Review 2 | `1357827a` (the fixes merged) | `fix-c` (engine, Delay, dancer), `fix-d` (model, keys, mixer, words, picker) | A second reset brought Delay's old line back; a listen differed from the export after a mid-song memory answer; a modulated note reserved the 48 s line; the dancers' analyser ran at rest and Espectro was deaf in the companion window; the time keys on a floating or maximised arrangement; cues keep the scene in effect; *Merge back* keeps the user's routing; the send bar's alignment; Delay's note list in a short window; the picker's scrollbar and *Metrónomo* whole |
| **Mid-phase** | `40a27bb7` (phase 3 merged, 7 043 tests) | **`fix-e`** (engine), **`fix-f`** (UI, model, content) | Below |

**fix-e** (`fix-e.md`, nothing left open): an output's delay was cleared on every change (a
0 → 20 ms ramp played silence) — now it glides, and a line exists when the delay is set or
automated; a whole `configure` ran on the audio thread every 16 frames under automation — now
`configure_keys` sets the moved keys alone, to the bits of a whole configure, for Tympanum and
Exemplar (a groove with lanes: < 50 whole configures for ~12 000 control points); voice
stealing cut a recorded kit's every mic with no fade — the victim's plays now fade on their own;
the Feel queue's overflow struck strokes early — a roll is scheduled one stroke at a time; a
recorded kit refused once stayed silent all session — a content change forgets refusals;
Tympanum's 32 output lines allocated for every unit — only the masked ones; a modelled kit's
pads all carried id 0 (any configure moved every ringing drum to the last pad's piece) — each
pad its own id, the seed rule written in ADR 0124.

**fix-f** (`fix-f.md`): a kit change never used `change_kit` (routes, levels and strips stayed
keyed to the old kit's mics) — every change now plans on a copy and carries the mixer by name,
then family, in one undo step; *Each mic to its own strip* failed with old sub-groups; a preset,
A/B or *Mix* switched a recorded Chorda back to the model and dropped Tympanum's kit; a user's
DrumGizmo kit installed but never played — it is now built per mic at install, like our packs;
a failed archive install blocked every retry — staging folders, free-space checks, orphans
cleaned; scene caches were never freed (one OS thread and one texture each) — pruned, one shared
worker; *Pieces to strips* named strips in English in Spanish; MuldjordKit's credit in its
author's words; VCAs out of the Out lists. Then, before the merge: *Install from a file…* off
the window's thread with progress and cancel, no doubled disk after a DrumGizmo build, a
modelled kit's mic names in the interface's language.

## The schema: 61 → 71

Every migration writes nothing unless said; every old fixture still loads; `fixtures/v71.cantus`
holds the last.

| Schema | Builder | What |
|---|---|---|
| 62 | `pads` | A track's record tap (`Before FX` default, skipped when default) |
| 63, 64 | — | Reserved for `ripple` and `delay`; both needed none (empty steps) |
| 65 | `cast` | The default dancer is Compás; a dancer saved 59–64 naming no character gets Fuga |
| 66 | contract / `tymp-engine` | An instrument's named outputs |
| 67 | contract / `tymp-engine` | Tympanum |
| 68 | contract / `chorda-src` | Chorda's source |
| 69 | `scene` | The scene's room look (floor, walls, rug as text keys) |
| 70 | `tymp-ui` | A strip added to an instrument's family (`MixerStrip::attached`) |
| 71 | `drums-hyb` | Tympanum's voice (generated or hybrid) |

Keymap 16 → **17** (`InsertTime`, `DeleteTime`, `CycleRipple`). `DSP_VERSION` stays **6**: no
golden hash moved this round (Chorda's candidates are behind neutral knobs; Tympanum has not
shipped; every other DSP change was null at its default to the bit). Manifest format **5**
(pack modes).

## Decisions recorded (ADRs)

New: **0116** insert and delete time, the section drag, the ripple mode · **0117** a pad's own
mixer track · **0118** hear the key, record before or after FX · **0119** Delay synced and as a
plug-in · **0120** the private bytes outside the heap · **0122** the Slate cast · **0123** an
instrument's named outputs · **0124** Tympanum (four amendments: the acoustic voice and Feel;
model 5 measured; the hybrid; the generated sound's causes; and fix-e's seed rule) · **0125**
Chorda's recorded source and mixer · **0126** the scenes as projected vectors (supersedes 0104
§1; 0104's per-pixel renderer lives on as *Photographic*). 0121 was not needed. Amended:
**0016** (a group is a band), **0042** (three pack modes: *Pack* for CC0/CC BY from the official
source; *From the author's site* — free, no stated licence, one click from the author's own URL,
never hosted by us, hash-pinned, falling back to the page and *Install from a file…*; *Manual* —
a login: the page opens and the user brings the file), **0098**, **0107**, **0110**, **0111**.
0123–0126 still read *Proposed*: the integrator marks them accepted with the owner's OK on the
features they hold.

## Chorda: the trail toward the Salamander (`chorda.md`)

**The bar the owner set.** The goal in his words: *"Deja Chorda lo más parecido posible al
Salamander"* — closeness wins over ADR 0101's loudness window (2026-11-11). After candidate 2:
*"Aún le falta a Chorda. Haz un análisis completo de las diferencias de los audios antes de que
me pidas probar"* — an independent analyst who did not know the model compared the audio
(`F:\projects\daw-r40\analysis\informe.md`), and a second analysis later (`analysis-2`); their
scripts became the acceptance suite. After candidate 3: *"aún nos falta bastante"*. After his
blind test of candidate 7 (below): *"cuando tengas una versión que ya creas buena dame el ABX"*
— no ABX per candidate; the bar is **(a)** the tuning gate holds, **(b)** his blind-test
complaints are gone and the top octave he liked is kept, **(c)** no regression against
candidates 7 and 8, **(d)** both analyses better.

**What the candidates found and built** (each part neutral at beta.14's values: model 2 at
beta.14's voicing renders to the contract commit's bits `0x663b_f285_d17a_ebdd`; the default
moves only when the owner picks a candidate):
- **c1–c2**: the bass was muddy, not thin (+7 … +17 dB at 250–500 Hz); the *fortissimo* was
  flattened by the output ceiling, not the body (crest 17.6 → 26.8 dB against the Salamander's
  25.4); seven octave register bands, the damped floor (the lowest strings' stalled loop mode),
  the dampers' hold.
- **c3–c5** (after the analysis): the unison's strings meet the hammer unevenly and a second
  polarisation loop for one- and two-string keys (the aftersound); a per-key arrival-time
  difference between the microphones; a bass formant at ~600 Hz; model 2's own keyboard
  (inharmonicity fitted to the Salamander's measured B; at 1, 1, 1 it is model 1's to the bit);
  the keyboard evened as a voicer evens it (4.0 → 2.6 dB rms key to key; the Salamander 1.7).
- **c6–c7**: each string of a unison its own stiffness, losses and bridge resonances (the
  "flanger" on A1–F#3 gone); the top octave's hammer and decay — the octave the owner later
  preferred to the Salamander's.
- **c8** (the owner's complaints traced first): *out of tune* = a stiffness scatter whose
  fractional allpass was clamped at ±0.99, leaving **D3 −46 cents, A#1 −49** (the last soft
  chord's fifths beat 6.6 Hz against 0.7) — fixed so no string leaves its pitch; *the vibration
  at the start of the ff chord* = C1's partials scattered six times wider in the bass, beating
  against C3 and G2 at 3.7 and 9.1 Hz; *nasal* = partials 8–9 from C3 to D#5 +9 dB.
- **c9**: tuning in the acceptance suite (a gate at 10 cents per key against the reference,
  P20 in `accept3`; a DSP test that strikes every key A0–C8 and fails past 3 cents); the bass
  stretch the gate found; the pedalled phrases' release on the reference's.
- **c10**: the key's own damper apart from the pedal's; middle C's partials 8–9 move with the
  register's loss law.
- **c11**: **the "nasal" middle was the strike point misplaced by the model** — placed at the
  key's fraction of the integer delay lines alone, it notched partials 12–13 across C2–C5 (the
  strike at 0.08 of the string, not 0.12); placed true, every partial 1–13 of C3–A4 within ±4 dB
  of the reference's; the soft hammer faster in the middle only; `metric2.py` floors every band
  at the threshold of hearing (the reference's 31–125 Hz in middle C is its recording's floor).
- **c12**: the middle's unison detune (C4's beats 12.1 → 9.2 dB against 8.7).
- **c13** (on `r40/chorda`): each key's own draw (`draw.<key>`) — seven keys whose unison landed
  badly (flanger over 0.2) re-voiced alone, none left; the release noise at the reference's.
- **c14**: middle C's own loss (`hf_key.<key>`: partial 9 at 1.5 s −51 → −58 dB against −61;
  its phrase's 2.26 kHz +5.3 → +1.6 dB); the fast run's 2–2.5 kHz dip traced to candidate 6's
  partial table (off in C4–B4, half in C5–B5). Bar (a), (b), (d) met; (c) not on three measures
  (two-stage beats 0.63 against c7's 0.55, C4's beats, the upper colour).
- **c15**: the bass grid's draws (two-stage beats 0.56, about even with c7), C4's draw (beats
  8.4 against 8.7), C5–B5 by their own loss (fast run −3.7 dB) — but the single notes' upper
  colour worse (1.97 against c10's 1.49): that measure and the fast run ask opposite things of
  C5–B5. Held for the owner's ear on c14.
- **c16**: C5–B5's partial 2 cut again (the held note won over the staccato run: upper colour
  1.97 → 1.48, fast run presence −3.7 → −4.6 dB), the bass's decay law (two-stage beats 0.56 →
  0.50), B1 and F#2 redrawn, **D3's stretched partials since c13 mended** (the soft chords' last
  D3–A3 fifth 4.46 → 1.18 Hz against the reference's 0.68 — the chord the owner called out of
  tune, still in c14), more presence in the bass. **The whole bar met in measures**: (a) every
  key within 10 cents (worst C1 +9.7); (b) no complaint measurable — middle C's 2.26 kHz +0.3 dB,
  no key A0–C5 flanged over 0.2, the top octave unchanged; (c) two-stage beats 0.50, C4's beats
  8.4 dB (8.7), upper colour 1.48; (d) first analysis and `accept3` under c7's. Its costs: the
  fast run's presence −4.6 dB, the ff chord's +2.6 dB. No engine change (`voicing\NEXT-c16.md`).

| Candidate | First analysis | `accept3` (with P20) | Round 38 metric (audible) | Tuning gate (worst key) |
|---|---:|---:|---:|---|
| c5 | 5.93 | 15.07 | 4.41 | — |
| c6 | 7.66 | 12.53 | 5.10 | — |
| **c7** (blind test) | 7.26 | 11.71 (16.27) | 5.10 | fail (D3 −44 c) |
| c8 | 7.42 | 11.65 (15.66) | 5.02 | fail (C#1 +16.9 c) |
| c9 | 7.39 | 11.59 (12.49) | 5.10 | ok (F#2 +9.5) |
| c10 | 6.98 | 11.23 (12.13) | 5.12 (4.55) | ok |
| c11 | 6.87 | 10.85 (11.75) | 4.93 (4.42) | ok (C#1 +9.7) |
| c12 | 6.95 | 10.89 (11.79) | 4.87 (4.36) | ok |
| c13 | 7.08 | 10.85 (11.75) | 4.79 (4.34) | ok |
| **c14** (the ABX out) | 6.95 | 11.07 (11.97) | **4.54 (4.01)** | ok (C#1 +9.5) |
| c15 | 6.95 | 11.26 | 4.60 (4.08) | ok (C#1 +9.4) |
| **c16** (the ABX now) | 6.94 | 10.96 | **4.49 (3.96)** | ok (C1 +9.7); bar (a)–(d) met |

Lower is closer in every column; the Salamander scores 0. The first analysis went *up* from c5
to c6 (treble, stereo, space) while the second analysis's items fell; `accept3` adds both. The
round 38 metric's target is ≤ 3.7; it is moving (5.12 at c10 → 4.49 at c16) but not there.

**The owner's blind test of candidate 7** (kit 5, headphones; he plays some piano and works in
audio): **ABX candidate 7 against the Salamander 16/16**; Chorda 2 (beta.14) against candidate
7 15/16; both catch trials answered "same" — a reliable listener. Preference: the Salamander in
six of seven phrases (the bass took him 28 plays — close); **candidate 7 in the top octave**
(*"golpes más naturales y bonitos"*). Candidate 7 against Chorda 2: 3–3 and a tie. His words on
candidate 7: *out of tune in places — the last chord* (soft chords, twice), *something odd in
the bass*, *more nasal* (middle C), *an odd frequency in the mids*, the ff chord *"something
vibrating… at the start"* yet *a richer tone*. Candidates 8–11 traced and fixed each of them.

**Kit 5, the listening kit for strangers (`abx5`).** Kit 4's file names leaked the blinding (a
phrase's three files shared 15 of 16 hex digits); kit 5's are keyed-PRF names and the page
knows each piano by a random token. Written for listeners (no rounds, no candidates, no names
until the reveal), Spanish and English, two parts (an ABX of 16 and 16 blind preferences with
two catch questions), the listener's form (playback, piano, audio work, age range), answers
kept across a reload, results online (each listener's own document; only the owner reads the
collection and gets the pooled summary) or as a JSON download. The transport rebuilt on Web
Audio (the loop button the owner found dead in earlier kits only never redrew); **410 checks
in Edge, Chrome and Firefox, over http and file://, 0 failed**.

**Status.** The owner asked *"al 14"* and candidate 14's ABX was published
(https://claude.ai/artifact/Pygc9sEG6bN1JQrbAzUxiS, kit `abx5-fff02f7322`); it carried D3's
stretched partials, the out-of-tune last chord of his complaint. **Candidate 16**, made by
measures (the owner approved it 2026-11-14), meets the whole bar in measures, so **the owner's
blind test is now candidate 16**: https://claude.ai/artifact/Jpwa91Tz4V8rbcRrgiXMKg (an offline
FLAC zip `compartir\cantus-chorda-abx16.zip` for listeners outside the organisation; the
preview `compartir\chorda-preview-16\`). The c14 page stays up, unused. `r40/chorda` is merged
(`927fd92a`: the `draw.<key>` and `hf_key.<key>` knobs, both neutral by default — no sample
moves; `DSP_VERSION` 6, the pinned beta.14 hash holds). Shipping a candidate as the default is
one commit: `DSP_VERSION` 6 → 7 with the goldens and the measured size of the change
(`CANTUS_GOLDEN_DUMP`), the 24 presets re-trimmed, ADR 0098's third amendment.

## Tympanum: the trail of the generated drums (`tymp-engine.md`, `ladder.md`, `drums-hyb.md`)

Every listen is a folder in `F:\projects\daw-r40\compartir\` with a `NOTA.md` in Spanish: the
model, the recording and the previous listen at one loudness, piece by piece and in a groove.
Everything is measured through the engine against the recorded kits (DRSKit for the studio
kit, Virtuosity for the jazz kit) with `tools/tympanum-fit/`.

| Listen | What it played | The owner |
|---|---|---|
| Checkpoint B (`tympanum-b`) | Gogodze through its seven mics; the modelled studio kit on Pulsus's modal voices (model 3) | *"Se escucha como batería; aún no diría que se escucha real, pero sí suena a batería — síguela mejorando."* |
| b2 | **Model 4, acoustic**: modes driven by the strike's acceleration, the stick's noise, tension glide, two heads, snare wires gated by the snare-side head, a cymbal's wash building | *"Sigue mejorándolo"* |
| b3 | b2 plus the player's figures (flam, drag, buzz, held roll) | *"suena súper feo, los platillos suenan como vidriosos, tiene muchísimo para mejorar"* — an independent drum analysis and a synthesis research note followed; a fresh builder |
| b4 | **Model 5, measured**: crash and hi-hat from DRSKit's hits — up to 256 modes, 32 band envelopes, the stick, per state and velocity | — |
| b5 | Every cymbal on model 5; the overheads in phase | — |
| b6 | The drums on model 5 (kick, snare, toms; bands to 35 Hz, the pitch glide per layer); the tables as a blob (`CMT1`); the CC0 jazz kit | The default kit is his to pick |
| b7 | The ride's bow per velocity; the virtual mics' paths 2.3× cheaper | *"va mejor pero aún está lejísimos"* |
| b8 | `NEXT-drums.md`: the honest handover of what model 5 cannot reach | *"Hybrid como opción, pero generated debe mejorar mucho más para que sea algo muy bueno y deseable de usar"* → `drums-gen` and `drums-hyb` |
| b9 | `drums-gen` 1: the groove's fine highs (6–16 kHz components per kHz 15 → 33, DRSKit 31), coherent drum onsets (the kick's onset correlation −0.12 → +0.93), the head's beam, the ride refitted with 440 partials | *"nos falta un montón"* — a bit more real, a whole lot still missing → the fifteen-cue analysis and the ladder |
| Ladder (`tympanum-escalera`) | Each recorded hit resynthesised through richer descriptions, blind MUSHRA + ABX | Below |
| **b10** | `drums-gen` 2: the fifteen cues | **Waiting for his ear** |
| **b11** (`tympanum-b11`) | `drums-gen` 3: the cost 18.7 → 7.3 % of a core, sound unchanged; the bands' skirts (a tom's low bands apart, a cymbal's high-passed), the studio kick's pitch, the studio snare's wires; **71 of 162** checks (b9 21, b10 57), none that passed fails | **Waiting for his ear** |

**The fifteen cues** (`analysis-drums-2/report.md`, an independent analyst): the most synthetic
cues ranked with numeric targets, each now a check per kit (`accept_b10.py`). The b9 render
passed **21 of 162**; b10 passes **57**. Found and fixed for b10: **the late, smeared attack
and the ~3 ms comb** — `head_beam` cut every mic off a head's axis with the far-field law, so
the snare's own mic heard its head through a 1.35 kHz low-pass and the overheads carried the
crack 3.5 ms later (near-field rule; a drum's highs drawn twice so close and far mics carry
uncorrelated highs; the studio mix's overheads −8 dB): the snare's strongest 2–16 kHz arrival
4.5 → 0.9 ms (DRSKit 1.1); **cymbal tails turned to hiss** — a band's noise capped by the
recording's own residual (the ride's 4–12 kHz flatness 0.05 → 0.02, DRSKit 0.01); the snare's
and toms' early noise under the fundamental cut; the kick's partials read after its pitch
falls; each layer's stick burst levelled through the engine. **Still open, measured**: the
toms' sub-sonic thump, the studio kick reading 70 Hz (76), the studio snare's wires 2–9 dB
weak, the first 50 ms crests, the groove's mid-band correlation (0.88, DRSKit 0.67), the jazz
ride's ping lines, the toms' comb.

**The representation ladder and the owner's MUSHRA** (`ladder`, 2026-11-13, headphones; one
listener, 0–100): model 5 calibrated parameters inside one fixed representation for nine
listens — can it reach the recording at all? Each recorded hit was resynthesised through richer
descriptions (an oracle: no fitting error, no engine, no mics) — R1 model 5's own, R2 plus the
hit's minimum-phase attack, R3 a 1/12-octave STFT noise table, R3b the same per 11.7-Hz bin, R3o
the oracle, R4 texture statistics (co-modulation), R5 the anchor.

| item | R1 | R2 | R3 | R3b | R3o | R4 |
|---|---:|---:|---:|---:|---:|---:|
| snare | 24 | **64** | 25 | 50 | 39 | 34 |
| kick | 71 | 63 | **100** | 100 | 100 | 100 |
| floor tom | 70 | **80** | 45 | 65 | 100 | 45 |
| closed hat | 60 | 67 | 3 | **100** | 75 | 21 |
| open hat | 45 | 23 | 13 | 38 | 50 | 8 |
| crash | **82** | 30 | 25 | 73 | 100 | 31 |
| ride | 70 | 25 | 60 | 40 | 30 | 30 |
| **mean** | 60 | 50 | 39 | **67** | 71 | 38 |

What it says: **R3b is the storable layer for hats and cymbals** (67, next to the oracle's 71;
the closed hat 100); **the kick is solved** by any fine noise table (100); **the snare and the
floor tom gain from the attack** (R2: 24 → 64, 70 → 80) and lose with an STFT layer (it smears
the wires and the head's decay); **co-modulation (R4) is the worst** (38) — the recorded cymbals
do not co-modulate; **model 5's crash rated 82** — above the hidden recording's 44 (the hidden
reference was missed on the crash and the ride, so those two items are weak). ABX against the
recording: everything heard (snare R3 and R4 16/16; crash R3 14/16, R4 16/16, R3b 15/16).

**The "iuuu".** He heard *"un sonido apreciable como 'iuuu iuuu iiuu' que se escucha casi al
final"* of the cymbals. Traced (`ladder_whistle.py`): not a gliding tone but **a tail that swells
in its last second** — the measuring render held an event ~3 s after the open hat's loudest
strike (the pedal closing on its ring), and the fits' widening late windows pulled it back as a
swell; **40 of 128 layers** of model 5 rose 2.2–3.2 s after the strike (the open hat's lows by up
to +22 dB). Fixed in `drums-gen` 2: clean-hit fits, **monotone tails** (`tables_blob.py`'s
`no_swell`, a test that no band swells after its peak), the render's pedal moved before a stroke.

**Model 6, the plan** (for round 41): the kick on R3's fine table; the snare, rimshot, sidestick
and toms keep model 5's 38 band filters (time-fine) with monotone tails and take **R2's five
minimum-phase attack grains** per piece, zone and velocity (next: a time-resolved attack above
1.5 kHz and a wire-rattle event stream); the hats and cymbals take **R3b's per-bin layer**
beside their modes (the modes stay — cutting them fails); co-modulation dropped. Estimated
**≈ 6.5–7 % of a core** for the model alone (model 5 at 7.9 % on a quiet machine), tables
0.7–3 MB a kit.

**The cost.** Measured at the end of `drums-gen` 2 on a machine with four other compiles:
model 5 alone **10.8 %** of a core (the ride 3.4, the crash 2.8, the hat 1.6, each drum
0.7–1.0); the full kit with every mic **18.6 %** — still over the ~10 % budget. `drums-gen3`
brought it down with the sound unchanged (`full_kit_dump` before and after: at most −96.1 dB
studio, −103.3 dB jazz re peak): **the full kit with every mic 7.3 %, model 5 alone 6.0 %**
(release, best of five, the machine shared with other builders, about a fifth of its threads
busy). The hot loops each in their own function (thin LTO had left them scalar when inlined),
the modes' sums over a list of live blocks, the drum's glide by a short series, the paths'
rings mirrored. **Still failing, measured**: the groove's mid-band L/R correlation (0.88 / 0.86
against 0.67 / 0.38), the toms' sub at high velocity, the first-50-ms crests (2 of 16), the
jazz groove's highs (−7 dB at 4–8 kHz) and lows (+10 dB sub), the jazz kick (its click, its
1–4 kHz T60), the ride. The ~3 ms comb is not an echo: it sits at −80 … −130 dB in the far
mics' highs, under the recordings' own noise floor.

**The hybrid** (`drums-hyb`, `hyb-refit`): fitted over 15 articulations × 5 layers × 4 takes.
Measured distance from the recording (mean |dB| over third octaves, 13 articulations × 4
velocities), version 2 on the merged model: jazz 0–50 ms **4.08 → 3.53 dB**, 50–200 ms 4.01 →
3.22; studio 4.29 → 3.46 and 4.17 → 3.13 — the closest yet in both windows for both kits; the
generated model alone came 0.5–0.7 dB closer than at version 1. The player costs 0.03 % of a
core; the residual 5.74 MB a kit for the session. It is mono and ends at 200 ms (after that it
is the model). Listens: `compartir\tympanum-hibrido-1\` (version 1) and
`tmp\hyb\v2\listen-{jazz,studio}\` (generated · hybrid · recording). `drums-gen` 3 made
version 3 of the packs for its model (tables `0x582AA7B6`; listens in
`tmp\hyb\v3\listen-{jazz,studio}\`): `tympanum-hybrid-jazz-3.zip` (5 176 330 bytes, SHA-256
`dc4536d6…`) and `tympanum-hybrid-studio-3.zip` (5 241 324, `1b7cd0dd…`), **uploaded to
`content-r40` with the owner's OK** (2026-11-14) and the public download verified; the
manifest's two entries are at version 3, and the ignored networked
`the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model` passes against them
on the final tree.

## The owner's decisions and words, in order

- **2026-11-11** — round 39 on his screen: *"no encontré nada malo nuevo"* (round 39's look
  accepted). Slate Lapis room **C**. The dancer: Compás *approved as is*; Síncopa and Tono "more
  cool"; more characters, a family with Compás. Chorda: *"lo más parecido posible al
  Salamander"*; he listens before a default ships.
- **2026-11-12** — the cast: all twelve (Onda the oscilloscope, the ribbon rejected; Metrónomo
  stays); a neon character inspired by a genre, original. Slate Paper "menos azul" compared —
  **today's kept**. Deleting the worktrees of rounds 26–39. Phase 3: recorded pianos, the
  instrument mixer, Tympanum (the name; modelled mics move; Grooves later, BACKLOG §5; a
  recorded kit in its own room by default; maple; the picture *"muy muy bonito"*). **The pack
  policy, three modes** (ADR 0042). Chorda 1 stays as *previous model*; the Salamander the
  default recorded piano until his ABX. Photoreal pictures, then the reversal to the designer's
  vector render (*"vamos frescos con el diseño mejor"*), then **configurable pictures**
  (*Drawn · Photographic*), *Photographic* *"casi excelente"* — and the correction: **Drawn
  stays the default** (memory) and looks as close as it can; the bar down the grand's tail
  *"se ve muy chido"*, kept. Zone tables sized per instrument; FLAC at install; Piano in 162 and
  Pianobook *Manual*; Headroom dropped.
- **2026-11-13** — Tympanum b3 *"suena súper feo, los platillos suenan como vidriosos"*; b7 *"va
  mejor pero aún está lejísimos"*; after b8 the hybrid **as an option**, generated first; the
  hybrid's packs uploaded (*yes*); the ABX handed to other listeners only from candidate 7;
  the blind test of candidate 7 (above); *"cuando tengas una versión que ya creas buena dame el
  ABX"*; **microtuning** for professionals, as an advanced option (a design note for round 41);
  after b9 *"nos falta un montón"*; the ladder's MUSHRA; checkpoint 5's pianos *"¡Van
  increíbles los pianos, excelente!!!"*; the ABX *"al 14"*.
- **2026-11-14** — the hybrid's packs version 2: *"Súbelo"*. He cannot listen for some days and
  approved three things meanwhile: these close-out documents, `drums-gen3` (the cost under
  budget and measurable cues, the eleventh listen, hybrid version 3 packs prepared, not
  uploaded) and Chorda's candidate 16 by measures. **No master and no beta.15 without his ear
  and his OK.** Later the same day: the hybrid's packs version 3 uploaded with his OK.

## Checks

The final tree, `2e3982a1` (every branch merged, `r40/drums-gen` at `4e622739` and
`r40/chorda` at `927fd92a` last):

- **Windows**, `cargo test --workspace -j 1 --no-fail-fast`: **7 175 passed**, 124 ignored,
  **0 failed** (the engine fuzz with it). Model 2's pinned beta.14 hash
  (`chorda::tests_voicing3::model_2_at_beta14s_voicing_renders_to_the_bits_it_did`) passed:
  `DSP_VERSION` 6 holds. Clippy `--workspace --all-targets -D warnings` clean. Fmt clean, run
  per crate (`cargo fmt -p`) and for `cantus-ui`, the test plug-ins and `xtask` by file with
  `rustfmt --check` (`cargo fmt --all` fails on Windows with os error 206, a command line too
  long).
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`): **7 134 passed**, 124 ignored, **0
  failed**; clippy (and the plug-ins without their editor) and `cargo fmt --all -- --check`
  clean.
- `cantus-sandbox --test session a_hang_never_blocks_the_interfaces_frames_and_is_ended`, which
  failed only under load in the earlier Linux run (5 of 5 alone), passed in both full runs.
- The ignored, networked
  `cantus-ui --test tympanum_hybrid_e2e the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model`
  passes against the version 3 packs on `content-r40`.
- Earlier in the close-out (`cbc9c23f`, Windows): 7 166 passed; its one failure,
  `THIRD-PARTY-NOTICES.md` stale, was regenerated at `060da9e5`.
- The release timing tests (`frame_budget`, `skins_budget`, the cast's `cast_bench`) and the
  scorecard were **not** measured on a quiet machine this round (agents ran throughout).

## What was verified only in emulation

Every picture was drawn by the CPU renderer and every gesture was a synthetic event through
`CantusApp::assemble`; no sound was judged by an agent's ear. The owner's steps are in
`docs/beta-checklist.md` (*Ronda 40*):

- The look answers, the bands on the desk, Tympanum's and Chorda's views, the cast and the
  ripple's gestures on a real screen with a real mouse.
- **Photographic on a real GPU and screen** (100 % and 150 %): the bake's time and peak were
  measured in an example process on a busy machine (Chorda full 0.99–1.7 s, peak 66 MB at
  1.5×); audio at 64 frames during bakes showed no added misses there.
- Cantus Delay was driven through its CLAP entry and the VST3 tests' host, never loaded in
  REAPER, FL Studio or Bitwig; the synced time through a ramp was measured, not heard.
- *Hear the key* and an *After FX* take through a real interface in duplex.
- The pack modes' download rules (*From the author's site*, *Manual*) only against a loopback
  server; the hybrid's packs downloaded for real through the public URL by a test; **the big
  packs** (bitKlavier's images, DRSKit, CrocellKit, MuldjordKit) were downloaded on the
  builders' machine, never through the app's own window by a user.
- A user's DrumGizmo kit from a file: the published XML files with hits written by the tests.
- Tympanum and recorded Chorda streamed only in offline renders, never under a live device at a
  low buffer.
- The memory map on one laptop (AMD Radeon 680M, its OpenGL driver); NVIDIA's, Intel's, Mesa
  and macOS were not mapped.

## What is left

**The owner's ear (first):**
- Chorda: **candidate 16's ABX** (https://claude.ai/artifact/Jpwa91Tz4V8rbcRrgiXMKg, the
  offline `compartir\cantus-chorda-abx16.zip`; it meets the bar in measures, nobody has heard
  it) and the listeners he hands it to; then which candidate ships as the default. The c14
  page stays up, unused.
- Tympanum: **b10** and **b11** (`compartir\tympanum-b11\`: drums-gen 3, 71 of 162 checks, the
  cost 7.3 %); the default kit (Studio Maple today); the **hybrid** against the generated voice
  (`tympanum-hibrido-1`, version 2's and version 3's listens in `tmp\hyb\v2\`, `tmp\hyb\v3\`;
  version 3's packs are the ones the app downloads).
- Chorda's recorded pianos: the ABX of bitKlavier against the Salamander
  (`compartir\pianos-c\`) — it decides the default recorded piano.

**The owner's decisions:**
- Cantus Delay's ids (forever), before master.
- A pad with its own track: its number in the accent colour, or the sheet's ink (kept).
- The top bar with both extras folds the typing key into *More* at 1920 × 1080: keep it, or
  fold another group first.
- Compás's *Cueing* mood; the cast's blues over a blue chroma key (kept as designed, moved only
  over a key); the ghost strips' true numbers (FL's way, kept).
- Paper's menus and dialogs: as finished as Night?
- Whether a recorded piano's *Room* strip adds one of our rooms by default (none today).
- *Photographic* as the default some day (one constant); Tympanum's default kit (in the picker
  since the ship); a new Chorda's level against its crest (*The ship*).
- The written CC BY grants (Headroom/Nilsson, Ivy Audio, Wilkinson) — drafts for him to send.

**Engineering, for round 41 or before the beta:**
- ~~Tympanum joins `InstrumentKind::ALL`~~ and ~~ship the chosen Chorda candidate~~: done at
  the ship (below). Their blind listening kits and the owner's verdicts are in the BACKLOG.
- Tympanum's picker thumbnail waits for its stage's picture to be baked.
- Chorda's model renders its five outputs apart (`MODEL_RENDERS_OUTPUTS`).
- Tympanum's **model 6** (above; the cost is under budget since drums-gen 3: 7.3 %); the
  measured open cues (drums-gen 3's «Still open»: L/R correlation, the toms' sub, the crests,
  the jazz groove's highs and lows, the jazz kick, the ride).
- A recorded piano of several images wants a larger or FLAC-backed stream cache (ADR 0050).
- An effect need the automation cannot foresee (an EQ's mode lane, Aula's response lane)
  switches at the next block's start (ADR 0119 §4): foresee it as Delay's floor does.
- The memory row and the release timing tests on a quiet machine; the compensation pool's lines
  measured; the OpenGL driver's ~105 MB floor moves only with another way of presenting (BACKLOG
  §4b).
- **Microtuning** (the owner's request, 2026-11-13; BACKLOG §0.2): one project tuning every
  pitched instrument follows, Scala `.scl`/`.kbm` and MTS-ESP; Chorda's diapason, temperaments
  (rotated to any key, presets by key), stretch, unison width and a per-key editor with a strobe
  tuner; Tympanum's tuning per piece in Hz and note, the two heads' tension, the toms tuned to
  the song's key — about one builder-round and a half.
- The pictures' small leftovers (`visual.md`, `scene-px.md`): faint lathe rings on photographic
  cymbals at 1.5×, a speckle on the floor toms, sparkle on chrome over a patterned rug.
- ADRs 0123–0126 marked accepted with the owner's OK.
- At the release: the site's manual pages (`web/assets/manual/`, `build-manual.ps1`, committed by
  hand) — round 40's new pages (`arrangement`, `chorda-pianos`, `tympanum`) are not there yet;
  the release notes; the README's and the site's cards for Tympanum (once it is in the picker),
  Cantus Delay as a plug-in and the recorded pianos.

## The ship (2026-11-14): candidate 16 the default, Tympanum in the picker

The owner: *"Add Tympanum at once. Ship to master with Chorda's newest version (if it's the
best) — but keep in the backlog listening and doing the ABX evaluation, the same for
Tympanum."* Built on `r40/integration` after the close-out (`014d3928`); master and the tag
stay the integrator's, on his word.

**Chorda: candidate 16 ships** (`DSP_VERSION` 6 → 7; ADR 0098's third amendment;
`chorda.md`, *Candidate 16 ships as the default*). `Voicing::default()` is the lab's spec
and tables, verbatim (`chorda_voicing_c16.rs`), over beta.14's voicing, which is now named
(`Voicing::beta14()`, its pin `0x663b_f285_d17a_ebdd` held); the board and room the designer
makes ahead are candidate 16's. Shipping it found three things only its parts show, each
fixed without moving its default sound (its new pin `0x717d_64b7_22b9_e71b` held across
them): *Width* re-levelled the outer keys through the evening (weighed now at the voiced
perspective); a model switched to 1 left model 2's roll-off and the sympathetic high-pass
frozen (cleared); blocks of 1 and 7 differed from 512 at −115 dBFS where a voice ended
inside a piece with the arrival-time difference on (its later side dropped at its end in
every piece; a long random phrase holds it at blocks of 1, 7, 64). No golden hash moved
(none renders model 2); old songs on model 2 play candidate 16, model 1 to the bit. The 24
presets were re-trimmed by `gain` (they sat 1.0–7.1 LU under their role's target). The
seven phrases: the new build's lab equals the lab exe of the owner's kit to the bit or under
−85.7 dBFS; the engine with no environment variable plays the same instrument (one-note
phrase correlation 0.999994; strikes of several notes draw their variation from the clip's
note ids). **Open, the owner's**: his blind ABX (BACKLOG), and a new Chorda's level, 10.4 LU
under the family's reference on the staging triad (listed as pending in
`instrument_reference.rs`): raising it squeezes the fortissimo into the ceiling.

**Tympanum: in the picker and in `InstrumentKind::ALL`** (ADR 0124's fifth amendment).
`READY` true; the device picker's *Drums* shelf beside Pulsus (`picker.dev.tympanum.*`, both
languages; shots `compartir\p3\ship\picker-drums-*.png`, the views
`tympanum-{kit,mixer}-{night,paper}-{100,150}[-es].png`); the agents' `INSTRUMENTS`; the
bench and the null corpus take it from `ALL`. Every kind-wide test found what it lacked:
the contract twins (the live hits are forgotten at rest — as good as new), the listening
renders' checkpoints (a modelled kit forks), *Learn MIDI controller* on its Kit knobs, bars
and switches and its Matrix cells (`controllers::stepped_inst`), the room's surface names in
both languages, a drummer staged on a beat (−22.8 LUFS, inside the family's window). Its
cymbals ring as real ones do (a ride to its −100 dB floor some 37 s after a hit), so the
contract holds it to exact silence 45 s after its notes and rest within 60 s (the heavy
chord rests after 43 s). E2E: `tympanum_pick_e2e` (listed in both languages, chosen through
`assemble`, a groove plays through the real engine wiring). **Open, the owner's**: its
blind listening kit and his verdict (BACKLOG); the picker's thumbnail of Tympanum shows its
stage before the picture is baked (an empty room behind the cards).

**Checks of the ship** (`601e244f`): Windows `cargo test --workspace -j 1 --no-fail-fast`
found six, the same six Linux found — the MCP tools list (`tympanum` added, re-written with
`CANTUS_TOOLS_DUMP=1`), the kit list's buttons without a help sentence, the stepped controls'
*Modulate…* unseen by the probe, three Chorda tests designing model 1's keyboard for a new
Chorda, and the demo and preset floors (Tympanum listed by name until the owner's verdict) —
fixed in `601e244f` and those targets run again: **Windows 7 183 passed**, 124 ignored, **0
failed**. Clippy `-D warnings` clean on Windows and Linux; fmt clean (by file on Windows,
`cargo fmt --all -- --check` in WSL). **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh test` on `601e244f`): **7 142
passed**, 124 ignored, **0 failed**.

## Documents of the close-out

This account; `docs/STATUS.md` (the header, «The next session: what round 40 left», the round's
section); `docs/next-session.md` (round 41's prompt); `docs/beta-checklist.md` (*Ronda 40 — lo que
espera tu oído…* and the owner's round-40 decisions; round 39's look recorded as accepted); the
manual in both languages — Tympanum's own page (`docs/manual/{en,es}/tympanum.md`), *Instrument
pictures* in the workspace chapter, *Bands on the desk* in the mixer chapter, Chorda's Play view
and a microphone's *Delay* cell; BACKLOG §0.2 (microtuning); `docs/claude-memory/` copied back.
The last two builders' numbers (candidate 16, drums-gen 3) and the final suite's counts were
filled in after their merges.
