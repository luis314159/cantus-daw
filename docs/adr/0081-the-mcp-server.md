# ADR 0081 — The MCP server: a protocol of our own, a pump on the UI thread, permissions in Cantus

**Status:** accepted (round 34) · **Date:** 2026-10-30 · **Extends:** ADR 0004 (commands,
`Origin`), ADR 0005 (real time), ADR 0024 (the kit), ADR 0031 §2.7 (OS code in cantus-rt),
ADR 0047 (licences) · `research/ai/mcp-server.md` · reviewed three times in the round (the
findings and their tests: `crates/cantus-mcp/tests/review{1,2,3}.rs`)

## Context

BACKLOG §7 phase B: Claude and other assistants drive Cantus end to end through the Model
Context Protocol, each tool one undo step through the same `CommandBus` as a click, safe by
design. The project lives on the UI thread (`CantusApp` owns the bus); clients speak two
generations of MCP — the handshake one (2025-06-18, 2025-11-25: `initialize`, a session) and
the stateless 2026-07-28 (`_meta` version and `clientCapabilities` on every request,
`server/discover`, `resultType`, -32022). The research recommended the `rmcp` SDK.

## Decision

1. **The protocol is written by hand** (`crates/cantus-mcp`, GPL): JSON-RPC 2.0 over std
   threads. `rmcp` would bring `tokio` into an application with no async runtime and macros
   that own the tool table we already have; what MCP asks of a server is small, and the hard
   part — the UI thread, permissions, undo, jobs — is ours either way.
2. **One `Server`, every transport** (the in-process client, the HTTP door, the stdio
   bridge through the door). Protocol methods are answered on the calling thread
   (`initialize`, `server/discover`, `ping`, `tools/list` — the registry's order, `title`,
   annotations from the class, schemas with object roots); what needs the project goes
   through a **door** to the UI thread, which is woken (`request_repaint`). Legacy:
   `initialize` agrees the version asked if spoken, else 2025-11-25. Modern: served
   statelessly; missing `clientCapabilities` → -32602, another version → -32022. Batches
   -32600, 4 MiB limit, unknown methods -32601, unknown tools -32602, a missing resource
   -32002 / -32602. Tool errors are `isError` results. `notifications/cancelled` drops a
   waiting call and withdraws its card.
3. **The pump** (`cantus_mcp::Agents`, in `CantusApp::ai`, turned by `CantusApp::frame`,
   made in `assemble`) serves each call: rate limits → permission → the server's arguments
   taken out (`expected_revision`, and `change` on project tools) → **the arguments checked
   against the tool's `inputSchema`** (`schema.rs`: type, properties — unknown keys
   refused, naming the ones allowed —, required, additionalProperties, items, min/maxItems,
   enum, const, the bounds, min/maxLength, anyOf/oneOf — a branch's keys and the keys
   around it one set; an optional argument sent as `null` taken as left out unless its
   schema allows `null`) → the tool's run: `Read` at once;
   `Edit` planned (planning that changes the project is refused) and executed with
   `Origin::Mcp` through `AgentHost::execute` — for `CantusApp` the same holds as
   `CantusApp::exec` — one undo step; `Job` on a worker with a handle; `Host` / `HostSaid`
   at once. Only Edit, Destructive and Files may change the project; a non-edit tool that
   does is reported, its answer withheld. Results of mutating calls carry the revision
   (and their `outputSchema` says so).
4. **`ToolCx` reaches the bus through the host** (`ToolCx { host }`), since the
   application *is* the host and owns the bus.
5. **Permissions decide in Cantus** (`permissions.rs`), from `Settings::ai` and grants:
   reads and transport run; edits ask unless *Allow edits* or *Allow for this session* —
   **one edit tool from one agent**; **destructive and file tools always ask**; files are
   refused until *Allow files*; a tool denied by name is refused; external agents are
   refused while *Allow external agents* is off; the AI area off refuses everything. A
   confirmation keeps its plan; if the project moved, the tool plans again and the user is
   asked again when the words or the preview differ (unless the decision is now *allow*);
   one from another project's time (`AgentHost::epoch`) is refused; it waits 120 s, or
   until its transport stops waiting, whichever is first — a re-ask never restarts it.
   Limits: per agent and per pool (in-process apart from external) 50 reads/s and 10
   edits/s with bursts; 8 confirmations per agent, 24 per pool; 4 jobs per agent, 16 in all;
   256 agents remembered; idle agents forgotten after an hour. Every call's outcome is in
   the activity, refusals and dropped calls included.
