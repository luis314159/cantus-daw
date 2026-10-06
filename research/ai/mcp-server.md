# An MCP server for Cantus

*Researched 2026-10-05. "[search]" marks a fact taken from a search-engine extract of the
cited page rather than from the page itself (the corporate proxy blocked many fetches);
re-check those before building on them.*

## 1. MCP in one paragraph, and where the spec is today

The Model Context Protocol is JSON-RPC 2.0 between a **client** (inside an AI app — Claude
Desktop, Claude Code, ChatGPT, Cursor, VS Code, Gemini CLI, LM Studio…) and a **server**
that offers **tools** (actions the model may call), **resources** (data the app or model
can read, addressed by URI) and **prompts** (user-invoked templates). Writing one server
makes Cantus drivable from every MCP client at once; it is the cheapest way to get "an LLM
does things in our DAW" because the client brings the model, the chat UI and the bill.

| Revision | What matters for Cantus | Source |
|---|---|---|
| **2026-07-28 (latest)** | **Stateless protocol**: no `initialize` handshake, no sessions or `Mcp-Session-Id`; each request carries protocol version / client info in `_meta`; a mandatory `server/discover`. Cross-call state goes in explicit server-minted handles. **Multi Round-Trip Requests**: a server that needs input (e.g. a confirmation via `elicitation/create`) returns `resultType: "input_required"`; the client retries with the answers. Tasks moved to an extension (`io.modelcontextprotocol/tasks`). `tools/list` order should be deterministic; `inputSchema`/`outputSchema` take full JSON Schema 2020-12. Deprecated (≥ 12 months): Roots, Sampling, Logging, HTTP+SSE, Dynamic Client Registration. | https://modelcontextprotocol.io/specification/latest (verified 2026-07-28 is "latest", fetched 2026-10-05); changelog https://modelcontextprotocol.io/specification/2026-07-28/changelog |
| 2025-11-25 | Icons on tools/resources/prompts; tool names 1–128 chars `[A-Za-z0-9_.-]`; URL-mode elicitation; experimental tasks; invalid `Origin` → HTTP 403; input-validation errors returned as tool errors (`isError`) so the model can self-correct; JSON Schema 2020-12 default. | https://modelcontextprotocol.io/specification/2025-11-25/changelog |
| 2025-06-18 | **Structured tool output** (`structuredContent` + `outputSchema`), **elicitation**, **resource links** in tool results, `title` fields, OAuth resource-server rules, `MCP-Protocol-Version` header, JSON-RPC batching removed. | https://modelcontextprotocol.io/specification/2025-06-18/changelog |

