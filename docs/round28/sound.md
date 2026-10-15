# Round 28 — sound: an acoustic drum kit to download, then Chorda

*Builder: sound · branch `r28/sound` (from `r28/integration` at `405bd9e`) · ADR 0042
(amended), ADR 0023 (amended), ADR 0059 · schema 28 on this branch (30 in the round's
table) · BACKLOG §5, §3e.*

**Both parts are done.** A: DRSKit 2.1 (DrumGizmo, CC BY 4.0) installs in one step from
Sound packs, mixed to stereo at install, a General MIDI kit in Exemplar with chokes,
velocity layers and round robins that never repeat a hit in a roll. B: Chorda, the
modelled grand piano, meets every measurable target of its *done when* and is in the picker
and the ballad demo. What stays is the owner's (the end of this file).

## A. The acoustic drum kit

**The kit, its licence read at the source** (ADR 0042's round-28 amendment has the whole
reading). **Naked Drums is refused**: the CC BY 4.0 licence on its SFZ port was put there
by the people who converted it; Wilkinson Audio, the maker, never stated a licence (it sold
the kit, then gave it away in 2016; its archived pages name none). **DRSKit 2.1 is
accepted**: DrumGizmo states CC BY 4.0 on the kit's page and in the `README.md` inside the
archive; their sentence about crediting it in compositions is quoted word for word on the
pack's page and, now, in About ▸ Credits. The archive's published MD5 matched the download;
the manifest pins its size (2 803 397 710 bytes) and our SHA-256.

**One step, verified, credited** (`cantus-content`): manifest format 3 gives a pack a
**build** step — `drumgizmo.rs` reads the kit (13 microphones, channels stored in another
order than the kit lists them), `kitbuild.rs` mixes each hit to 24-bit stereo (per
microphone gain, equal-power pan, polarity, bleed on the drums it does not face; per drum
balance; deterministic sums), measures every hit, writes the SFZ (velocity layers by level
on SFZ's curve, round robins of three per layer, General MIDI keys plus DrumGizmo's extras,
the hi-hat choke with a 60 ms release, the groups named by the manifest's labels, the
credit at the top) and prunes what it does not keep; the install's budget is the build's
unpacked size; a failed build installs nothing. Sound packs shows *Mixing the microphones:
n of m hits*. The pack is on the picker's **Drums** shelf, with 64 voices and Exemplar's
**drum rows** on (the piano roll and the channel rack show a row per drum by name).

