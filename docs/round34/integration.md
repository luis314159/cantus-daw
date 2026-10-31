# Round 34 — integration: AI

**The ask** (the owner, 2026-10-28 and 2026-10-30, `docs/next-session.md` of round 33): AI,
in this order — **an MCP server** for Cantus, so Claude and other assistants drive it end
to end (tracks, clips, notes, the session's scenes, the mixer, effects and instruments with
their parameters in their units, automation, export), **each tool one undo step** through
the same `CommandBus`, with what BACKLOG §7 asks after round 28's demo (`audition` with
numbers back in one call, per-track analysis and a masking matrix, the music linter, the
machine-readable catalogue of every unit, the chunky tools) and what Cantus needs inside
for them (a range render from a saved state, analysis taps in the offline render, "live =
offline" checked); safe by design. Then **stems** by a model downloaded when needed, in its
own process, its quality measured (SDR); **audio to MIDI** (melody, polyphony, drums) with
the same care; and **the mastering assistant** on the master channel.

## How

- **Item 0 had nothing**: `macos.yml` and CI on public 07867bfd green with nothing retried,
  no issues on `cantus-daw`, no owner notes, the Mac session not done (beta.9 = Windows and
  Linux).
- Contract `b4568467` on `r34/integration` (`docs/round34/BUILDING.md`: ownership, marked
  lines `// r34:<name>`, briefs; the tool table's types in `cantus-ai/src/tool.rs` and the
  registry in `cantus-ui/src/ai_tools.rs`; ADRs 0081–0086; schema 48 reserved for stems).
  **The licence decision at the contract**: everything AI lives in GPL crates (`cantus-ai`,
  `cantus-mcp`, `cantus-ml`, `cantus-ui`); the MPL crates get only what is ours and generic
  (the range render, analysis taps, the harmony linter, onset detection).
- **Five builders** in `F:\projects\daw-r34\{mcp,tools,listen,stems,transcribe}`. `mcp`,
  `listen` and `stems` started at once; `tools` started from mcp's first delivery (the
  dispatch, the UI-thread pump, permissions, the in-process client — merged at 2b5f1790);
  `transcribe` from stems' first delivery (the helper process, ONNX Runtime, model
  downloads, one job end to end).
- **The slot script** (`F:\projects\daw-r34\slot.sh`): five builders on a 31 GB machine, so
  every cargo command and every WSL check queued for one of three build slots or the one
  WSL slot. Builders tested only the crates they touched (never `--workspace`) and deleted
  its WSL clone's `target/` when done.
- **Reviews, one agent each, read-only**: one reviewer per branch and one for every fix
  round — mcp six (10, 14, 15, 12, 13 and 10 findings), tools four (18, 11, 10, 4), listen
  three (12, 13, 6), stems three (10, 16, 6), transcribe one (11) — and one on the
  integrated tree (**13 seams**). Every finding fixed with a test that fails without it
  (where a test could not fail first — a new API, a measurement — the builder said so).