**Tool annotations** (hints, all optional): `title`, `readOnlyHint` (default false),
`destructiveHint` (default true), `idempotentHint` (default false), `openWorldHint`
(default true). The schema itself warns clients never to base decisions on annotations
from untrusted servers — so Cantus must enforce its own permissions server-side (§5) and
use annotations only to help clients show the right prompt.
(`schema/2026-07-28/schema.ts`, https://github.com/modelcontextprotocol/modelcontextprotocol)

**Governance.** Anthropic donated MCP to the Agentic AI Foundation, a directed fund of the
Linux Foundation co-founded with Block and OpenAI (late 2025) [search]
(https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation).
The SDK code is moving from MIT to Apache-2.0 — both GPL-3.0-compatible.

**Clients lag the spec.** Many clients in the field still speak 2025-06-18 or 2025-11-25.
Cantus should accept both the 2025-11-25 session model and the 2026-07-28 stateless one,
which the official Rust SDK does (§7).

## 2. Transports, and where the server lives

The project lives in the running app (`CantusApp` owns the `CommandBus`), so the server
must reach *that* process. MCP defines two transports
(https://modelcontextprotocol.io/specification/2026-07-28/basic/transports):

- **stdio** — the client launches the server as a subprocess and talks newline-delimited
  JSON-RPC over stdin/stdout. Every desktop client supports it; it is what Claude Desktop
  extensions use.
- **Streamable HTTP** — one POST endpoint; each response is JSON or a request-scoped SSE
  stream. Security rules: servers **MUST** validate `Origin` (DNS rebinding) and answer
  403 when invalid, **SHOULD** bind to `127.0.0.1` and **SHOULD** authenticate
  (https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http).

**Recommended shape: two front doors, one implementation.**

```
 Claude Desktop / Claude Code / Cursor / VS Code / Gemini CLI / LM Studio
        │ stdio                                   │ Streamable HTTP
        ▼                                         ▼
  cantus-mcp.exe (tiny bridge) ── local pipe ──► Cantus app: MCP endpoint on 127.0.0.1:<port>/mcp
                                                   │  (tokio task, cantus-ai / new cantus-mcp crate)
                                                   ▼
                                     request queue → UI thread → CommandBus (Origin::Mcp)
                                                   │
                                     offline render / analysis on worker threads
```

1. **In-app Streamable HTTP endpoint**, off by default, switched on in *Settings ▸ AI ▸
   Allow external agents*. Binds `127.0.0.1` on a fixed default port (configurable),
   validates `Origin`, requires a bearer token generated per install (shown with a copy
   button and a "regenerate" action). Clients that speak HTTP (Claude Code, Cursor, VS
   Code, Gemini CLI, LM Studio) connect directly.
2. **`cantus-mcp` stdio bridge** — a tiny executable shipped beside `cantus.exe` that a
   client launches. It forwards JSON-RPC to the running app over a named pipe (Windows) /
   Unix socket, or reports "Cantus is not running — open it and enable external agents".
   This is the ableton-mcp pattern (a socket into the host) without a second language.
3. **Headless mode (later)**: `cantus-mcp --project song.cantus` opens a project file with
   no UI for batch jobs (render, analyse, convert) — the same tool table, saving on exit.

Never a remote/public endpoint: ChatGPT and the Anthropic API's MCP connector only reach
public HTTPS servers (the connector: "Local STDIO servers cannot be connected directly",
https://platform.claude.com/docs/en/agents-and-tools/mcp-connector, beta header
`mcp-client-2025-11-20`, fetched 2026-10-05). A user who wants that can tunnel at their own
risk; Cantus should not ship a tunnel.

**Threading (ADR 0005).** The MCP server runs on a `tokio` runtime owned by the AI layer.
A tool call becomes a message to the UI thread (the only owner of `CommandBus`), which
executes it at the start of the next frame and replies through a oneshot channel. Reads
(`describe`, resources) take a snapshot of the `Project` the same way. Rendering and
analysis run on the existing worker threads (`cantus_engine::render`, stems, peaks). Nothing
reaches the audio thread except the snapshot the UI already publishes.

## 3. What a DAW should expose — the tool catalogue

Principles: **few, chunky, typed tools** (models do better with 30 clear tools than 300
thin ones; Ableton's best-known server has 37, the biggest REAPER ones 180–600 and are
reported as hard for models to use); stable names (each is a promise, as `bridge.rs`
says); ids returned by every create so the next call can address the object; every input
validated with errors returned as `isError` tool results; units in names (`_db`, `_beats`,
`_hz`); musical positions in **bars:beats** *and* beats, never ticks. One table in
`cantus-ai` generates the MCP tools, the assistant panel's tools and their docs.

| Group | Tools (proposed names) | Annotations | Maps to |
|---|---|---|---|
| Read | `get_project`, `get_track`, `get_clip_notes`, `get_mixer`, `get_device` (params with ranges/units), `list_devices` (every instrument/effect kind, native and hosted, with param keys), `get_selection`, `get_history` | readOnly | `describe_project` grown; JSON + XML forms |
| Transport | `set_tempo`, `set_time_signature`, `set_loop`, `play`, `stop`, `set_playhead` | idempotent; play/stop not destructive | `SetTempo`, `SetLoopRegion`… transport (not undoable — say so) |
| Tracks | `add_track` (instrument kind, name, colour), `remove_track`, `rename_track`, `set_track_mix` (gain_db, pan, mute, solo), `move_track` | remove = destructive | `InsertTrack`, `RemoveTrack`, `SetTrack*` |
| Instruments | `set_instrument` (any native kind or hosted plug-in id), `set_instrument_param` (by `NativeSpec` key), `load_preset` | — | `SetInstrument`, `SetInstrumentValue`, `SetInstrumentParam` |
| Notes and clips | `add_clip`, `add_notes`, `replace_notes`, `remove_notes`, `transpose`, `quantize`, `humanize`, `set_clip_bounds`, `duplicate_clip`, `move_clip` | replace/remove destructive | `InsertClip`, `InsertNotes`, `SetNotes`, `RemoveNotes`, `SetClipBounds` |
| Drums | `set_pad_sample`, `write_pattern` (step grid → notes) | — | `SetPadSample`, `InsertNotes` |
| Mixer | `route_track`, `add_strip`, `set_strip` (gain/pan/width/mute/solo/eq), `add_send`, `set_send`, `set_master` | — | `SetStrip*`, `SetStripSend`, `SetMaster*` |
| Effects | `add_effect` (every `EffectKind`, any strip incl. master), `set_effect_param`, `bypass_effect`, `move_effect`, `remove_effect`, `replace_effect` | remove destructive | `InsertEffect`, `SetEffectParam`, `SetEffectBypass`, `MoveEffect`, `ReplaceEffect` (round 20) |
| Automation | `add_automation` (target, points) — once ADR 0027 lands | — | automation commands (schema 19) |
| Audio | `import_audio` (path inside an allowed folder), `place_audio_clip` | openWorld (files) | `InsertAudioSource`, `SetClipAudio` |
| Render and analyse | `render_preview` (range, returns a resource link to a WAV), `analyse_loudness` (integrated/short-term LUFS, true peak, LRA, per track or master), `analyse_spectrum` (octave-band energy, masking hints), `analyse_key_tempo` | readOnly (writes only to a temp cache) | `cantus_engine::render`, `render_stems`, `cantus_dsp` loudness |
| Undo | `undo_last_ai_action`, `list_ai_actions` | destructive (undo) | `CommandBus::undo` limited to `Origin::Mcp` tops |
| Export | `export_audio`, `export_midi`, `export_stems`, `export_dawproject` | **denied by default** (`Permissions::conservative`) | interop module |
| Generation (phase 4) | `generate_audio`, `separate_stems`, `audio_to_midi` | openWorld, long-running → MCP tasks extension | `generative-audio.md` |

**Resources** (URIs, `application/json` unless stated): `cantus://project`,
`cantus://tracks/{id}`, `cantus://clips/{id}/notes`, `cantus://mixer`,
`cantus://devices` (catalogue with parameter keys, ranges, units — what makes
`set_*_param` usable), `cantus://history`, `cantus://renders/{id}.wav` (`audio/wav`,
returned as resource links by `render_preview`). Resources let a client attach context
without a tool round trip and keep the system prompt small.

**Prompts** (user-invoked templates, both locales): *Explain my mix* (reads mixer +
loudness, answers in words, changes nothing), *Write a part* (track, bars, style),
*Arrange an intro / a drop*, *Master for streaming* (target −14 LUFS integrated, −1 dBTP —
suggests a chain, applies only on approval), *Tidy the project* (names, colours, routing),
*Teach me this device* (reads `get_device` and the device docs).

**Audio for the model.** MCP tool results can carry `audio` content (base64) as well as
text and images. Most clients today ignore audio; descriptors (LUFS, spectrum bands,
crest factor, onset density, key/tempo estimates) are what models can reason about now.
Return both: numbers in `structuredContent`, a link to the WAV for clients that can listen.

## 4. Permissions and confirmation

MCP says "there SHOULD always be a human in the loop with the ability to deny tool
invocations"; clients SHOULD show inputs and confirm sensitive operations; servers MUST
validate inputs, enforce access control and rate-limit
(https://modelcontextprotocol.io/specification/2026-07-28/server/tools). Clients differ:
Claude Code and Cursor ask per tool unless allow-listed; ChatGPT treats every tool without
`readOnlyHint` as a write needing confirmation [search]
(https://developers.openai.com/api/docs/guides/developer-mode). Cantus cannot rely on any
of that, so:

1. **The server decides, in Cantus.** Every call goes through `Permissions::decide(tool)`
   with a per-origin policy (`Origin::Mcp` has its own policy, distinct from the panel's).
   Classes: *read* (auto), *edit* (ask, or auto once the user ticks "allow edits for this
   session"), *destructive* (remove/replace/undo — ask), *files and export* (denied unless
   enabled), *network/generation* (ask, shows cost).
2. **Confirmation happens in the Cantus window**, not in the client: a non-modal card in
   the AI activity strip ("Claude Code wants to: add 32 notes to *Bass* (bars 1–8)") with
   *Allow*, *Allow for this session*, *Deny*, and a preview — the commands are built with a
   dry run (`to_command` without `execute`) so the card can show a diff and the piano roll
   can ghost the notes. The tool call waits (with a timeout that returns an `isError`
   "the user did not answer"). For 2026-07-28 clients the server can instead return an
   MRTR `elicitation/create` so the client asks; keep the in-app card as the source of
   truth either way.
3. **Visible agent mode.** While an external agent is connected, the transport bar shows a
   chip with the client's name; one click disconnects and revokes the session. The audit
   log (`AuditEntry` with `Origin::Mcp`, including `Rejected`) is browsable as *AI
   activity*.
4. **Rate limits and size limits**: N edits per second, max notes per call, max render
   length — reject with a clear `isError`.

## 5. Undo: one AI action = one undo step

MCP defines nothing about undo. The REAPER servers that do it well wrap each request in
`Undo_BeginBlock`/`Undo_EndBlock` so **one tool call is one named undo step**
("MCP: delete track") [search] (TwelveTake-Studios/reaper-mcp, Oisub/reaper-mcp); the
popular Ableton server has no undo at all
(https://github.com/ahujasid/ableton-mcp, read 2026-10-05). Cantus gets this almost free:

- Each mutating tool builds **one `Command`** — a `Command::Batch` when it needs several —
  labelled `cmd.ai.<tool>` and executed with `Origin::Mcp` (or `Origin::Ai` from the
  panel). One Ctrl+Z undoes exactly that tool call; the undo menu reads "Undo AI: add
  notes".
- **Turn grouping (optional).** A model often makes 5–20 calls for one request ("write a
  verse"). The panel knows where a user turn starts and ends, so it can merge a turn's
  steps into one (`execute_merging` with a per-turn `MergeKey`, which only merges steps of
  the same origin, then `end_gesture()`). MCP has no notion of a turn; offer
  `begin_change(label) -> handle` / `end_change(handle)` tools so a well-behaved agent can
  group, and fall back to one step per call.
- **"Undo all of this agent's changes"**: the audit log has `revision` and `origin`; the
  UI can undo back to the revision where the agent connected, provided no user step is
  interleaved (if one is, say so and stop at it — never undo the user's work silently).
- **Concurrency.** The user keeps working while an agent works. Every mutating tool takes
  an optional `expected_revision`; if the project moved, return an `isError` with the new
  revision so the model re-reads. Ids are stable, so most calls still apply.
- **Fuzz.** New commands for AI tools join `undo_invariants.rs`; a test drives the tool
  table through an in-process MCP client and asserts undo-all restores the project.

## 6. How clients connect (configuration the docs must show)

| Client | How | Notes |
|---|---|---|
| **Claude Desktop** | `%APPDATA%\Claude\claude_desktop_config.json` → `"mcpServers": {"cantus": {"command": "C:\\Program Files\\Cantus\\cantus-mcp.exe"}}`; better: a **Desktop Extension** `.mcpb` bundle (zip + `manifest.json`, built with `@anthropic-ai/mcpb`; `.dxt` renamed `.mcpb` on 2025-09-11) installed by double-click [search] | https://github.com/modelcontextprotocol/mcpb — confirm the manifest accepts a native binary server before committing to it |
| **Claude Code** | `claude mcp add --transport http cantus http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`, or stdio: `claude mcp add cantus -- "C:\Program Files\Cantus\cantus-mcp.exe"`; scopes `local` (default), `project` (`.mcp.json`, shareable), `user` [search] | https://code.claude.com/docs/en/mcp |
| **Cursor** | `~/.cursor/mcp.json` or `.cursor/mcp.json`, key `mcpServers`; stdio or HTTP [search] | https://cursor.com/docs/mcp |
| **VS Code / GitHub Copilot** | `.vscode/mcp.json`, key **`servers`**, `type: "stdio" \| "http"`; agent mode [search] | https://code.visualstudio.com/docs/copilot/chat/mcp-servers |
| **Gemini CLI** | `~/.gemini/settings.json`, `mcpServers` with `command` or `httpUrl` [search] | https://geminicli.com/docs/tools/mcp-server/ |
| **LM Studio** (local models) | MCP host since 0.3.17, `mcp.json` in Cursor's format [search] | https://lmstudio.ai/docs/app/mcp — gives a fully offline "agent drives Cantus" path |
| **ChatGPT** | Developer mode, remote HTTPS servers only; full write support on Business/Enterprise/Edu [search] | Not a target: would need a public tunnel |
| **Anthropic / OpenAI APIs** | MCP connector (`mcp_servers` + `mcp_toolset`, beta `mcp-client-2025-11-20`) / Responses `mcp` tool | Public URLs only; irrelevant to a local DAW. The assistant panel calls tools directly instead (`llm-assistant.md`). |

Cantus should offer *Settings ▸ AI ▸ External agents ▸ Copy setup for…* with a button per
client that writes or copies the right snippet (token included), and a one-click
*Install in Claude Desktop* via `.mcpb`. The **MCP Registry**
(https://registry.modelcontextprotocol.io, `server.json` + `mcp-publisher`; preview since
2025-09, API frozen at v0.1 on 2025-10-24, GA unconfirmed) [search] can list Cantus under
`io.github.<owner>/cantus` once the server is stable.

## 7. Rust MCP SDKs

| Crate | Version / licence (checked) | Notes |
|---|---|---|
| **`rmcp`** — official, `modelcontextprotocol/rust-sdk` | **3.5.1, Apache-2.0** (crates.io API, 2026-10-05; the repo LICENSE keeps MIT for older contributions) | Tier-1 SDK. Supports 2026-07-28 and stays compatible with 2025-11-25 and earlier. Macros `#[tool]`, `#[tool_router]`, `#[tool_handler]` (schemas from `schemars`). Features: `server`, `client`, `macros`, `elicitation`, `auth`, transports `transport-io` (stdio), `transport-child-process`, `transport-streamable-http-server`/`-client`, `transport-async-rw`. Edition 2024, Rust ≥ 1.88. Breaking changes between majors (a 3.x migration guide exists). https://github.com/modelcontextprotocol/rust-sdk |
| `rust-mcp-sdk` (rust-mcp-stack) | 2.0.0, MIT [search] | Claims full 2026-07-28 support and conformance. https://docs.rs/crate/rust-mcp-sdk/2.0.0 |
| `pmcp` | MIT [search] | stdio, HTTP, WebSocket. |
| `mcp-attr`, `mcpkit`, `turbomcp` | unverified | Smaller. |

**Recommendation: `rmcp`**, behind Cantus's own `ToolSpec` table (so the SDK is an
adapter, not the model — `idea.md` §12's advice). Pin the major version; one golden test
snapshots `tools/list` so a schema change is a reviewed diff. All candidates are
GPL-3.0-compatible; the server lives in a GPL crate (`cantus-ai` or a new `cantus-mcp`),
never in the MIT/Apache core crates (ADR 0001).

**Cantus as an MCP *client*** (`idea.md` §4.4) uses the same crate's `client` feature: the
assistant panel can mount external servers (a sample-library search, a music-theory
server, a lyrics tool) as extra tools under the same permission gate. Later phase.

## 8. Prior art — DAW MCP servers (none official)

No DAW vendor ships an official MCP server as of 2026-10-05 [search]
(https://chatforest.com/guides/mcp-music-audio-production/). Community projects:

| DAW | Project | Bridge | What it exposes | Lessons |
|---|---|---|---|---|
| Ableton Live | **ahujasid/ableton-mcp** (MIT, ~3.1k stars) | Python server ↔ Live MIDI Remote Script over TCP `localhost:9877` | 37 tools: session/track/clip/device get-set, MIDI and audio tracks, notes, tempo, load from browser, fire/stop clips, transport, arrangement and locators | **No undo tools**; telemetry on by default; third-party devices unreliable, no audio export, long requests time out [search] (https://github.com/ahujasid/ableton-mcp; https://mcp.directory/blog/ableton-mcp-complete-guide-2026) |
| Ableton Live | Simon-Kansara/ableton-live-mcp-server (MIT, ~340 stars) [search] | OSC via AbletonOSC | transport, tracks, clips | OSC is lossy for structured edits |
| REAPER | TwelveTake-Studios/reaper-mcp (~180 tools), Oisub/reaper-mcp, danishaft/reaper-mcp, shiehn/total-reaper-mcp (600+ ReaScript functions) [search] | Lua/ReaScript bridges via files or sockets | nearly the whole API | **One tool call = one named undo step**, read-back verification, preflight validation, file access denied by default — the patterns to copy. Huge catalogues are hard for models. |
| Logic Pro | koltyj/logic-pro-mcp (MIT, Swift) [search] | CoreMIDI, Accessibility, AppleScript, OSC | 8 tools, 7 resources | UI scripting is brittle |
| FL Studio | karl-andres/fl-studio-mcp, veenastudio/flstudio-mcp [search] | virtual MIDI (loopMIDI) + FL MIDI-controller and piano-roll scripting | notes, transport, some mixer | FL has no socket API; the workaround shows the demand from FL users — Cantus's audience |
| Bitwig | WigAI [search] | a Java extension running an MCP server **inside** Bitwig at `localhost:61169/mcp` | transport, device params, clip/scene launch | The in-process HTTP design recommended here |
| Ardour | ardour-mcp (Rust) [search] | OSC to port 3819 | transport, track mute/solo/rec/gain, actions | Rust MCP over a DAW works |
| Studio One / Fender Studio Pro | NeanderthalMan/studio-one-mcp [search] | reads `.song`, bridge device inside the app | ~1000 commands | |
| Pro Tools | "Pro Tools MCP" (Blue Elevator) [search] | PTSL gRPC | podcast post-production; read-only by default | |

**Where Cantus is ahead by construction:** every other server bolts onto a DAW through a
scripting or MIDI side door and inherits its gaps (no undo, no audit, no permissions,
racy state). Cantus's server would sit on the same command bus as the mouse: typed,
validated, undoable as one step, audited by origin, with confirmation inside the app.
That is a genuine first — worth saying in the release notes.

## 9. Security checklist

- Off by default; `127.0.0.1` only; `Origin` validated (403); bearer token per install,
  regenerable; the stdio bridge connects through a per-user named pipe with an ACL.
- **Prompt injection:** track names, clip names, notes in a project from someone else, and
  file metadata are *data*. Never put them in the system role; quote them in tool
  results; destructive and export actions always confirm regardless of what the text says.
- File tools confined to the project folder and folders the user adds; no arbitrary path
  reads/writes; export denied unless enabled.
- No shell; no scanning or loading of *new* plug-in binaries from a tool call without
  confirmation (a hosted plug-in is native code).
- Rate and size limits; a kill switch; the whole AI area disabled by one setting.
- Follow https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices
  (show the full launch command before one-click installs; random, user-bound handles).

## 10. Size of the job (for the backlog)

| Step | Work |
|---|---|
| M1 | `ToolSpec` gains `title`, `annotations`, `output_schema`; one tool table drives bridge + MCP; grow from 7 to ~30 tools; dry-run; tests through an in-process client (`rmcp` client feature). |
| M2 | In-app Streamable HTTP endpoint (rmcp `transport-streamable-http-server`), token, Origin check, UI-thread marshalling, confirmation card, AI activity view, per-origin permissions. |
| M3 | `cantus-mcp` stdio bridge + `.mcpb` bundle + *Copy setup for…*; docs EN/ES; registry entry. |
| M4 | Render/analyse tools (loudness, spectrum, key/tempo), resource links to WAVs; turn grouping; `undo_last_ai_action`. |
| M5 | MCP client side in the panel; tasks extension for long jobs (generation, separation). |
