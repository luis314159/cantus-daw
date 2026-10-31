# Round 34 — mcp: the rest of the section

Builder **mcp**, branch `r34/mcp`. The first delivery (`docs/round34/mcp-d1.md`, merged at
2b5f1790) gave the protocol, the pump, permissions and the test client. This delivery gives
everything else in the brief's "## mcp", plus four reviews' fixes. ADR 0081 is brought up
to date. `r34/integration` is merged in, last at 7c6638a3 (21f3d4b8: tools' review 2, listen's review 1, transcribe).

## What was done

1. **The HTTP door** (`cantus-mcp/src/http.rs`, `http_wire.rs`) is Streamable HTTP on
   `127.0.0.1` only, off by default. It opens only while the AI area is on and *Allow
   external agents* is on. It checks, in this order:
   - `Host` and `Origin` (403);
   - the endpoint;
   - the door's proof;
   - the bearer token, one per client (`mcp-clients.json`) plus the default, compared in
     constant time (401).

   No body is read before those checks pass. It also has:
   - head and body deadlines;
   - a connection slot returned by a guard;
   - `catch_unwind` around each connection;
   - a call dropped when its client hangs up;
   - sessions bound to their token;
   - `MCP-Protocol-Version` against `_meta` (-32020);
   - a refusal that lingers so the client reads it;
   - bridge instances that are forgotten when idle.
2. **The stdio bridge** (`cantus-mcp/src/bridge.rs`, `bin/cantus-mcp.rs`) is the program an
   MCP client starts. It:
   - proves the door on every connection the token goes over;
   - runs requests on a pool of 16, with the rest queued;
   - never sends a request that was cancelled before it went out;
   - runs `initialize` first and alone;
   - answers with clean errors: -31000 not running, -31001 bad token, -31002 not Cantus,
     -31003 settings folder not private.

   It carries a Windows version resource (`build.rs`).
3. **The confirmation card** (`views/ai_card.rs`) sits in the corner-card stack. It shows:
   - "<client> wants to:" and the plan's sentence (a `HostSaid` tool's own sentence);
   - the preview, unless it only repeats the sentence;
   - what the call touches, and the time left;
   - *Allow*, *Allow for this session* (edit tools only), *Deny*, and ✕ (which denies).
4. **The agent chip** in the top bar (`views/ai_chip.rs`, `Group::Ai`):
   - It shows while an agent has called in the last half hour; its lamp is lit while one
     waits.
   - A click opens the activity; a right-click opens Settings ▸ AI.
   - It folds after Transport, the position and Tempo (importance 3).
5. **The AI activity panel** (`views/ai_activity.rs`) lists:
   - each agent, with *Undo all of this agent's changes* and *Disconnect*;
   - waiting calls;
   - jobs, with their progress;
   - every call and its outcome, localised.
6. **Settings ▸ AI** (`views/ai_settings.rs`) has:
   - the one off switch;
   - *Allow external agents*, the port and the door's state;
   - the token, with *Regenerate*;
   - *Copy setup for…* six clients (Claude Code, Claude Desktop, Cursor, VS Code, Gemini
     CLI, LM Studio), each client with its own token;
   - a Claude Desktop extension (`.mcpb`: MCPB manifest 0.3, `server.type = "binary"`,
     the bridge inside);
   - edits, files and their folders, and tools refused by name.

   A packaged bridge is started the way its package allows (`ai_setup::bridge_launch`):
   - an AppImage: `$APPIMAGE --mcp-bridge`;
   - a Flatpak: `flatpak run --command=cantus-mcp $FLATPAK_ID`.
7. **The one off switch** (`Settings::ai.enabled`) hides the card, the chip, the panel and
   the door, and makes every call refused.
8. **Packaging on three systems.** `cantus-mcp(.exe)` ships:
   - beside `cantus.exe` in the installer and the portable zip (xtask; its test checks 11
     files);
   - in the AppImage (AppRun `--mcp-bridge`);
   - in the Flatpak (`/app/bin`);
   - in the macOS bundle (`Contents/MacOS`);
   - in release.yml's builds.
9. **Owner-only secrets** (`cantus-rt/src/private.rs`, the only OS code):
   - `secure_dir`: this user's folder, tightened to 0700, or on Windows a protected
     owner-only DACL that its files inherit; refused only when another user could swap it.
   - `create_secret`: owner-only from creation (`O_EXCL|O_NOFOLLOW` 0600, or `CreateFileW`
     with its own descriptor and `CREATE_NEW`).
   - `owned_by_me`.
   - `open_no_follow` and `real_path`, used by `cantus_ai::open_touch`.
10. **Arguments are checked against `inputSchema`** (`schema.rs`):
    - An optional `null` counts as left out.
    - The keys of anyOf branches and the keys around them form one set.
11. **Sentences** are filled in one pass (`cantus_ai::fill`). A user's name can never word
    the sentence: its quote marks are made plain and its braces full-width. English stands
    behind a missing translation.
12. **The manual** (EN and ES) has a new section, "AI agents (MCP)" / "Agentes de IA (MCP)".
    **Shots** of the card, the activity, the settings and the bar are in
    `docs/round34/mcp/`: both themes, both languages, the bar at 1920 px and 1100 px, with
    the chip shown and hidden.
13. **`tools/list` is held by a golden file** (`cantus-ui/tests/golden/mcp-tools-list.json`,
    60 tools after merging tools). `CANTUS_TOOLS_DUMP=1` writes it again.

## Real clients (no model tokens spent)

`cargo run -p cantus-ui --example mcp_headless -- <dir> 39581 300` runs a real `CantusApp`
(catalogue, permissions, pump) turned by headless frames, with its door open. Against it:

