# Round 20 briefs

## automation — automation clips and lanes (ADR 0027, schema 19)

The bar: FL Studio's automation clips (a clip in the playlist whose curve drives a
parameter: points with tension, step/slide modes, the articulator tools — flip, scale,
smooth, LFO/"create sequence"), and Ableton/Bitwig/Cubase lanes under a track. Decide the
model in ADR 0027 (the playlist plan in STATUS called automation clips "take" and FL's
articulators "worth stealing wholesale").

- **Model** (cantus-core): what can be automated — every effect parameter (strips and the
  master, `StripId::MASTER`), native instrument parameters (`NativeSpec` keys), Firmus
  params, track gain/pan, strip gain/pan/width, sends, master gain, tempo only if it
  falls out cleanly (a tempo track is a later block — say so). A target is stable across
  saves (ids and parameter keys). Points in ticks with value and curve tension; a clip
  window like any clip (reuse `Clip`'s window machinery if it fits: move, split, trim,
  loop). Commands with inverses, undo fuzz arms; schema 19 with a migration (writes
  nothing) and fixture `v19.cantus`.
- **Engine**: sample-accurate (or on the 16-sample control grid the effects already
  glide on — decide and justify) evaluation of every active automation in the snapshot;
  no allocation; offline render equal to real time; block-size independence; a parameter
  with no automation renders exactly as before (null test). When automation is present
  the parameter's knob follows it (the UI reads the automated value) and a manual move
  while playing is either overridden or latched (decide; FL overrides).
- **UI**: *create automation clip* from any parameter's right-click (knob, fader, effect
  editor, instrument editor — one shared entry through the kit's `Param`), and from the
  last-touched parameter (FL's "Tools ▸ Last tweaked ▸ Create automation clip"); the clip
  in the arrangement drawn as its curve; an editor (in the clip, and a larger one in the
  piano-roll area) to add/move/delete points, drag tension, step/slide, select and scale;
  articulators: flip vertical/horizontal, scale, smooth, randomise a little, LFO shapes;
  the automated knobs show it (a ring or a dot). Render and look.
- **Tests**: null; block-size independence; render = real time; a ramp on a fader lands
  the expected gains; save/open; undo of every edit; the knob shows the automated value.

## device-ux — device windows that are easy to close, and swapping devices

- **Window chrome** for every device window (instruments, Pulsus II, effects, hosted
  plug-ins — `views/workspace_device.rs`, the instrument frame `views/inst/frame.rs`, the
  effect frame `views/fx/frame.rs`): obvious **close (×)**, **minimise** (collapse to its
  title bar or to a dock chip you click to bring back), and **maximise / restore**, drawn
  with the kit, with hover states and tooltips, in the title bar where a musician looks
  (FL's plug-in windows: close, minimise, a detach, a menu). Esc closes the focused
  device window; a key toggles the selected track's instrument window (F8 or what the
  keymap has free — check `keymap.rs`). Remember size/position per device. Today the
  owner finds it hard to know you must unfold an option — remove that friction.
- **`apply_pick`** (owned by you, `crates/cantus-ui/src/picker.rs`): *ReplaceInstrument*
  keeps the track, its clips, notes, strip, inserts, name (unless it was the default
  name) and colour, as **one undo step**; offers to keep matching parameters where kinds
  share a model; a kit → melodic swap keeps notes. *ReplaceEffect* keeps the slot's
  **position**, mix, bypass and sidechain/MIDI source where the new kind takes them, as
  one undo step (a new core command `ReplaceEffect { strip, effect, slot }` with an
  inverse is cleaner than remove+insert — add it and its fuzz arm). Hosted choices go
  through `cantus_host` (effects exist; instruments come from the hosted-inst builder —
  call its API if present, else leave a clear TODO and a disabled entry).
- **Entry points** that open the picker (`app.open_picker(...)`): a **swap button** in
  every device window's title bar; the track header's instrument chip (click → replace);
  the channel rack's channel button; the inspector's instrument row; every mixer insert
  slot (click empty slot → AddEffect; right-click filled → *Replace…*); the inspector's
  insert tiles; *Add track…* → AddInstrument. Drag a device from the picker onto a
  slot/track is a plus if the browser builder exposes drag payloads (coordinate via the
  types in `picker.rs`).
- Tests: close/minimise/restore through published geometry; replace instrument keeps
  clips and is one undo step; replace effect keeps position/mix/bypass and is one undo
  step; every entry point opens the picker with the right purpose.

## browser — every device on one screen

The bar: Bitwig's and Logic's device browsers, Native Instruments' Komplete Kontrol / NKS
browser (a grid of instrument tiles with artwork), FL Studio's plugin picker (Plugin
database ▸ a grid of plug-in thumbnails), Ableton's browser categories. The owner: *"you
can see every available instrument on screen to choose — it looks very, very pro."*

