# Round 34 — mcp, first delivery: the protocol, the pump, permissions, the test client

Builder **mcp**, branch `r34/mcp` (from the contract b4568467). ADR 0081 (first version).
What `tools` starts from.

## What was done

1. **`crates/cantus-mcp`** (GPL, new): the MCP protocol written by hand — JSON-RPC 2.0
   (`jsonrpc.rs`), both generations in one `Server` (`server.rs`): `initialize` /
   `notifications/initialized` (2025-06-18, 2025-11-25, a session id minted), the stateless
   2026-07-28 (`_meta` version and `clientCapabilities` on every request, `server/discover`,
   `resultType`, `serverInfo` in `_meta`, -32022 with `supported`/`requested`), `ping`,
   `tools/list` (deterministic, the registry's order, `title`, annotations from the class,
   `inputSchema` with `expected_revision` added to mutating tools, `outputSchema`),
   `tools/call` (`content` text + `structuredContent`; `isError` on tool errors),
   `resources/list|read|templates/list` (level-1 URI templates, blobs in base64),
   `prompts/list|get` (localised by the host), notifications accepted, unknown methods
   -32601, batches -32600, a 4 MiB limit on a message. **No reason found to switch to `rmcp`**
   (ADR 0081 §1): it would bring `tokio` into an application with none, and its macros
   would own a table we already have.
