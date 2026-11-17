# The website for the effects side round (branch `fx/web`, 2026-11-17)

The owner's orders: every new effect on the site like the others (card, big pictures, detail page,
sound), and **"Try it"**: the CC0 DI guitar through each unit's presets, dry or wet, with the
editor's picture per preset, pre-rendered. The site is published only by the integrator, at the
very end. `fx/web` is `fx/integration` (82b55e97) with `side/web` (675b4852: the light-theme
contrast fixes and the scroll motion) merged in.

## 1. The cards and pages (`web/src/units_data.rs`, EN and ES)

- **New:** Vinculum, Lenis, Cedo, Purus, Comparatio (ADRs 0139–0143), written from
  their reports (`fx/wave-b`, `fx/wave-c`, the unit branches) with only what was measured. Their
  misses are kept off the cards and listed in each entry's comment.
- **`MERGE:` notes** mark what a later branch settles:
  - Cedo, Purus and Comparatio have `presets: None` (their preset files held only the starting
    preset) and `plugin: false` (their export was still planned). Lenis's 14 presets and its
    export need confirming once its step 3 lands.
  - Purus's and Comparatio's window sentences describe design sheet v1. They have no verdict yet.
  - Cedo's "a shape you draw" holds only if its on-screen shape editing lands.
  - Every new card's `big`/`thumb` sizes are estimates until `check_units.py --fix` runs.
- **`fx/wave-b` also carries draft cards for Vinculum and Lenis.** The merge will conflict
  there: keep `fx/web`'s, which corrects two claims. Vinculum's opto and vari-mu laws are not
  "published". Lenis delays its input by its latency at its defaults; it does not pass it
  untouched.
- **Changed cards:**
  - Phasis names its lamp-vibe circuit (six features, the null sentence, a decimal comma in
    Spanish).
  - Tabula says the amp with its cabinet and the rotary are pedals on the board (true in
    `pedal_allowed`), and "nearly any" effect, because Loquax, Fractus, Speculum, hosted
    plug-ins and Reverb are not pedals. No factory board holds Arca yet; a board ending in it
    would show the owner's "pedalboard with amp + cab inside" on first open.
  - Fractus's join figure is now as measured: −62 to −71 dB splatter, where a hard cut's is −3
    to −14 dB.
- `EFFECTS` is 42.

## 2. Pictures

The pipeline is the existing one (`shoot_skins` → `web/tools/units_pictures.py` →
`tools/shots/check_units.py`):

- `shoot_skins` now shoots Saltus, Arca, Rota, Loquax, Fractus and Caelum. Waves B and C are
  there as commented lines, waiting for their `EffectKind`s.
- `units_pictures.py` names all eleven and skips a unit with no shots.
- Tabula's pictures stay `TABULA_WEB=1 shoot_tabula`'s (the rig playing), already in the branch.

Until the merge, the new cards point at pictures that do not exist yet. The local preview
dressed them in design-sheet PNGs, in a scratch copy of `dist/` only, never committed.

## 3. Sound

- **Card excerpts (the ▶ and "Hear its presets"):** `render_unit_sounds` already renders every
  audible effect, so the new units get theirs at the merge with no change.
- **The guitar sounds the owner asked for are in Try it.** Until the merge, every new unit and
  every guitar unit has a committed manifest of **stand-ins**:
  - **Source:** the units' own development renders from `compartir/<unit>/sound`. Each was
    checked against its README.
  - **Shaping:** cut to four bars (bars 5–8 of the 90-BPM progression, so the loop joins),
    brought to −19.3 LUFS by a gain alone, peaks ≤ −1.5 dBFS before encoding.
  - **Format:** MP3 CBR 96 kbit/s at 48 kHz, as the site's other excerpts.
  - **Measured after decoding:** 68 files, highest peak −3.2 dBFS, no clipped sample,
    −19.7…−19.8 LUFS, 7.7–10.7 s, 8.6 MB.
  - **Credits:**
    - The guitar is "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI)".
    - Loquax's voice is "Britton Rea, public domain (Wikimedia Commons)".
    - Cedo's pad is synthetic and its kick is Cantus's Pulsus.
    - Vinculum's mix is Cantus's drums and bass under the CC0 guitar.
    - Purus's hum is synthetic.

  A stand-in says it is a development render and claims no preset level.

## 4. Try it (`web/src/tryit.rs`)

**Where it appears:**
- on each unit's page that has a manifest;
- on `/effects/` (`#try`), with a unit picker;
- as a link from the home's "Hear what they do".

**What it does:**
- **Choosing:** pick a unit, pick a preset, Play.
- **Dry and wet:** both takes start on one Web Audio clock and loop, so Dry and the unit's name
  only move two gains (a 20 ms crossfade) and the music never moves.
- **A new preset** joins its pair at the same point of the loop, crossfaded.
- **Level:** every take is at one loudness. A manifest rendered from presets says what level
  the preset itself left.
- **Picture:** the editor's picture for the preset (the unit's own until a manifest names
  one), in the page's theme and language.

