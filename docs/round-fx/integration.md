# The effects side round — the integrator's account

Branch `fx/integration` (from `master` `dd4ad68b`, beta.14), worktree
`C:\projects\daw-fx\integration`. The owner asked on 2026-11-13, in a second session beside
round 40, for more effects that make Cantus complete (BACKLOG §3b), each *"un panel con
personalidad propia y sus gráficos bien pulidos y profesionales como FL Studio y otros"*. Five
units, four builders in two waves, one contract (`docs/round-fx/contract.md`), the rules in
`BUILDING.md`, the briefs in `briefs.md`, each builder's report beside this file.

**Nothing here has left `fx/integration`**: no push, no merge to `master`. The round is merged
**after round 40 lands on master**, as round-41 material (see *At the merge* below).

## The five units

| Unit | Kind | What it is | ADR | Presets |
|---|---|---|---|---|
| **Cribrum** | `AutoFilter` | Two analog-modelled filters (SVF, transistor ladder, OTA cascade, diode ladder, Sallen-Key; low/band/high-pass, notch, peak, a talking vowel) in series or parallel, moved by a song-synced LFO, an envelope follower (with sidechain) and a drawn curve; the window draws the music sieved as grains under both responses. Starting preset *Bar sweep*. | 0127 | 18 |
| **Fragor** | `Pedal` | A board of up to three circuit-modelled distortion and fuzz pedals in any order — green overdrive, op-amp distortion, silicon distortion, germanium fuzz (Bias = a dying battery), triangle muff — each solved from its schematic every sample (DK method), a gate before them, 2×/4×/8× oversampling; a valve amp glowing behind the board, energy along the cables. | 0128 | 15 |
| **Lapsus** | `TimeShaper` | Gross Beat class: 36 time and 36 volume patterns, chosen apart and played together by knob, pad or note; drawn on the beat (tape stops, half time, stutters, rolls, reverse, scratches, gates, pumps); joins crossfaded, a 32-tap sinc read, a tape stop ending on the bar's sample at any tempo. Starting preset *Half time*. | 0129 | 17 |
| **Vetus** | `LoFi` | Six modules in a recording's order — bits, sample rate, wow and flutter, age (dropouts, wear), noise (crackle, hiss, hum) and the band of tape, radio, telephone or gramophone — and Character, which ages them all; a cassette or an LP at the centre (a view setting), the spectrum beside it. Starting preset *Worn cassette*. | 0130 | 14 |
| **Libra** | `Utility` | Gain, balance, width, channel operations (swap, mono, one side, mid, side), polarity, DC removal, a delay per channel in whole samples; a brass balance at the centre (the beam tilts with the balance, the pans hold each channel's level), a tuner and a goniometer around it. Inserts as a wire. | 0130 | 14 |

Every unit: a null test at its defaults, the table-driven `fx_contract` (block sizes
1/37/64/512 at tolerance 0, split invariance, the exact rest twin), no allocation on the audio
thread (`rt_memory`), a render from bar 5 equal to the whole render's bars 5…, a starting
preset (or its reason for none), every control a `Command` with undo and its parameter menu,
the CLAP/VST3 export bit-identical to the engine, an end-to-end test through
`CantusApp::assemble` (`cribrum_e2e`, `fragor_e2e`, `lapsus_e2e`, `vetus_e2e`, `libra_e2e`), a
page in `docs/plugins/`, a manual section in English and Spanish, the AI's unit description and
the site's card. What each measured is in its builder's report and ADR.

## How it was built

1. **The contract** (`9fe58632`, `docs/round-fx/contract.md`): the five kinds as exact
   pass-through stubs with one parameter `out` (`ALL[26..31]`, never reordered), schema 72
   (`fixtures/v72.cantus`), empty locale areas, the stub ADRs 0127–0130.
2. **Wave 1**, Cribrum and Fragor, merged at `122ada78` and `0abc7763`; **wave 2**, Lapsus
   (`dd7b9edf`), then Vetus and Libra (`fx/vetus`, which took `fx/integration` in at
   `60f45871` and was fast-forwarded to `60cc1be6`).
3. **The integrated review** after Lapsus found Fragor and Lapsus wearing near-identical faces
   (amber glass) and Lapsus's blue on Cribrum's hue: Lapsus v3 (matte teal-petrol), and
   `kit/skin/side.rs`'s distinctness tests for the round.
4. **This close-out** (below): the bench's heavy settings, the AI's words, the README, the
   colourways held to the house rule, the site, the whole suite.

## The owner's decisions and verdicts

All in `C:\projects\daw-fx\compartir\<unit>\` (`verdict.md` and the design folders).

- **Every unit's display stays a dark screen in both themes** (his correction, 2026-11-13);
  **finer strokes** everywhere (*"que se vea muy profesional"*, compared with FL Studio and the
  best plug-ins).
- **Cribrum**: all three colour variants, selectable — the **colourway** mechanism (ADR 0127
  §Colourways): Nocturne (the face), Brass, Petrol. Kept: the grains, one Filter card, one
  movement source at a time. Of the guitar wahs he chose the *deep* one: *Cry wah*.
- **Fragor**: palette **A, warm amber** with glowing valves behind the board (*"metele más
  detalles valvulares"*); elaborate original art per pedal; **pedals of any size**; v3
  approved (*"se ve chido, síguele"*); **energy flowing along the cables**. *"Bias"* in
  Spanish too. Palettes B and C now offered as colourways (Cream, Steel — below).
- **Lapsus**: the palette kept (*"a DJ deck at midnight"*); **separate banks as Gross Beat**
  (36 time + 36 volume patterns); after the integrated review, **its material changed, its
  approved colours kept** (v3, matte teal-petrol, ice-blue controls, amber time curve).
- **Vetus**: **both objects**, the LP and a cassette, chosen as a view setting; the
  **Cassette** palette the default (Sleeve and Walnut colourways); Character shown as a year;
  a module's controls live while it is off; no vents; a cleaner Slate. v2 approved
  (*"síguele, se ve chido"*).
- **Libra**: **a brass balance at the centre** (his choice: *Libra* is the scales), the tuner's
  reference A beside the tuner, every element saying what it is, the Spanish words; v2
  approved.

## The close-out (this session)

- **The bench** (`cantus-bench/src/kernels.rs`): Cribrum, Fragor and Lapsus measured at their
  real heavy settings (the engine tests' `heavy.rs`), not the contract's placeholder gain.
- **The AI's effect words** (`cantus-ai/src/tools_mixer.rs`): a short true description of each
  of the five instead of *"being built: for now each passes the sound through"*; the tools
  golden regenerated with `CANTUS_TOOLS_DUMP=1` (that sentence only, twice). The contract's
  stale stub comments taken out (`starting.rs`, `heavy.rs`, `preset_audit.rs`, the plug-ins'
  test harness, the fixture writer). Nothing else in the tree, the docs or the site spoke of
  the five as unbuilt.
- **The README**: the five in the effects grid and each a section from its site card (thirty
  effects; thirty-one with the older Reverb). `stated_preset_counts` holds their counts.
- **Colourways held to the house rule** (`kit/skin/side.rs`): the tests now hold **every
  colourway offered**, not only the default faces — no material and accent within 20° of
  another unit's face or colourway, no two of the round's sheets in the same two voices (ADR
  0127 §Colourways 6 amended). Fragor's palettes B and C are now **offered colourways**
  through Cribrum's mechanism (`unit_colourways`, the window's ⋯ menu; `fragor_colourways`
  through the boot path): **Valve** (the face), **Cream**, **Steel**. What failed, fixed as
  little as it could be:
  - Cribrum **Petrol**: the orange 24° → 20° (19° from Chorda's gold, both lacquer).
  - Fragor **Cream**: brushed metal → printed card (paper), the hue kept — brushed metal near
    its brass is Aula's (46°) and Calor's; its brick second voice → a bottle green (the brick
    beside the brass spoke in Vetus's two voices and sat on Cribrum Brass's body).
  - Vetus **Walnut**: the brass 44° → 31°, an old gold (8° from Anulus's gold on wood).
  - Fragor **Steel**: passed as drawn.
  Pictures for the owner: `C:\projects\daw-fx\compartir\colourways\` (Slate EN and Paper ES at
  100 %, with a README).
- **The site**: `shoot_skins` shoots the five and `units_pictures.py` names them — the cards,
  the pages (EN and ES) and the README's windows and thumbnails, Fragor's now made like every
  other unit's; `check_units.py --fix` (Fragor, Libra, Vetus re-sized); each card's three preset
  excerpts (`render_unit_sounds`); `EFFECTS: [Unit; 28]` → 30 — **the web crate did not
  compile** (it is outside the workspace); now `cargo check` for wasm32 and with `ssr`, and
  `cargo fmt --check`, pass in `web/`. Trunk is not installed on this machine, so the release
  bundle and the prerender were not built (CI's `site.yml` and `site-budget.yml` do). The
  manual's site pages regenerated (`build-manual.ps1`) with the five sections.
- Found on the way, not changed: **Aula's three site excerpts** (`web/assets/units/sounds/
  aula-*.mp3`) no longer match what Aula renders today (every other unit's excerpt differs
  from a fresh render only by the version in its tag; the renders are deterministic). Aula's
  DSP did not change in this round: they predate round 39's last Aula changes. Re-render them
  (`render_unit_sounds`) at the merge.

## The suite (Windows, 2026-11-14, `fx/integration` at the close-out)

- `cargo test -p cantus-core -p cantus-dsp -p cantus-engine -p cantus-project -p cantus-plugins
  -p cantus-ui -p cantus-ai -p cantus-bench --no-fail-fast`: **5 909 passed, 0 failed, 94
  ignored** (508 test binaries and doc-test runs).
- The engine on three workers with the schedule fuzz (`CANTUS_WORKERS=3 cargo test -p
  cantus-engine --features fuzz`): **1 208 passed, 0 failed** (17 ignored).
- The null hashes (`cargo run -p xtask -- null-hashes --baseline`): **x86-64 = v2 = v3, 178
  items, every one the same bits** — each of the five at its defaults renders the dry signal's
  own hash (a null), and at its heavy settings the same bits on every level.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy -p cantus-plugins
  --no-default-features --all-targets -- -D warnings`: clean. `cargo fmt -p` on every crate
  touched; in `web/`, `cargo fmt --check`, `cargo check --target wasm32-unknown-unknown` and
  `cargo check --features ssr`.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh fx/integration clippy|test` with
  `CANTUS_CI_DIR=~/cantus-ci-fx` — the script fetches the local branch from the repository, so
  nothing was pushed): clippy with `RUSTFLAGS=-D warnings` clean, the plug-ins without their
  editor clean; `cargo test --workspace`: **6 949 passed, 0 failed, 109 ignored**.
- Not run: `cargo fmt --all -- --check` (fails on this machine with os error 206; `cargo fmt -p`
  per crate instead), the release bundle of the site (no Trunk here).
- The Windows build folder was deleted after the suite (C: had 49 GB free; the Linux build
  needed the room) and the WSL one after its run.

## Verified only in emulation (for `docs/beta-checklist.md` at the merge)

- **Every sound by measurement and rendered files**, not by playing live through an
  interface: nobody has played a guitar through Fragor (or Cribrum's wahs, Lapsus, Vetus) on an
  armed track, monitoring, at 4× (Fragor adds 1.1 ms). Steps: a guitar into an interface, the
  unit on the armed track, monitoring on, each starting preset, then the guitar sets in
  `compartir\<unit>\sound\guitar\` compared by ear.
- **The CLAP/VST3 exports** in the plug-ins crate's own host emulation (bit-identical to the
  engine, notes through both formats, Lapsus's banks posted without locks) — not yet in a
  third-party DAW (REAPER, Bitwig, FL Studio): open each of the five, its window, a colourway
  chosen in the ⋯ menu (kept for the session only), automation of one parameter.
- **The frame budget with the window open and audio playing** was measured on a loaded
  machine only (Fragor's UI pass 7.6 ms mean; Vetus 1.8–2.5 ms; Cribrum's and Lapsus's not
  measured): measure all five on a quiet machine against ADR 0110's budget.
- **Fragor's CPU per pedal** was measured while other builders compiled (upper bounds).

## Open for the owner

- **Listen**: each unit's guitar set and presets (`compartir\<unit>\sound\`), and whether each
  starting preset is the right first sound (Cribrum *Bar sweep*, Fragor's green overdrive,
  Lapsus *Half time*, Vetus *Worn cassette*; Libra inserts as a wire).
- **Look**: the colourways changed to keep the house rule (`compartir\colourways\`), Fragor's
  new Cream and Steel in particular; **Lapsus v3** (ice-blue controls, `compartir\lapsus\
  design\v3\`) has no recorded verdict yet.
- **Decide**: Lapsus's patterns count 4/4 bars from the song's start (a 3/4 song's bars are not
  followed) — follow the meter, or keep it as Gross Beat does; whether Lapsus gets colourways
  too (the mechanism is ready); Cribrum's *Cry wah* could close further between notes with
  `env.range` (12 dB), not rendered — he liked it as heard; Fragor's pedal sizes are a view
  preference of the session (keeping them in the project is a model change).
- **The site's `side/web` branch** (motion v1–v4, contrast to WCAG AA on every page; LCP
  1.96 s against the 2.0 s budget, measured on this busy PC): his eye on the motion, and the
  LCP measured again on a quiet machine before it ships.

## At the merge after round 40

1. ~~**Renumber**: schema 62 → the next after round 40's last; ADRs 0130–0133 → after round
   40's last.~~ — done at the final merge (`fx/final`): schema 62/63/64 → 72/73/74, ADRs
   0130–0141 → 0127–0138, every cross-reference with them (this file's numbers are the final
   ones); the table in `final-merge.md`.
2. **The golden ids** (`cantus-plugins/tests/golden/ids.txt`) are append-only: round 40's and
   ours both append — keep both, in merge order.
3. **The `side/web` branch's site work** (seven commits on top of `master` `dd4ad68b`, its
   account in that branch's `docs/round-fx/web.md`): merge it with this branch's `web/` changes (the cards, `units_data.rs`, the pictures, the
   excerpts); rebuild the site and run `site-budget.yml`'s checks (`trunk build --release`,
   the prerender, `wasm_budget.py`, Lighthouse, `tools/contrast.mjs`, `tools/motion.mjs`).
   Re-render Aula's excerpts (above).
4. **BACKLOG** (from `briefs.md` and the builders):
   - §6a: **the effects running live in the browser** — the units' DSP compiled to WASM in an
     AudioWorklet, the kit editor by egui on a canvas or a lighter control, the visitor's own
     file or microphone; measure first the module's size against the site's budget, CPU at
     128-frame quanta, whether `SharedArrayBuffer` (COOP/COEP on Cloudflare) is needed, and the
     output equal to the native render to the bit.
   - **web-fx — try the effects on the website** (`briefs.md`, asked 2026-11-13): a *Try it*
     player per effect on the reference guitar progression and one or two demo sources,
     rendered by Cantus through each unit and several presets, Bypass ↔ On on a shared clock,
     each preset with its window's picture; a render tool in the repository; the site's
     budgets, mobile, keyboard, both languages.
   - **Cantus cannot read 32-bit integer FLAC** (`claxon` refuses it: "header without bits per
     sample info"); the CC0 guitar samples are 32-bit and were converted to WAV to render. A
     user dropping such a file in Exemplar or the browser meets the same error. Decide for the
     sample loading path: another decoder for more than 24 bits, or a clear message.
   - Fragor's wave 3 (the pedalboard, the pitch unit, the rotary speaker, the amp —
     `briefs.md`), and reduced motion for the cables' energy when the app gets the setting.
5. **STATUS.md**: a round section for the side round, the header's test counts after the
   merged suite on Windows and Linux.