2. **The UI-thread pump** — `cantus_mcp::Agents`, held in `CantusApp::ai`
   (`cantus-ui/src/ai_mcp.rs`): requests queue through a door (a channel + `request_repaint`
   so an idle window answers), the frame drains them (`ai_mcp::frame`, called from
   `CantusApp::frame`), runs them against `ai_tools::registry()` with **`CantusApp` as the
   host** (`ai_host.rs`: `ToolHost` and `AgentHost`), executes plans with `Origin::Mcp` under
   the same holds as `CantusApp::exec`, answers through a oneshot. **Wired in
   `CantusApp::assemble`** (the boot's path), tested by `tests/ai_mcp_boot.rs`, which fails if
   it leaves that path or if the frame stops turning the pump.
3. **Jobs** on worker threads with handles; **`wait_job`, `cancel_job`, `list_jobs`**;
   **`begin_change`/`end_change`** (one undo step per turn), **`undo_last_ai_action`**
   (only the agent's own top step), **`list_ai_actions`** — `cantus_mcp::generic::TOOLS`,
   registered last in `ai_tools::registry()` by one marked line.
4. **Permissions by class** with the user's grants (`Settings::ai`, additive:
   `enabled`, `external_agents` — off —, `allow_edits`, `allow_files`, `folders`,
   `denied_tools`) and *Allow for this session*; the confirmation queue (the card that shows
   it comes in the next delivery; tests answer it through `Agents::answer`); rate limits;
   every call's outcome in the activity log, refusals included.
5. **The in-process client** `cantus_mcp::client::TestClient`: the protocol end to end with
   no socket.
6. The history names an agent's step **"AI: <what>"** (`ai.history.step`, both locales).

## The API the `tools` builder uses

**A tool** is a `ToolDef` in `cantus_ai::tools_project::TOOLS` (or a `cantus-ui` list).
Its functions get a `ToolCx`:

```rust
pub struct ToolCx<'a> { pub host: &'a mut dyn ToolHost }   // changed from { bus, host }
cx.bus()      // &CommandBus  — the project, history, revision
cx.bus_mut()  // &mut CommandBus — allocate ids (new_track, alloc_id…); never execute
cx.project()  // &Project
cx.host.as_any_mut().downcast_mut::<cantus_ui::CantusApp>()  // tools in cantus-ui only
```

- `ToolRun::Read(f)` answers at once. `ToolRun::Edit(f)` returns an `EditPlan { command,
  said, result }` — **never executes**; the pump asks (by class), executes with
  `Origin::Mcp` (one undo step, or merged into the agent's open change), and answers
  `result` plus `"revision"`. `ToolRun::Job(f)` returns a `Job { said, work }`; `work` runs
  on a thread (`JobCx::progress`, `JobCx::cancelled`) and ends in `JobDone::Answer(v)` or
  `JobDone::Then(v, plan)` (the edit planned back on the UI thread). `ToolRun::Host(f)`
  runs at once (transport, selection).
- **Do not** declare or read `expected_revision`: the server adds it to every mutating
  tool's schema and the pump checks and removes it before your function sees the args.
- Errors: return `ToolError` (`bad(name, reason)`, `NotFound`, `Refused`, `Failed`) — the
  agent gets an `isError` result with the message.
- `Said::new("ai.<key>").arg("name", v)`: the confirmation, the history ("AI: …") and
  `list_ai_actions` word it with the host (`tf`), so put the key in `ai.en.txt` and
  `ai.es.txt` under a `# tools` block.
- **Resources and prompts**: fill `cantus_ai::tools_project::{RESOURCES,
  RESOURCE_TEMPLATES, PROMPTS}` (`ResourceDef { uri, name, title, description, mime_type,
  read: fn(&ToolCx) -> Result<ResourceBody, ToolError> }`, `ResourceTemplateDef` with
  `uri_template: "cantus://tracks/{id}"` and `read(&ToolCx, &TemplateVars)`, `PromptDef`
  with locale keys for `title`/`description`/arguments and `get` returning
  `PromptMessage { role, said }`). `ToolError::NotFound` from a read is the protocol's
  "resource not found". `cantus_ui::ai_mcp::catalog` joins them (a marked line there for
  another builder's list). A test there fails if a prompt key lacks a Spanish twin.

**Calling tools from tests** (in `cantus-ui`, with the real host):

```rust
let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
cantus_ui::ai_mcp::wire_with(&mut app, &egui::Context::default(), cantus_ui::ai_mcp::catalog::app());
let mut c = cantus_mcp::client::TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
// or TestClient::modern(server) for 2026-07-28
app.settings.ai.allow_edits = true;          // or answer the confirmation:
let r = c.call_tool("add_track", json!({"kind": "unda"}), &mut || {
    cantus_ui::ai_mcp::pump(&mut app);
    if let Some(a) = app.ai.agents_mut() {
        let ids: Vec<u64> = a.asks().map(|x| x.id).collect();
        for id in ids { a.answer(id, cantus_mcp::Answer::Allow); }
    }
});
assert!(!r.is_error, "{}", r.text);   // r.structured: Option<Value>, r.raw
```

`tests/ai_mcp_pump.rs` has it as `app()` and `call()` to copy. Without the application,
`cantus-mcp/tests/common/mod.rs` has a host over a bare bus (`Rig`). An undo that "restores
the project exactly" compares with `next_id` set aside (ids are never reused), as the undo
fuzz does.

## How it was tested

- `cantus-mcp`: 16 unit tests (JSON-RPC parsing and codes, the door, annotations, URI
  templates, permissions = `default_decision` with nothing changed, the rate bucket, handles,
  base64) and 32 integration tests: `protocol.rs` (13: conformance per version written from
  the specification's pages), `pump.rs` (9), `changes_jobs.rs` (10).
- `cantus-ui`: `tests/ai_mcp_boot.rs` (assemble makes the server; `tools/list` is the
  registry; a call is answered by `CantusApp::frame` alone), `tests/ai_mcp_pump.rs` (5: one
  named undo step through the application, the user's Undo restoring it exactly, the
  settings deciding, a host tool reaching the application and its very bus, the agent's
  undo through the application's Undo, the settings file additive), the catalogue's test,
  the registry's test.
- **55 new tests** (cantus-mcp 48, cantus-ui 7), all passing on Windows and on Linux (WSL
  Ubuntu 24.04, headless); cantus-ai's 7 still pass. `cargo clippy --workspace
  --all-targets` with `-D warnings` clean on Windows and on Linux (`tools/wsl/check.sh`).
- The boot test fails if `ai_mcp::wire` leaves `assemble` (`is_wired`) or if `frame` stops
  turning the pump (the call is never answered).

## Marked lines (`// r34:mcp`)

- `Cargo.toml`: `"crates/cantus-mcp"` in `members` and in `default-members`, and
  `cantus-mcp = { path = … }` in `[workspace.dependencies]`.
- `crates/cantus-ai/src/tools_project.rs`: `RESOURCES`, `RESOURCE_TEMPLATES`, `PROMPTS`
  (empty; `tools` fills them).
- `crates/cantus-ui/Cargo.toml`: `cantus-mcp.workspace = true`.
- `crates/cantus-ui/src/lib.rs`: `pub mod ai_mcp;`.
- `crates/cantus-ui/src/ai_tools.rs`: `cantus_mcp::generic::TOOLS,` in `registry()`.
- `crates/cantus-ui/src/app.rs`: the field `ai`, its `Default`, `ai_mcp::wire` at the end of
  `assemble`, `ai_mcp::frame` in `frame` after the session's.
- `crates/cantus-ui/src/settings.rs`: the field `ai` and its default.
- `crates/cantus-ui/src/views/history.rs`: the two row names through `ai_mcp::history_name`.

Contract change in my own file: `cantus-ai/src/tool.rs` — `ToolCx { host }` with
`bus()/bus_mut()/project()`, `ToolHost::bus/bus_mut`; new `ResourceDef`,
`ResourceTemplateDef`, `TemplateVars`, `ResourceBody`, `PromptDef`, `PromptArg`,
`PromptMessage` (re-exported from `cantus_ai`).

## For the owner

Nothing yet: this delivery has no window of its own (the card, the chip, the activity view
and Settings ▸ AI come next).