- `views/picker*.rs`: a large overlay or window (the kit's look) with a **grid of tiles**,
  one per device: native instruments (Firmus, Pulsus, Pulsus II and the eight), native
  effects (all, including Fervor, Latitudo, Culmen), and hosted CLAP/VST3 plug-ins from
  the scan database (`cantus_host`), each with a **drawn thumbnail** — for native devices
  a miniature of the device's real look (render the editor once to a texture through
  `cantus_ui::raster`, or a vector miniature in each device's hue — decide; cache them),
  for hosted ones a generated card (vendor, name, format badge, category glyph).
- Left: categories (Instruments — Synths, Keys, Bass, Drums, Samplers; Effects — EQ,
  Dynamics, Space, Distortion, Stereo, Mastering, Utility; Hosted; Favourites; Recent),
  a search field (name, vendor, category words), sort; a details pane (description from
  the device docs, its presets list, format, vendor) and a **preview** for instruments if
  feasible (play a short phrase through the device offline — optional).
- Opens for every `PickPurpose`, filters instruments/effects by it, keyboard navigation
  (arrows, Enter, Esc, type to search), double-click or Enter applies through
  `apply_pick`; favourites and recents persisted in the user's data (not the project).
  Also reachable as a browsing panel from the workspace (View ▸ Devices) with drag
  payloads onto tracks/slots if device-ux wires drop targets.
- Write ADR 0029 (the picker and swapping) with device-ux adding its section.
- Tests: the picker lists every native kind and every scanned plug-in; filter/search;
  keyboard; apply calls `apply_pick` with the right choice; thumbnails cached; frame
  budget for 60+ tiles. Render it (instruments and effects views, both themes) and look.

## hosted-inst — CLAP/VST3 instruments on tracks (ADR 0028)

STATUS (round 17) left it: *"Hosted CLAP/VST3 instruments on tracks: an adapter from
`cantus_host::AudioThread` to `InstrumentUnit`, on `cantus-host` — never a second host."*

- Model: an instrument spec for a hosted plug-in (`InstrumentSpec::External(...)` reusing
  `ExternalPlugin` from the effect side: format, id, name, vendor, state, params) — schema
  19 on your branch (see BUILDING: the integrator renumbers). A missing plug-in opens and
  keeps its data, plays silence, and says so.
- Engine/host: the adapter; notes (with velocity, note-off, all-notes-off on stop/seek),
  sample-accurate within a block; MIDI CC/pitch bend; transport/tempo into the plug-in;
  latency into delay compensation; state save/restore with the project; offline render;
  the plug-in's own editor window opening from the track (device-ux's chrome).