6. **Who an agent is, and turns.** A session client is its session; a stateless one is its
   transport's credential — the HTTP token it carried (`http:<client>`; each client of
   *Copy setup for…* has its own token) and, for the bridge, its process
   (`Mcp-Cantus-Instance`, at most 32 per token) — never its `clientInfo` name. Changes:
   `begin_change(label)` → a handle; mutating tools take `change`; a session's change is
   also joined implicitly; a change ends after 2 minutes idle, but never while one of its
   calls waits on the card or a job will join it. The history names an agent's step
   "AI: <the change's label, or what the call said>"; steps are found by number alone (an
   imported project undone keeps them). `undo_last_ai_action` undoes the top step only if
   it is the agent's — the card names that step, and nothing else is undone — and
   `undo_all` stops at the first step not the agent's, saying which.
7. **Jobs**: their own thread, a panic its failure, progress, cancel; the call that starts
   one waits 3 s, else answers its handle (`job-` + 128 random bits, structured: a job
   tool's `outputSchema` is `{type: object, anyOf: [result, handle]}`). `JobDone::Then` is
   decided again when the job ends — permission, epoch — and asked on the card when the
   user must be asked now and was not asked at the start or the project moved; it joins
   the change it started in. `cancel_job` withdraws its card; a job whose starting call is
   cancelled, or whose client hangs up, before that call is answered is cancelled too
   (nobody holds its handle).
8. **The HTTP door** (`http.rs`): `127.0.0.1` only, port 39571 by default (Settings ▸ AI),
   open only while the AI area is on and external agents are allowed. Before reading a
   body: `Host` must be the loopback and `Origin` (if any) a loopback origin (403), then the
   bearer token, compared in constant time over every token (401). A head has 15 s, a body
   30 s; a connection's slot is given back by a guard, each connection runs under
   `catch_unwind`; a call whose client hangs up is dropped; a refusal is sent, our side
   shut, and what the client still sends drained briefly (bounded), so the close is not a
   reset that loses the answer. A bridge instance unheard of for an hour gives its place
   up (32 per token, least recent first). POST answers JSON (202 for a
   notification); GET is 405 (no stream); DELETE ends a session; a session is used only
   with the token that opened it (404 otherwise); `MCP-Protocol-Version` is honoured and
   must equal a modern request's `_meta` version (-32020).
9. **The token and the door's file** (`home.rs`): `mcp-token`, `mcp-clients.json` (a
   token per client) and `mcp-door.json` (`{port, pid, version}`, while the door is open)
   in the `mcp` folder of the user's settings folder — the same folder `app::config_dir()`
   names. Only `mcp/` is made private; the settings folder is left as the application
   keeps it (an elevated run, a portable drive moved between PCs). Tokens come from the
   OS's random source, written atomically through a temporary created owner-only *from its
   first instant* (Unix `O_CREAT|O_EXCL|O_NOFOLLOW` 0600; Windows `CreateFileW`, a link
   never followed, with a protected DACL granting **this user by SID** full control,
   `CREATE_NEW`), never over or from a file another user owns. `mcp/` is this user's —
   or Administrators', when the user is one of them (what an elevated run makes) — made
   private (0700, a umask's 0775 tightened; on Windows the same SID-only DACL, inherited),
   and nobody else can swap it (its parent not writable by others unless sticky, nor by a
   group other than the user's own private group). A drive that keeps no owner (FAT32,
   exFAT, some shares) is refused in words, never compared. All of it in
   `cantus-rt/src/private.rs`, its Windows tests reading the DACL and owners back from the
   system and from injected descriptors.
10. **The stdio bridge** (`cantus-mcp`, a binary beside `cantus.exe`): newline-delimited
    JSON-RPC on stdin/stdout forwarded to the door. **The door proves itself on every new
    connection, before the token is sent on it**: a fresh nonce, answered with its
    HMAC-SHA-256 under the token (`Mcp-Cantus-Proof`, a 204 that keeps the connection),
    then the request on that same stream (the proof has 5 s; silence is "not Cantus") —
    never a proof remembered from an earlier
    connection, so a program that took the port after Cantus closed (its door file left by
    a crash) never sees the token. (Chosen over a pipe or a Unix socket of our own: the TCP
    door is the one HTTP clients use anyway, and one door is one set of checks.) It serves
    requests on a pool of 16, the rest queued, the stdin thread only reading — so `ping`
    and `notifications/cancelled` are never held back — and a request cancelled before it
    was sent is never sent (a cancellation is kept only while its request is queued);
    `initialize` runs alone, first. With Cantus not running, or its
    door closed, every request gets a clean error (-31000) saying what to do; something
    else on the port gets nothing (-31002); a regenerated token says so (-31001); a
    settings folder that is not private, too (-31003). An AppImage's bridge is the image
    with `--mcp-bridge`, a Flatpak's `flatpak run --command=cantus-mcp …` — what *Copy setup
    for…* writes.
11. **What the window shows** (all hidden while the AI area is off): the confirmation card
    in the corner-card stack — who asks (the client's own name, quoted, invisible and
    reordering characters taken out, elided in its middle, beside what let it in: the
    token's client — or Cantus itself — `Caller::via`, and, as what it says and no more,
    whether it is a bridge — drawn whole), "wants to:", what it
    would do (a host tool gives its own sentence, `ToolRun::HostSaid`), what it touches,
    the time left; *Allow*, *Allow for this session* for an edit tool, *Deny*; the ✕
    denies; keyed by its call, and answering only once it has been in front 0.6 s, so a
    stray second click never answers the next call; the top bar's chip (`Group::Ai`,
    shown while an agent called in the last half hour; its lamp lit while one waits; a click
    opens the activity, a right-click Settings ▸ AI); the AI activity panel (agents with
    *Undo all of its changes* and *Disconnect*, the waiting calls, jobs with progress, every
    call and its outcome); Settings ▸ AI (the switch, external agents, the port and the
    door's state, the token, *Copy setup for…* six clients, a Claude Desktop extension
    `.mcpb` — MCPB manifest 0.3, `server.type = "binary"`, the bridge inside —, edits,
    files and their folders, tools refused by name).