- **The integrated review's 13 seams**, each fixed by its owner with a test: an agent's
  jobs stop when another project opens (they ran on, and only their edit was refused); a
  name made only of joiners or invisible characters is no name; an agent's Pulsus kit and a
  converted drum track carry the interface's pad names; the ML helper built and checked in
  every CI workflow that builds the MCP bridge; every registered tool's input schema closed
  to the bottom and every job that can end in an edit of a mutating class, checked over the
  whole registry; an agent's `audio_to_midi` and `separate_stems` hold the helper in use from
  prepare to the job's end (one counter, the service's); session clips refused by the
  conversion in words that say why; the ML helper and its Visual C++ runtime moved into
  `ml\` (below); an agent's stems removed when its edit is not made; a model being removed
  cannot start; an agent's separation reads the audio folder when its edit is planned (a
  *Save As* mid-job); `LICENSE` and the README's crate table name `cantus-ml`;
  `font_coverage` scans the models' manifest.

## What each part brought

| Part | ADR | What |
|---|---|---|
| mcp | 0081 | `crates/cantus-mcp` (GPL): the MCP protocol written by hand (JSON-RPC 2.0 over std threads, no async runtime) for both generations — the session one (2025-06-18, 2025-11-25) and the stateless 2026-07-28; the UI-thread pump with `CantusApp` as host, each planned command run with `Origin::Mcp` and named "AI: …" in the history; permissions by class in Cantus (reads and transport run; edits ask; destructive and file tools always ask; files off until allowed); jobs with handles, `wait_job`, `cancel_job`, `begin_change`/`end_change`, `undo_last_ai_action`; the HTTP door on `127.0.0.1` only, off by default, a bearer token per client; the `cantus-mcp` stdio bridge that proves the door on every connection; owner-only secrets in `<settings>/mcp/` (`cantus-rt/src/private.rs`); the confirmation card, the agent chip, the AI activity panel, Settings ▸ AI with *Copy setup for…* Claude Code, Claude Desktop, Cursor, VS Code, Gemini CLI and LM Studio, and a Claude Desktop extension (`.mcpb`); one off switch; the bridge in the installer, zip, AppImage, Flatpak and macOS bundle |
| tools | 0082 | 47 tools of the project and the application in one table (read, the song, tracks and instruments, clips and notes, drums, the session, the mixer and effects, automation and modulators, presets, comping, export, removal), each mutating call one undo step whose undo gives the project back exactly; the chunky tools `write_part`, `voice_chords`, `pedal_from_chart`, `automate`/`replace_automation`; the unit catalogue generated from the parameter tables (`list_units`, `get_unit`, `cantus://units`); the music linter and harmony in `cantus-core/src/harmony*.rs` (MPL); resources and five prompts; the legacy bridge now the assistant panel's door to the same table |
| listen | 0083, 0084 | Speculum's headless ear; `cantus_engine::listen` — the export's render with analysis taps that change no sample; **range renders from checkpoints** (`Processor::fork`, every unit forkable) equal to the whole render to the bit, also after a later edit; live = offline checked on the application's own engine; `audition`, `analyse_tracks`, `masking_matrix`, `set_mix_targets`; **the mastering assistant** (`master_assist`, `apply_master_chain`, `replace_master_chain`, the prompt *Master for streaming*, the panel from the master strip's menu): nine platforms with their sources, six genre curves or a reference track, a chain of our own effects with every setting and why, A/B at matched loudness, *Apply* as one undo step |
| stems | 0085 | `crates/cantus-ml` (GPL) and its `cantus-ml` helper process (a crash, a hang or a GPU failure never takes Cantus down); **ONNX Runtime** chosen by measurement, downloaded on demand with its telemetry off; models through the content manager (licence shown and accepted, resumable, hash-checked, removable; HT-Demucs's weights rebuilt from Meta's own file, Cantus hosts nothing); *Separate stems…* on a clip or track (4 or 6 stems, *Fast* or *Best*) into a bus and a track per part in one undo step, the parts summing to the original under every pan law; provenance in the project (**schema 48**, `SetAudioProvenance`) and an "AI" badge; Settings ▸ Models; `separate_stems` for agents |
| transcribe | 0086 | *Convert to MIDI…* on an audio clip and `audio_to_midi` for agents: **polyphony** by Spotify's Basic Pitch with its note creation ported to Rust decision for decision; **melody** by Cantus's own pitch tracker (measured better); **drums** by our own detector (`cantus-dsp/src/onset_drums.rs`, MPL) and, optionally, Magenta's Onsets and Frames Drums; into a new track through the clip's own time mapping (tempo, warp, slip, reverse, loops), one undo step |
| Showcase | — | README: *AI that works with you* (six pictures); the landing page: *AI that works with you, on your computer* (EN/ES); manual pages EN/ES for agents, the tools, listening, stems, audio to MIDI and the mastering assistant |

## Measured