**Accessibility and layout:**
- buttons with `aria-pressed`, arrow keys, Home and End in the preset list, a `role=status`
  while loading;
- both themes, both languages, one column under 52 rem;
- the site's rise-on-scroll, and a fade when the picture changes (opacity only, nothing under
  reduced motion).

**Loading:**
- **Lazy:** the manifest (about 2 KB) is fetched when a unit is shown, the takes on the first
  Play, and the pictures carry `loading="lazy"`. The home page fetches none of it.
- **Wasm budget:** the player is about 50 lines of JavaScript in a wasm-bindgen inline snippet,
  because the same calls from Rust pushed the WebAssembly over its 250 000 B gzip budget. It now
  weighs 247 947 B.
- `/snippets/*` is cached as immutable, `/tryit/*` for a day.

**The manifest (`web/assets/tryit/<unit>.json`):**
- `origin` (`presets` | `dev`), `note`, `source`, `credit`: each a pair, EN then ES;
- `loop` and `dry`;
- `presets[]`: `id`, `name`, `what`, `wet`, `level_db` (or null) and `pic` (a stem:
  `<pic>-{dark,light}[-es].webp`, or null for the unit's picture).

A test (`tryit::tests`) checks:
- every listed unit has a card and a manifest;
- every take is under 400 KB;
- every picture a manifest names exists.

## 5. What the integrator runs at the merge (from the repository root, after every fx branch)

```text
# 1. The cards: resolve units_data.rs (keep fx/web's Vinculum and Lenis), settle each MERGE note.
# 2. The pictures — uncomment wave B/C's lines in examples/shoot_skins.rs first.
U="saltus arca rota loquax fractus caelum phasis vinculum lenis cedo purus comparatio"
cargo run --release -p cantus-ui --example shoot_skins -- target/shots/skins $U
python web/tools/units_pictures.py target/shots/skins $U
cargo run --release -p cantus-ui --example shoot_skins -- target/shots/skins-es --es $U
python web/tools/units_pictures.py target/shots/skins-es --es $U
python tools/shots/check_units.py --fix
# 3. The card excerpts, every unit (rewrites web/src/units_sounds.rs):
cargo run --release -p cantus-ui --example render_unit_sounds -- web
# 4. Try it — the guitar units' takes, manifests and per-preset pictures (4 presets a unit):
cargo run --release -p cantus-ui --example render_tryit -- C:\projects\daw-fx\refs\audio\carbonthief\x\SampleData web target/shots/tryit
python web/tools/tryit_pictures.py target/shots/tryit
# 5. The site:
cd web && trunk build --release && cargo run --release --features ssr -- prerender dist
python tools/wasm_budget.py dist && cargo test --features ssr && cargo fmt --check
node tools/contrast.mjs dist --quick && node tools/motion.mjs dist     # Git Bash: MSYS_NO_PATHCONV=1
```

**What `render_tryit` does:**
- it renders the progression through each factory preset on the engine's own unit, with the song
  playing and its latency removed;
- it leaves out the init patch, the drum, vocal, bus and master presets, and Fractus's
  note-played lanes;
- it shoots the editor as the preset browser leaves it, fed as playing.

**Where its output goes:**
- it overwrites the stand-ins of the eleven guitar units (Phasis included);
- the shared dry take is `assets/tryit/guitar.mp3`;
- the non-guitar stand-ins (Loquax, Vinculum, Lenis, Cedo, Purus) stay development renders
  until someone gives them their own render.

**Size:** expect about 4 × (130 KB + 4 pictures of ~80 KB) a unit, about 2 MB, so about 20 MB
for eleven units. Use `--presets 3` to trim.

**Listen before publishing:** the selection is automatic, and a preset made for drums or a
voice is skipped only by its role tag.

## 6. Verified here, and not

Verified on the built site (Trunk 0.21.14, `cargo test --features ssr` 25 passed, clippy adds
no warning, `cargo fmt --check` clean, 120 pages pre-rendered):
- **WebAssembly:** 247 947 B gzip.
- **Contrast:** every text at AA on `/effects/` (EN and ES) and on seven unit pages (Fragor,
  Caelum, Vinculum, Purus, Cedo, Lenis, Comparatio), in light and dark at 1440 and 390 px
  (36 views).
- **Motion:** `motion.mjs` passes.
- **The player under a script:** in headless Edge it plays, switches preset and goes to Dry.
  Headless keeps the audio context suspended without a real click, so the progress line was not
  seen moving.

**Not compiled here** (the orders: no workspace builds on this machine while three builders run):
- `crates/cantus-ui/examples/render_tryit.rs`, written against the APIs that `render_unit_sounds`,
  `render_tabula_guitar`, `render_lapsus_guitar`, `shoot_tabula` and `shoot_skins` use;
- the six new lines of `shoot_skins.rs`.

Both are rustfmt-clean. Build them first at the merge.

**For the owner, by ear:** the stand-ins' choice of takes, and whether Try it should start on
the unit or on the dry.

Screenshots for the owner: `C:\projects\daw-fx\compartir\site\v1\` (`index.md` there).
