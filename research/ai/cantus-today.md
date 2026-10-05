# What Cantus already has for AI, and what is reusable

*Read on 2026-10-05 at `403ca06` (branch `r20/research-ai`). Sources: `crates/cantus-ai/`,
`crates/cantus-core/src/{command.rs,bus.rs}`, `docs/adr/0004-command-bus.md`,
`docs/adr/0001-license.md`, `idea.md` §3, §4, §7, §8, §11, §12, `docs/STATUS.md`.*

## 1. The design intent (idea.md §4)

`idea.md` put AI *before* the feature list on purpose: "designed in, not added on". Five
principles (§4.1) are binding on everything in this folder:

1. Never blocks or touches the audio thread — all inference is async, in its own crate,
   and reaches the project only through the command bus.
2. Provider-agnostic — no feature depends on one vendor's API.
3. Local-first for whoever wants it — full AI capability with zero bytes sent out.
4. Total transparency — the user sees what (audio, MIDI, project state) goes to which
   provider, when and why.
5. Reversible and auditable — every AI action goes through the same undo/redo as a click.

§4.2–4.6 list the backlog this research turns into a plan: multi-provider abstraction
with keys stored encrypted locally and never in a project, per-task provider choice,
fallback, streaming, cost tracking and spend limits (§4.2); local LLMs through llama.cpp
or Ollama and local audio models through ONNX Runtime or `tract` — Demucs, denoise,
MusicGen / Stable Audio (§4.3); **MCP in both directions** — Cantus as an MCP *server*
that agents drive, and as an MCP *client* that uses external servers (§4.4); concrete
features — composition and mixing assistants, local stem separation, text-to-sample,
natural-language control ("bring the drum bus up 2 dB"), auto-tagging, repair (§4.5);
and a risk table whose last row is a product rule: *"AI suggests, the human decides — no
AI feature may be required to produce a complete piece of music in this DAW"* (§4.6).
§7 adds: API credentials in the OS keychain (`keyring`), never in project files or
plain-text config; per-request disclosure; a verifiable offline mode. §11 names `tokio`
for the AI runtime, in-house `reqwest` clients per provider behind one trait, and "Rust
MCP SDK, or an in-house JSON-RPC implementation". §12 warns the Rust AI/MCP ecosystem is
young — prefer thin in-house code to a heavy dependency.

## 2. The command bus (ADR 0004) — the reason AI is safe in Cantus

Every mutation is a `cantus_core::Command` (about 110 variants today: transport, tracks,
strips, sends, groups, instruments and pads, effects and hosted-plug-in params/state,
clips, audio pool, recording settings, notes, and `Batch`). `CommandBus::execute(cmd,
origin)` applies it, stores the **inverse**, bumps the revision and writes an
**audit entry**. Facts that matter for AI, checked in `bus.rs`:

| Fact | Where | Why it matters |
|---|---|---|
| `Origin { User, Script, Ai, Mcp, System }` on every history and audit entry | `command.rs:16`, `bus.rs:23-37` | The panel and an MCP agent can be told apart in the history, the audit log and the permission layer, with no change of semantics. |
| `Command::Batch { label_key, commands }` = one undo step | `command.rs:533` | **One AI action = one undo step** is already expressible: the bridge wraps an agent's multi-command edit in a `Batch`. |
| `execute_merging(.., key)` + `end_gesture()` merge a gesture into one step, only when the origin matches | `bus.rs:158-170, 190` | An agent that "nudges" a parameter several times in one turn can merge into one step without swallowing a user's step. |
| `AuditKind { Execute, Undo, Redo, Rejected }` with `seq` and `revision` | `bus.rs:32-46` | Rejected agent commands are logged — the basis of an "AI activity" view. |
| `Command` is `serde` | ADR 0004 | A turn's edits can be shown as a diff, replayed, or sent over MCP as-is. |
| History capped at 10 000 steps | `bus.rs:78` | No special limit needed for AI. |
| Undo fuzz (`crates/cantus-core/tests/undo_invariants.rs`) | idea.md §6 | Any new command an AI feature needs joins the fuzz like every other. |

The engine never sees commands; it sees immutable `Arc<Snapshot>`s compiled after a
command applies, so nothing an agent does can reach the audio callback (ADR 0004,
ADR 0005).

## 3. `crates/cantus-ai` today (GPL-3.0-or-later, `#![forbid(unsafe_code)]`)

Dependencies: `cantus-core`, `serde`, `serde_json`, `async-trait`, `thiserror`. No HTTP,
no runtime. `cantus-ui` lists it as a dependency but does not call it yet (no
`cantus_ai` use in `crates/cantus-ui/src`). STATUS: "done (no live provider yet); 3 tests".

### `provider.rs`

- `AiProvider` (async, `Send + Sync`): `id()`, `display_name()`, `is_local()`,
  `complete(AiRequest) -> Result<AiResponse, AiError>`.
- `AiRequest { system, messages: Vec<AiMessage{role, content: String}>, tools:
  Vec<ToolSpec>, max_tokens }`; `AiResponse::{Text(String), ToolCalls(Vec<ToolCall>)}`.
- `ToolSpec { name, description, input_schema: serde_json::Value }` — JSON Schema, which
  is exactly what Anthropic `tools[].input_schema`, OpenAI `function.parameters`, Gemini
  `functionDeclarations[].parameters`, Ollama `tools` and **MCP `Tool.inputSchema`** all
  take. One catalogue can feed every provider *and* the MCP server.
