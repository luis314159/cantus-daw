# ADR 0125 — Chorda's recorded source, its mixer, and the three pack modes

**Status:** Proposed for round 40 phase 3 (contract; builder `chorda-src`) · **Date:**
2026-11-12 · **Schema:** 68 · **Builds on:** ADR 0023, ADR 0030, ADR 0042 (amended here,
§4), ADR 0050, ADR 0059 and 0098 (Chorda's models), ADR 0104 (Chorda's stored look), ADR 0123, ADR 0126 (the scenes)
(named outputs) · **Research:** `F:\projects\daw-r40\research-p3\` (`packs.md` pianos,
`formats.md` §2, §6)

## Context

The owner (2026-11-12): Chorda takes **recorded piano packs**, free or bought, *following
the standard* (SFZ, Decent Sampler, SF2, the Salamander/bitKlavier naming), with a curated
list of good free ones — while the **modelled** piano stays the goal (the `chorda` builder
voices model 2 toward the Salamander and owns `cantus_dsp::chorda*` and the engine's
`inst::chorda`). And Chorda's internal mixer, with outputs to strips. His later answers the
same day: a recorded pack shows the matching piano type in Chorda's picture (now drawn by
the scene module, ADR 0126); **Chorda 1 stays in the list as "previous model"**; **the default recorded piano is
the Salamander** until his ABX of bitKlavier; movable or added mics on the **modelled**
piano are **later** (BACKLOG) — its mixer has *Hammers, Player, Audience, Room,
Mechanics*; and packs come in **three modes** (§4).

## Decision

### 1. The source is a text key

`cantus_core::inst::chorda_source`: `SOURCE_KEY` = `source`; absent, Chorda plays its
model (every older song); `pack:<pack>/<instrument>` a recorded piano (`ChordaSource::
{Model, Recorded(RecordedPiano)}`). The model's version stays `model` (ADR 0059), so the
source list (`SourceChoice`) is: **Chorda 2** (the model), **Chorda 1 — previous model**
(`model` = 1), then every installed recorded piano, the Salamander first
(`DEFAULT_RECORDED`).

### 2. Through Exemplar's engine, not a copy

A recorded source keeps the pack's zones **in Chorda's own spec**, Exemplar's key space
under `rec.` (`rec.z12.lo`, `rec.z12.sample`, `rec.g0.gain`): a pack's update never changes
a song (ADR 0059's rule), the model's keys never collide, and the assets load by the path
Exemplar's do (`is_asset_key`). `chorda_source::snapshot_spec`, called once in
`Snapshot::compile`, turns such a Chorda — **in the snapshot only** — into the Exemplar
spec of its zones, its outputs carried; the engine plays it with **Exemplar's own unit**
(streaming, round robin, release triggers, its buses). The project still says *Chorda*: its
editor, presets, the agents and the switch back see one instrument. A switch makes the
engine build the other unit (units are keyed by kind), which is what a source change is.

Rejected: a sampler inside Chorda's engine unit (it would put `chorda-src` in the `chorda`
builder's file, and copy Exemplar's voice into Chorda); reading the pack's files when the
snapshot compiles (I/O in a pure function, and a song that changes with the pack).

### 3. The mixer and the switch

- **Perspectives and noises are named outputs** (ADR 0123): a recorded pack's mics (the
  Salamander 1, bitKlavier 9, the Estate Grand 3) and its noises where the source has them
  (hammer and release, pedal, keys, release resonance); for the **model**, the five fixed
  outputs `MODEL_OUTPUTS` — *Hammers, Player, Audience, Room, Mechanics* — which the `chorda`
  builder renders as model-2 buses after its voicing pass, null at their defaults. Each
  perspective: level, pan, width to mono, delay, polarity, mute/solo, purge (`o{n}.*`);
  each its own strip in one click, summing to the stereo mix to the bit.
- **The switch** model ↔ recorded (`chorda_source::switch`) is **one undo step**: the source
  key, the recorded zones written or cleared, the outputs carried by name then family (a
  perspective's strip kept), the automation of keys the new source still has kept; notes
  never touched.
- **Installing from a file** a bought SFZ, Decent Sampler (buses and `AUX_STEREO_OUTPUT_n`)
  or SF2 piano (one perspective — SF2 has none) makes a user pack the source list offers;
  the Salamander/bitKlavier naming becomes an SFZ at install (`PianoBuild`, manifest
  format 5).
- **The picture** is the scene module's piano (ADR 0126, builder `scene`, which also
  switches Chorda's view to it): a recorded pack shows the matching piano type — concert
  grand for bitKlavier's Steinway D, grand for the Salamander's C5, upright for Upright
  Piano KW or Old Piano — and no brand mark. `chorda-src` says which type a pack is
  (`chorda_source::scene_piano`); `scene` draws it.

### 4. ADR 0042 amended: three pack modes

`cantus_content::manifest::PackMode`, manifest **format 5**:

1. **Pack** — CC0, CC BY 3.0/4.0: one click, as the Salamander today: downloaded from the
   official source URL, SHA-256 pinned, the credit shown. **Only for an accepted licence
   id** — the manifest's validation refuses anything else (a test holds it).
2. **From the author's site** (`author_site`) — free to download but no licence stated
   (Piano in 162, Headroom Piano, Naked Drums): one click **directly from the author's
   official URL** — Cantus never hosts, mirrors or bundles it — SHA-256 pinned; the pack's
   page says *downloaded from <author>'s site* and shows the author's terms
   (`author_page`). If the file or the URL changed (hash mismatch, 404, HTML instead of an
   archive) Cantus opens the author's page and offers *Install from a file…*. Licence:
   `LicenseRef-author-terms` (`AUTHOR_TERMS`) or an accepted id.
3. **Manual** — the source needs a login, a registration or an e-mail (Pianobook): Cantus
   opens the author's page; the user downloads it and drops the file on Cantus or uses
   *Install from a file…*, which recognises it by the source's pinned size and SHA-256.
   Cantus never fetches the source URL itself.

Both 2 and 3 require `author_page` (https). A manifest older than format 5 cannot say any of
it. Never bypass the proxy or TLS; a Zscaler quarantine answers HTML for ~10 minutes — a
fetch checks magic bytes and says so.

## Consequences

- Schema **68**: the source key and the `rec.` keys; the migration writes nothing.
- Chorda now accepts `o{n}.*` (ADR 0123) and `rec.*` keys; neither is listed in `params()`,
  so its knobs, its CLAP/VST3 export and its automation menus are as before.
- The curated list (packs.md): Upright Piano KW (CC0) and bitKlavier Grand (CC BY 4.0, an
  install step writing its SFZ) as *Pack*; Accurate-Salamander only if its licence file
  allows; the Salamander already ships; Piano in 162 and Headroom Piano as *From the
  author's site*; Pianobook's as *Manual*.

### 5. As built (`chorda-src`, round 40 phase 3)

- **Exemplar's outputs.** A group plays on an output (`g{m}.out`, set, never automated);
  Exemplar renders by output only when its groups name two or more, a lane's mix is not
  unity or an output plays on a strip of its own — every other instrument renders exactly as
  before. Decent Sampler's `AUX_STEREO_OUTPUT_n` (directly or through `BUS_k`) is the group's
  output; a group heard on the main output stays there.
- **Zones sized to the instrument** (the coordinator's decision): Exemplar's keys reach
  `z8191`; a snapshot stamps an Exemplar that needs more than 1024 zones with the next power of
  two (`zcap`), and the engine builds a unit that size off the audio thread, matched by
  `UnitKind::Zones` — so a small instrument pays nothing, and several bitKlavier images (662
  zones each) or Accurate-Salamander's per-key map play at once.
- **A set of packs.** A piano that comes as one pack per microphone image (bitKlavier) is one
  entry of the list (`PianoBuild::set`); each image keeps its output across packs
  (`PianoPerspective::output`: House ORTF 0 … Binaural 8; the hammers and pedal on *Releases*,
  16). Images that are off (`o{n}.on` = 0) load nothing; turning one on reads the piano again
  in one undo step.
- **FLAC at install.** A piano build keeps every integer-PCM WAV as FLAC, decoded again and
  compared sample for sample before the WAV goes (`pianobuild_flac`); `PianoBuild::unpacked_size`
  is the unpack's budget, `installed_size` what stays.
- **Modes in practice.** *Pack*: the Salamander, Accurate-Salamander (its minimum edition: the
  others gate releases and resonance behind controllers Exemplar does not read), bitKlavier's
  nine images, Upright Piano KW, Old Piano FB. *Manual*: Piano in 162 (Ivy Audio offers it by
  torrent only) and Pianobook's Helsinki 1967 Pro (a login) — listed with their pages; their
  files come back through *Install from a file…* as user packs. **Headroom is dropped**: the
  author's own download is Kontakt-only, and its SFZ is a third party's port.
- **The Mixer view** is the instrument window's fourth view (`View::Mixer`) for the kinds with
  named outputs (Chorda, Tympanum). Chorda's is drawn on `tymp-ui`'s shared instrument desk
  (`views/inst/inst_mixer*`): the piano in its room beside the selected output's card (a
  set's images on disk, what they take, *Get the other n*), *Outputs* (*All to Main* · *Each
  mic to its own strip* · the mix's preset), then *Piano* pinned left (the whole piano's level
  — `gain`, or the recorded piano's `rec.gain` — and the model's width), the outputs under
  their families' heads, *Room* pinned right (the model's room level as a fader and its size;
  a recorded piano's room is in its far mics: no fader). An image not on disk is a dashed
  placeholder that offers its download; an image on disk loads or unloads from its top cell
  (one undo step).
- **The one-click routes reach Chorda**: `inst_out::definition` names a recorded Chorda's
  outputs (the images that are on, its *Releases*); the model's five join once its renderer
  plays them apart (`chorda_source::MODEL_RENDERS_OUTPUTS`, the `chorda` builder's to flip) —
  until then the row says the model plays them together on Main and the routes leave it there.
- **Per-output meters**: Exemplar's unit reports each output's peak of the last block in its
  telemetry (`tel::OUT_PEAK`: outputs 0–13, *Releases* in the last slot) when it renders by
  output; a strip on the desk shows its strip's meter.
- **Install from a file… is one function** (`chorda_source::install_any`): a pack's own file
  by size and hash installs as that pack; anything else becomes a user pack whose instruments
  play in the asking instrument — Chorda's pianos and Tympanum's kits (*Your own kits*).
- **The output delay (`o{n}.delay`), lines only where asked** (the coordinator's decision):
  when a snapshot's Exemplar has any `o{n}.delay ≠ 0`, `UnitKind::of` asks for a unit with
  exactly those lines (`UnitKind::Exemplar { zones, delays, room }`, `delays` a bit per
  output, `sampler::outs::delay_mask` — the shared rule), built off the audio thread and
  swapped in with the snapshot; a unit with no delay owns none. The lines are `tymp-engine`'s
  `OutDelay` (whole samples, ≤ 50 ms): each output's lane is mixed, then delayed, its tail fed
  silence to the block's end while it rings; the unit is not at rest while a line rings. A
  change in *which* outputs are delayed builds a new unit (as a bigger zone table does); a
  change in a delay's length is a `configure`.
- **Add our room** (a recorded piano's *Room* strip): Exemplar's `room.add` (0 none, 1 a small
  room, 2 a studio, 3 a hall — Tympanum's boxes) and `room.level`; a Chorda's `rec.room.*`.
  The room is `tymp-engine`'s late field (`cantus_dsp::tympanum::room::Room`), fed the mono sum
  of every output, its tail on the main output; built only when asked (the same unit kind),
  so off is null — no room, no memory. Aula's convolution can take its place when it lands.
- **Chorda's Play view as designed** (delivery 6, `chorda_play.rs`): the source list; the
  scene's card (its pickers and room card are `scene`'s); the keyboard under it; *Hammer ·
  Resonance · Output*; *The piano* at the right (lid, where you listen and the mixer's mics,
  *Open the mixer*, the pedals and the dampers' curve). No macro or modulation rows in Play
  or Mixer (Edit keeps them); *Room* and *Width* live in the Mixer, *Dampers* is a pedal row.

## What the tests hold

- Contract: `chorda_source` round trip; `rec.` keys are Exemplar's; the v68 fixture's
  source; the manifest's mode test (`licences.rs`).
- A model Chorda renders to the bit what it did; a recorded Chorda renders what Exemplar
  renders with the same zones, to the bit; the switch is one undo step (undo fuzz); Exemplar's
  default output touches no sample and its split plus buses equal the unsplit at 512/333/64
  frames (`exemplar_outputs.rs`); a 1500-zone instrument gets `Zones(2048)`
  (`exemplar_zones.rs`); Decent Sampler buses from the guide's own examples
  (`ds_outputs.rs`); the bitKlavier naming from its published README listing and a published
  WAV through the FLAC step (`piano_build.rs`); *Install from a file…* (`user_pack.rs`).
- End to end through `CantusApp::assemble` (`chorda_source_e2e.rs`): a piano installed from a
  file plays through Chorda as Exemplar does; the Mixer's *Each mic to its own strip*,
  clicked, makes a strip per output in order and the master stays the same to the bit; undo
  twice brings the model back to the bit; Tympanum's *Install from a file…* takes a kit the
  manifest does not know and plays it.

## Amendment — the instruments read whole at every scale (fx/final, the site's pictures)

**Context.** Shooting the site's pictures found four things in round 40's instrument views:

- at the editor's 200 % Chorda's and Tympanum's views lost most of their words: the
  desk's `mixer_look::say` (which the instruments' `iu::words` paint through) set its line
  in a box of the 100 % height, while its words were set at the style's scale. From about
  165 % the line was taller than the box allowed and `text::paint` dropped it;
- Chorda's Play row ran *Velocity curve*'s knob out of the Hammer card. Under the piano
  alone its eight knobs had about 520 points, and the cells were laid out at a fixed size
  past the card's edge;
- a long name in a list ran under its trailing button: "Accurate-Salamander Grand Piano"
  under *785 MB* in Chorda's source list, the line under "MuldjordKit" under Tympanum's
  *2.7 GB*;
- Tympanum's pieces row cut its last tile (China) at the row's end, out of reach.

**Decision.**

- `say` sets its box at the style's scale (the 100 % look unchanged, to the pixel).
- Chorda's player cards run along the foot under the source list as well as the piano,
  each as wide as its share of the knobs. A row of cells too wide for its card draws them
  a size smaller (`organ::parts::fitting`), and the designed short forms are short enough
  for that size (*Curve*, *Symp.*, *Gan.*).
- In a list, a name too long beside its button wraps to a second line in its own column
  (the row a line taller), and the line under it keeps beside the button too.
- A piece's tile is as wide as its widest name and key ask; a kit wider than its row
  scrolls sideways.
- The Mixer's short strip lines lose their "pair ·" ("over them", "the bench"), and
  Spanish's "todos los micros" becomes "sus micros".

**What the tests hold.** `tests/instruments_scale_e2e.rs`, over the app made by
`CantusApp::assemble`, hosts the editor as a window sized to it shows it:

- views: Chorda (Play, Mixer, Edit) and Tympanum (Kit, Mixer, Matrix);
- scales: 100, 150 and 200 % at the size where each view lays out whole, plus the shell's
  own 1040 × 680 and the narrowest window the editor allows (three quarters of it);
- languages: English and Spanish.

Every word whole at 100 % is painted whole at the other scales. No word is cut: not
elided, not dropped, and at a view's whole size not painted past its card. No two words
overlap, and none runs under a button. Before the fix it failed with 573 problems (most
of them words dropped at 200 %).

**Then (the coordinator's decisions on the two items left open):**

- **Each instrument's editor opens at its own size.** `views::inst::editor_size` is the
  shell's 1 040 × 680, or more where one of the instrument's views lays itself out whole
  only in more:
  - Chorda opens at 1 100 × 680, for its Mixer (1 100 × 600 plus the header and footer);
  - Tympanum opens at 1 200 × 692, for its Mixer's width and its Kit's height
    (1 180 × 640).

  The editor registers this as its whole size (`DeviceWindow::whole_size`). The first
  time it opens, a dock leaf smaller than that floats it. The window is sized to that
  body plus its frame and tab bar, inside the workspace's area. Where the area is
  smaller, the view scrolls as before. A place and size the user gave the editor are
  kept (`workspace_device::place_of`). The narrowest window it allows stays the shell's
  three quarters.
- **The shell's header never cuts an instrument's name.** The shell measures the name on
  its plate. Squeezed, the header gives up, in turn: the track's part ("(Piano)") — shown
  whole or not at all — then the swap cap's word, the preset well's spare width, undo and
  redo, and finally A/B.
- The site's and the picker's pictures are taken at the editor's own size
  (`picker_thumbs::REV` 4).

`tests/instruments_scale_e2e.rs` holds both:

- each instrument, opened fresh in the assembled app, opens at its size, its default view
  uncut (only the lists at the left scroll);
- at the narrowest window, every instrument's name is whole in its header, in English and
  Spanish, and the header cuts nothing.