- **Prove it with real, free, open-source instruments.** Find and download a few CLAP
  and/or VST3 instruments with licences that allow it (e.g. Surge XT, Dexed, OB-Xd,
  Odin 2, Vital's open fork Vitalium, Geonkick, sfizz — check each licence and what the
  official binaries are), install them into a folder of your own outside the repo (e.g.
  `C:\projects\daw-r20\plugins\`), scan them with `cantus_host`, and write tests (ignored
  by default, driven by an env var pointing at the folder) that load each, play a chord,
  and assert non-silent, finite output and a state round trip. Record what worked, what
  did not and why in `docs/round20/hosted-inst.md` (a compatibility table).
- The picker's `PickChoice::Hosted` for instruments goes through your API; give
  device-ux a function to call (`CantusApp::set_hosted_instrument(track, PluginInfo)` or
  similar) and say so in your report.
- Tests (always-run) with the repo's test plug-ins (`crates/cantus-test-plugins`; add a
  tiny test instrument there if needed): notes in → audio out, block-size independence,
  offline = real time, state round trip, a missing plug-in.

## research — content: free instruments and rights-free samples (research only)

No code. Write `research/content/README.md` plus one file per topic, with sources and
dates, for the owner to decide later:

1. **Rights-free samples** (drums — kicks, snares, hats, claps, percussion, one-shots —
   plus loops, instruments and FX): where to get them with licences that allow bundling
   or redistribution inside a GPL app (CC0 / public domain first; CC-BY and its
   attribution duty; "royalty-free" licences that forbid redistribution as samples —
   flag them). Candidates to check: Freesound (CC0 filter), the Salamander/other free
   piano and drum sets (licence of each), Musical Artifacts, archive.org collections,
   Philharmonia (licence), the University of Iowa MIS, SampleSwap, 99Sounds, Bedroom
   Producers Blog free packs, Splice/Cymatics "free" packs (licence terms), Hydrogen's
   drumkits (licences per kit), DrumGizmo kits, SFZ/sfizz community libraries, VSCO 2 CE
   (CC0), the Versilian Community Sample Library, and anything better you find. For each:
   licence, size, quality, formats, whether redistribution is allowed.
2. **Where to keep them** (not in the repo): Git LFS vs GitHub Releases assets vs a
   separate content repo vs a CDN/object storage with a manifest and checksums; a
   download-on-demand **content manager** in the app (first-run "download the factory
   library", per-pack install, versioning, integrity, offline installers); how FL
   Studio, Ableton (Packs), Bitwig (Package Manager), Reaper and MuseScore (Muse Sounds)
   do it; size budgets; what to ship inside the installer (a small core kit) vs on
   demand. Recommend one design with costs.
3. **Free instruments to preload**: open-source and freeware CLAP/VST3 instruments,
   their licences (GPL/MIT/proprietary freeware), whether a GPL DAW may bundle or
   download them (aggregation vs linking), trademark issues (e.g. Vital vs Vitalium),
   platforms, quality, and which would best complement Cantus's eight native instruments
   (orchestral, piano, guitar, drums).
4. Recommendations, in priority order, with the legal points the owner must decide.

## research-ai — AI in Cantus: MCP, LLM providers, generative audio (research only, backlog)

The owner (2026-10-05): *"add to the backlog AI features — MCP features to do things in our
DAW with an LLM, integration of generative audio AI, and direct AI integration (the APIs of
the best-known providers, both LLMs and audio generators)."* No code. Write
`research/ai/README.md` plus one file per topic, with sources and dates:

1. **What exists in Cantus**: read `crates/cantus-ai/` (`AiProvider`, the tool catalogue,
   permissions, the command bridge — "no live provider yet"), ADR 0004 (every mutation is
   a reversible `Command`, which is what makes AI edits auditable and undoable) and
   `idea.md`'s AI sections. Say what is reusable as is.
2. **An MCP server for Cantus**: the Model Context Protocol (current spec, transports —
   stdio, streamable HTTP), which tools/resources/prompts a DAW should expose (read the
   project, add tracks/clips/notes, set parameters, mix, render, analyse loudness, explain
   a mix), permissions and confirmation, undo as one step per AI action, how Claude
   Desktop / Claude Code / other clients would connect, the Rust MCP SDKs available.
   Prior art: DAW MCP servers that exist (Ableton, Reaper, Logic, FL), what they expose.
3. **Direct LLM integration** inside the app (an assistant panel): the providers' APIs
   and current models — Anthropic (Claude: the current family is Claude 5 — Opus 5.5
   `claude-opus-5-5`, Sonnet 5.5 `claude-sonnet-5-5`, Haiku 4.5 — verify on the official
   docs; tool use, streaming, prompt caching), OpenAI, Google Gemini, Mistral, and local
   models (Ollama, llama.cpp) for offline/private use; API-key storage on the user's
   machine, cost, latency, privacy; a provider-agnostic design behind `AiProvider`.
4. **Generative audio**: APIs and models for music, stems, loops, one-shots, sound
   effects and voice — e.g. Stability AI (Stable Audio), ElevenLabs (sound effects,
   music, voice), Google (Lyria / MusicFX), Suno and Udio (official API status), Meta
   AudioCraft/MusicGen and other open models runnable locally, stem separation (Demucs,
   etc.), audio-to-MIDI, AI mastering. For each: access (API/local), licence of the
   model and **of the generated output** (can a user release it commercially?), cost,
   quality, latency, formats, and how it would land in Cantus (a generated clip into the
   audio pool, a sample pad, a stem split into tracks).
5. **Recommendations**: a phased plan (MCP first? the assistant panel? generation?), the
   legal/ethical points the owner must decide (training-data provenance, output rights,
   disclosure), and what fits the GPL licence.