**What Exemplar gained** (ADR 0023's round-28 amendment): **round robin in order** — a clip's
note takes its turn by its ordinal (`NoteEvent::ordinal` through `note_on_seq`), so a roll
never plays the same hit twice and a render from any start cycles alike; live notes cycle
per key; `rr.order` (By note / In order) in the tools row, schema 28 writing *By note* into
every older Exemplar so old songs keep their sound (v28 fixture). **Drum rows** (`kit`):
`Lanes::Keys` named by the SFZ groups' `group_label` or General MIDI's names. SFZ
`group_label` names Exemplar's groups.

**The mixdown, measured** (ADR 0042's table): a stereo mix at install (490 MB, 123 MiB of
heads, one voice a hit, 591 zones) against a stem per bus (1.47 GB, 1 773 zones) and every
microphone a zone (3.2 GB, 7 683 zones). Faders per microphone need an instrument with
several outputs (BACKLOG §3e item 11): left there.

**Measured on the real archive** (ignored tests `kit_build::the_real_drskit_installs_mixed_
from_its_archive`, `CANTUS_DRSKIT_ARCHIVE`; `pack_drums::the_real_drskit_plays_a_groove_at_
the_calibrated_level`, `CANTUS_DRSKIT`): installs in 104–128 s to 527 files, 489 577 116
bytes, peak −4.9 dBFS; imports in 0.09 s; a groove at −26.2 LUFS / −7.4 dBTP, everything at
velocity 127 at −0.9 dBTP (gain 0 dB by ADR 0039's rule); 186 velocity layers, each with at
least two hits in turn. Shots: `F:\projects\daw-r28\shots\sound\` (Sound packs before and
installed, the picker, About, Exemplar with the kit, the rows; 75/100/200 %, EN/ES).

**Tests:** `cantus-content/tests/kit_build.rs` (19), `cantus-dsp` `exemplar_tests4.rs` (5),
`cantus-core/tests/sampler_kit_lanes.rs` (3), `cantus-engine/tests/inst_sampler.rs` (2 new),
`cantus-project` schema 28, `cantus-ui/tests/pack_drums.rs` (6), `about.rs` (DRSKit's
credit and its authors' words).

## B. Chorda — the decision, before code (2026-10-15)

**Decision: Chorda can be finished this round to the measurable part of its *done when*,
and I build it now.** What follows is why, what "finished" means in numbers, how the
patents are avoided, and what stays the owner's.

### Why it can be finished

- **The engine's pieces exist.** Gravis's bidirectional waveguide string
  (`gravis_string.rs`: rails, phase-delay tuning to a fraction of a cent, a one-pole loss,
  first-order dispersion allpasses), Lamina's hammer shape and strike scaling
  (`lamina_voice.rs`), the voice allocator (`voices.rs`), voices split across the pool
  with a post-voice stage summed in voice order (ADR 0038: Unda, Calor and Exemplar already
  do it), the modulation matrix, the kit shell (Play / Edit / Matrix) and the instrument
  contract tests (`inst_contract.rs`: block size, determinism, headroom, silence). Chorda
  is new physics on a known chassis, not a new chassis.
- **The physics is published and closed-form.** Waveguide strings (Smith, *PASP*), a
  dispersion filter designed to the stiff-string law `f_n = n f₀ √(1 + B n²)` (Van Duyne &
  Smith 1994; Rauhala & Välimäki 2006; a least-squares phase fit), loss filters from two
  decay times (Välimäki et al. 1996), a hysteretic felt hammer (Stulov 1995) solved against
  the string's wave impedance with a fixed number of iterations (Chaigne & Askenfelt 1994;
  Bank 2000), unisons coupled at a bridge admittance (Weinreich 1977: beating and two-stage
  decay), phantom partials from tension modulation (Bank & Sujbert 2005), dampers and the
  pedal as felt losses, and a soundboard as a post-voice filter. The data the brief asks
  for is readable here: Balázs Bank's thesis (measured string parameters, decay times and
  the soundboard model; `home.mit.bme.hu/~bank/thesis`), Rigaud, David & Daudet's
  parametric models of inharmonicity and tuning across the keyboard (DAFx 2011, from
  measurements of several pianos), Chaigne & Askenfelt's strings, and the perceptual
  tolerance of decay (Tolonen & Järveläinen, −25 % / +40 %, via Bank et al. 2003).
- **The cost fits.** A voice is one to three strings of ~100 operations a sample (a delay
  line, a fractional delay, 4–16 dispersion sections, a loss filter, the bridge); 64
  pedal-down voices are ~20 k operations a sample, and the shared stages (the sympathetic
  strings at the keys' own pitches, the soundboard) a few thousand more — inside 25 % of one
  core at 256 frames if written for it (independent strings side by side, coefficients
  computed off the audio thread), and split across the pool besides.

### What "finished" means — the tests that will hold it

1. **Tuning:** on every one of the 88 keys, one string alone (unisons off), every partial
   below 5 kHz (at most 32) within **1 cent** of the target `n f₀ √(1 + B n²)`, with `f₀`
   from the stretched tuning and `B` from the inharmonicity model across the keyboard —
   measured on the rendered sound (FFT, interpolated peaks over seconds), not computed from
   the filter's formula.
2. **Decay:** each partial's T60 measured on the rendered sound (unisons off, pedal up, key
   held) within **−25 % / +40 %** of the published measurements at the notes they were
   published for, and of the model fitted to them in between.
3. **Half-pedal:** continuous — the decay of a held note changes monotonically and smoothly
   as the pedal moves from up to down (no step between two models, no click), and
   re-pedalling (up and down again within a note) leaves no discontinuity.
4. **Cost:** 64 voices with the pedal down inside **25 % of one core at 256 frames**
   (measured, best of several runs, on this machine; the number in ADR 0059).
5. **The family's rules (ADR 0023):** a control at its default changes no sample, the same
   bits at block sizes 1, 37, 64 and 512, offline equal to real time, parallel equal to
   serial, exact rest once the sound has died, headroom (no factory preset past 0 dBFS on
   the dense riff), deterministic randomness (hashes of the note).
6. **The product:** a `model_version` stored per instance, presets made with care, the kit
   shell with a picture of what plays (strings, hammer, pedal), `docs/instruments/chorda.md`,
   the manual in both languages, `READY = true`, a demo song.

### The patents (US; Google Patents' status, not a legal conclusion)

- **Modartt US 7,915,515** (to 2027-10-20): no presynthesis of per-partial frequencies and
  dampings from a soundboard parameter for a partial-based synthesizer. Chorda's strings are
  waveguides — partials are not synthesised one by one — and the dispersion and loss
  filters are designed from the string alone (its `f₀`, `B` and decay times); the
  soundboard is a separate filter after the strings.
- **Casio US 8,178,772**: no resonators tuned to delays that do not match the keys' pitches.
  The sympathetic strings are the keys' own strings, at their speaking pitches.
- **Yamaha US 10,818,278**: no second loop at a non-speaking pitch (no duplex scale model).
- **Yamaha US 8,729,376**: no delay loop excited by a band-split sampled waveform — the
  hammer is a physical model, no sample excites anything.
- Kawai US 9,330,648, Casio US 10,109,268 / 11,881,196, Yamaha US 12,039,960: their
  structures (stored resonance in a sample player, register IRs, excitation routed to
  low-string channels, a noise-derived damper envelope) are not used.
- The legal review against every claim stays the owner's, before release
  (`docs/beta-checklist.md`).

### What stays the owner's

The blind listening test (≥ 20 listeners against Pianoteq 9 and a sampled grand), the
legal review, and the presets' last word by ear.

### If a number cannot be met

If one of the four measurable targets proves out of reach with the published approach,
Chorda stays hidden (`READY = false`), this file says which and by how much, and the
integrator decides — nothing half-done reaches the picker.

## B. Chorda — what was built and measured

Built as decided above; ADR 0059 holds the decisions, `docs/instruments/chorda.md` the
instrument. In short: published tables per key (Rigaud et al.'s inharmonicity and aural
tuning, Bensa et al.'s strings, Chaigne & Askenfelt's hammers); waveguide strings with a
designed dispersion (Abel et al.'s second-order allpass sections refined by
Levenberg–Marquardt), a linear-phase loss filter and a resistive bridge (two-stage decay);
Stulov's hysteretic hammer; phantom partials; dampers with a continuous half-pedal law; the
soft pedal; the keys' own strings ringing in sympathy; a multirate soundboard and the
radiation's rise; 88 voices across the pool, summed in key order; designs in a lock-free
registry computed beside the window (at start-up, after a device change, when a project's
units are built), an offline render waiting for its own.

| Done when | Measured | Held by |
|---|---|---|
| Every partial within 1 cent of the stretched target, every key | designed: worst 0.47 ct (44.1 kHz), 0.49 ct (48 kHz); on the rendered sound: worst 0.40 ct | `chorda::tests_design`, `tests_string` |
| Per-partial T60 within −25 % / +40 % of published | 0.96–1.01 of Bensa et al. at C2, C4, C7 | `tests_string::a_string_decays_as_bensa_et_al_measured` |
| Half pedal continuous | 241 → 41 dB/s from up to down, monotonic, largest step 28 % of the range; re-pedalling without a jump | `tests_piano` |
| 64 pedal-down voices < 25 % of a core, 256 frames | 18.2–19.1 % (release, best of three, this PC) | `measure_sixty_four_pedal_down_voices` (ignored) |
| Null at defaults, block sizes, offline = real time, parallel = serial, exact rest | to the bit | `inst_chorda.rs` (engine), `inst_contract.rs`, `tests_engine` |
| The product | `model` stored; ten presets with four macros; Play / Edit / Matrix with the piano drawn from its numbers; manual EN/ES; `READY = true`; the ballad demo | `cantus-ui/tests/inst_chorda.rs`, `demos.rs`, `picker.rs` |

Also measured: prompt sound 24.6 dB/s against aftersound 8.7 dB/s at middle C; sympathy −17
dB with the pedal against −32 dB without; both hands fortissimo with the pedal −3.3 dBFS, a
mezzo-forte middle C −22.7 dBFS; every preset under −6 dBFS on the dense riff; a keyboard's
design about two seconds on eight threads.

**What the contract caught.** The first engine counted its 64-sample chunks from each
block: the sympathetic strings looked ahead in the chunk for drive and learnt which keys
sounded at chunk starts, so blocks of 1 against 512 differed (`split::chorda`,
`voices::chorda`, the skipped twin). The chunks now end on the stream's own grid, the
dampers learn on it, the strings wake on drive seen, and `tests_engine::notes_and_pedals_
inside_a_block…` (a chord at frame 0, staccato notes dying mid-block, pedal moves on odd
frames) fails on the old code and passes now.

**Schema.** None for Chorda: the kind and its keys are new (no old project holds one); the
shares are fractions shown in %, like every other instrument's. Schema 28 on this branch is
Exemplar's `rr.order` (A); the round's table numbers it 30 — the integrator chains it.

**Shared files touched (additive):** `views/inst/mod.rs` (`InstEditors::chorda`),
`views/inst/organ_parts.rs` (Chorda's placed cells), `app.rs` (warm the designs at
`assemble` and after a device change), `views/about.rs` (an author's note under its credit),
`demos/ballad.rs` (its piano is Chorda), tests `picker.rs` (the count follows the ready
kinds), `instrument_levels.rs` (Chorda's presets), `demos.rs` (a comment),
`about.rs` (DRSKit's credit), `engine/tests/rt_memory.rs` (Chorda's voices).

## Left

Owner-only (on `docs/beta-checklist.md`):

- Chorda's **blind listening test** (≥ 20 listeners against Pianoteq 9 and a sampled grand)
  and the presets' last word by ear.
- Chorda's **legal review** against the patents ADR 0059 names.
- Playing DRSKit and Chorda by hand (an 88-key keyboard with a continuous pedal).
- Asking Wilkinson Audio for Naked Drums' licence in writing, if the owner wants that kit
  too (ADR 0042).
- `F:\projects\daw-r28\downloads\` keeps the DRSKit archive (2.7 GB, for
  `CANTUS_DRSKIT_ARCHIVE`), the installed mix (`drsmix`, 469 MB, for `CANTUS_DRSKIT`) and the
  papers read (15 MB); the unpacked kit (4.1 GB) is deleted. Keep or delete.

Nothing else is open.