- `Disclosure { provider, local, bytes, includes_project_state, tool_count }` computed
  from a request *before* it is sent — the transparency principle in code.
- `NullProvider` — every feature must degrade against it.
- `Permissions { auto_approve, denied }` → `Decision::{Allow, AskUser, Deny}`;
  `conservative()` auto-approves `describe_project` and denies `export`.

### `bridge.rs`

- `tool_specs()` — seven tools: `describe_project`, `set_tempo`, `add_track` (Firmus),
  `add_notes` (beats, pitch, velocity; creates or grows a clip), `set_track_gain`,
  `add_effect` (`delay`/`reverb` only), `set_synth_param` (eight Firmus params).
- `describe_project(&Project) -> String` — an XML-like summary (name, tempo, signature,
  per track: id, name, gain, pan, mute, clip and note counts).
- `to_command(&ToolCall, &mut CommandBus) -> Result<Option<Command>, BridgeError>` —
  validates arguments, resolves ids, allocates new ids through the bus, and returns a
  command (a `Batch` labelled `cmd.ai_add_notes` for notes). The caller executes it
  with `Origin::Ai`. Tests prove an agent's edits are ordinary, undoable and audited.

## 4. What is reusable as is, what must change

| Piece | Verdict | Notes |
|---|---|---|
| Command bus, `Origin::{Ai, Mcp}`, `Batch`, audit, undo fuzz | **Reuse as is** | The foundation for both the panel and MCP. Only addition: an *AI turn* wrapper that collects a turn's commands into one `Batch` labelled with the user's request (see `mcp-server.md` §5). |
| `ToolSpec` (JSON Schema) | **Reuse as is** | Maps 1:1 to MCP `Tool{name, description, inputSchema}`; add optional `title`, `output_schema` and *annotations* (read-only / destructive / idempotent) — MCP 2025-06-18+ fields. |
| `Permissions` / `Decision` | **Reuse, extend** | Add per-origin policy (panel vs MCP client), per-session "allow for this session", and classes (read / edit / render-export / network / files). Keep `conservative()` as the default. |
| `Disclosure` | **Reuse, extend** | Add audio seconds/bytes when audio is attached, estimated cost (tokens × price), and the destination host. |
| `describe_project` | **Reuse, grow** | Keep the compact XML for prompts; add a JSON resource form for MCP (`cantus://project`) and deeper views (strips, inserts, sends, clip windows, notes of one clip) on demand, so prompts stay small and cacheable. |
| `tool_specs` / `to_command` | **Reuse the pattern, grow the catalogue** | Seven tools reach only Firmus, gain, two effects. Needed: every instrument kind (`SetInstrument`, `SetInstrumentValue` with `NativeSpec` keys), strips/sends/routing, every effect kind and `SetEffectParam`, clip ops, select/quantize/transpose, master chain, render/analyse. Generate from one table so the panel and MCP never diverge. A `dry_run` that returns the commands without executing gives "preview before apply". |
| `AiProvider::complete` | **Extend** | Missing for a real assistant: streaming (a `complete_stream` returning a `Stream` of text / tool-call deltas), multi-block messages (text + tool_use + tool_result + image/audio parts — today `content: String` cannot carry a tool result back), `tool_call_id`s, usage (tokens in/out/cached) for cost, stop reason (incl. refusal), cancellation, and a capability descriptor (tools? vision? audio in? context size? caching?). See `llm-assistant.md` §6. |
| `AiError` | **Extend** | Add `RateLimited{retry_after}`, `Auth`, `Network`, `Cancelled`, `Refused{category}`, `ContextTooLong`. |
| UI | **New** | No panel, settings page, key storage, or locale area for AI exists yet. Locale keys belong in a new `locales/areas/ai.*` (EN + ES). |
| Engine | **Reuse** | Offline render (`cantus_engine::render`, `render_to_wav`), stems (`render_stems*`), MIDI and DAWproject export (`cantus_project::{midi, dawproject}`), loudness helpers (`cantus_dsp::{terminus_loudness, comprimo_loudness}::lufs`) already exist: a `render_preview` / `analyse_loudness` tool is wiring, not research. |
| Audio pool and import | **Reuse** | `Project::audio_pool`, `InsertAudioSource`, `SetClipAudio`, `import_audio` (round 19) are where a generated or separated file lands — the same path as a drag-and-drop. |

## 5. Constraints carried into every proposal

- **Licence (ADR 0001):** `cantus-ai` is GPL-3.0-or-later; `cantus-core/dsp/engine/
  project` are MIT OR Apache-2.0 and must never depend on AI crates. Any MCP or provider
  dependency lives in `cantus-ai` (or a new GPL crate such as `cantus-mcp`). Dependencies
  must be GPL-3.0-compatible (MIT, Apache-2.0, BSD, MPL-2.0, ISC are; "non-commercial"
  model *weights* are a separate question — `legal.md`).
- **Bundled content (ADR 0001, idea.md §8):** only original or CC0 samples/presets ship.
  Model weights are content too: none should ship in the installer; download on demand
  with the licence shown (same content-manager design as `research/content/`).
- **Real time (ADR 0005):** no AI code on the audio thread, ever; inference results
  arrive as commands or as files in the audio pool.
- **No AI required (idea.md §4.6, §15):** Cantus stays a complete DAW with AI off; the
  AI area is opt-in and hidden behind one switch.