12. **Sentences** (`Said`): a key and arguments that are text, a nested `Said`, bars, or a
    user's name (verbatim, in the language's quotes — its own quote marks made plain, its
    braces never a placeholder) — worded whole by the host (`Said::render`), each frame
    filled in one pass (`cantus_ai::fill`), so a Spanish frame holds Spanish phrases and
    nothing an argument brings in is filled again; a key missing in a language reads in
    English, one missing everywhere still shows its arguments.
13. **Tests drive it end to end with no socket** (`client::TestClient`) and on real sockets
    and the real bridge program.

## Consequences

- The installer and an update never wait for the bridge (an AI app keeps it running as
  long as it is open): the new one is written as `cantus-mcp.exe.new` and swapped in —
  the old one deleted if it is not running, else moved aside under a name of its own
  (`cantus-mcp.exe.old-<ticks>`), and put back if the swap fails, so an install that
  stops anywhere leaves a bridge. A per-user installer has no reboot queue: copies aside
  go at the next install or the uninstall, which waits for every one and names the AI
  app to close.

- One protocol implementation; tools are plain functions over `ToolCx`; resources and
  prompts are tables too, joined by `cantus_ui::ai_mcp::catalog`.
- File tools open through `cantus_ai::open_touch` (resolved inside `ToolHost::file_roots`,
  opened without following a link and without truncating, judged by where the opened
  handle really is — `/proc/self/fd`, `F_GETPATH`, `GetFinalPathNameByHandleW` — and only
  then emptied for a rewrite: a folder swapped for a link after the check loses nothing).
- A user gesture ending while a change is open splits it into two steps, never merging with
  the user's.
- Ids are never reused, so "undo restores the project exactly" compares with the id counter
  aside, as the undo fuzz does.
- The macOS bridge and bundle compile on the macOS runner; a real Mac confirms the rest
  (`docs/mac-session.md`).

## What the tests hold

`crates/cantus-mcp/tests/`: `protocol.rs` (conformance per version, object roots),
`pump.rs`, `changes_jobs.rs`, `http.rs` (real sockets), `bridge.rs` (the real program),
`review1.rs` … `review5.rs`; the unit tests of each module (JSON-RPC, the door,
the catalogue and templates, permissions and buckets, the schema check, HTTP framing, the
home files, handles). `cantus-ai`: `resolve_touch`/`open_touch`, `Said::render`.
`cantus-rt`: the private folder (under a real umask of 002, a parent open to all, the
DACL read back on Windows), the secret made owner-only, a handle's real path. `cantus-ui`: `tests/ai_mcp_boot.rs` (made by `assemble`,
served by the frame), `ai_mcp_pump.rs`, `ai_mcp_ui.rs` (the door, the card, the chip, the
off switch, the activity, the roots, the settings folder), the catalogue's and setup's unit
tests, and the shared Settings and top-bar sweeps.