- **MCP Inspector CLI 0.17.2** over HTTP:
  - `tools/list` returned 60 tools;
  - `tools/call list_jobs` returned `{"jobs":[]}`;
  - a wrong token was refused.
- **MCP Inspector CLI 0.17.2** through the stdio bridge (`CANTUS_DATA_DIR=<dir>`):
  - `tools/call get_project` returned the project;
  - `resources/list` worked.
- **`claude mcp add`** (HTTP with the token; the bridge) followed by **`claude mcp list`**:
  both servers showed "✓ Connected". Both entries were removed afterwards.

## Reviews

- Review 1 (10 findings) and review 2 (14 findings): see their commits; tests are in
  `tests/review1.rs` and `tests/review2.rs`.
- Review 3 (15 findings): tests in `tests/review3.rs` and the unit tests.
- Review 4 (12 findings), each with a test that fails without its fix:
  - #1, #2, #4, #8: 4b0ed01c.
  - #3, #9, #11, #12: ebb77d26. The decision for #3 (prove every connection, no cached
    proof) is recorded in ADR 0081 §10.
  - #7: d5653947.
  - #5: 2aa16ae1.
  - #6, #10: 0e4f206c. Follow-up 7f8d378d: only quotation characters count as a
    language's quote marks. The Linux run caught a host frame's period turning `Song.wav`
    into `Song"wav`.
- Review 5 (13 findings), each with a test that fails without its fix:
  - #1 (high), #10: 77161b54. A drive that keeps no owner is refused in words, never a
    null SID in a comparison. Links are not followed to read an owner or to make a secret.
  - #2 (high): 16992e45. The secrets moved to `<settings>/mcp/`, the only folder made
    private. It grants the user by SID, and an Administrators owner counts as the user
    when the user is an administrator.
  - #3: e1ec8985. The installer and an update move a running bridge aside instead of
    waiting for it; the uninstaller still waits and asks.
  - #4, #5: 819cebfb. The card is keyed by its call and is armed after 0.6 s. A client's
    name is quoted and cleaned, and shown beside what let it in.
  - #6, #8, #9: 277ce83c. The drain is bounded in time, the proof has 5 s, and the
    bridge's cancellations are bounded by its queue.
  - #7: 7127753f. macOS signing and the CI checks.
  - #11, #12, #13: 4122d4ce.
- Review 6 (10 findings):
  - #1: 809ddda2. The macOS check uses `if`, not `!`, and a CI test refuses negated
    workflow commands.
  - #2, #4: ac855426. The bridge is written as `.new` and swapped in, the old one put back
    on failure; no reboot is asked for.
  - #3, #5, #6, #7: 39c7f595. The card always names the token, and a bridge only as what
    it says. "via" is drawn whole and the name elided. Invisible characters follow
    Unicode's tables, with ZWJ and ZWNJ kept. The card is armed visibly and a screen
    reader announces it.
  - #8: afc82761.
  - #9: bc969aa2.
  - #10: 948517ec.
- Three timing races in the tests are closed by ordering, not by longer waits (cdd75024).
  For example, `review3::a_change_stays_open_while_its_job_runs` set the change's idle time
  to zero before the job's call reached the pump, so a pump could end the change first. The
  idle time is now zeroed only once the job runs.

## Tests (Windows, this branch after the merge)

- `cantus-mcp`: 123, as 30 unit tests plus `protocol`, `pump`, `changes_jobs`, `http`,
  `bridge`, `review1`–`review4`. Plus 1 Unix-only test in `bridge.rs`.
- `cantus-rt` `private`: 2 on Windows, 4 on Unix.
- `cantus-ai` lib: 13.
- `cantus-ui`:
  - lib: 397;
  - `ai_mcp_boot`: 1, `ai_mcp_ui`: 8, `ai_mcp_pump`: 6;
  - `topbar_layout`: 7, `ship_frictions`: 7, `kit_lint`: 4, `workflow`: 21.
- Workspace clippy (`-D warnings`, all targets) and `cargo fmt --all --check` were run
  after the fixes.

## Also fixed on the merged tree

- `transcribe_convert::the_tools_said_names_its_target` used `Said`'s old arguments (text).
  Since the mcp contract they are `SaidArg`, so the test did not compile on the merged tree.
  It now words the sentence as the card does (`AgentHost::say`).
- The tools/list golden file was regenerated twice, on purpose: after tools' review 2, then
  after listen and transcribe (`replace_master_chain`, `audio_to_midi`).

- The installer and an update never wait for `cantus-mcp.exe`: an AI app keeps it running
  as long as it is open (review 5 #3). The new bridge is written as `cantus-mcp.exe.new` and
  swapped in (`CANTUS_BRIDGE_SWAP`). If the old one is running, it is moved aside under a
  name of its own, and it is put back if the swap fails (review 6 #4). Nothing asks for a
  reboot: a per-user installer cannot queue one (review 6 #2).
- The uninstaller waits for every program, including the bridge and any copies moved
  aside. When only the bridge still runs, the Retry/Cancel message names the AI app to
  close (`update.installer.close_bridge`, EN and ES).
- This is held by `package_tests::the_uninstaller_removes_exactly_what_the_installer_writes`,
  and makensis compiles the script.

## Owner-only

- A real Mac run of the bridge and bundle (`docs/mac-session.md`).
- Signing `cantus-mcp.exe` with the rest.
- Claude Desktop installing the `.mcpb` (Claude Desktop is not on this machine). The
  manifest and zip are unit-tested.
- The manual's *Known issues* still says "AI features" are not in this beta. That is the
  integrator's line to change when round 34 ships.