- **MCP with real clients** (no model tokens spent): MCP Inspector CLI 0.17.2 over HTTP and
  through the stdio bridge — `tools/list` 60 tools, calls answered, a wrong token refused;
  `claude mcp add` (HTTP and the bridge) then `claude mcp list`: both "✓ Connected".
  `tools/list` is held by a golden file (63 tools on the integrated tree).
- **The music linter**: 24 000 notes over 1 600 chords in under 40 ms (160 ms before review
  1); the tools' fuzz, 64 seeds × 80 random calls over 22 kinds, all undone and redone
  exactly.
- **Listening**: the showcase song (169.7 s) rendered whole with 10 checkpoints in 144.3 s
  (round 28: 157–217 s without); its last section from the checkpoint at bar 73, 20.7 s of
  audio in **14.2 s**, identical to the whole render. A checkpoint's memory: 80 MB whatever
  the song (a fork copied every cell of 256 tracks and 256 strips; 2.8 GB peak at the
  20-minute cap) — being fixed: an image keeps only the cells in use (6 MB for a one-track
  song) and the checkpoints keep to a 512 MB budget (`r34/listen`, a46c98b2).
- **The mastering assistant**: 7 demo songs × Spotify / Apple Music / Club (21 cases),
  rendered for real after applying — worst **0.07 LU** from the target, true peak **≥ 0.09
  dB under the ceiling** in every case, the prediction equal to the real reading to 0.01.
- **Stems — SDR on MUSDB18-HQ's 50 test tracks**, scored as the paper does (BSS Eval v4,
  museval): drums **10.05**, bass **9.78**, other **6.42**, vocals **8.93**, average
  **8.80 dB** — the HT Demucs paper's numbers to their two decimals. Our stems against
  PyTorch's on the same song 63–90 dB; *Best* against `apply_model(shifts=…)` 70–90 dB. A
  3-minute song (4 stems, *Fast*, processor) in 65 s at 2.66 GB peak (PyTorch 59 s, 1.76
  GB); the 50 test tracks at 1.96× real time; a cancel inside a real run stops in 34 ms.
  First download 99 MB on Windows, 96 MB on Linux (runtime and model). DirectML refuses the
  graph on this machine's Radeon 680M, so separation runs on the processor (ADR 0085 §5).
- **Audio to MIDI**: Basic Pitch in Cantus against Spotify's `basic_pitch` 0.4.0 on 19
  TRIOS files — posteriorgrams bit-identical, **3 286 of 3 286 notes identical**, 47–73×
  real time. Melody (note F-measure): our tracker 0.805 on melodies rendered through our
  instruments and 0.769 on TRIOS's solo stems, against Basic Pitch's 0.746 and 0.699.
  Polyphony (onsets only): 0.609 rendered, 0.703 on TRIOS. Drums on MDB Drums' held-out
  tracks, pooled: our detector 0.691, with the drum model **0.811** (kick 0.963, snare
  0.885, hi-hat 0.738, cymbals 0.640, toms 0.442); on full mixes 0.611, and **0.795** with
  the drums separated first. An hour of take: the interface's reader 15 MB (was
  0.97–1.27 GB), converted in 1.4 min (notes) and 3 min (drums) in a debug build.

## Causes found, not waits

- **mcp's flaky change tests**: three timing races closed by ordering, not by longer waits
  (cdd75024). `review3::a_change_stays_open_while_its_job_runs` set the change's idle time
  to zero before the job's call reached the pump, so a frame could end the change first;
  the idle time is now zeroed only once the job runs.
