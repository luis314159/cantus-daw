# Round 36 — integration: the professional studio

**The ask** (the owner, 2026-11-02, `docs/next-session.md` of round 35): BACKLOG §0.4 item 6 —
the recording mode (§4c whole, with the decisions of 2026-10-10), studio mixing (VCA faders,
mixer snapshots recalled whole or per channel and automatable as scenes), external instruments
and effects with an automatic latency ping, Mackie Control / HUI surfaces, an Edison-class
audio editor, and ARA 2 measured first and decided in an ADR. **Done when** a 64-track session
records at the lowest buffer without dropouts in the mode and leaves it identical, a VCA moves
its group with each channel's automation intact, a snapshot recalls exactly, an external synth
plays on time after the ping, a Mackie surface (or its emulation) drives the mixer, and the
audio editor cleans a take and returns it to the clip — and an MCP agent can use all of it.
Midway the owner added: sell it better on the landing page and the README (instruments and
effects with a click for detail and a bigger picture), and ship **macOS as a preview** in
beta.11 (decided 2026-11-03).

## How

- **Item 0 had nothing new**: no owner notes, the Mac session not done; `gh` is not installed on
  this machine, so the issues of `cantus-daw` and `macos.yml`'s FLAKY lines were not read
  through it (nothing was reported to the owner either).
- **The contract** `7393f1c2` on `r36/integration` (`docs/round36/BUILDING.md`: ownership,
  marked lines `// r36:<name>`, briefs, the code facts surveyed first — freeze's per-strip
  hash and edit lock were the foundation of the recording mode); ADRs 0092–0097 and schemas
  50–52 reserved; five empty locale areas registered.
- **Five builders** in `F:\projects\daw-r36\{record,mix,external,editor,ara}`, the slot script
  (three build slots, one WSL slot). **ara** researched first and reported a go/no-go before
  any code (ADR 0097: the SDK is Apache-2.0, compatible with GPL; go). A sixth, **showcase**,
  joined when the owner asked for the landing page and README.
- **Reviews, one agent each, read-only**: one per branch (five), one over all the fix rounds,
  one over the integrated tree. Every finding fixed with a test that fails without it.
- **What the reviews caught** (the reason they exist): external instruments that never sounded
  in the app although dozens of unit tests passed; HUI tables written from memory, wrong, with
  an emulation that shared the mistakes; a frozen bus deaf to its members' VCAs; the editor's
  cleanup able to delete another project's files in the same folder; an ARA partial render
  over the raw source; and, only visible once merged, mix's cues re-applying switches on frozen
  strips and doubling the armed track in the baked master. These taught the new working rules
  in `CLAUDE.md` (an end-to-end test per feature, protocols tested with outside bytes, small
  rounds with a mid-round integrated review, explicit staging, disk before a suite), approved
  by the owner on 2026-11-03.
- **API limits** cut two builders once; each was resumed with its context when the owner said
  *Reanuda*.

## What each part brought

