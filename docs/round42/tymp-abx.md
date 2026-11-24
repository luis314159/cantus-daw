# Round 42 — `tymp-abx`: Tympanum's blind listening kit

Branch `r42/tymp-abx` (from `r42/integration` aac9f2044). The owner: *"¿No hay ABX de
Tympanum?"* — there was none for the shipped instrument: round 40 had non-blind listens
(`tools/tympanum-fit/make_listen.py`) and an offline MUSHRA of resyntheses that never went
through the engine (`docs/round40/ladder.md`). This builds the blind kit of **Tympanum as it
ships**, at the class of Chorda's (`docs/round40/abx5.md`), for the owner and the friends he
sends it to. It closes BACKLOG's *"Tympanum's blind listening kit"* item up to the owner's
verdict (which stays open: *done when his verdict is recorded with the numbers*).

## The paths

| what | where |
|---|---|
| the kit (FLAC, offline: what a listener receives) | `F:\projects\daw-r42\listen\tympanum-abx\` (`index.html`, `audio\` 38 files, `LEEME.md`, `README.md`) |
| the zip to send | `F:\projects\daw-r42\listen\ABX-Tympanum.zip` (59.8 MB, 41 entries at its root as Chorda's, `PK`, tested) |
| the same kit as WAV (to publish as a claude.ai page; same id, names, samples, page) | `F:\projects\daw-r42\listen\tympanum-abx-wav\` |
| the key (never sent) | `F:\projects\daw-r42\listen\tympanum-abx-key\` (`key.json`, `levels.csv`, `hits.csv`) |
| the packs (content root) | `F:\projects\daw-r42\downloads\tympanum\content\` — the two hybrid packs (v3) installed by Cantus's pack code from the manifest (SHA-256 checked); DRSKit and Virtuosity (v1, the manifest's version) are junctions to round 40's installs in `F:\projects\daw-r40\downloads\tymp-engine\content\` |
| the generator | `crates/cantus-ui/examples/tympanum_abx/` (`main.rs`, `items.rs`, `subject.json`, `readme.{es,en}.md`) |
| the shared page, its check, the shared code | `crates/cantus-ui/examples/abx_kit/` (`abx.html`, `abx_browser_check.mjs`, `kit.rs`, `names.rs`) |

```text
cargo run --release -p cantus-ui --example tympanum_abx -- F:/projects/daw-r42/listen/tympanum-abx \
    --wav-twin F:/projects/daw-r42/listen/tympanum-abx-wav