- **listen's pre-roll that would not converge**: a range no checkpoint covers pre-rolls,
  and on the showcase 2–16 s of pre-roll stayed 1.4–2.5 dB from exact. Measured unit by
  unit (`preroll_by_unit`): **Space's FDN modulation rotors** (8.3 dB) and **Organum's
  rotary** (−1.4 dB) turn from their reset, not from song time. No DSP change this round
  (the integrator's decision); ADR 0083 says so, a listen says when it is not exact, and
  BACKLOG §4b has *free-running unit state anchored to song time*.
- **The 80 MB checkpoint**: a fork of the whole engine copied every track and strip cell
  whatever the song used (review 3's measurement); being fixed by keeping only the cells
  in use, under a byte budget (above).
- **The drum model's dead hi-hat**: Magenta's 8-hit map folds every hat into pitch 46, but
  the third-party ONNX conversion's output `Gather` read pitch 42. Exporting all 88 outputs
  proved 46 carries the hats; the content manager patches that one index in a copy at
  install (exact bytes, found once, the copy's SHA-256 checked). Hi-hats with the model went
  from the detector's 0.647 to 0.738.
- **The Visual C++ DLLs that shadowed other programs**: shipped beside `cantus.exe`, every
  program of the install folder loaded our `vcruntime140.dll` first, an AI app's
  long-running MCP bridge held them during an update, and plug-ins found our
  `msvcp140.dll` before the system's. `cantus-ml.exe` and its runtime now live in `ml\` of
  the install folder.
- **Linux deletes open files, Windows refuses**: stems' feature tests shared the process's
  `CANTUS_DATA_DIR`, and one test's ending deleted a folder another still wrote to; each
  test now has its own folder.
- **An hour of drums failed as "hung"**: the detector ran silent longer than the client
  waits; each factorisation step now says it is alive (a test fails without it).

## Results

On `48188b83` (every branch merged), on a quiet machine, with `RUSTFLAGS=-D warnings`:

- **Windows**: `cargo test --workspace` 5 590 passed, 0 failed, 82 ignored; the engine fuzz
  (`--features fuzz`, `CANTUS_WORKERS=3`) 985 passed; `cargo clippy --workspace
  --all-targets` clean; `cargo fmt -p` clean for every package.
- **Linux** (WSL Ubuntu 24.04, `tools/wsl/check.sh`, a clone of its own): clippy clean; 5 574
  passed, 0 failed, 82 ignored.
- **macOS**: by the runner at publish (`macos.yml`).

## What only the owner can do

- **A clean Windows** (a machine or VM that never had the Visual C++ redistributable): a
  separation and a conversion run — the DLLs in `ml\` are found by the loader's order, but
  this machine has the redistributable system-wide.
- **The Mac session** (`docs/mac-session.md` row 17 and later): the MCP bridge and the ML
  helper inside the signed bundle (library validation off for the helper), a separation on
  CoreML and on the processor with the time of each.
- **Signing `cantus-mcp.exe`** (and `cantus-ml.exe`) with the rest of the programs.
- **Claude Desktop installing the `.mcpb`** extension (Claude Desktop is not on this
  machine; the manifest and the zip are unit-tested).
- **Ears**: what `write_part`, `voice_chords` and `pedal_from_chart` write on Chorda, Lamina
  and Gravis; the mastering assistant's masters on real material and its A/B (the curves
  are guides from published averages, not anyone's ears).
- **Your own recordings into MIDI**: a sung line, a piano, a drum loop — do the notes feel
  right?
- **A real FAT32 or exFAT USB stick**: an agent's export onto it (no hard links there: the
  no-replace rename) and Cantus's settings on it (a drive that keeps no owner is refused in
  words).
- **The legal review** gains Spotify's Basic Pitch (Apache-2.0) and Cantus's port of its
  note creation, the third-party ONNX conversion of Magenta's drum model (Apache-2.0 per
  its card), Meta's HT-Demucs (MIT; no separate licence stated for the weights) and the
  Visual C++ runtime's redistribution terms — beside what rounds 30–33 left (model names,
  Chorda, the CLA, Concentus's names).
- Everything rounds 30–33 left (`docs/STATUS.md`): the hardware of round 33, the Apple
  account, a real Linux desktop, Flathub, code signing, the RVR1960 permission, the
  trademark filing, the UMC in duplex; REAPER and FL Studio for the comparisons.
