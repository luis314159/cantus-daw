# ADR 0085 — Machine-learning models: a helper process, ONNX Runtime, weights from their makers

**Status:** accepted (round 34, stems builder: first delivery, then the feature) · **Date:** 2026-10-30 ·
**Extends:** ADR 0030 (content on demand), ADR 0042 (licences of content), ADR 0047
(licences: AI in GPL crates), ADR 0063 (helper processes) · the helper's `PROTOCOL` 1 ·
model manifest format 1 · schema 48 (`Project::provenance`)

## Context

Round 34 brings stem separation and audio to MIDI (BACKLOG §7, `research/ai/
generative-audio.md` §4–5). Both run neural networks of tens of millions of parameters
for a minute or more per song. Three questions had to be answered before either feature:
where the networks run (not in Cantus's process: a crash, a hang or a graphics driver's
failure must not end a session), on what inference runtime, and where the weights come
from (never the repository or the installer, ADR 0030; only licences that allow
redistribution and commercial use of the output, ADR 0042).

## Decision

### 1. A helper process, `cantus-ml`, with jobs over its pipes

`crates/cantus-ml` (GPL) holds the helper binary and its client:

- **Protocol** (`proto.rs`): JSON lines on the helper's standard pipes. Requests `start
  {id, job, args}`, `cancel {id}`, `quit`; events `hello {protocol, version, jobs}`,
  `started`, `progress {done 0–1, stage}`, `done {result}`, `failed {error}`,
  `cancelled`, `log`. A job's arguments and result are its own JSON (`spec.rs`), so a new
  job adds no message. Lines are bounded (1 MiB): audio never crosses the pipe, files do.
- **The helper** (`helper.rs`, `main.rs`): its standard handles moved off the pipes before
  any runtime loads (a library that prints cannot break a line), no crash dialog, an end
  with Cantus (the job object on Windows, the parent-death signal on Linux, a kqueue watch
  on macOS — as the plug-in sandbox, ADR 0063). Jobs run one at a time on a worker thread
  from a **table** (`jobs.rs`: a name and a function); a cancel sets the job's flag, which
  the job checks between steps and which tells ONNX Runtime to stop inside a run
  (`RunOptions::terminate`).
- **The client** (`client.rs`, `MlClient`): starts the helper on a thread of its own when a
  job first needs it — never on the interface's thread, never near the audio thread — and
  sends one job at a time from its queue. A helper that ends while a job runs fails that
  job as `Crashed` (how it ended, as the system says); one silent for 120 s while a job
  runs is ended and the job fails as `Hung`; one that does not greet in 30 s, is missing, or
  speaks another protocol fails the job as `Start`/`Protocol`. The next job starts a fresh
  helper; queued jobs are not lost. `poll()` never blocks; `wait()` is for workers and
  tests.

### 2. ONNX Runtime, its library downloaded on demand

Measured on this machine (AMD Ryzen 7 7735HS, 8 cores, 16 threads, Windows 11; two other
builds running, so absolute times are pessimistic and the ratios are what count), one
HT-Demucs segment (7.8 s of stereo audio), the same graph:

| Runtime | One segment | A 3-min song (31 segments) | Peak memory | Ops covered | On every system |
|---|---|---|---|---|---|
| **ONNX Runtime 1.31.1** (`ort` 2.0.0-rc.15, `load-dynamic`) | **1.80 s** | **~56 s** (65 s end to end in the helper, all included) | 2.8 GB (5.2 GB with the memory-pattern planner on) | all | Windows, Linux, macOS arm64 (1.31.1) and Intel (1.23.2); GPU providers exist (DirectML, CoreML, CUDA) |
| tract 0.23.9, 8 threads (`multithread-mm`) | 4.39 s | ~136 s | 0.87 GB | all | everywhere, pure Rust, no GPU |
| tract 0.23.9, 1 thread | 9.39 s | ~291 s | 0.87 GB | all | |
| candle-onnx 0.11 | — | — | — | not built: its build needs `protoc`, a tool no other part of Cantus needs; candle's real path is a hand port of the network (as `demucs-rs` does on Burn), a second implementation to keep in step | |
| *PyTorch 2.15 (Meta's code, the reference)* | *~1.9 s* | *59.1 s* | *1.76 GB* | | |

ONNX Runtime is **2.4× faster than tract** and as fast as Meta's own PyTorch code; tract's
only advantages are memory and having no native library. A user waits about a minute
instead of two and a half, so **ONNX Runtime is the runtime**. tract also differed from
ONNX Runtime by 59 dB SNR on the same input, where ONNX Runtime matches PyTorch at
100 dB.

- **The library is downloaded on demand**, its own SHA-256 checked by the helper before it
  loads it (and the model's graph and weights before their session: a file planted in the
  user's folder is never run), through the content manager with the first
  model (pinned version, size and SHA-256, ADR 0030), and loaded at run time from that
  file (`ort`'s `load-dynamic`): nothing is linked or downloaded at build time, the build
  is one for every system, and a machine without the library has no models — nothing else
  changes. Sources: Windows — Microsoft's own build from its Python wheel on PyPI
  (content-addressed, 14.9 MB; the C package from GitHub is 83 MB, of which 430 MB
  unpacked are debug symbols), whose `onnxruntime.dll` is the C library; Linux and macOS —
  Microsoft's C packages from the GitHub release (11.4 MB, 36.9 MB). Intel Macs get
  1.23.2, the last release built for them, and the crate is compiled against API 23 so
  either version loads.
- **Telemetry stays off.** Microsoft's official builds send usage events unless
  `ORT_DISABLE_TELEMETRY=1` is set before the library loads (its `Privacy.md`): the client
  sets it in the helper's environment, the helper sets it again in its own, and the
  environment is built `with_telemetry(false)`.
- **Sessions**: graph optimisation at its highest level, the memory-pattern planner off
  (5.2 GB → 2.8 GB at the same speed), the runtime's own thread count (the physical
  cores). The helper's whole peak on a 3-minute song: **2.66 GB**.
- **GPU**: the protocol has `device: cpu | auto`; `auto` falls back to the processor and
  says so in a `log` event. The graphics runtimes (DirectML on Windows, CoreML on Apple
  silicon) come with the feature's delivery: the numbers above are the processor's.

### 3. HT-Demucs, its weights from Meta, its graph ours

The model is **HT-Demucs v4** (MIT code and weights: the repository's README releases
Demucs under MIT, licence file pinned at commit e976d93e), 4 stems, and its 6-stem version
(guitar and piano; its maker calls the piano weak). Better-scoring separators (BS- and
Mel-RoFormer) have community weights under CC BY-NC-SA, which ADR 0042 refuses.

- **The graph** is exported by `crates/cantus-ml/tools/export_htdemucs.py` from Meta's
  own `.th` with PyTorch's dynamo exporter, **the network only**: the STFT, its inverse
  and the sum of the two branches are Cantus's (`stft.rs`, `demucs.rs`), as the original
  code computes them around the network. 2 087 nodes, 0.85 MB without its weights, against
  the 24 765 nodes and 316 MB of the one community export on Hugging Face (StemSplit's,
  whose STFT is convolutions in the graph: 2.4 s a segment, 5.1 GB at load). The export is
  deterministic (byte-identical twice) and checked: the whole model as Cantus runs it
  against PyTorch's `HTDemucs.forward` on the same segment, **1.9 × 10⁻⁵ largest
  difference, 100.6 dB** (6 stems: 1.6 × 10⁻⁵, 90.7 dB).
- **Cantus hosts nothing.** Every weight in the graph is *external data* in
  `<name>.weights`, in half precision as Meta's `.th` stores it, cast to float32 inside
  the graph. The graphs (gzip, 0.39 MB and 1.27 MB: no weights in them, only the
  exporter's own small constants — shapes and sine positions) are compiled into
  `cantus-content`; at install the content manager downloads **Meta's own file** from
  `dl.fbaipublicfiles.com` (84 MB, its SHA-256 pinned) and the licence, and Cantus
  **rebuilds `<name>.weights` from it** (`models_th.rs`: the `.th` is a zip; a pickle names
  each tensor's storage, offset, shape and strides — `models_pickle.rs` reads just enough of
  pickle, never resolving or calling anything — and a map from the export says where each
  tensor goes). The rebuilt file's hash is checked against the export's: it is byte for
  byte the file the export was verified with. Meta's file is then removed (the weights are
  in place).
- **Faithfulness end to end**: a 3-minute track separated by the helper against Meta's
  `demucs.apply.apply_model` (shifts 0, overlap 0.25) on the same file: drums 84.9 dB,
  bass 80.8 dB, other 82.9 dB, vocals 65.6 dB signal-to-difference, largest difference
  5.9 × 10⁻⁴ (the demo songs have no vocals: that stem's level is near silence). The 6-stem
  model on the funk demo: 63–86 dB. The spectrogram and its inverse match PyTorch's to
  2 × 10⁻⁴ and 1 × 10⁻⁵ (`tests/stft_reference.rs`), the track's ends included, where
  demucs's own inverse is not the input (`_spec` drops two frames there that `_ispec` puts
  back empty) and Cantus reproduces that as it is.

### 4. Models through the content manager

`cantus-content/src/models.rs`: a manifest compiled in (`manifest/models.json`, format 1)
lists the runtimes per system and the models, each with its licence (SPDX, an allow-list:
MIT, Apache-2.0, BSD-2/3-Clause, CC0, CC BY 3.0/4.0 — ADR 0042 extended to models; never
NC, ND, SA), its credits, a **provenance card** (authors, paper, code, weights' source,
training data as the maker states it — for HT-Demucs, MUSDB18-HQ and 800 undisclosed songs
of Meta's — published quality, what Cantus does to the files), its files with sizes and
hashes, and how it is prepared. `Models` drives the content manager (`Manager`, its
downloads resumable, verified, proxy pages refused — ADR 0030) under `<data dir>/Models`:
`accept` records the model's and the runtime's licences as shown (`accepted.json`; a
changed licence asks again), `install` is refused until then, then fetches the runtime if
missing and the model, prepares it on a worker thread, and `ready` names the graph and the
library for the job; `remove` removes the model and the runtime when no other model needs
it. Validation refuses a licence off the list, a plain-`http` address, a graph whose map
was made from another file than the manifest names, a model without a runtime for every
system.

### 5. Separate stems… (the feature, round 34's second delivery)

- **Where**: an audio clip's menu (the selection's audio clips when the clip is in it), a
  track's menu (every audio clip on it), and the agent tool `separate_stems`. The panel
  shows what it separates in the user's words ("“Song.wav” on “Mix”"), the model (4 or 6
  stems), *Fast* or *Best*, *Use the graphics processor when it can*, the model's size,
  licence, runtime and provenance before its first download, then the separation's
  progress with *Cancel*; *Hide* lets it run with the panel closed.
- **The audio**: each source file the clips play is decoded (`decode_any`) and converted to
  44.1 kHz on a worker (`cantus_engine`'s windowed-sinc resampler) when it is not a WAV at
  that rate already; the helper separates it; the stems go back to the file's own rate and
  exactly its length. So a stem clip is **the original clip with another file under it** —
  its window, offset, fades, gain, warp and slip unchanged (the pitch editor's notes are
  dropped: they are the mix's) — and plays in step with it, sample for sample at the file's
  rate. A mono file gives stereo stems; the decoder gives one or two channels.
- **One undo step** (`stems_ui::plan`, `cmd.separate_stems`): the stems' files with their
  provenance, a strip group and a bus strip named after the track (*Mix stems*, short enough for a
  strip), feeding where
  the first clip's track fed (so the song sounds as before); per part a strip on the bus
  and a track right under the last separated track (the original's level and balance), the
  clips' copies on it; the original clips muted, never deleted. Files go in the project's
  `Audio/Stems/<file> — <model>/` (the session's scratch folder before the first save);
  a cancel or a failure removes them.
- **Quality**: *Best* is demucs's own shift trick, four passes moved by `k/4` of half a
  second and averaged — fixed offsets where demucs draws random ones, so a song separates
  the same way every time; held to PyTorch's `apply_model(shifts=2)` with the same offsets
  at 70–90 dB.
- **Measured on MUSDB18-HQ's 50 test tracks** (the helper, *Fast*, scored as the paper
  does — BSS Eval v4, museval's metrics, 1-second windows, the median of each track's
  windows and then over the tracks): drums 10.05, bass 9.78, other 6.42, vocals 8.93 dB,
  average **8.80 dB** — the paper's Table 1 to its two decimals.
- **GPU**: `device: auto` registers DirectML (Windows) or CoreML (Apple silicon) and falls
  back to the processor, saying why in the log. Measured here: the Windows runtime Cantus
  downloads (the CPU build) has no DirectML; ONNX Runtime's DirectML build 1.24.4 refuses
  this graph (an operator's parameters, `0x80070057`, after the 1-D transposed convolutions
  — rewritten as 2-D they pass and the next one fails) and runs the community graph out of
  memory on this laptop's Radeon 680M (shared memory). So the processor runs it here;
  CoreML is for the Mac session (`docs/mac-session.md` row 17); CUDA (a 240–450 MB
  runtime, NVIDIA only) is not offered. The runtime Cantus downloads for Windows and
  Linux is the processor's build, so there the switch *Use the graphics processor when it
  can* is not offered: the panel says "not available with this runtime" and the job asks
  for the processor (review 2 #11). The manifest's runtime entry names its graphics
  provider (`gpu`: `coreml` for macOS); a DirectML runtime, once one runs this graph,
  turns the switch on for Windows by its entry alone.
- **Provenance** (schema 48): `Project::provenance`, a map beside the pool by source id
  (not a field of `AudioSource`, so no literal of it changed), set with
  `SetAudioProvenance` (its exact inverse); removing a source takes its entry and the undo
  brings both back; copy and paste carry it; a load drops entries for files not in the
  pool. DAWproject has no place for it and does not carry it. Clips playing such a file
  wear an "AI" badge in their title bar.
- **The app's one service** (`ml_service.rs`, `CantusApp::ml`): one model store and one
  helper client, made in `assemble` and polled in the frame; each job's updates go to
  whoever started it through a channel fed by the service's own thread, so an agent's job
  runs while the window is minimised. An idle helper is ended after three minutes;
  *Settings ▸ Models ▸ Remove* ends it first so nothing holds the files.
- **Review 2** (each held by `crates/cantus-ui/tests/stems_review2.rs` unless named):
  the part strips and the bus start at the level that gives back the pan law's centre
  (+3.01 dB under −3 dB), so the parts sum to the original at unity — a render null test,
  the original against its stems, under −3, −6 and 0 dB; changing the project's pan law
  later (the mixer's menu, `stems_ui::pan_law_command`) trims every separation's part
  strips and bus by the change in the same undo step, so they keep matching (review 3); each part's clip is the original
  clip whole (crossfades, comp window and seam, lane, mute) with an id of its own and no
  link; clips on several tracks are refused, session clips never taken; a run belongs to
  the project it started on (`CantusApp::epoch`) and is cancelled when another opens;
  workers wake the window; a cancel removes what a worker wrote only after it answers, and
  an agent's run removes its converted input on every way out; files are converted a
  block at a time — each block the engine's resampler on just the input it needs, the
  whole file's result to the bit — and a file whose stems would pass a WAV's 4 GB is
  refused up front; the room a run needs is checked first; *Best*'s average says its
  progress and asks the cancel per block (`separate_pipeline.rs`); the hashes the job
  checks are the build's own weight map's, not the marker's, and a start never hashes
  the models (`cantus-content/tests/models.rs`); *Remove* waits for no one and is refused
  while the helper is in use; the licence row says only what the licence gives.
- **Integrated tree** (review of r34/integration): an agent's `separate_stems` chooses
  its folders at prepare and makes them on its worker; they go when the separation fails,
  when the job is dropped before its edit, when the edit cannot be planned, and — once
  planned — when the edit is never made (`run::Pending`, settled each frame: its files in
  the pool keep them; another project, or no agent's question still waiting, removes
  them). The plan reads the project's audio folder as it is then (a Save As during the job
  moved it). Every agent's model job holds `MlService::hold_ml_agent()` from prepare until
  its job ends (`MlService::ml_agents()`, part of the helper being in use); a model whose
  removal waits for the helper is not ready to start (`stems_ui::models::ready`).
- **Shipping**: on Windows `cantus-ml.exe` in a folder of its own, `ml\` of the install
  folder, with the Visual C++ runtime's four DLLs beside it (14.40 or later, from the build
  machine's redistributable; Microsoft's terms allow it app-local) — never in the install
  folder itself, where every program loads its own folder's `vcruntime140.dll` first (the
  MCP bridge an AI app keeps open would hold them during an update) and the plug-ins those
  programs load would find our `msvcp140.dll` before the system's (integration review);
  `ClientConfig::default_exe` looks in `ml\` first. The installer, its uninstaller, the zip
  and an update (the installer again) all take the folder from the staged list. Elsewhere
  the helper sits beside the program: the AppImage, the Flatpak, the macOS bundle (signed
  with the entitlements that turn library validation off), and the release build.

## Consequences

- One build for every system and driver, as always; machine learning costs nothing until
  a user accepts a model's licence. Download: 99 MB on Windows (15 MB runtime + 84 MB
  model), 96 MB on Linux; on disk 104 MB / 115 MB.
- Speed is ONNX Runtime's on the processor: about 3× real time on this laptop's CPU.
  Graphics processors are the next lever (§2), measured when they land.
- Weights' availability is Meta's: if `dl.fbaipublicfiles.com` stopped serving them, the
  export script and the map rebuild a hostable file from any copy with the same hash.
- The MPL crates get nothing of this (ADR 0047): `cantus-ml` and `cantus-content` are GPL.

## How another job is added (audio to MIDI)

1. Its arguments and result as serde types (a file of its own, like `spec.rs`'s
   `SeparateArgs`), shared by client and helper.
2. `fn job(args: Value, ctx: &mut Ctx) -> Result<Value, Stop>` — `jobs::args` to parse,
   `ctx.runtime.model(&runtime_spec, &graph, log)` for the session (cached between jobs),
   `Model::run(&[Input { name, shape, data }], &cancel)` for named float tensors in and out,
   `ctx.report.progress(done, stage)` and `ctx.report.check()?` between steps,
   `jobs::result(&r)` to answer.
3. One line in `jobs::JOBS`. The client starts it with `MlClient::start("<name>", args)`.
4. Its model in `manifest/models.json` (licence, provenance, files; a `prepare` kind if the
   maker's file is not already an ONNX graph — Basic Pitch's `nmp.onnx` is, so it needs a
   kind that only checks it).

## What the tests hold

- `cantus-ml`: the protocol's messages and framing; the STFT against PyTorch's own
  numbers; the separation in process with a fixture network whose answer is known — the
  time branch exact across every segment join (2 × 10⁻⁶), the spectral branch exact away
  from the windows' ends, short and mono tracks, integer WAVs, the wrong rate refused,
  fixtures refused outside tests; the helper process — a job end to end with progress, a
  cancel within 2 s that leaves no stems, a queued job cancelled, a crash, a hang (ended
  and said), a helper killed from outside, a missing or slow helper, unknown jobs and bad
  arguments, the client's drop ending the helper. Ignored (files or network): the real model
  against PyTorch (`htdemucs_real.rs`) and the whole path from nothing
  (`install_and_separate.rs`: licence accepted, runtime and model downloaded from their
  makers, weights rebuilt, a track separated; its stems' residual equal to PyTorch's).
- `cantus-content`: the bundled list valid with every licence permissive and Spanish
  names; the compiled-in graphs hash to their maps; validation refuses what ADR 0042
  refuses; an install end to end from `file://` fixtures with a small PyTorch file of our
  own — the licence gate, the weights rebuilt bit for bit, the maker's file removed, the
  install and acceptance remembered across a restart, a changed licence asking again,
  removal taking the runtime, weights that do not rebuild to their hash failing cleanly, a
  proxy's page refused; the pickle reader against Python's bytes and hostile input.