| Part | ADR | Schema | What |
|---|---|---|---|
| record | 0092 | — | *Prepare to record*: every strip feeding the master rendered after inserts, sends and effect automation and before fader/pan, keyed by freeze's content hash (only changed strips re-render), streamed by the reader thread; armed strips live; master live or **baked** (hosted master plug-ins rendered, or the master stays live and is named); mix controls live (pre-fader senders keep fader and pan) or locked; everything else refused with the mode's notice through the bus's lock (execute, merge, rebase, undo, redo, controllers, surfaces, agents, the editor); never in the undo history, never in a saved file; arming a baked track brings it live without a re-render; refuses Link/clock/MTC following; external units, failed renders and regions a cue solos into stay live with their reason; the bar (buffer, round trip as the driver reports it, CPU before/now, Leave); low-latency monitoring forced and named; the reader serves every stream's next block first and runs above normal priority (`cantus_rt::thread::raise_io_thread`); `prepare_to_record`, `get_recording_mode`, `leave_recording_mode` |
| mix | 0093 | 50 | VCA strips (no audio; dB summed at the gain stage with the −96 dB floor as silence; nested and diamond VCAs counted once; cycles refused; mute/solo follow; a 0 dB VCA is a bit-exact null; ridden VCAs at the 16-sample control point); groups with a VCA; snapshots (state, never structure; all or named channels; recall-safe; update keeps a partial snapshot partial; A/B); **cues** — a snapshot at a bar compiled to automation holds and a switch timeline (mute, solo, polarity, swap, FX switch, bypass, EQ, the master's) with a 16-sample declick; solo-safe; 9 agent tools |
| external | 0094, 0095 | 51 | External instrument (MIDI route + return pair) and effect (send + return) units; the latency ping (burst train and note onset, to the sample, failures named, confidence measured) as the track's reported latency, re-based on a buffer change; held-note tracking, note-offs never dropped; offline renders never play gear (one choke point in the processor; export, bounce, freeze, video and bakes refuse; listen renders leave it out and say so); Mackie Control and HUI codecs, state machine and **virtual units**, tables tested with bytes from TouchMCU's notes and theageman's HUI document; one undo step per surface gesture; keep-alive, meter refresh, SysEx handshake; the track menu's *External instrument/effect…*; 4 agent tools |
| editor | 0096 | 52 | The audio editor from a clip and back: each edit a new content-addressed file in `Audio/<project>/` and one undo step; cut/paste/silence/fades/gain/normalise (peak, RMS, LUFS with a peak ceiling)/invert/reverse/DC/remove silence; joins at zero crossings with 3 ms equal-power crossfades; denoise by profile (`cantus_dsp::denoise`, bit-exact at amount 0); spectral view with region attenuation and a detail view at the native hop; declick (AR interpolation); paste across rates and channel counts; open and paste from file; drag a selection out as a clip; markers, regions and loop points saved with the source; *Remove unused audio* (asked first, siblings' files and pending agent edits safe); refused on ARA-bound clips and in the recording mode; 3 agent tools |
| ara | 0097 | 53 | New GPL crate `cantus-ara` (bindings from ARA_API 2.3.0 per architecture, layout tests, the document controller); an ARA-bound clip gets its own in-process VST3 instance as the region's playback renderer, rendered offline on a render thread (cancel, debounce, range re-renders only over a matching base, windowed memory, latency pre-roll, head/tail, the tempo map) and played through the engine's `SampleCache::set_clip_renders` (the engine stays `forbid(unsafe_code)`); the plug-in's editor from the clip; each edit gesture an undo step (`SetClipAraState`); crash/hang markers (the next start plays sources and offers Retry), *Play sources only*; offline renders wait for ARA to settle; `cantus-test-ara` counts every breach of ARA's call rules; 3 agent tools |
| showcase | — | — | The landing page: the recording mode (measured tiles, the bar, what it does not do), 32 instrument and effect cards each opening an accessible detail dialog (large picture in both themes, features, presets, A/B where an excerpt exists), the professional studio, platforms *Windows · Linux · macOS (preview)* with the macOS note; README: download table, the mode, the studio, an instrument and effect grid with `<details>` per unit; `shoot_showcase`, `shoot_skins --es` |

## Measured

- **The recording mode** (64 tracks, each EQ, compressor, delay and reverb; this machine):
  live 224–590 % of each block's time serial, 83–238 % on the pool — it cannot play at any
  buffer; in the mode 2–4 % mean; master baked 0.4–0.9 %. One-minute real-time runs, bench
  thread in MMCSS like the callback: **0 disk underruns** at 32–1024 frames with 64 and with 128
  streams; lowest buffer with no block over budget in a minute: 64 frames for 64 streams, 128
  for 128. Preparing 64 tracks of a one-minute song: 236 s.
- **Cues**: a full-desk cue on 64 strips compiles to 64 lanes (was 31 168 values) in 0.07 ms.
- **ARA** (debug, our test plug-in): 20 clips of 30 s over a 10-minute take — 231 MB of renders
  (a copy per clip would be 4.6 GB); a 1 s edit in a 3-minute clip heard after 31 ms.
- **Editor**: denoise leaves noise-only frames ≥ 12 dB quieter with speech-like content within
  1 dB; a mid-wave cut steps 0.1 raw, under 0.03 with smooth joins.
- **The external ping**: to the sample on a virtual wire at 32–1024-frame blocks.

## Checks

- Windows on `9acf6195` (the release tree): clippy clean workspace-wide (`-D warnings
  --all-targets`); the whole suite **6 291 passed, 0 failed, 89 ignored**; the engine fuzz
  (`CANTUS_WORKERS=3`) **1 085 passed**. One earlier run saw `agent_tools`' export overwrite
  refused once with *Access is denied* (a scanner holding the new file); three reruns passed —
  BACKLOG §8 *Writes that survive a scanner*.
- Linux (WSL Ubuntu 24.04, `~/ci-r36-integration`): the whole suite on `4188f15d` **6 253
  passed, 0 failed, 89 ignored**; clippy (and the plug-ins without their editor) and
  `cargo fmt --all -- --check` clean on `9acf6195` (the two commits after it touch only the
  site, two shoot examples and the version).
- macOS: the release runner builds the universal disk image and runs the suite.
- The integrator's own slips, caught before they shipped: a too-wide `git add` staged the
  owner's untracked `docs/monetize.md` into a merge (amended out); a merge union lost a brace
  in `rt_memory.rs` and a glyph the faces lack reached a locale (`font_coverage`); the main
  tree's full-debug `target/` reached 100 GB and failed the suite with nonsense errors.

## What is left — the owner's only

- **Hardware**: the lowest stable buffer on a real interface with and without the mode and with
  the master baked (ASIO and WASAPI, duplex, 10 minutes) and a take's timing; a hardware synth's
  ping and its timing in playback and export; an effect through a loopback cable; a real Mackie
  Control (handshake, motors, a two-fader ride) and a real HUI — all listed in
  `docs/beta-checklist.md`.
- **ARA**: write to Celemony about the name and logo; run `tools/ara/build-test-plugin.sh` once
  (Celemony's ARATestPlugIn — this session was not allowed to fetch and build third-party code);
  try a commercial ARA plug-in by ear. No plug-in is named as compatible anywhere.
- **Ears**: noise reduction, declick and the 3 ms joins on real recordings (against RX,
  Audition, ReaFIR); a VCA ride and a cue's switches in a real song.
- **macOS preview**: the Mac session (`docs/mac-session.md`) and the Apple account for signing.
- **Decisions open to the owner**: trim, a two-fader link and a second fader law left out
  (ADR 0093 §4); stems of an ARA-bound clip separate the file, not the plug-in's render (ADR 0097).
