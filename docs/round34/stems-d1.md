# Round 34 — stems, first delivery: the ML helper, the runtime, models on demand

Branch `r34/stems` (merged with `r34/integration` at 2b5f1790). ADR 0085 holds the
decisions and every number; this is the builder's summary and the hand-over for
`transcribe`.

## What is done

1. **The runtime, measured and chosen: ONNX Runtime** (`ort` 2.0.0-rc.15, `load-dynamic`,
   API 23), its library downloaded on demand. One HT-Demucs segment on this Ryzen 7 7735HS:
   ONNX Runtime 1.80 s, tract 0.23.9 4.39 s (8 threads) / 9.39 s (1), candle-onnx not
   buildable without `protoc`; PyTorch (Meta's code) ~1.9 s. A 3-minute song end to end in
   the helper: **65 s, 2.66 GB peak** (PyTorch: 59 s, 1.76 GB, both with two other builds
   on the machine). Telemetry off (`ORT_DISABLE_TELEMETRY=1` set by client and helper, and
   the API). Table and reasons: ADR 0085 §2.
2. **`crates/cantus-ml`** (GPL) and its **`cantus-ml` binary**: jobs over its pipes (start,
   progress, cancel — inside a run too —, result), started off the interface's thread by
   `MlClient`, a crash, a hang, a kill from outside, a missing or slow helper each reported
   as what it is and cleaned up; the next job gets a fresh helper.
3. **Models through the content manager** (`cantus-content/src/models*.rs`): a compiled-in
   manifest with licence, credits, provenance card, sizes and hashes; the licence accepted
   once (model and runtime, asked again if it changes); downloaded on demand, resumable,
   verified; removable (the runtime goes with the last model). **Cantus hosts nothing**:
   the graph (ours, no weights) is compiled in, the weights are rebuilt at install from
   Meta's own `.th` and checked byte for byte.
4. **One separation end to end from a test**: in CI, `helper_process.rs` runs the real
   helper with a fixture network; ignored, `install_and_separate.rs` takes the whole path
   from nothing (licence, downloads from Microsoft and Meta, weights rebuilt, a track
   separated) and `htdemucs_real.rs` holds the result to PyTorch's.

## Numbers

| What | Result |
|---|---|
| Our graph vs PyTorch's `HTDemucs.forward`, one segment | 1.9 × 10⁻⁵ largest difference, 100.6 dB (6 stems: 90.7 dB) |
| A 3-min track, helper vs `apply_model` | drums 84.9, bass 80.8, other 82.9, vocals 65.6 dB; largest difference 5.9 × 10⁻⁴ |
| 6 stems, funk demo | 63.0–86.3 dB |
| STFT / inverse vs PyTorch | 2 × 10⁻⁴ / 1 × 10⁻⁵ (values up to ~30 / ~1) |
| Stems' residual on a test signal | 18.9 dB (PyTorch's own on it: 19.0 dB) |
| Graph | 2 087 nodes, 0.85 MB (gzip 0.39 MB); 6 stems 3.8 MB (gzip 1.27 MB) |
| Download, Windows / Linux | 99 MB / 96 MB (runtime 15 / 11 MB + model 84 MB) |
| Install from a fresh folder, here | 2.6 s (the proxy had the files cached) |

## How another job is added (for `transcribe`)

ADR 0085, "How another job is added": its serde types; `fn job(args, ctx) -> Result<Value,
Stop>` using `jobs::args`, `ctx.runtime.model(spec, graph, log)` (the session is cached
between jobs), `Model::run(&[Input { name, shape, data }], &cancel)` → named `f32`
outputs, `ctx.report.progress / check`, `jobs::result`; one line in `jobs::JOBS`; the
client calls `MlClient::start("transcribe", args)`. The STFT in `stft.rs` is HT-Demucs's
exactly (n_fft 4096); Basic Pitch computes its own CQT inside its graph.

**How a model is declared and downloaded**: an entry in
`crates/cantus-content/manifest/models.json` — id, version, names (EN/ES), task, family,
licence (the allow-list in `models::LICENCES`), credits, provenance, `files` (pinned
`https` URLs, size, SHA-256: the maker's own), `prepare` (how its files become what the
job loads; `demucs_th` today — Basic Pitch's `nmp.onnx` needs a kind that only checks
it), `runtime: "onnxruntime"`. `Models::accept(id)`, `install(id)`, `poll()` once a frame,
`ready(id)` → graph and library paths. The bundled manifest is validated by a test.

## How it was tested

`cargo test -p cantus-ml -p cantus-content -p xtask` (Windows, `-D warnings`):
cantus-ml 21 (+2 ignored), cantus-content 78 (+3 ignored, 8 of them models), xtask 36.
`cargo clippy --workspace --all-targets -D warnings` clean on Windows; Linux (WSL,
`tools/wsl/check.sh`): workspace clippy clean (and the plug-ins without their editor),
the same crates' tests all passing (xtask 35 there). The ignored
ones were run here: `htdemucs_real` (4 and 6 stems, 20 s and 3 min) and
`install_and_separate` (from a fresh folder over the network), all passing.

```text
CANTUS_ORT_LIBRARY=…/onnxruntime.dll CANTUS_HTDEMUCS=…/htdemucs.onnx \
CANTUS_DEMUCS_REFERENCE=…/ref/<track> cargo test -p cantus-ml --release --test htdemucs_real -- --ignored --nocapture
CANTUS_ML_NET=1 cargo test -p cantus-ml --release --test install_and_separate -- --ignored --nocapture
```

The references come from `crates/cantus-ml/tools/reference_htdemucs.py` (Meta's code,
deterministic), the STFT fixture from `tools/reference_stft.py`, the graphs and maps from
`tools/export_htdemucs.py` (torch 2.15, demucs 4.1.0, onnx 1.23, onnxscript 0.7.2,
onnxruntime 1.31.1, in a Python environment under `F:\projects\daw-r34\models\`).

## Marked lines (files others own)

- `Cargo.toml`: `"crates/cantus-ml"` in `members` and `default-members`, and
  `cantus-ml = { path = … }` in `[workspace.dependencies]` — `# r34:stems`.
- `crates/cantus-content/src/lib.rs`: `pub mod models; models_pickle; models_th;` —
  `// r34:stems`.
- `xtask/src/notices.rs`: `"cantus-ml"` in `ROOTS` — `// r34:stems`;
  `THIRD-PARTY-NOTICES.md` regenerated (new: `ort`, `ort-sys`, `libloading` 0.9 (ISC)).

## For the integrator

- **New dependencies**: `ort` =2.0.0-rc.15 and `ort-sys` (MIT OR Apache-2.0, no build-time
  download: `load-dynamic` only), `libloading` 0.9 (ISC). Regenerate the notices after the
  merge.
- **Where the weights are hosted**: nowhere of ours. Meta's `dl.fbaipublicfiles.com`
  (the `.th` files, MIT), Microsoft's PyPI wheel (Windows runtime) and GitHub releases
  (Linux, macOS runtimes), GitHub raw at a pinned commit (demucs's LICENSE).
- **Packaging** (the helper beside `cantus` in the installer, zip, AppImage, Flatpak, the
  macOS bundle) comes with the feature in the next delivery, by marked lines in mcp's
  packaging files — the helper is not started by the app until then. On macOS the helper
  will need `disable-library-validation` to load Microsoft's dylib (as the sandbox has):
  for `docs/mac-session.md`.
- Nothing in an MPL crate; no schema change yet (48 comes with provenance in the feature).

## Owner-only

Nothing in this delivery.