PW_DIR=F:/projects/daw-r40/tmp/pw node crates/cantus-ui/examples/abx_kit/abx_browser_check.mjs F:/projects/daw-r42/listen/tympanum-abx
python -I tools/listening/abx_read.py <results.json> F:/projects/daw-r42/listen/tympanum-abx-key/key.json
```

## Generalised, not forked

Chorda's kit (`chorda_abx`) had its page, check and blinding written for three pianos. They
are now one kit for every instrument:

- **`abx_kit/abx.html`** (moved from `chorda_kit/`): what is compared is a **subject file**
  the generator fills into `__SUBJECT__` — the texts laid over the page's own (lead, help,
  the sources' names `voice_<id>`, the credits paragraphs and links), the items' names in
  both languages, the pairs the preferences count, the trials of each ABX, the form's
  instrument question (`play`: `piano` keeps Chorda's `about.piano`; `drums` for Tympanum),
  the seconds a trial and a question take (the "minutes left"). Part 1's trials now draw
  **every item equally often** (Chorda's drew 16 from three shuffled rounds of 7 phrases,
  so a phrase could come once or three times). Chorda's subject is
  `chorda_kit/abx_subject.json`, its words unchanged.
- **`abx_kit/kit.rs`**: the loudness match (cut to one length, fade, BS.1770 to a target,
  lowered together for the peak ceiling, the second pass for the gate — Chorda's code moved,
  not rewritten), the page's fill (every placeholder checked), the subject's one-line JSON, the
  24-bit FLAC/WAV writers; `abx_kit/names.rs` (moved from `chorda_abx/`): the random names and
  the veil. `chorda_abx` uses both; its output is the same as before.
- **`abx_browser_check.mjs`**: reads the counts, the instrument question, the sources' names
  and the credits from the kit's `SUBJECT`; what must stay hidden is a table by subject id;
  `--mock <abx.html> <subject.json>` builds a stand-in kit for any subject; `ABX_SAVE=<dir>`
  keeps every downloaded results file. Run on a Chorda mock: 190 checks, 0 failed (Edge +
  published mode) — Chorda's kit still passes on the generic page.
- **`tools/listening/abx_read.py`**: any kit; a per-item table at the end (ABX right/answered
  per item and part, every preference as `chosen > other`, the catch questions passed or not).

## What the kit compares

Three voices, the same MIDI through each, through the engine's offline render (the export's
path, `EngineHandle::compile_for_render` + `render`, latency compensated): **generated**
(`model:<kit>`, the default voice), **recording** (`pack:<pack>/kit` through Tympanum's
recorded mode, read by the app's own loader `tympanum_load`), **hybrid** (`model:<kit>` with
`voice=hybrid`, its residual read from the installed pack). Each modelled kit against the
recording it was measured from (ADR 0124): the **studio kit vs DRSKit 2.1** (CC BY 4.0; the
studio hybrid pack was fitted on DRSKit) and the **jazz kit vs Virtuosity Drums** (CC0; the
jazz pack on Virtuosity). Both residuals were fitted on this build's model (hash 1479190454 =
`model_hash()`): the hybrid heard is the one that ships. The tool refuses a kit whose hybrid
render equals the generated one (a pack the engine did not read): here the hybrid differs from
the model by −3 to −11 dB per item.

- **Part 1** — ABX generated vs recording, 24 trials (each of the 12 items twice).
- **Part 2** — 26 preferences: every item generated vs recording and hybrid vs recording, plus
  two catch questions (one clip under two names: this kit draws the toms' recording and the
  ballad's generated voice), never the first question. *"¿Cuál suena más a una batería acústica
  real, bien grabada?"*, with an optional word why.
- **Extra (optional)** — ABX hybrid vs recording, 24 trials.

About 20–25 minutes, 12 more for the extra (said on the page).

### The items, and why

| id | kit | what | why |
|---|---|---|---|
| `kick` | studio / DRSKit | 36 at 45, 85, 125, 1.6 s apart | the piece most kits get right: a control item where "can't tell" is plausible |
| `snare` | studio / DRSKit | 38 at 45, 85, 125 | the hardest drum to model (the ladder's snare scored 24–64 of 100) |
| `rimshot` | studio / DRSKit | 40 at 70, 100, 127 | the snare's other common stroke; the hybrid changes it most (−3.1 dB) |
| `toms` | studio / DRSKit | 48, 45, 41 at 80, then at 120, 1.3 s apart | three pitches, two dynamics: where the model's dynamics show (below) |
| `hat_closed` | studio / DRSKit | 42 at 45, 85, 125, pedal closed | the busiest piece in any groove |
| `hat_open` | studio / DRSKit | 46 at 45, 85, 125, pedal at 0.95 (CC4) | a noisy, long decay: the "iuuu" the owner heard in the ladder's cymbal tails |
| `crash` | studio / DRSKit | 49 at 50, 90, 125, 4 s apart, 5 s tail | long tails: where a model's decays and swells are exposed |
| `ride` | studio / DRSKit | 51 at 45, 80, 115, 2.5 s apart | the ping and the wash |
| `pop` | studio / DRSKit | 74 BPM, 4 bars + the last crash: verse on closed hats, a lift on looser hats with a crash, a 16th tom fill | the owner's music (pop/worship) |
| `rock` | studio / DRSKit | 116 BPM: a crash in, eighths, hard backbeat, an open hat closed by the foot, a snare-and-toms fill | loud playing, the hat's pedal, fills |
| `bossa` | studio / DRSKit | 126 BPM: side stick on the 2-bar bossa clave, the kick's dotted pulse, closed hats then the ride, a soft tom phrase, a soft crash | the Latin item; the side stick (DRSKit has a real one; Virtuosity's key 37 is a stick shot, so this item is on the studio pair) |
| `ballad` | jazz / Virtuosity | 66 BPM: ride eighths, half-time snare, foot on 2 and 4, two crash swells (16 32nds rising from velocity 18 to 96) each landing on a crash | the ballad with ride and swells; the jazz kit's one item, on the kit whose ride and crash were measured |

Pieces alone are where a model is most exposed (no masking, the decay heard to the end); the
grooves are where users hear it. Every piece is played at two or three dynamics in one clip,
so the velocity response is judged too. The items lean to the studio kit (the default modelled
kit); the jazz kit has one item. A jazz-kit kit is a change to `items.rs` (`Pair::Jazz`).

## How levels and microphones were matched

- **Microphones: each kit's own mix** — every mic at its default level and pan, as a user hears
  a kit on loading it (and what the model and the hybrid were fitted against: the fit rendered
  each recorded kit's full mix through the engine). No mic soloed or muted: DRSKit's 11 mics
  (ambience, hi-hat, two kick, overheads, ride, two snare, three toms), Virtuosity's 7, the
  modelled kits' virtual mics.
- **Stereo: the modelled voices mirrored.** Measured, not assumed: the studio kit stands as
  its design draws it (audience's view: hats right, ride left) and DRSKit as it was miked
  (drummer's view: hats left, ride right) — every cymbal and the toms on the other side, a
  give-away in any ABX. Per pair, the tool compares the modelled voices' left/right balance
  with the recording's as they are and swapped, and swaps left and right of the generated and
  hybrid clips (nothing else) when that is closer: studio 95.2 dB of summed mismatch as they
  are, 31.7 mirrored → mirrored; jazz 3.1 → 1.7 → mirrored (the same design; Virtuosity's image
  is narrow). Written in `key.json` (`stereo`) and in the page's credits. **This is a product
  finding:** a user switching between Tympanum's modelled kit and a recorded kit hears the hats
  jump sides (below).
- **Loudness** (`abx_kit/kit.rs`, as Chorda's): each item's three clips cut to one length,
  100 ms fade-out, 80 ms of silence before the first hit, ITU-R BS.1770 integrated to −23 LUFS
  (drums sit lower than a piano's phrases), lowered together when a clip's peak would pass
  −1 dBFS; 24-bit, 48 kHz. Within an item the clips match to 0.01 LU; across items the target
  falls to −26.1 (snare), −28.4 (pop), −29.7 (ride) and −31.0 LUFS (toms) — because the
  **generated voice's peaks** are far above the recording's at equal loudness (below). The
  readmes say that the clips of *one question* match.
- The velocities within a clip are left as each voice plays them (the match is one gain per
  clip), so the dynamics are part of what is judged.

## What the measures already say (`levels.csv`, `hits.csv` in the key)

Not a verdict — the owner's ear is — but where to listen:

- **Peaks.** Crest factor (peak − loudness) generated vs recording: toms 30.0 vs 23.8 dB, pop
  27.4 vs 20.8, ride 28.7 vs 25.1, snare 24.7 vs 20.5, rock 21.8 vs 19.3. At its default level
  the generated studio kit peaks **over 0 dBFS** (toms +3.1, pop +3.8, snare +1.6, rock +1.8 dBFS
  in the engine's float render) where DRSKit stays at −3 to −5: a mix that clips its master
  unless lowered.
- **Dynamics** (each hit's first 80 ms). The generated rack toms do not grow from velocity 80
  to 120 — they get *quieter* (high −18.6 → −19.2 dB, mid −20.7 → −21.4; floor −24.4 → −18.0)
  where DRSKit's grow +6 to +7 dB each; the generated kick's soft-to-hard span is 10.5 dB
  against DRSKit's 15.4 (its soft hits too loud), the snare's 16.7 against 13.1 (its soft hits
  too soft); the hats, the crash and the ride follow the recording within ~2.5 dB.
- **Width.** Side/mid energy: the generated drums are narrower than DRSKit's (kick −26.6 vs
  −15.6 dB, snare −14.4 vs −8.3, grooves −14 to −16 vs −10 to −12): DRSKit's room and
  overheads spread every drum more.
- **The hybrid** changes the generated clips by −3 (rimshot) to −11 dB (kick); it keeps the
  model's peaks and dynamics (it adds under the attacks; it does not reshape the level).

## What was verified

- `cargo clippy -p cantus-ui --example tympanum_abx --example chorda_abx --example
  chorda_pianos -- -D warnings`: clean. Every changed `.rs` through `rustfmt --edition 2021`.
- `cargo test -p cantus-ui --example tympanum_abx --example chorda_abx`: 7 + 11 green — the
  shared page has every placeholder, a matched group is cut, faded and at one loudness under
  the ceiling (`kit.rs`), each item named once and never naming a kit or voice, every hit
  inside its range, the swells rise, the subject names every voice and item in both languages,
  the readmes never name a source; Chorda's names tests where they moved.
- **Through Cantus's pack code**: the two hybrid packs downloaded and installed from the
  bundled manifest (version 3, SHA-256 checked by the manager); the residuals' model hash equals
  the build's; the recorded kits read by `tympanum_load` (DRSKit: 16 pieces, 25 articulations,
  11 mics, 3 696 files; Virtuosity: 54 pieces, 61 articulations, 7 mics, 4 180 files); no
  streamed note cut.
- **The page in real browsers** (`abx_browser_check.mjs` on the shipped FLAC kit: Edge and
  Chrome installed, Firefox 132 of Playwright — installed for this round, the earlier copy was
  gone — each over http (Web Audio) and file:// (`<audio>`), plus the published mode against a
  stub of `window.claude`): **489 checks, 0 failed** (kit `drums1-28ac6d2f91`). Every control: the language toggle; the form (Start
  disabled until the playback device; *¿Tocas la batería?*, years, audio work, age, nickname
  recorded); A / B / X playing, lit, switching at the same place, restarting when pressed
  again; Stop; **Loop** (lit and `aria-pressed`, a clip played into its end starts over, a jump
  into its last 0.3 s plays on, off it unlights and the clip ends); the position bar; the keys
  (A B X, space, L, 1 2; never while typing a note; space on a focused button presses nothing);
  answers, a changed answer counted, *← Anterior / Siguiente →*; part 1 drawing each of the 12
  items exactly twice; the 26 preferences with **both catch questions among them** (one clip
  under two names, byte-identical); a reload resuming with *Bienvenido de nuevo*; the extra part;
  the reveal naming the three voices per trial; every credit link (DRSKit, CC BY 4.0,
  Virtuosity Drums, CC0 1.0, Cantus) and both credit paragraphs; **the results JSON
  downloaded** (form, every answer with its time, start / end / export times); *Empezar de
  nuevo*; published: the listener's own `abx/<id>` written with no source named, the owner's
  summary (24/24 and 12/24, pooled 36/48, by device and by *¿Tocas la batería?*, catch-passers).
  Besides: every file the page names exists, 38 files = the page's table, no two names share
  more than 9 of 16 hex digits, the page's data never says tympanum / drskit / virtuosity /
  model / hybrid / record / studio / jazz / cantus / …, nor the readmes, and the first screen
  names no source. The WAV twin: **489 checks, 0 failed**. A Chorda stand-in kit (`--mock` with
  Chorda's subject) on the same page: 316 checks, 0 failed (Edge, Firefox, published mode).
- **The results read back**: the downloaded JSON of each browser run kept (`ABX_SAVE`) and
  read by `abx_read.py` with the key: the score and p, every trial and preference by voice name,
  the per-item table, both catch questions found (`catch(recording): …`, `catch(model): …`).
- **Chrome and a burst of WAV fetches**: the first runs on the WAV twin lost Chrome's browser
  process now and then (exit 0x80000003, a deliberate crash, while the check clicked through
  26 answered questions in about two seconds — 4 WAVs of up to 5 MB fetched and decoded per
  question). The page now loads an item's clips 120 ms after it is shown and the next item's
  600 ms after, unless the item changed (a press loads its clip at once): before, 2 of 5 Chrome runs on the
  WAV twin crashed; after, 4 of 4 and both full runs passed. A real listener never clicks that
  fast, but one reviewing answers with *Siguiente →* could have. The check now waits for the
  first preference clip to start (up to 6 s) instead of a fixed 150 ms, since the clip may
  still be arriving when a question has just shown.

## Only the owner's ear can judge

- Whether he tells the generated studio kit from DRSKit at all, and on which items — the
  per-item table of `abx_read.py` says where; the prediction from the measures is the toms, the
  snare and the grooves (peaks, dynamics, width), the cymbal tails if the "iuuu" is still
  there.
- Whether the hybrid closes the gap he hears (the extra part and the *hybrid vs recording*
  preferences).
- Whether the mirroring is right to do (it hides an orientation difference that a user *does*
  hear when switching kits) — and whether Tympanum's modelled kits should face the drummer's
  way, as the recorded packs do. Not changed here: it is the instrument's, an ADR's decision.
- Whether −23 LUFS (with the quieter items at −26 to −31) is comfortable on his headphones
  and on a friend's speakers.
- Not verified here: Safari (no Mac); the published page on claude.ai itself (the WAV twin is
  103 MB: two publishes of ≤ 64 MB, as Chorda's — `tools/listening/README.md`).
