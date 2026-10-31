# Cantus — status and handoff

**Updated:** 2026-10-31 (round 34: **AI** — an **MCP server** written by hand (both protocol generations, the HTTP door on `127.0.0.1` off by default with a token per client, the `cantus-mcp` stdio bridge, permissions decided in Cantus, the confirmation card, the agent chip, the AI activity, Settings ▸ AI with setups for Claude Code, Claude Desktop, Cursor, VS Code, Gemini CLI and LM Studio; each call one undo step named "AI: …"; ADR 0081); **63 tools** with the unit catalogue, the music linter and the chunky writers `write_part`, `voice_chords`, `pedal_from_chart`, `automate` (ADR 0082); **listening** — range renders from checkpoints equal to the whole render to the bit, analysis taps, live = offline checked, `audition`, masking — and **the mastering assistant** on the master, nine platforms, worst 0.07 LU from the target (ADRs 0083–0084); **stems** by HT-Demucs in the `cantus-ml` helper on ONNX Runtime, SDR 8.80 dB on MUSDB18-HQ, equal to the paper (ADR 0085, schema 48); **audio to MIDI** — Basic Pitch note for note with Spotify's, our tracker for a melody, drums by our detector and Magenta's model (ADR 0086). Version **0.1.0-beta.9** (Windows and Linux; macOS waits for the Mac session). Schema 48, `DSP_VERSION` 5, keymap 14. Created by Luis Ángel Almazán — lalmazan.com) · **Tests:** 5 590 pass on Windows (82 ignored), the engine fuzz 985 at `CANTUS_WORKERS=3`, 5 574 on Linux (WSL Ubuntu 24.04); clippy clean on both

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## The next session: what round 34 left

Round 34 is on **master** (squashed) and published to `luis314159/cantus-daw`; **`v0.1.0-beta.9`**
is released for Windows and Linux. The account: `docs/round34/integration.md`; one report per
builder beside it (`mcp`, `tools`, `listen`, `stems`, `transcribe`). Under the owner's rule
what is left is the owner's own, or new work:

1. **A clean Windows** (or a VM) that never had the Visual C++ redistributable: a separation
   and a conversion run with the DLLs Cantus ships in `ml\`.
2. **Agents for real**: Claude Desktop installing the `.mcpb` extension; signing
   `cantus-mcp.exe` and `cantus-ml.exe` with the rest; a real FAT32 or exFAT stick (an
   agent's export onto it, the settings on it refused in words).
3. **Ears**: what `write_part`, `voice_chords` and `pedal_from_chart` write; the mastering
   assistant's masters and its A/B on real material; your own recordings into MIDI (a sung
   line, a piano, a drum loop).
4. **The legal review** gains Basic Pitch and our port of its note creation, the ONNX
   conversion of Magenta's drum model, HT-Demucs and the Visual C++ runtime's terms.
5. **Everything rounds 30–33 left**: the Mac session (`docs/mac-session.md`, now with row 17
   and later: the bridge and the ML helper in the signed bundle, CoreML) and the Apple
   account, the hardware of round 33 (a Launchpad or an APC, Link against Live, MIDI clock
   and MTC, an MPE controller), a real Linux desktop, Flathub, compacting the WSL disk image,
   code signing, the RVR1960 permission, the trademark filing, the UMC in duplex; REAPER and
   FL Studio for the comparisons.
6. **Round 35: the dancing character** (BACKLOG §6i, whole — left to Claude by the owner,
   2026-10-29), with video export as its risk. BACKLOG §7 keeps phase C (the assistant
   panel) and phase E (generation) open. The prompt is `docs/next-session.md`.

## The next session: what round 33 left

Round 33 is on **master** (squashed) and published to `luis314159/cantus-daw`; **`v0.1.0-beta.8`**
is released for Windows and Linux. The account: `docs/round33/integration.md`; one report per
builder beside it (`session`, `launcher`, `sync`, `mpe`). Under the owner's rule what is left is
the owner's own, or new work:

1. **Hardware** (`docs/beta-checklist.md`): a Launchpad or an APC (the Pro MK3's side buttons and
   Stop Clip, the APCs' blink rates and colours); Link against Live or a phone app (LinkHut could
   not be built here); MIDI clock and MTC with a device, or loopMIDI / Windows MIDI Services for a
   loopback; an MPE controller and an MPE plug-in.
2. **Ears**: a performance — launching, legato and follow actions on real material.
3. **Everything rounds 30–32 left**: the Mac session and the Apple account, a real Linux desktop,
   Flathub, compacting the WSL disk image, the legal review (model names, Chorda, the CLA), code
   signing, the RVR1960 permission, the trademark filing, the UMC in duplex; REAPER and FL Studio
   for the comparisons.
4. **Round 34: AI** (the owner's order, 2026-10-28): the MCP server first, then stems, audio to
   MIDI and a mastering assistant. **Round 35: the dancing character** (BACKLOG §6i, whole —
   left to Claude by the owner, 2026-10-29). MIDI 2.0's transport is sized in ADR 0080. The prompt
   is `docs/next-session.md`.

## The next session: what round 32 left

Round 32 is on **master** (squashed) and published to `luis314159/cantus-daw`; **`v0.1.0-beta.7`**
is released for Windows and Linux. The account: `docs/round32/integration.md`; one report per
builder beside it (`skin`, `score`, `instruments`, `dynamics`, `modulation`). Under the owner's
rule what is left is the owner's own, or new work:

1. **Look and listen** (the owner's eyes and printer): the units' faces in both themes, the
   score's engraving against Dorico or MuseScore, a printed page, the MusicXML in MuseScore 4
   (installed on this machine).
2. **The legal review** now also lists Concentus's "Juno I/II" and "Dimension 1–4" (kept by the
   owner's decision, 2026-10-28) beside the cores' vehicle model names, Chorda and the CLA.
3. **Everything rounds 30–31 left**: the Mac session and the Apple account (beta.8 can then
   ship the `.dmg`), a real Linux desktop, Flathub, compacting the WSL disk image, listening to
   round 31's features, the UMC in duplex, code signing, the RVR1960 permission, the trademark
   filing; REAPER and FL Studio installed for the comparisons.
4. **Round 33, in the owner's order (2026-10-28)**: Ableton's core — the session view, Ableton
   Link, MIDI clock and MTC, MPE; then **round 34: AI** (the MCP server first, then stems, audio
   to MIDI and a mastering assistant). The prompt is `docs/next-session.md`.

## The next session: what round 31 left

Round 31 is on **master** (squashed) and published to `luis314159/cantus-daw`; **`v0.1.0-beta.6`**
is released for Windows and Linux. The account: `docs/round31/integration.md`; one report
per builder beside it (`automation`, `midi`, `workflow`, `editing`, `vehicles`). Under the
owner's rule what is left is the owner's own, or new work:

1. **Listening** (the owner's ears): the arpeggiator's feel, the swings against an MPC, the
   chord voicings, each starting preset, the crossfade curves on real material, Touch and
   Latch passes, a keyboard's bend, mod wheel, sustain and release on Lamina and a hosted
   synth.
2. **The Mac session** (`docs/mac-session.md`) and the Apple account — beta.7 can then ship
   the `.dmg` (`CANTUS_RELEASE_MACOS=true`); a real Linux desktop; Flathub; and the rest of
   round 30's owner list (the UMC in duplex, code signing, the legal reviews — now also the
   cores' vehicle model names, one table in `engines.rs` —, the RVR1960 permission, the
   trademark filing, compacting the WSL disk image).
3. **After beta.6, same session (on master, not in a release yet — beta.7 carries them)**:
   BACKLOG **§6h point 2 is done** (ADR 0073, `docs/round31/hosted-view.md`): hosted
   plug-ins open their own editor; *Plug-in window | Cantus view* switches to their
   parameters drawn with the kit (pages, search, meters, the parameter menu on every
   control), *Use as default for this plug-in* is kept in the settings by format and id; in
   and out of process; the shared frame's header no longer elides a name it has room for.
   Reviewed twice (8 findings, then the fix round), each fix with a test that fails without
   it. **The macOS FLAKY of beta.6's run** (`session::a_crash_mid_song…`) was the rig's
   fixed 64-block settle racing an isolated plug-in's restart after its new latency — test
   code only, a test that forces it (`docs/round31/mac-flaky.md`).
4. **Round 32, in the owner's order (2026-10-27)**: §6h point 1 — a face of its own for
   each of our units (`kit::skin`) — and the score from the piano roll (§0.2 notation:
   engraved staff, MusicXML and PDF). After them, when there is what to do it with:
   Ableton's core (session view, Link, MIDI clock/MTC, MPE), the measured comparison with
   REAPER and FL Studio once they are installed here, the Mac once it is borrowed. The
   prompt is `docs/next-session.md`; §7a (singing voice) stays for later.

## The next session: what round 30 left

Round 30 is on **master** and published to `luis314159/cantus-daw` (pushed 2026-10-22 with
the owner's permission given in advance). The account: `docs/round30/integration.md`; one
report per builder beside it (`linux`, `macos`, `ship`). Under the owner's rule what is left
is the owner's own:

**Released 2026-10-23: `v0.1.0-beta.4` for Windows and Linux** (the installer, the portable
zip, the plug-ins and the AppImage; the website's Linux card says *Available* by itself). The
Mac was held back on the owner's word: after seven runner rounds (Mac-only fixes: no
allocation on the callback's first block, the helper's crash seen at once, a helper started
off the UI thread, fsync instead of `F_FULLFSYNC` for the sample cache, tests that assumed two
systems) the macOS suite still fails on timing in GitHub's virtual Mac — deadline waits end
0.4–4.4 ms late even on a time-constraint thread (`ipc_process.rs`), and the sandbox's
`session`/`bridge_faults4` timing. `release.yml`'s Mac job runs only while the repository
variable `CANTUS_RELEASE_MACOS` is `true`; beta.6 turns it on once the Mac session has
measured a real Mac and the suite is green there.

**Released 2026-10-23: `v0.1.0-beta.5`** — the Linux fix for isolated plug-ins (a helper
started on a short-lived thread was ended by `PR_SET_PDEATHSIG` when that thread ended; the
starting thread now watches the helper for its life). The same commit was the **first fully
green macOS run** on GitHub's runner (`macos.yml` reruns a failed test binary up to twice and
reports it as *FLAKY on macOS VM*: `bridge_faults4` needed it). Still open for the next round:
`session_faults5::an_instance_removed_during_a_dialog_is_ended_after_it` fails now and then on
Windows runners (2 instances destroyed under the dialog in the beta.5 release run; the job
was re-run) — find the cause, not a longer wait.

1. **The Mac session** (`docs/mac-session.md`): the `.dmg` past Gatekeeper, the microphone
   prompt, an interface in duplex for 10 minutes, a real plug-in's editor in and out of
   process, menus and keys, trackpad and Retina, CPU on Apple silicon, an update from an
   older build, a project opened from Finder. **The Apple Developer account** and the five
   `APPLE_*` secrets for a notarised image.
2. **A real Linux desktop** for an hour: an interface in duplex on PipeWire, real plug-in
   editors (Surge XT, Vital, a JUCE VST3) on X11 and Wayland, rtkit granting real time;
   submitting the Flatpak to Flathub.
3. **Still standing**: the UMC in duplex and the interface table, complete songs, SignPath
   or Certum, the RVR1960 e-mail, the IMPI filing, the legal review of Chorda and the CLA;
   compacting the WSL disk image (C: fell to 2.8 GB this round).
4. **Next round (31, proposed)**: BACKLOG §0.4 item 3, the daily gaps (automation modes and
   the piano roll's CC lanes, arpeggiator/chords/scales, crossfades and clip gain, a
   command palette, undo history, templates) and effects that sound on insert. The
   prompt: `docs/next-session.md`.

## The next session: what round 29 left

Round 29 is on **master** (pushed 2026-10-21 with the owner's permission given in advance)
and published to `luis314159/cantus-daw` through `tools/public/publish.sh`; the tag
`v0.1.0-beta.1` there built the Windows installer the website's download button serves. The
account: `docs/round29/integration.md`; one report per builder beside it (`tempo`, `warp`,
`controllers`, `isolation`, `freeze`). Under the owner's rule — every gap a feature leaves is
closed in the same round — what is left is the owner's own:

1. **By hand and by ear** (`docs/beta-checklist.md`, round-29 block): the warp listening set
   (`F:\projects\daw-r29\shots\warp\listen\`), a ramp and a metre change, markers and
   *Export between markers*, MIDI learn with a real controller (relative mode, feedback), a
   real plug-in in its own process (also one with a licence dialog, and *Reload* after
   killing its helper), a heavy track frozen and unfrozen, a voice with low-latency
   monitoring; **the installer of `v0.1.0-beta.1`** downloaded from cantusdaw.com, installed,
   opened, uninstalled (SmartScreen warns while it is unsigned).
2. **Still standing**: the UMC in duplex and the interface table, complete songs, SignPath
   or Certum, the RVR1960 e-mail (the beta greets with the RV1909), the IMPI filing, the
   legal review of Chorda and the CLA.
3. **Next round (30, decided 2026-10-20): the three systems** — Linux complete (PipeWire and
   JACK, real-time priority, plug-in editor windows on X11 and Wayland, isolation end to
   end, an AppImage in CI) and macOS prepared without a Mac (BACKLOG §2's list), then one tag
   publishing three installers. The prompt: `docs/next-session.md`.
4. **Then** (BACKLOG §0.4's order): effects that sound on insert (§3b, option B), Chorda's
   sound by ear, §4b's performance scorecard, the MCP server (§7 phase B).

## The next session: what round 28 left

Round 28 is on **master** (pushed 2026-10-16 with the owner's permission given in advance). The
account: `docs/round28/integration.md`; one report per builder beside it (`voice`, `modfx`,
`topbar`, `pitch`, `dynfx`, `leaders`, `sound`, `demo`) and one per review round (`fix2` …
`fix6`). Under the owner's rule — every gap a feature leaves is closed in the same round —
what is left is the owner's own:

1. **Listen and judge** (`docs/beta-checklist.md`, round-28 block): Tonus and the pitch
   editor on a real voice (demo projects in `F:\projects\daw-r28\shots\voice\demo\` and
   `…\pitch\demo\`), the twelve effects and their presets, Chorda (and the blind test against
   Pianoteq 9 and a sampled grand; the legal review of its method against the patents named
   in ADR 0059), DRSKit by hand, modulators and comping, the top bar against FL's strip on
   the shots in `F:\projects\daw-r28\shots\topbar\`, and *With All Your Heart*
   (`F:\projects\daw-r28\shots\demo\`).
2. **Decided since (2026-10-16)**: an inserted effect sounds at once — inserting loads a
   musical starting preset, the defaults stay a null (option B, BACKLOG §3b); **Chorda does
   not sound good yet** to the owner — make it sound like a first-rank grand by ear against
   references before Chorda II (BACKLOG §3e). Still the owner's: keep or delete
   `F:\projects\daw-r28\downloads\` (the DRSKit archive the real-kit tests read); optionally
   ask Wilkinson Audio for Naked Drums' licence in writing. The round's worktrees are
   removed (branches kept locally; `builds\`, `shots\`, `downloads\`, `reviews\` kept).
3. **Still standing from round 26**: the UMC in duplex, complete songs, the installer on a
   clean machine; SignPath or Certum; the public-day choices; the RVR1960 e-mail; the IMPI
   filing; compacting the WSL disk image.
4. **Next work** (BACKLOG): §4b's performance scorecard; §7 phase B — the MCP server, with
   what the demo builder learned composing in code (§7, 2026-10-15); effects that sound on
   insert (§3b, option B); **Chorda's sound** by ear, then Chorda II (mic perspectives, the
   lid, per-key voicing); per-mic drum faders (multi-output instruments,
   §3e item 11); the rest of §3b (distortion pedals, delays, reverbs, vocoder…).

## The next session: what round 26 left

Round 26 is on branch **`r26/integration`** (worktree `F:\projects\daw-r26\integration`),
to go to master as **one squashed commit** when the owner says (the public builder's first
commits named the owner's work address in documents; squashing keeps those blobs out of
master's history). The account: `docs/round26/integration.md`; one report per builder
beside it. Under the owner's rule this round — every gap a feature leaves is closed in the
same round — what is left is the owner's own, or upstream:

1. **The owner, by hand, with the real build** (`docs/beta-checklist.md` §1, which has the
   round-26 list): the UMC in duplex for 10 minutes; 2–3 complete songs; the installer on a
   clean machine, including its new **Privacy** page and the *Check for updates* box; listen
   to the **Salamander** (first load, 8 and 16 layers, the pedal noises); Alt+N, Alt+F5 and
   painting with B; the cable band, the typed level, the one text field (Esc cancels, a
   click away commits); a track named in Japanese.
2. **The owner's decisions**: SignPath or Certum — SignPath refuses commercial dual
   licensing and `CLA.md` §2.1/§3 keep one open (remove the clause, ask SignPath, or sign
   with Certum/Azure; `docs/signing/README.md`); on the day the repository goes public, the
   work address in 248 commits (keep, or rewrite to the GitHub no-reply address), a new
   repository or the existing one, whether `docs/claude-memory/` is published
   (`docs/round26/public.md`). The RVR1960 e-mail, the IMPI trademark filing.
3. **The machine**: the WSL disk image (`ext4.vhdx`, 147 GB on C:, 8 GB used inside) needs
   an administrator to compact it (`wsl --shutdown`, then `diskpart`: `select vdisk
   file=…\ext4.vhdx`, `attach vdisk readonly`, `compact vdisk`, `detach vdisk`).
4. **Upstream only**: egui's own allocations on a hover (7 in release); egui's lack of text
   shaping (Arabic and Hebrew names draw unjoined and left to right — a known issue in the
   notes and the manual).
5. **Done since** (round 27, 2026-10-14, branch `r27/gestures`, ADR 0051): BACKLOG §6g —
   a double-click on a track header opens its instrument, a click **exactly on the name's
   letters** renames it (typing never does; Esc cancels), every *Open editor* and F8 bring
   the window to the front (restored if minimised; F8 closes only what is already in
   front), a double-click on a clip opens the piano roll, or the channel rack when the rack
   can show the clip without losing anything (`docs/round27/gestures.md`). The owner, by
   hand: a real CLAP/VST3 instrument minimised, brought back with *Show*, F8 or a header
   double-click. **Next:** the beta (the owner's tests and decisions above), then BACKLOG
   §4b's performance scorecard, §6e's top bar, §3c/§3d's pitch editing.

## The next session: what round 25 left

Round 25 is on **master** (14ddaf5, pushed 2026-10-11 as one commit of `r25/integration`):
the contract (piece A: core routing, `SendTap`, schema 23), the builders
`r25/{interaction,matrix,desk}` (worktrees in `F:\projects\daw-r25\`) and `r25/look`
merged, then the fixes of three reviewers' 27 findings and the owner's click mode. Then,
on the owner's word, the three things the report had left (branch `r25/polish`): the
0 dB pan law exactly unity (`DSP_VERSION` 4, no golden moved), the hint bar keeping the
modifiers whole, the automatic icon colour for a light taskbar; and a key to the cable
band's lines (the owner asked why the demo's kick cables were dotted — its sidechain
keys). And the licences revised (ADR 0047): the engine crates MPL-2.0, the application
GPL-3.0-or-later, `CLA.md`, `TRADEMARKS.md`, `CONTRIBUTING.md`, the website's wording. The
owner's to-do list for the public beta, in Spanish, with the trademark search, the signing
choice and the RVR1960 permission: **`docs/beta-checklist.md`**. Then (2026-10-12): the
**cable view options** (the owner: "only the current channel's sends") — a filter key beside
Off · Selected · All: what the selection shows (out / in / both), which kinds (outputs,
sends, keys), the detail (send levels, outputs to the master), quick views, kept per layout
(`docs/round25/cables-view.md`, ADR 0043 §7); the **RVR1960 gated** until its publisher's
written permission (`verse::RVR1960_PERMITTED`, not in the build; the RV1909 the default;
the request `docs/rvr1960-request.md`); the ™ on the site, README and About; the website's
17 screenshots in Cantus Night (`shoot_site` makes the hero). And **Cantus Night refined for long
sessions** (2026-10-13, the owner: "more pleasant to work for hours, gradients like the
website"): calmer surfaces and text (primary 12.7:1), track colours veiled in large fills,
the website's light as four theme tokens (a top sheen on bars, copper and verdigris glows
behind empty grounds), no extra allocation or draw call (`docs/round25/night.md`, ADR 0046
§8). Reports: `docs/round25/`
(`integration.md` is the account).

1. **The owner, by hand, with the real build**: route by drag and by two clicks, with and
   without Ctrl and Alt, and undo each; a PRE send heard before the fader (pull the fader
   down: the send keeps sounding); the matrix (Alt+F9); move a group by drag and by
   Alt+arrows; drop a track header on a strip; switch the twelve themes; change the taskbar
   icon's colour with Cantus **pinned** to the taskbar (does the running button follow, or
   keep the shortcut's icon? — if the latter, set a relaunch icon on the window); the
   round-24 checks (2048-frame ASIO recording, a large save while playing, linked clips).
2. **The beta** (unchanged from round 24): a 10-minute live run on a real device with 0
   overruns; the hardware matrix (UMC in duplex); an installer run by hand; the tag
   `v0.1.0-beta.1`; SignPath, the trademark search, songs with the friction log.
3. **Small things left** (`docs/round25/integration.md`, `look.md`): an armed route
   survives the desk being hidden (Esc lets it go); typing a level in the route menu; the inspector's *Add
   send* could offer *New return*; cable lanes re-share as hovering lights more cables;
   ~17 allocations a frame while a desk control is hovered (the hint bar); swatches in the
   Look tab's base-theme menus; CC BY in `cantus-content` (the manifest test, a credits view,
   the notices' Content section — ADR 0042).
4. **Then** the rest of §6d.2 (an Inspector row for a link group, a "Link to…" picker),
   a loom model of `cantus_rt::fork`, a quiet-machine `xtask perf --full --save` baseline,
   and BACKLOG §3e (the instruments researched this round).

## The next session: what round 24 left

Round 24 is on branch **`r24/integration`** (worktree `F:\projects\daw-r23\beta-hardening`,
on master a9871e2), integrated from `r24/{beta-rest,linked,perf2}` (worktrees in
`F:\projects\daw-r24\`, their diffs applied; they can go once this is committed) and the
soak's journal fix. Report: `docs/round24/integration.md`. The 2-hour soak on this tree:
**passed** — 120 minutes on a quiet machine (7 202 s), no frame over 250 ms, handles and threads flat, private bytes +70 MB with 199 s of recorded audio (allowed 96 MB + the audio): BETA blocker 3's long session holds (round 23's master had failed at 65 min on the journal's fsync).

1. **The owner, by hand, with the real build**: record with a large ASIO buffer (2048) —
   the pass must start and stop cleanly; save a large project while it plays (the window
   must not stop); link a Pulsus pattern, edit one copy, *Make unique* another, and listen
   that nothing else changes; the export and clipboard checks round 23 left; Lamina's
   level (still ~4 LU under the others, capped by the headroom rule) by ear.
2. **The beta**: BETA blocker 2's second half — a 10-minute live run on a real device
   with 0 overruns (now that the demos sit ≤ 32 % offline); the hardware matrix (UMC in
   duplex); an installer run by hand; then the tag `v0.1.0-beta.1`. The owner's items:
   SignPath, the trademark search, two or three songs with the friction log.
3. **Then** BACKLOG §6d.3 (mixer cabling — researched and designed:
   `research/mixer-routing/notes.md`, `docs/round25/mixer-routing-design.md`, with 11
   questions for the owner and six builder pieces; no schema change needed), §6d.4 (the white-and-blue
   default theme and logo), the rest of §6d.2 (an Inspector row for a link group, a
   "Link to…" picker), a loom model of `cantus_rt::fork`, and a quiet-machine
   `xtask perf --full --save` baseline for this machine (a Ryzen 7 7735HS, not round 22's
   i7).

## The next session: what round 23 left

Round 23 is on branch **`r23/beta-hardening`** (worktree `F:\projects\daw-r23\beta-hardening`,
base master 2ad043b), **not committed** — the owner commits it when they ask. Copy and
paste was built beside it on `r23/clipboard` (`F:\projects\daw-r23\clipboard`) and its
diff applied here; that worktree can go once this branch is committed. BACKLOG §1's two
bugs (the export dialog, text after a theme change) and §6d.1 (copy and paste) are done;
`docs/BACKLOG-excellence.md` was left alone because the owner has uncommitted edits to it.

1. **The owner, by hand, with the real build** (`cargo build --release -p cantus`):
   - export: every list of the dialog opens and chooses; a selection's tail rings out
     alone; a stems export with a hosted plug-in (a commercial one — the tests use ours);
   - copy and paste: Ctrl+C, close Cantus, open another project, Ctrl+V; Ctrl+V after
     copying text in Notepad (falls back to the in-app copy, or nothing after a restart);
     two Cantus windows; *Edit ▸ Paste* (eframe answers the paste request two frames
     later — tested, not yet seen in a real window);
   - switch Dark ↔ Light a few times (and *System* with the OS flipping) and look at the
     labels.
2. **The rest of the beta** (`docs/BETA.md`, round 22's list): the hardware matrix (the
   Behringer UMC in duplex, plugged in *before* starting Cantus), a 2-hour soak
   (`CANTUS_SOAK_MINUTES=120`), instrument gain staging, the recovery dialog in
   `examples/shoot*`, Save off the UI thread, an installer run by hand, then the tag
   `v0.1.0-beta.1`.
3. **Then** performance wave 2 (round 22's item 2) and the owner's §6d requests: linked
   clips (§6d.2 — the clipboard's format `cantus-clip/1` will need a v2 for a link id),
   mixer cabling (§6d.3), the white-and-blue default theme (§6d.4 — unblocked: the theme
   text bug was its prerequisite).

**On the F: machine** (the one this round ran on): `cargo` is not on PATH (prefix
`export PATH="$HOME/.cargo/bin:$PATH"`); incremental builds stop linking after a few
edits (MSVC LNK2019 on `anon.*.llvm` symbols — put `incremental = false` under
`[profile.dev]` and `[profile.test]` in the worktree's untracked `.cargo/config.toml`, or
`CARGO_INCREMENTAL=0`); and Windows' installer detection refuses to start the test
binaries named `audio_setup*` and `cantus_update` ("requires elevation", os error 740) —
run them copied to a neutral name (`audio_stp-….exe`); all 24 of their tests pass.

Left by round 23 (its docs list each): a hosted insert on the master is a wire in
*through the master* stems (one instance cannot be in two places); the stems' float
scratch folder sits beside the destination; Ctrl+V with no text on the system clipboard
sends no event at all (egui-winit), so it cannot even say "nothing to paste"; the
playhead split (`clips::split`) still silences the right half of a looping clip (the
clipboard's crop avoids it); automation clips are not pasted across projects.

## The next session: what round 22 left

The owner's goal is the first public beta (`docs/BETA.md`). Next, in order:

1. **Round 23 — beta hardening** (`docs/BETA.md`): the hardware matrix (the Behringer UMC
   and other interfaces, in duplex), a 2-hour soak (`CANTUS_SOAK_MINUTES=120`), instrument
   gain staging, the export dialog's bar wired to `render_with_progress` (pool added it;
   export's UI still pulses), the recovery dialog in `examples/shoot*`, Save off the UI
   thread, an installer run by hand (taskbar grouping, an update over a round-21 install),
   then the tag `v0.1.0-beta.1`.
2. **Performance, wave 2** (`docs/perf/plan.md`, ADR 0031): the demos at 256 frames with
   zero misses are gated by one instrument each (Calor, Unda, Exemplar take 50–70 % of a
   period alone) — **voice-level sub-tasks** on the pool (summed in voice order, so the bits
   do not move) and the CLAP `thread-pool` extension; hand SIMD kernels and FMA used on
   purpose (a `DSP_VERSION` bump: v3 alone gave 0.995× of v2); a fast `log10` for the
   meters; a quiet-machine `xtask perf --full --save` (every round-22 number was taken on a
   machine loaded by the other builders); `loom` models of the queues and cells; CI with
   `CANTUS_WORKERS=3 cargo test -p cantus-engine --features fuzz`.
3. **The owner**: try the round-22 build (`C:\projects\daw-r22\builds\cantus.exe` on the first machine; elsewhere `cargo build --release -p cantus`) with the Behringer UMC; apply for SignPath; a
   trademark search for "Cantus"; make two or three songs and log the friction; decide on
   mirroring the ASIO SDK zip as a release asset (beta-docs' note) and on the in-app
   FlexASIO install; before the repository is ever made public, take the research
   screenshots out of its history (`docs/BETA.md`).

**The owner's test build (2026-10-07).** `C:\projects\daw-r22\builds\cantus.exe`,
`cantus 0.1.0-beta.1 (x86-64)`, built by `cargo build --release -p cantus` in the fresh
clone at 745fb70 (the code of master 7a841ec; only documents changed after it). What it is:

- **The multicore live engine is on by default**: the stream's processor gets
  `Workers::auto()` (4 workers on the round's i7-12700H); *Settings ▸ Audio ▸ Audio cores*
  changes it. Every round-22 feature is in it (export to WAV/FLAC/MP3/Ogg, crash recovery,
  the count-in without the metronome, `DSP_VERSION` 3).
- **One program, every driver**: WASAPI and ASIO, the SDK linked (`--audio-drivers` →
  `audio drivers: WASAPI, ASIO`, `asio sdk: linked`); it listed ASIO4ALL v2, Realtek ASIO
  and the UMC ASIO Driver on this machine.
- **Known**: the Behringer UMC has never been tried in duplex with the interface present —
  plug it in **before** starting Cantus (its driver crashed a process that loaded it while
  the interface was unplugged); Realtek's own ASIO driver delivers no buffers on this laptop
  (a driver fault) — use WASAPI or ASIO4ALL for the built-in audio.
- It is the plain single program, with `cantus-scan.exe` (the plug-in scanner, built by
  `cargo build --release -p cantus-host --bin cantus-scan`) copied beside it so
  third-party plug-ins can be scanned; the packaged release (`cargo run -p xtask --
  package`) adds the `cantus.exe` launcher and the v2/v3 programs.

**Tests at the close (both finished).** Integration tree: 1 824 green (185 test binaries),
the engine's fuzz at `CANTUS_WORKERS=3` green, clippy clean; Calor's per-voice CPU test
failed once under load and passed alone. Fresh clone: 1 823 of 1 824, the one failure
`inst_bass::a_cpu_budget_holds` (CPU timing under load), which passes alone. Round 23
should make both timing tests robust to a loaded machine (a median over several runs, or
ignored by default like the other measurements).

Left by the round-22 builders (their docs list each): Spatium, Organum, Pulsus and the keyed
gate cannot rest yet; round 21's `_set_FMA3_enable(0)` can go now that `powf` is ours;
hosted plug-ins in stems; RF64, AAC/Opus, album art; the old stems dialog in
`views::interop`; v4 never run (no AVX-512 here).

## Round 34: AI — the MCP server, the project's tools, listening and mastering, stems, audio to MIDI (requested 2026-10-28)

**The ask:** BACKLOG §7 phases B and D, with round 28's demo wishlist. Five builders (`mcp`,
`tools`, `listen`, `stems`, `transcribe`) from contract `b4568467`, every cargo command
queued through a slot script (three build slots, one WSL); one reviewer per branch, one for
every fix round, one on the integrated tree (13 seams) — every finding fixed with a test.
Account: `docs/round34/integration.md`.

| Part | What changed |
|---|---|
| MCP server (ADR 0081) | `crates/cantus-mcp` (GPL): MCP written by hand, JSON-RPC 2.0 over std threads, both generations (2025-06-18/2025-11-25 and the stateless 2026-07-28); the pump on the UI thread with `CantusApp` as host, each planned command one undo step with `Origin::Mcp`, named "AI: …"; permissions by class decided in Cantus; jobs with handles, `wait_job`/`cancel_job`, `begin_change`/`end_change`, `undo_last_ai_action`; the HTTP door on `127.0.0.1` only, off by default, a token per client compared in constant time; the `cantus-mcp` stdio bridge proving the door on every connection; owner-only secrets in `<settings>/mcp/` (`cantus-rt/src/private.rs`); the confirmation card, the agent chip, the AI activity, Settings ▸ AI (*Copy setup for…* six clients, a `.mcpb` for Claude Desktop), one off switch; the bridge in every package; `tools/list` held by a golden file (63 tools) |
| Project tools (ADR 0082) | 47 tools in one table (36 + `replace_automation` in `cantus-ai`, 10 in `cantus-ui`): read, the song, tracks and instruments, clips and notes, drums, the session, the mixer and effects, automation and modulators, presets, comping, export, removal; `write_part`, `voice_chords`, `pedal_from_chart`, `automate`; the unit catalogue from the parameter tables; the music linter and harmony in `cantus-core/src/harmony*.rs` (MPL); resources and five prompts; the tools' fuzz (64 × 80 calls); `InsertNotes`/`SetNotes` sort by (start, pitch, id) |
| Listening (ADR 0083) | Speculum's headless ear; `cantus_engine::listen` (the export's render, analysis taps that change no sample); `Processor::fork` and every unit's `fork` — range renders from checkpoints equal to the whole render to the bit, after a later edit too; live = offline on the application's own engine; `audition`, `analyse_tracks`, `masking_matrix`, `set_mix_targets`; a pre-roll where a unit cannot be copied, said |
| Mastering assistant (ADR 0084) | `master_assist`, `apply_master_chain`, `replace_master_chain`, the prompt *Master for streaming*; the panel from the master strip's menu and the master chain's *Add effect*: nine platforms with sources and dates, six genre curves or a reference track, a chain of our effects with every setting and why, A/B at matched loudness, *Apply* as one undo step; 21 demo cases within 0.07 LU, never over the ceiling |
| Stems (ADR 0085) | `crates/cantus-ml` (GPL) and the `cantus-ml` helper process; ONNX Runtime (measured against tract and candle), downloaded on demand, telemetry off; models through the content manager (licence accepted, resumable, hash-checked, weights rebuilt from Meta's file); *Separate stems…* (4/6 stems, *Fast*/*Best*) into a bus and a track per part in one undo step, level-true under every pan law; `Project::provenance`, `SetAudioProvenance`, schema 48, the "AI" badge; Settings ▸ Models; SDR 8.80 dB on MUSDB18-HQ, the paper's; on Windows the helper and its Visual C++ runtime in `ml\` |
| Audio to MIDI (ADR 0086) | *Convert to MIDI…* and `audio_to_midi`: Basic Pitch's `nmp.onnx` with its note creation ported to Rust (3 286 of 3 286 notes identical to Spotify's); melody by our pitch tracker (0.805 vs 0.746); drums by our detector (`cantus-dsp/src/onset_drums.rs`, MPL) and Magenta's Onsets and Frames Drums (its hi-hat output re-pointed at install); through the clip's own time mapping, one undo step |
| Showcase | README: *AI that works with you* (six pictures); the landing page: *AI that works with you, on your computer* (EN/ES); manual pages EN/ES; release notes; PRIVACY's AI section; round 35's prompt (the dancer) |
| Causes | mcp's change tests raced on the idle zero (closed by ordering); the pre-roll's limit is Space's FDN rotors and Organum's rotary (no DSP change, BACKLOG §4b); a checkpoint was 80 MB whatever the song (cells in use only, a byte budget); the drum model's hats read pitch 42, not 46; the VC++ DLLs beside `cantus.exe` shadowed other programs and plug-ins (moved to `ml\`) |

## Round 33: Ableton's core — the session view, Link, MIDI clock and MTC, MPE (requested 2026-10-28)

**The ask:** BACKLOG §0.2 *Clip launching / session view* and *Sync and networking*, §0.4 point 5.
Four builders (`session`, `launcher`, `sync`, `mpe`) from contract `e0853ec1`; one reviewer per
branch, one for every fix round, one on the integrated tree (10 seams) — every finding fixed with
a test. Account: `docs/round33/integration.md`.

| Part | What changed |
|---|---|
| Session engine (ADR 0076) | `Track::slots`, `Project::session`, `SessionClip`, `Command::Session`, schema 46; a pure planner (bar-line launches, trigger/gate/toggle/repeat, legato, seeded follow actions, scene tempo and metre) and a conductor on the interface's side playing the performance as a projection — no session code on the audio thread; the lead = two device blocks + the compile's p95 + 2 ms, late presses caught up on their boundary, per-track compiles, cut notes listed off the audio thread; recording into slots (count-in, fixed length, overdub, MIDI and audio laid ahead for a seamless replay, crash recovery into the slot) and into the arrangement in one undo step; session audio and warp renders preloaded; launches from stop wait for Link's quantum or a master's Start |
| Session view (ADR 0077) | The Session pane (`Tab`), clip and scene panels, envelopes, editing both ways with the arrangement, keymap 14, MIDI learn of slots/scenes/stops/quantise, Launchpad Mini MK3/X/Pro MK3 and APC mini mk2/APC40 mkII (given back on quit, faders free for learn), Settings ▸ MIDI's inputs table (Notes, Controls, Clock, MTC, MPE, Grid) that fits and scrolls |
| Sync (ADRs 0078–0079) | `crates/cantus-link` (Link in Rust, every up IPv4 interface rescanned, hostile peers capped, tests on loopback only); the MPL seam `cantus-engine/src/sync.rs`; following with the beat matched when heard; clock and MTC out stamped per sample and sent at speaker time, one connection per output port shared with controller feedback; clock in (α-β-γ tracker) and MTC chase by piece number; tempo proposed to Link only when the user or a scene asks; the top bar's Sync group |
| MPE (ADR 0080) | `Note::expr` at MIDI 2.0 resolution, `SetNoteExpr`, `SetInstrumentExpression`, schema 47; Unda, Calor, Modulus, Gravis per note; live MPE from marked ports (other ports' RPN and panics kept out); recording per member channel; CLAP note expressions, VST3 note expression, MPE over MIDI (sandbox protocol 7); MPE MIDI files (zones sized from polyphony) and DAWproject; three lanes under the piano roll; MIDI 2.0's translation layer |
| Showcase | README: *The session view, Link and MPE* (six pictures); the landing page: *Play it live* (EN/ES) |
| Causes | F: filled twice while three builders linked (targets now deleted as each merges); a lib test installed the light palette while another read it (Linux only); timing tests under load made robust |

## Round 32: a face of its own for each unit, and the score from the piano roll (requested 2026-10-27)

**The ask:** BACKLOG §6h point 1 (each unit's own face, `kit::skin`, the exported plug-ins
wearing it) and §0.2 *Notation* (the score from the piano roll: an engraved staff, MusicXML,
PDF, printing, editing). Five builders (`skin`, `score`, `instruments`, `dynamics`,
`modulation`) from contract `13986457`, then a showcase pass; ten one-agent reviews, every
finding fixed with a test. Account: `docs/round32/integration.md`.

| Part | What changed |
|---|---|
| The kit (ADR 0074) | A `Skin` is data — surface tint, accents per theme, material, display, knob and switch shapes (custom painters allowed), lettering, plate, ornaments; `Style.skin`, `Style::skinned` (the contrast floor after every tint and over every material), the worn style inside a unit's window, `display_style` for drawing inside a display (fills and polarity of the display's own ground), voices by the display's polarity, cue hues that never wear the accent; *Units wear their own faces* on the boot path; thumbnails keyed by a stable digest and pruned; the frame budget with three distinct rich faces (p95 ≤ 4 ms at 200 % and 2 px/pt) |
| The faces | Instruments: Unda, Calor, Modulus (membrane keys), Exemplar, Gravis, Lamina, Organum (stop tabs), Chorda (soundboard and hammers), Pulsus. Dynamics: Comprimo (VU needle, 65 ms), Terminus, Multiplex, Sibilus, Porta, Culmen (LED-ring encoder), Tonsor, Impetus, Discrimen. Modulation: Concentus, Flexus, Phasis, Tremor, Vibrans, Anulus (ring knob), Translatio, Fluctus. Tone and space: Aequalis, Fervor, Spatium, Latitudo, Speculum, Tonus. Delay, Reverb and Firmus keep the plain look |
| The score (ADR 0075) | `crates/cantus-score`: reading for the page (straight and tuplet grids per beat), two voices per staff, ties and rests after Gould, beams by metre, spelling by key and mode, clefs and 8va, a grand staff that follows the hands, dynamics from velocity, tempo and metre marks; engraving in Bravura (spacing by duration, systems broken over the whole piece, a last system at least half full justified); MusicXML 4.0 out and in (XSD-valid, opened in MuseScore 4), PDF (fonts subset, windings normalised), printing (the path passed as data); the staff under the piano roll, the Score pane, per-track score settings (`SetTrackScore`, schema 45), editing on the staff (keymap 13); the music font only on the app's path |
| Showcase | README: every effect and instrument in its face, a score section (49 pictures); the landing page: a score section and a gallery of the 32 faces, EN/ES |
| Causes | `freeze.rs`'s sweep deleted other runs' renders in a shared temp folder (test code); an instrument card built its rings by hand and wore the accent (one builder now) |

## Round 31: the daily gaps an FL or Ableton user hits first (requested 2026-10-25)

**The ask:** BACKLOG §0.4 item 3 and what round 30 left: the flaky sandbox tests' causes;
automation write/touch/latch and the piano roll's lanes; note effects, scale and grooves;
crossfades and clip gain; a command palette, an undo history, templates; effects that sound
on insert. Four builders (`automation`, `midi`, `workflow`, `editing`) from contract
`697550fa`, plus an illustrator for the cores' vehicles; six reviews, **57 confirmed
findings, all fixed with tests**. Account: `docs/round31/integration.md`.

| Part | What changed |
|---|---|
| Automation (ADR 0068) | Read/Write/Touch/Latch per track and master, the top bar's arm and touch glide, one pure writer driven each frame, a pass folded into one undo step; every project replacement ends a pass into the song it rode (`reset_bus_to`); `Note::release`, `Clip::lanes` (bend, pressure, CC 0–119) compiled and fired before note-ons, chased on play/seek/wrap only while playing, rested at clip end, stop and removal; lanes as heard through bake, crops and glue; schema 42, keymap 12 |
| Note effects (ADR 0069) | `cantus-engine/src/notefx/` in the track's note path — arpeggiator, chords, scale; live and clip notes through one path, release velocity carried; *Bake to notes*; `Project::key` with 19 scales; grooves (pool, extract, commit, pattern groove, validated on paste); MIDI export as played or written; DAWproject reports what it cannot carry; the palette adds note effects; schema 43 |
| Editing (ADRs 0070, 0071) | `AudioRegion::cross` over an overlap (equal power / linear / S) on the comp seams' path, made by the bus in the edit's own undo step; a split never changes the sound; clip gain envelopes in content time (tempo-mapped on moves, repeated per loop, repaired on load); `cantus_core::starting` — a starting preset per unit through `CantusApp::new_insert`, *Neutral settings*, the exported plug-ins too; schema 44 |
| Workflow (ADR 0072) | The palette (`Ctrl+Shift+P`), the history (`Alt+H`, step ids and the saved mark), the start page at launch with four starters, user and track templates (renewed ids, routing by name, plug-in states asked first); hermetic hosted tests; the Audio page's cores as engines and vehicles (SVG illustrations, `engines.rs`), its real-time line in every form and language |
| Causes | sandbox: a value's echo after a save left a stale state; liveness from a progress word, App Nap off; a helper's processing thread serving before a load is answered (an isolated plug-in's first block was dry on a busy machine); tests: a support scan, the user's plug-in database, the process-wide language |

## Round 30: Cantus on three systems (requested 2026-10-22)

**The ask:** releases of Windows, Linux and macOS at the end of the round — Linux complete,
macOS everything that can be prepared without a Mac, the owner's Mac checklist, one tag for
three installers, the owner's frictions. Run lean on the owner's word (few tokens): three
builders (`linux`, `macos`, `ship`) from contract `8153666`, one reviewer per branch and one
on the integrated tree — **20 confirmed findings, all fixed with tests**. Master merged in
four times (the other session released `v0.1.0-beta.2` and `beta.3` for Windows meanwhile),
so this round ships **0.1.0-beta.4**. Account: `docs/round30/integration.md`.

| Part | What changed |
|---|---|
| Linux | cpal's PipeWire and JACK hosts (JACK opened at run time, a stand-in libpipewire in the AppImage, 1.0.5 headers against 0.3.48's library); PipeWire the default only with a real sink or `pipewire-pulse`, JACK re-probed on the device page; the real-time line; `window/x11.rs` and `runloop::serve` for CLAP timers/fds and VST3 `IRunLoop`; X11 only when its server answers; XDG config/data with a resumable move; the scanner's `/usr/local/lib/clap` and `VST3_PATH`; `packaging/linux/` (AppImage, Flatpak, `deps.sh`); CI under xvfb and an `appimage` job. Measured in WSL: PipeWire 0 xruns at 128/64/32, JACK 0 at 128 |
| macOS | `packaging/macos/` (Info.plist, `.icns`, entitlements incl. JIT for plug-ins, `build-dmg.sh` universal, signed and notarised with secrets, ad hoc without); `ipc.rs` address waits, 31-byte shm names; workgroups by device; kqueue parent watch, dyld crash blame; `.clap`/`.vst3` bundles and `window/cocoa.rs` (also in the helper); the Mac keymap preset, `macmenu.rs`, trackpad, `macopen.rs` (Finder and Dock); `macos.yml` on every push, null hashes' macOS leg; `tools/macos/xcheck.sh` (whole-workspace clippy for Apple targets from Windows) |
| Releases | `release.yml`: Windows, Linux (22.04) and macOS jobs, one publish job checking sums and the six names; `cantus-update::release::Target`, AppImage replaced atomically (only when running from its own mount), the Mac's `.dmg` opened, old downloads cleaned; `web/src/shelf.rs`; `docs/releases/TEMPLATE.md` |
| Frictions | Settings in eight pages with a sidebar, the *Options* menu, `Ctrl+,`/`⌘,`; *Scan for plug-ins* / *Find more plug-ins…* in empty shelves; the start-up scan in `CantusApp::assemble` and its notice; the count-in chip, *Also on play* (global), `stop_transport` |

## Round 29: warping, MIDI learn, plug-ins in their own process, the tempo track, freeze (requested 2026-10-16)

**The ask** (the owner, 2026-10-16): a beta that competes with FL Studio and Ableton in what
people use every day — (1) warping, time-stretch and pitch-shift of our own, élastique-class,
on round 28's voice base and a phase-locked vocoder; (2) MIDI learn of any parameter; (3)
third-party CLAP/VST3 out of process; (4) a tempo track with ramps, metre changes and
markers; (5) freeze, bounce in place and low-latency monitoring; (6) the owner's frictions
(none given). Mid-round the owner asked for BACKLOG §0.4's ordered list, decided round 30
(the platforms), and made the repository public as `luis314159/cantus-daw`.

**How.** A contract commit (`7fde16b`) declared `cantus_core::tempo::TempoMap` with constant
bodies (bit-identical to `Ticks::to_seconds`), five locale areas, undo-fuzz arms 43–47 and
`BUILDING.md`; seven read-only mappers wrote code maps first. Five builders in worktrees
under `F:\projects\daw-r29\` — never more than three compiling at once — each kept through
every fix round. Every branch was reviewed adversarially after delivery and after each fix
round, and the integrated tree twice: **24 reviews, 1 172 confirmed findings (127 high)**, each
fixed with a test seen failing first or answered; from 2026-10-20, on the owner's word, small
reviews (four to six agents). Account: `docs/round29/integration.md`.

**Decisions:** ADR 0060 (the tempo map; the unwarped clip reads from its content origin and is
re-anchored by take), 0061 (the stretcher, our own from papers, MPL; the licences reviewed),
0062 (controller mapping), 0063 (plug-ins out of process; ADR 0005 rule 3 amended for a
bounded wait), 0064 (freeze, bounce, low latency); 0049 amended (the public repository).
**Schema 41** (33 tempo, 34 warp, 35 controllers, 36 isolation, 37–38 freeze, 39 warp's
automatic markers, 40–41 controllers), `DSP_VERSION` 5, keymap 9, bridge `PROTOCOL` 6.

| Part | What changed |
|---|---|
| Tempo | `Project::{tempo_points, metre_changes, markers}`; `TempoMap` (segments, closed-form ramps, no allocation on lookup); the engine, click, modulators, synced effects, recorder, CLAP/VST3 transports and exported plug-ins through the map; the tempo lane with points, ramps and metres, the ruler's markers (jump, loop, select, export between), the top bar's tempo at the playhead; MIDI and DAWproject tempo maps both ways |
| Warp | `cantus_dsp::stretch` — Complex (phase-locked vocoder, formants kept, transients held, stereo locked), Rhythm, Melodic (round 28's PSOLA), Re-pitch; analysis at import (tempo, transients, downbeat) on a worker; `AudioRegion::warp` with markers, automatic markers kept apart; the *Warp* tab, *Quantise audio*, *Set the project tempo from this clip*, Alt-drag; renders cached off the audio thread, export waits for them (offline = real time to the bit); DAWproject `<Warps>` |
| Controllers | A lock-free MIDI filter on every port's callback; `Mapping` (source, target, mode, range, curve, takeover, feedback) to any automatable target, mute/solo/arm, insert bypass and mix, transport and tempo; learn from a right-click or learn mode; project and global maps, the *Controllers* pane; feedback to motor faders and LED rings; moves recorded as automation; program change and pressures reach instruments |
| Isolation | `cantus-rt` shared memory and IPC (model-checked); `cantus-sandbox` helpers with kill-on-close jobs, heartbeat and hang limits; `BridgedBackend` lock-step at the backend seam; crash cards and *Reload* with the last good state; four placements per plug-in; 1.30× CPU with 32 plug-ins *Together* (ADR 0063 §5) |
| Freeze | `Frozen` strips and tracks with an edit lock that classifies every command; renders keyed by content, streamed from the project's `Freeze` folder; bounce in place (track, clips, range, with or without effects; `Track::lateness` keeps a bounce on the grid); low-latency monitoring that bypasses other tracks' latency while the take lands to the sample |
| Public | `luis314159/cantus-daw` fed only by `tools/public/publish.sh` (deterministic rewrite, no screenshots, no RVR1960, no-reply identities, gitleaks); every workflow guarded to run there, the site deploy included; Google's site verification; the website's *Coming soon* for Linux and macOS |

## Round 28: the top bar, voice, twelve effects, Chorda and the drums, modulators and comping (requested 2026-10-14)

**The ask** (the owner, 2026-10-14): what needs neither the owner's hardware, their
paperwork nor other software — (1) the top bar à la FL (§6e); (2) the real-time auto-tuner
(§3d) and the Melodyne-class pitch editor (§3c) on one base; (3) the modulation family and
the dynamics/analysis family (§3b); (4) a downloadable CC BY acoustic kit and, if research
allowed it complete, the modelled piano Chorda (§3e/§5); (5) take comping and Bitwig-style
modulators on any parameter, hosted plug-ins included (§0.2). Mid-round the owner added a
showcase song "that sounds super professional", and asked what an AI would need to make
great music through MCP (answered from the demo builder's experience, BACKLOG §7).

**How.** A contract commit (`a327c53`) declared the thirteen effect kinds and Chorda as inert
stubs wired into every list, schema 25 and the builders' locale areas; `BUILDING.md` held the
rules and seven briefs. Seven builders in worktrees under `F:\projects\daw-r28\` — never more
than three compiling at once — then a demo builder and six fix rounds. Every branch was
reviewed adversarially (five lenses plus two focus lenses, a refuter per finding), the fixes
reviewed again, and the whole integrated tree once: **20 reviews, 454 confirmed findings
(21 high)**, each fixed with a test seen failing first or answered. Account:
`docs/round28/integration.md`.

**Decisions:** ADR 0052 (the top bar), 0053 (the pitch base and Tonus), 0054 (the pitch
editor; amends 0051 §4 — an audio clip opens the pitch editor), 0055 (the modulation family),
0056 (dynamics and analysis), 0057 (modulators and hosted automation), 0058 (comp seams,
alternate comps, `Clip::comp`), 0059 (Chorda); amended 0023 and 0042 (Exemplar's kit rows,
round robin in order; DRSKit, Naked Drums refused). **Schema 32**, `DSP_VERSION` 5 (an
Aequalis band raised from 0 dB never glided), keymap 8 (`L` PAT/SONG, `Shift+T` tap).

| Part | What changed |
|---|---|
| Top bar | FL's two rows in 54 pt: PAT/SONG with pattern mode in the engine, transport, tempo with tap and a metre editor, metronome (count-in, level, three voices, accent), position, the focused editor's snap, CPU/RAM/voices/drops, the master meter (peak, LUFS M/S/I, true peak held), inputs, the audio chip, tools, the project name renamed in place and the window title; customise (show, hide, reorder), every group on the bar at 1920 px with a real device, folding by importance into a working ⋯ at any width and zoom (11 172 layouts tested) |
| Voice | A streaming YIN tracker and an offline pYIN-style analysis with epochs; TD-PSOLA at a fixed 128 samples (2.67 ms at 48 kHz) and offline with ratio, formant and time curves; **Tonus** — key and 13 scales, a one-octave note map, MIDI targets at their sample, speed, humanise, flex, vibrato, formants, throat; notes into the exported CLAP/VST3 |
| Pitch editor | Analysis on a worker with a cache; notes with centre, drift, vibrato; per-note pitch, centre, drift, vibrato, formant, gain, timing, split, join; macros; key; A/B; MIDI out; double; the Pitch lane in the arrangement; quantise to the grid or to a track; renders off the audio thread, bounce = playback to the bit |
| Effects | Concentus (Juno I/II/I+II, Dimension), Flexus (through-zero, no latency with it off), Phasis, Tremor, Vibrans, Anulus, Translatio (69 dB sideband rejection), Fluctus (drawn curve), Tonsor (true-peak ceiling, 1–16×), Impetus, Discrimen (parts sum to the input bit for bit), Speculum (scope, spectrum, spectrogram, vectorscope, loudness against EBU 3341/3342, docked or floating); every insert hears the song position as late as its audio; delay compensation re-planned when a latency moves, without a dropout |
| Modulators | LFO, envelope, follower, steps, random on tracks, strips and the master; any parameter — native, hosted (sample offsets), sends, Chorda's except its tuning; bake to automation; recording automation from a plug-in's own window |
| Comping | Equal-power seam crossfades rendered by the engine, seams moved as a unit, take numbers, alternate comps (switch, new, rename, delete, flatten, merge), grouped comping, pitch edits kept, comp windows marked in the model (`Clip::comp`) |
| Sound | **DRSKit** (CC BY 4.0) installed in one step, mixed from 13 mics to stereo, GM map, hi-hat choke, round robin in order per lane, the disk space checked; **Chorda** — waveguide strings, the hammer, unisons, sympathetic strings at their speaking pitch, soundboard, half pedal, dampers, una corda; every partial within 0.5 cent on all 88 keys at 44.1/48/96 kHz; 64 pedal-down voices in ~18 % of a core; designed off the interface thread |
| Demo | *With All Your Heart* (cinematic pop, 116 BPM, 2:53): Chorda pedalled from a lane, all thirteen new units, four modulator kinds, 26 automation clips; −11.3 LUFS, −1.14 dBTP; first in *File ▸ Demo songs…* |

## Round 26: the beta ready in everything that needs neither hardware nor paperwork (requested 2026-10-13)

**The ask** (`docs/next-session.md`): CI green on Linux and Windows; the public repository
prepared in a copy (history without third-party screenshots and the RVR1960, a secret
scan, the commands for the day); GitHub Actions cheaper (minutes ran out); SignPath's
application and signing-policy page as drafts, its conditions verified; release notes for
rounds 23–25 in English and Spanish and a direct download; round 25's small things; CC BY
in `cantus-content`; the rest of §6d.2. Mid-round the owner added: *close every gap a
feature leaves, in this round*; the README's pictures; BACKLOG §6g.

**How.** A contract commit and five builders in worktrees (public, release, mixer,
content, linked), a sixth (sampler) opened when the Salamander showed two engine gaps.
Every branch reviewed adversarially (three to five lenses, each finding handed to a
skeptic told to refute it; 1–5 reviews per branch, two of the whole merged tree); every
confirmed finding fixed with a test seen failing first. Reports: `docs/round26/`.

**Decisions:** ADR 0048 (the CI budget: documents-only pushes run nothing, macOS weekly
against its last good run, null hashes and audit by path, perf by hand; the stable
installer name; version resources on every signed file; OSI-only components), ADR 0049
(going public: what leaves the history, the gate script, the two routes — executed only
when the owner says), ADR 0050 (streaming samples from disk; the cache; the reader's wake
as a counted exception to ADR 0005 rule 3). Amended: 0024 §15 (one text field), 0037
(link group row, Link to…, clip browser, paint tool), 0042 (what landed), 0043/0044
(steady lanes, the pinned band, glyph-sized marks, the typed level), 0046 (swatches,
the text-selection token). **Schema 24** (controller-triggered zones), **keymap 7**
(Alt+N, Alt+F5). No golden hash moved; `DSP_VERSION` 4.

| Part | What changed |
|---|---|
| CI | `ci.yml` Linux + Windows only, no run for documents alone (a test keeps every file a test reads triggering), `concurrency` cancels old runs; `macos.yml`, `null-hashes.yml`, `audit.yml`, `perf.yml`, `windows-release.yml` by path or schedule; ≈ 590 → ≈ 110 billed minutes for an ordinary push |
| public | a rewritten copy (155 → 30 MiB), gitleaks + our scanner clean, `tools/public/check.sh` (forbid list kept outside the repo, mandatory gitleaks, image and verse fingerprints); the RVR1960 out of the build |
| release | `Cantus-windows-x64-setup.exe` beside the versioned file, the site's button downloads it; staged packaging for SignPath; the uninstaller signed and checked; the installer's Privacy page; ASIO from three host files only (no Microsoft driver code); `webpki-roots` and egui's own fonts gone; notes and manual EN/ES; the README's 32 pictures from `examples/shoot_readme.rs` |
| mixer | an armed route lets go when the desk hides; a typed level in the route menu; the inspector's Add send offers New return; lanes placed in two passes (the selection's first; property test over 2 000 desks); the band pinned on screen; swatches in the base-theme menus; *salida* / *Destino*; one kit text field for every field in the app; every hint whole at 1280 × 720 |
| content | CC0 / CC BY 3.0 / 4.0 with credit in the manifest, the receipt, Sound packs, About ▸ Credits and the notices; a strict tar.gz reader; the Salamander Grand Piano (8 or 16 layers) in the instrument picker; the missing-pack card |
| linked | the inspector's Link row; *Link to…* by name (Alt+N; 2 000 clips selected: 5.7 ms to open); the clip browser (Alt+F5) with drag-to-place and the paint tool (B); waveforms on audio tiles |
| sampler | Exemplar streams from disk: heads in RAM, one reader thread, a compact cache with checksums and a size limit; 1.9 GB → ~115 MiB for the Salamander's 8 layers, reloads in 0.3 s, loading off the interface thread with progress; streamed = in memory to the bit; controller-triggered zones (pedal noises) |
| fonts | system fonts for names in any script, loaded lazily off the interface thread (IME included); the nine characters that drew as boxes reworded or drawn |

## Round 25: mixer cabling, the theme family and the taskbar mark (requested 2026-10-10)

**The ask.** BACKLOG §6d.3 — cables between mixer strips, FL Studio's routing made
visible — with the owner's twelve answers (`docs/round25/mixer-routing-design.md` §10:
drag *or* click from source to destination; plain = a send at 0 dB, the strip keeps its
master output; Ctrl = replace the output; the selected strip's cables by default, a switch
for all; pre/post-fader clear and easy; a doubled signal allowed but marked; the FL arrow
row kept in Extra large; a chain summary line; Alt+F9 routing matrix; tracks dropped on
strips; strips and groups movable). Then, mid-round: more themes, a default that matches
the website, the website's mark as the taskbar icon with a colour the user can change,
the in-app logo still configurable.

**How.** A contract commit (piece A: `routing::{route_check, feeders, doubled, edges,
plan, order}`, the batches in `routing_actions`, `Send::tap` with the engine's pre-fader
tap, schema 23) and three builders in worktrees (interaction, matrix, desk), merged; three
adversarial reviewers found 27 real problems — all fixed with a test seen failing first,
plus the click mode the first build had missed. A fourth builder made the theme family
and the icon in parallel. Reports: `docs/round25/`.

**Decisions:** ADR 0041 (mixer routing: gestures and their meaning), ADR 0042 (content
under CC BY accepted with the credit shown; NC, ND, SA refused), ADR 0043 (the cable band
on the desk), ADR 0044 (desk gestures, click mode §10), ADR 0045 (pre-fader sends in the
engine, the routing matrix), ADR 0046 (the theme family, Cantus Night as the default, the
mark and its live colour). Schema 23. Keymap file version 6 (Alt+F9, Shift+F10 route
menu). No golden hash moved. After the push: `DSP_VERSION` 4 — the 0 dB pan law and
the balance exactly unity at centre and on the near side (one rounding step, ≤ 6e-8; no
golden moved); the automatic icon colour (ADR 0046 amended); the hint bar's modifiers
whole; a key to the cable band.

| Part | What changed |
|---|---|
| desk | each strip has a port; the selected strip's cables (or all, or none) drawn in a band under the desk, in lanes, with level and PRE labels, a doubled path marked; a cable can be selected, moved by its end and deleted |
| gestures | drag from a port, or click it then click the destination (Esc cancels); plain = send at 0 dB, Ctrl = the only output, Alt = a sidechain key; refused strips hatched with the reason; one undo step each; a track header dropped on a strip routes it; strips and groups move by drag or Alt+arrows without splitting a group; send automation follows a moved send |
| pre/post | a send taps before or after the fader (`Send::tap`, schema 23, old projects post); one chip says PRE or POST, on the send rows, beside the pill and in the Inspector; a delayed PRE send nulls at tolerance 0 |
| matrix | Alt+F9: every source × destination as cells (level, PRE, key), click to route, Ctrl+drag copies down a column, Shift+drag is fine; the typing piano keeps working while it has focus |
| Extra large | FL's arrow row kept; the chain summary line instead of insert slots (a click opens the Inspector) |
| themes | twelve themes from one 65-role definition: **Cantus Night** (new default, the website's night palette) and **Cantus Paper**, Daylight (the white-and-blue of §6d.4) and Blue Hour, Ultraviolet, Lagoon, High Contrast dark and light, plus the four older ones; previews on the Appearance page; a contrast floor tested for each; switching to and from each theme is pixel-identical; saved choices kept |
| icon | the website's mark without a tile, drawn at 16–256 px (a solid C below 24 px); the `.exe` and installer icon; the taskbar and title-bar icon recoloured live (as designed, theme accent, eight presets or any colour); the in-app logo stays configurable and can follow the icon's colours |

## Round 24: the beta's rest, linked clips, performance wave 2 (requested 2026-10-09)

**The ask.** *Commit and push round 23, give me a build to try, then go on with the
backlog.* Round 23 went to master (cead440) and the test build to
`F:\projects\daw-r23\builds\cantus.exe`; round 24 took the beta items that need no
hardware, the first of the owner's §6d requests and the performance floor (BETA blocker 2).

**How.** Three builders in their own worktrees in one workflow — build, two adversarial
reviewers each, fix (29 findings, all real, all fixed with a failing test first). A 2-hour
soak ran on master meanwhile and failed at 65 minutes on an 18.5 s frame: the crash
journal's `fsync` on the window's thread under a disk kept busy by the builds — fixed, and
the fix exposed a race at the start of a recording, which beta-rest had also found and
fixed. Reports: `docs/round24/`.

**Decisions:** ADR 0037 (linked clips: a link group per clip, content kept equal across
the group, voice tags independent of the link so linking changes no sample), ADR 0038
(voices on several threads: per-voice lanes summed in voice order, `cantus_rt::fork`),
ADR 0039 (instrument output calibration: a hidden trim, schema 21 writes the old level),
ADR 0040 (save off the interface thread). `InsertStripSend`, the exact inverse of removing
a send (found by the §6d.3 study: an undone removal came back at the end of the list).
Schema 22. Keymap file version 4. No golden
hash moved; `DSP_VERSION` stays 3.

| Part | What changed |
|---|---|
| gain staging | Lamina +5 dB and Exemplar's placeholder +8 dB to sit with the other instruments (Lamina capped by the headroom rule: no factory preset past 0 dBFS at velocity 127); projects saved before keep their level, bit for bit (a v20 fixture renders to its old hash); user presets too; the demos' trims adjusted so they sound as before |
| save | the frame only copies the project (0.2–0.4 ms on the demos; 2 ms on a 54 MB project that took 3.8 s to write); one save at a time, a second queued; only the revision written is marked saved; New/Open/demos/recovery/quit wait; a queued save never writes a project that replaced the one it was asked for; errors in both languages |
| recovery | `shoot_recovery` (3 offers × 2 themes × 2 languages); the empty space under the text and a repeated title fixed |
| linked clips | a copied pattern is the same clip until *Make unique* (Alt+U); *Select linked copies* (Shift+Alt+L), *Link selection to this clip* (Alt+L); audio clips share gain, fades, pitch, reverse and effects; a link mark, a ×N count, the siblings outlined; paste/duplicate linked or unique by a setting; edits that grow a copy grow the siblings that showed the same window; `cantus-clip/2`; groups repaired (and said) on load; undo fuzz arms |
| performance | Calor, Unda and Exemplar split their voices across idle pool threads (lanes summed in voice order, so the bits do not move; the schedule fuzz stalls inside the fork protocol too); the demos measured over whole songs at 256 frames: worst window ≤ 32 % of the budget (startup 52 → 23 %, house 48 → 25 %, ballad 59 → 22 %); `CANTUS_VOICE_SPLIT=0` turns it off; CI runs the fuzz |
| soak fixes | the crash journal is written on a thread of its own (a slow disk no longer stops the window); a recording pass closes itself only after the transport was seen running (a stale "stopped" report could end it at once) |

## Round 23: the beta blockers (requested 2026-10-09)

**The ask.** *Only the beta blockers, in order: the export dialog's bugs — audit every
option, reproduce each failure, a test per option comparing the exported file with what
the dialog said, fix them, wire the bar to `render_with_progress`, add the dialog to the
shoot examples; the text that sometimes looks wrong after a theme change — reproduce it,
a pixel test (a round trip of themes gives the same picture), find the cause, fix it; the
two CPU tests robust to load; Ctrl+C / Ctrl+X / Ctrl+V in the piano roll and the
arrangement as Commands with their inverse and undo-fuzz arms, strings in en/es.* Own
worktree, only `crates/` and `docs/` (another session builds the landing page in `web/`),
no commit unless asked, tests first, clippy clean.

**How.** A read-only audit workflow first (two export auditors, every finding checked by
two skeptics — 27 findings, 54 votes, none refuted; a theme investigator; a CPU-test
survey; a clipboard designer), then a test reproducing each finding, then the fix. Copy
and paste was built in parallel by a builder in its own worktree, reviewed by two
adversarial reviewers (8 findings, all fixed) and merged. The finished branch was
reviewed again by three reviewers with a skeptic per finding (13 of 14 real, all fixed).
Reports: `docs/round23/beta-hardening.md`, `docs/round23/clipboard.md`.

**Decisions:** ADR 0036 (the clipboard: `Command::Batch` per cut and paste, no new
command, no schema bump; a `cantus-clip/1` JSON text on the system clipboard that
survives a restart and crosses projects; Ctrl+C/X/V read from egui-winit's `Event::Copy /
Cut / Paste`), ADR 0035 (what an export writes: a range stops at its end and its tail only
rings out; `RenderOptions::compensate_latency`, off by default, on for exports; stems
through the hosted instances; stems committed all or none; the dialog owns the keyboard).
No golden hash moved; no `DSP_VERSION` bump; no schema bump.

| Part | What changed |
|---|---|
| export dialog | **every list (tail, quality, rate, LUFS target, ceiling) opened under the dialog** and could not be clicked — fixed (each list a sublayer of the dialog); Esc closes an open list first; Enter exports; the window's shortcuts stand aside while it is open; a range the project lacks stays unchosen with its note; a **Loop** range; the note names the right bars; the bar follows the render and Stop stops it; the status line shows progress; *Export audio…* after stems opens on the mix; MP3 modes keep their choice; presets match only what the dialog shows; the summary tells a missed loudness target, clipped samples, stems as stems; errors in Spanish; a corrected extension; the old stems dialog removed. Shots: `shoot_export` from 5 kinds to 11 (the rate list open, clicked through the new `raster::shoot_events`) |
| export engine | a selection's (or loop's) tail **played the music after it** — the transport now stops at the range's end, on the transport's own sample (`transport::sample_at`); a latent project made the mix **late by L and cut its end** — `compensate_latency`, so a mix equals its `00 Master` stem; **stems ignored hosted plug-ins** — `render_stems_hosted` / `render_stems_to_folder_with`; a stopped re-export **deleted the earlier stems** — all-or-none commit; FLAC failed at 192 kHz (flacenc's 96 kHz limit, found by the new tests) — written at 96/88.2 kHz, the dialog says so. Tests: `cantus-export/tests/options.rs` (every format × rate × channels, depth, bitrate frame by frame, VBR, Ogg quality, dither, preset, read back by independent decoders), `cantus-ui/tests/export_options.rs` and `export_files.rs`, `cantus-engine/tests/render_range.rs`, `cantus-host/tests/stems_hosted.rs` |
| theme text | the kit's cache of laid-out text guessed a font-atlas rebuild from the atlas's fill ratio, which a dark↔light switch does not lower on a GPU-wide atlas: labels pointed at moved glyphs (blank or wrong). It now watches egui's text options and font faces; `theme::apply` repaints and keeps the fader cap. `cantus-ui/tests/theme_roundtrip.rs`: kit text and the real views, Dark → Light → Dark, with zoom, language and every theme in between, the same pixels (4 729 differed before) |
| CPU tests | `inst_bass::a_cpu_budget_holds` and Calor's per-voice test take the best of five windows (the house rule: load only adds time); the sampler's budget too; the stress frames hold the fastest frame (and the cold first frame to its own ceiling); the hot-plug bounds 150 ms |
| clipboard | Ctrl+C / Ctrl+X / Ctrl+V **did nothing in the real window** (egui-winit sends `Event::Copy/Cut/Paste`, never a key press) — read from those events; notes, clips and time ranges, in the piano roll and the arrangement; paste at the right-click point, the time selection or the playhead, snapped down to the grid; rows keep their spacing; audio sources reused or inserted (`recorded = false`); automation clips only within the same project; one undo per cut or paste with its own label; the piano roll and channel rack take the edit focus when clicked; a `cantus-clip/1` text that survives a restart. Undo fuzz arms 27–30; `cantus-core/tests/clipboard.rs`, `clipboard_crop.rs`, `cantus-ui/tests/clipboard.rs`; shot `shoot_clipboard` |

## Round 22: the multicore live engine and beta hardening (requested 2026-10-07)

**The ask.** *Finish the drivers work, then the optimisation backlog — the multicore engine
first — and make the engine the very best;* then *what is missing for the first official
beta?* and, at the end, *push, and make sure a clone on another computer has everything,
as if it were this one — screenshots, research and all.*

**Decisions:** ADR 0031 §2 and §5.3 (the pool, as built and measured; the CPU levels), ADR
0032 amended (the launcher and the two programs), ADR 0033 (export formats), LAME's LGPL
allowed (the integrator's ruling: LGPL code in a GPL-3.0-or-later program whose source is
published); `DSP_VERSION` 3. Briefs and reports in `docs/round22/`.

| Builder | What changed |
|---|---|
| pool | the live engine on a pool of real-time workers: the graph's state in try-only cells (`cantus_rt::cell`, Miri-checked; `cantus-engine` stays `forbid(unsafe_code)`), the scheduler's Chase–Lev pool driven by the processor, clustering and ranks, the critical path's head on the callback, bounded workers in MMCSS that park after a tenth of a period (spinning costs a laptop's turbo clock), the EcoQoS opt-out at stream start, *Settings ▸ Audio ▸ Audio cores*. Parallel = serial **to the bit** on random sessions at W ∈ {1, 3, 7} and under the schedule fuzz; `engine.s64.b128` **0.25× at the median, 0.20× at p99** (paired runs). The demos at 256 frames still miss on this laptop: each is gated by one instrument. Also export's `render_with_progress` and soak's E1 (an effect removed and undone fast) and E6 (the count-in clicks without the metronome) |
| build | `crates/cantus-launch`: a 335 KB `cantus.exe` that starts `cantus-x86-64-v3.exe` or `-v2.exe` by the processor (`CANTUS_CPU`, `--launcher-info`); `xtask package` builds and checks both and ships `cantus-scan.exe` (**missing from every earlier release**: hosted plug-ins could not be scanned); one taskbar button (AppUserModelID); `xtask null-hashes` — 97 items, plain/v2/v3 the same bits, in CI on Windows and Linux; `xtask perf --cpu / --ab`. v3 is not faster than v2 on today's code (0.995×) |
| math2 | `math::powf` of our own (`exp2(y·log2 x)` in f64 from `const`-built tables: ≤ 0.5 ulp, 10–16 ns vs libm's 35–50), `math::db_to_gain`; counter-based TPDF dither (R10), so Terminus and Culmen rest with dither on; Aequalis's dynamic residual and stale sections fixed (the heavy EQ rests). `DSP_VERSION` 3: goldens moved by ≤ 6e-8 (−144.5 dBFS). Porta −37 %, Sibilus −26 to −29 % |
| export | *File ▸ Export audio…*: WAV 16/24/32f, FLAC (flacenc), MP3 (LAME, CBR/VBR, gapless), Ogg Vorbis; song, selection or stems (per strip or track, optionally through the master chain); dither; BS.1770 loudness targets with a true-peak ceiling held by Culmen's limiter; tags; presets Streaming, CD, Stems, Podcast; background render, *Stop* leaves no partial file |
| soak | a soak harness (40-minute runs, no leak, no hang); **crash recovery** (a session journal and *Recover your work* at start, takes included, one undo step); autosave on its own thread (it froze the window up to 8.9 s); clean refusals of damaged or newer files; the `.bak` kept good; New/Open ask about unsaved work; a new audio track comes armed; the count-in clicks without the metronome |
| beta-docs | `THIRD-PARTY-NOTICES.md` generated (`xtask notices`, 298 crates, an allow-list test), `PRIVACY.md`, the About window, the EN/ES manual (`xtask manual`), issue templates, release notes, version `0.1.0-beta.1` (no tag) |
| integrator | `CLAUDE.md`, `rust-toolchain.toml` (1.97.1), `docs/DEVELOPING.md` (a new machine from a fresh clone — proved on 2026-10-07: a clone into an empty folder fetched the SDK, built the release with WASAPI and ASIO (it found ASIO4ALL, Realtek and the UMC) and ran the suite: 1 823 of 1 824 green, the one failure a CPU-timing test (`inst_bass::a_cpu_budget_holds`; Calor's per-voice cost did the same in the integration run) that fails only while the machine is loaded and passes alone), `docs/instruments/TEST-PLUGINS.md`, the research screenshots and `docs/claude-memory/` in the repository (the owner's request; the repository is private), the manual and release notes brought up to the soak's fixes |

## The next session: what round 21 left

The owner's goal is the first public beta (`docs/BETA.md`). Next, in order:

1. **Round 22 — the multicore live pool** (`docs/perf/plan.md`, Phase 1 as amended): the
   graph implements `sched::Graph` on its plan (`GraphPlan` already exposes nodes,
   successors, predecessors, inputs, master); per-node `AtomicRefCell` state and per-worker
   scratch; Chase–Lev deques (the sched report's recommendation) with ranks and clustering;
   the serial fallback; workers promoted with `cantus_rt::promote_worker(.., Waiting::Bounded)`
   after a bounded spin (≈ half a period) and then park — **never spinning inside MMCSS**
   (rt's finding: one spinning MMCSS thread throttles the callback too); the CLAP
   `thread-pool` sub-queue; every engine test at W = 0 and W ∈ {1, 3, 7}; the schedule fuzz
   over the null corpus. Alongside: the x86-64-v3 build (*build*), the f32 `powf` fast path
   (math's report: `exp2(y·log2 x)` in f64; libm's `powf` is 5–8× slower than the UCRT's on
   per-sample callers — Porta, Sibilus, the Terminus/Culmen meters), counter-based dither
   (R10, so Terminus and Culmen can rest with dither on), the session recorder (*replay*),
   and a quiet-machine re-run of `xtask perf --full --save` (every wave-1 number was taken on
   a loaded machine).
2. **Round 23 — beta hardening** (`docs/BETA.md`): the hardware matrix, soak and recovery
   tests, MP3/FLAC export, instrument gain staging, third-party notices and privacy, the
   quick-start manual, issue templates, version `0.1.0-beta.1`.
3. The owner: try `C:\projects\daw-r21\builds\cantus.exe` with the Behringer UMC; apply for
   SignPath (code signing); a trademark search for "Cantus"; make songs and log friction.

Left by the wave-1 builders (their docs list each): Spatium, Organum, Pulsus and the keyed
gate cannot rest yet; Aequalis's dynamic residual (a one-line fix that moves bits); the
pre-faulting of the delay-line pool; RTWQ vs MMCSS; Realtek's own ASIO driver delivers no
buffers on this laptop (a driver fault; WASAPI works); `xtask bundle --no-build` lays out the
last-built plug-in under every name (round-20 bug).

## Round 21: performance, wave 1, and ASIO that works (requested 2026-10-06)

**The ask.** *Launch the new round with the optimisation backlog — the multicore engine
first; make the audio engine the very best, taking the best ideas from today's engines and
from papers.* During the round the owner found the default build had no ASIO and the ASIO
build lost ASIO4ALL: *"shouldn't one build work with any driver? That's how modern DAWs
work."*

**Decisions:** ADR 0031 (the performance architecture, reviewed against the state of the
art), ADR 0025 §10 and ADR 0032 amended (one build, ASIO always in it); `DSP_VERSION` 2.
Briefs and reports in `docs/round21/`.

| Builder | What changed |
|---|---|
| asio | cpal 0.18.2 and asio-sys 0.4.0 patched in `third_party/` (both directions' buffers in one `ASIOCreateBuffers` before one `ASIOStart`; a running driver stopped before the lock; resets before the first buffer ignored — ASIO4ALL sends one at 0.7 s; a driver opened by name loads only itself); Cantus's device states (starting, running, stalled, restart, lost), one automatic restart, the reopen order, the driver's real buffer and rate read back; `xtask asio-sdk` and one Windows edition with ASIO always linked. ASIO4ALL duplex and output-only pass 60 s |
| rt | new crate `cantus-rt`, the only home of OS/unsafe code: MMCSS "Pro Audio" for the callback (p99 wake lateness 54.7 → 2.1 ms under load), worker promotion safe by default (never MMCSS while spinning), P/E-core and cache topology, 128-byte padding, pre-faulted and locked memory, the denormal guard and its re-assert after hosted plug-ins, the EcoQoS opt-out |
| at-rest | an exact `at_rest()` + `skip(frames)` + `rest_blocker()` on every unit; 9 of 12 effects and 6 of 7 instruments can rest; split-invariance tests at heavy settings and random splits (1–4096) and twin tests; three detector flushes that hovered at their threshold fixed |
| graph | the processor as an explicit graph on one thread: a plan compiled on the UI thread when the routing changes, per-node passes, the canonical gather (equal to the old scatter on 3 240 random topologies, to the bit), note outboxes, FTZ/DAZ inside `process`, limits 64 → 256 with refused tracks counted; `engine.s64.b128` ≈ 0.86×, `s32` ≈ 0.64× (paired runs on a loaded machine) |
| sched | `sched/` — both queue designs (Vyukov, Chase–Lev with home seeding and mailboxes) behind one trait, the pool (bounded spin, park after silence, the continuation slot, serial fallback with hysteresis), zero allocation, PCT-shaped schedule fuzz with replay tokens; Chase–Lev recommended |
| math | `cantus_dsp::math` over pinned `libm` for every transcendental (lint-enforced in dsp, core, engine, plug-ins), `powi` of our own; `DSP_VERSION` 2 — four goldens re-blessed, the largest change −88.5 dBFS; renders are the same bits on every machine |

## The next session: what round 20 left

Round 20 (below) is merged. The owner's priority for the next round is **performance**
(`docs/BACKLOG-excellence.md` §4b): ADR 0031 and `docs/perf/plan.md` — read the plan's
**last section** (the state-of-the-art review, amendments R1–R16), which supersedes its
earlier order. Start with R1 (the WASAPI callback runs at normal priority today) and Phase 0
(the graph in the serial engine), then the multicore live pool. The bench builder's
`docs/perf/baseline.md` is the starting line.

Decisions waiting for the owner (each builder's doc has the detail):
- **FlexASIO in the app**: the first-run wizard links to it; downloading and running its
  installer with admin rights from inside Cantus needs the owner's explicit yes (the
  installer already offers it as an optional step).
- **Code signing**: SignPath Foundation (free), Azure Trusted Signing (~US$10/month; check
  enrolment from Mexico) or Certum Open Source (~€30–70/year) — `docs/round20/signing.md`.
- **Content**: CC-BY packs as opt-in downloads; hosting on the authors' releases vs a
  `cantus-content` mirror; showing the browser and the starter kit on first run; an
  original 808/909 kit rendered from Pulsus — `docs/round20/samples.md`.
- **RVR1960**: a written confirmation from SBU/ABS about the non-commercial clause.
- The legal points in `research/content/README.md` and `research/ai/README.md`.

Left by the builders (their docs list each): latch/touch automation recording, hosted
plug-in parameters as automation targets; hosted instruments in stem exports and
multi-output instruments; arrow-key navigation in the sample browser; preset browsing in
the device picker; audible instrument preview; the picker's "keep matching settings"
toggle; MP3 import; real-hardware runs of plug-and-play and the headphone cue; macOS and
Linux packages.

## Round 20: automation, devices, content, hosted instruments, product (requested 2026-10-05)

**The ask.** Automation clips; closing and minimising instruments made easy and graphical;
swapping instruments and mixer effects made easy; *"see every available instrument on
screen to choose — it looks very, very pro"*; free instruments preloaded (proving
compatibility); rights-free WAVs and where to keep them (not the repo) — *"put an agent on
the WAVs right away"*; research for AI (MCP, LLM APIs, generative audio); then, from the
backlog of excellence: a verse at every start (on by default, switchable), an installer
with signing and updates, audio setup that is direct, a benchmark; and the design of a
performance architecture that beats the market, reviewed against market engines and papers.

**Decisions:** ADR 0027 (automation clips and lanes, schema 19), 0028 (hosted instruments,
schema 20), 0029 (the device picker and swapping), 0030 (content and samples), 0031 (the
performance architecture, with its state-of-the-art review), 0032 (installer, signing,
updates); ADR 0025 §11 (audio setup). Briefs in `docs/round20/`; each builder's report is
`docs/round20/<name>.md`.

| Area | What changed |
|---|---|
| Automation | clips with FL's six point shapes and tension, lanes under tracks, the large editor and articulators (flip, scale, smooth, random, normalise, LFO); every strip/master/send/effect/native-instrument/Firmus parameter; *Create automation clip* on any parameter's right-click and Alt+A for the last tweaked; evaluated on the 16-sample grid with sample ramps for gains — null when absent, block-size independent, bit-identical offline; automation follows a swapped effect |
| Devices | every device window has Swap, minimise (to a tray in the status bar), maximise/restore and close; Esc and F8; `Command::ReplaceEffect`; swapping keeps clips, routing and matching settings in one undo step; the picker opens from title bars, track chips, the rack, the inspector, every mixer slot and Add track; drop targets |
| Browser | a gallery of every native and hosted instrument and effect with thumbnails rendered from the real editors, shelves, search in EN/ES, favourites and recents, details, keyboard; a docked Devices panel with drag payloads |
| Hosted instruments | CLAP/VST3 instruments on tracks (sample-accurate notes, CC/pitch bend, transport, latency in delay compensation, state, offline render, the plug-in's window); 19 free open-source plug-ins proved (Surge XT, Dexed, OB-Xf, Six Sines, Firefly Synth 2, JC303, RipplerX, sfizz); instrument latency now compensated for native instruments too; stems no longer silence native instruments |
| Samples | `cantus-content`: a manifest of verified CC0 packs (starter kit of 81 drums, Karoryfer, VCSL, VSCO 2 percussion…), downloaded on demand into the user's data folder with SHA-256, resume and proxy handling; a sample browser (audition, drag to Pulsus pads, Exemplar, the rack and the arrangement); the Sound packs window |
| Audio setup | an always-visible audio chip and popover, a first-run setup (three clicks to an armed audio track), plug-and-play, settings per interface, channel names and a headphone cue, problems with fix buttons, a "Get ASIO…" window (FlexASIO, ASIO4ALL, 13 makers) |
| Verse | a verse on the splash and a card at start, RVR1960 (293 verses, under its 500-verse allowance) with RV1909 selectable, WEB in English, following the live language switch; on by default, switchable in Settings ▸ General |
| Product | `xtask package`: an NSIS per-user installer, portable zip, plug-ins zip, ASIO variant, optional latest-FlexASIO step; a signing pipeline proved with a test certificate; auto-update from GitHub Releases (no telemetry); a tag-driven release workflow; `LICENSE-GPL` added |
| Bench | `crates/cantus-bench` and `xtask perf` (a gate against stored baselines); a callback histogram and **dropout blame** in the engine shown on the status line; DAWbench-style DSP/VI tests defined in `docs/perf/benchmark.md`; today's numbers in `docs/perf/baseline.md` — the 10-track demo needs **103 % of one core at 128 samples**, the engine caps tracks and strips at 64 without saying so, start-up takes 5–6 s, the 64-strip mixer frame is over budget |
| Research | `research/content/` (rights-free samples, storage, free instruments, legal points), `research/ai/` (MCP, LLM providers, generative audio, a phased plan), `research/engine/` (market engines, real-time systems, scheduling papers, DSP quality, verification) |
| Integration fixes | the sample browser's dock width; keymap file v3 fills in round 19's keys too; one instrument-window path for native and hosted instruments |

## The next session: what round 19 left

Round 19 (below) made Cantus record. What it left, in the order that pays most:

1. **Try it on a real interface.** ASIO builds (`--features asio`, steps in
   `docs/round19/devices.md`) and runs output-only on Realtek ASIO; with input and output
   open together that driver stops calling back after a few buffers (cpal rebuilds the
   driver's buffers when the second direction is added). The owner has a Behringer UMC
   driver installed: test duplex on it. If it stalls too, the fix belongs in cpal's ASIO
   host (create both directions before the first `ASIOStart`). The loopback measurement has
   only been proved through a simulated cable.
2. **Warping.** Audio clips keep their bar positions when the tempo changes and play at
   their own speed (ADR 0025 §1). Time-stretch, a tempo track and warp markers are the next
   block — and the one that makes audio and the tempo track (STATUS round 17 item 2) one
   job.
3. **Crossfades and clip gain envelopes**; MP3 import (no decoder in the tree); dropping
   from Cantus's own browser; disk streaming once projects outgrow memory.
4. **Instrument gain staging** (`docs/round19/fixes-mix.md`): native instruments sit
   well below the effects' reference level at unity; decide a calibration (it changes
   existing projects' sound, so it is a versioned decision per instrument).
5. Each builder's own *left* list in `docs/round19/<name>.md`, and what round 17 left
   (below), which still stands.

## Round 19: recording, the master chain, mastering effects, demos (requested 2026-10-04)

**The ask.** *Record from an audio interface professionally, with every feature of the
DAWs that are best at recording, then polish everything so songs can be made in Cantus;
demo songs, and a startup song that shows what the DAW can do.* Mid-way: *effects on the
master channel — very, very important — and effects for mastering: a Maximus,
saturation/colour like Blood Overdrive or Fruity WaveShaper, stereo control like Patcher's
mid/side.* The owner's mission: the best DAW in the world, made with excellence
(Colossians 3:23).

**Decisions:** ADR 0025 (recording and audio clips) and ADR 0026 (the master chain and
three mastering effects); schema **18**. A contract commit (`r19/contract`) declared the
model, the engine's recording API and the stubs; nine builders worked in parallel
(`docs/round19/BUILDING.md` and the briefs), two more fixed what their reports surfaced;
everything was merged on `r19/integration`. Each builder's report is in
`docs/round19/<name>.md`.

| Area | What changed |
|---|---|
| Model | `Clip::audio` (an audio clip is a clip: split, trim, slip, glue, move work unchanged), `Project::audio_pool`, `Clip::lane` (take lanes), `Track::{input, armed, takes_open}`, `RecordSettings` (count-in, punch, take mode, loop takes, click mode and level), `AudioRegion::slip_frames` (sample-accurate placement), `Master::effects`, `StripId::MASTER`; nine audio commands, `SetMasterFxEnabled`, one resolver for every effect chain (`Project::effects_of`) |
| Engine | input ring with drift handling and meters; monitoring Off / Auto / On through the strip's inserts; recording to 32-bit-float WAV on a writer thread (header every 0.5 s, a full ring padded with silence and reported live); take latency = device out + plug-ins + device in + measured ring wait + offset; audio-clip playback to the sample (fades with five curves, gain, reverse), block-size independent and bit-identical offline; windowed-sinc conversion to the device rate on a loader thread; click modes; pre-roll so a count-in at bar 1 is heard whole; live MIDI echoed to every armed instrument track; the master's insert chain before the master fader, its latency in the heard position, `Meters::master_gr` |
| Devices | driver choice (WASAPI; **ASIO** behind the `asio` feature, builds here), input devices and channels, any sample format, measured latencies, stall detection, a loopback latency measurement (noise bursts, FFT cross-correlation, agreement check) |
| Recording UI | audio tracks; arm lamp, input chip, monitor key and input meter on the header; the strip's input menu and arm; an inspector *Recording* section; record options (count-in, punch with a ruler brace, loop takes, Stack / Replace / Overdub, click); multitrack passes as one undo step; take lanes with swipe comping, promote, delete unused, explode; the live take drawn as it grows; Alt+R / Alt+C / Alt+P / Alt+T, Ctrl+Shift+A |
| Audio clips UI | peaks at three levels of detail on a worker thread with a disk cache; waveforms in the clip's ink with gain and fades drawn; fade and gain handles; normalise, reverse, curves; drag-and-drop and *Import audio…*; audio files moved/copied into `<project>/Audio/` on save, *Collect and save*, relink of missing files; the audio-clip inspector; the piano roll refuses audio clips |
| Master | FL's master track: slots on the desk (XL) and in the inspector's master sheet, device windows, presets, A/B, analyser, hosted plug-ins; stems optionally *through the master chain* |
| Effects | **Fervor** (saturation: tape, tube, overdrive, fuzz, clip, fold, a drawn curve; oversampling; harmonics display), **Latitudo** (stereo: width by band, phase-coherent mono bass, M/S tone, mono-safe Haas, vectorscope and correlation), **Culmen** (maximizer: three bands + full band of two-sided dynamics into a true-peak brickwall, loudness targets, *learn drive*); each null-tested, block-size independent, and in the CLAP/VST3 build bit-identical to the native unit — ten plug-ins in the export |
| Fixes | take latency measured end to end and placed to the sample (`tests/rec_e2e.rs`); Culmen on the master's reduction meter; Gravis's slap levelled; every factory preset under full scale on a dense riff (93 renders); Terminus's true-peak overshoot fixed (worst case −0.016 dB) |
| Demos | six songs (house, worship ballad, funk, lo-fi, cinematic, trap) using every native instrument and effect, mastered on the master with Latitudo and Culmen to about −14 LUFS and −1.1 dBTP; *File ▸ Demo songs…* (Ctrl+Shift+N); the program opens on Neon Tide's drop |

## The next session: what round 17 left

Round 17 had two halves, both below: the native instruments, and "compatible with the
market's DAWs". What is next, in order:

**1. What the instruments left**, in the order that pays most:
- *Host hooks the builders designed and could not wire* (their docs name each): the song's
  **tempo and position** for every unit (LFO and arp sync, a tempo-locked rotary, glide in
  note values) — pass `crate::fx::SongPosition` through `InstrumentUnit`; **Gravis's kick
  duck** (`kick_in(ahead)`: the engine already holds every track's sorted notes, so the
  bass can duck *before* the kick with no latency) and **song key** (needs a project key);
  **Pulsus's per-pad outputs** (`DrumPad::output` and `process_add_bus` exist; the track
  loop must route each bus to its strip).
- *Hosted CLAP/VST3 instruments on tracks*: an adapter from `cantus_host::AudioThread` to
  `InstrumentUnit`, on `cantus-host` — never a second host.
- *The whole-block strip stage* (item 2 below) — agree it with the track side here.
- *Each instrument's P1 list* (in its doc): Unda's wavetable editor and export, Exemplar's
  disk streaming, Calor's host-synced LFOs, Modulus's AI programming, Pulsus's drag-and-drop
  and rack chips, and the modelled grand piano (Chorda) — after a legal read of Modartt's
  patent, which expires 2027-10-20.
- *One preset library across instruments*, with audition through the user's own clip
  (`research/instruments/family.md` §6).
- *Bundled content*: ADR 0001 allows only original or CC0 samples; a CC-BY set (Salamander
  Grand) needs the owner's ruling.

**2. What round 17 left**, the ones worth doing first:
- *A whole-block strip stage.* Inserts run once per note-event segment, sometimes for a
  single sample: legal for every plug-in standard, costly for hosted ones. Render tracks
  per segment and strips once per block. Agree it with the instruments session first —
  both edit `processor.rs`.
- *The other six editors in plug-in windows.* Porta's real editor runs inside other DAWs
  through `FxHost` (ADR 0020 §8); the rest is a mechanical port each, planned there.
- *Hosting:* crash isolation for a plug-in that is already running (the scan is isolated,
  a running plug-in is not); the plug-in window on macOS and Linux; per-plug-in DPI; CLAP
  `render`, `tail`, `remote-controls`, `undo`; a VST3 `restart()` in the contract; the
  generic editor's window should fit its content (it is sized like our big editors).
- *Interchange:* AAF/OMF and audio in DAWproject wait for audio clips; exact tempo and
  time-signature changes wait for a tempo track; MIDI clock and Ableton Link are written up
  in ADR 0021, not built.
- *The engine:* flush denormals (FTZ/DAZ) in the audio callback — the export build does,
  and it made Sibilus 6 % faster without changing a sample; the compressor's and limiter's
  histories line up with what is heard only when the effect is last in its path (the
  engine would have to report each effect's input lag).
- *Licence:* ADR 0001 made Cantus GPL "because of VST3"; the VST3 SDK has been MIT since
  3.8, so `cantus-host` could be permissive. The user's decision.
- The plug-in matrices' missing features (round 16's list, unchanged).

Then the arrangement work that was next before the user's direction changed:

Round 13 took the mixer the rest of the way to FL's strip on the user's direction (ADR
0015; the round's own section is below): sends, solo, submixes, EQ, the slot mix, the
multi-selection. The mixer is now blocked on things that are not the mixer's — audio input
for FL's *external audio in*, an effect with a sidechain input for sidechain sends — so
the order below stands, with the mixer's remainder folded into the audio-clips block.
What is left, in order:

**1. What hangs off the time selection.** It exists, it is visible as a column across every
track, and today two things use it: loop-to-selection (`Shift+L`) and zoom-to-selection
(`Z`). The playlist plan's step 5 is the rest, and every one of them is now a small job
instead of a large one: **insert / delete space** (ripple every clip after the selection),
**quantize clip starts**, **select the clips inside the selection**, **render the
selection**. Do insert/delete space first — it is how a song's structure actually gets
changed, and it exercises the selection as an *edit* anchor rather than a view one.
`CantusApp::time_selection` is the field; `clips.rs` is where the commands go, as free
functions over a `Project` so they are tested without a window.

**2. Swing as a clip property** — decision 4 of the drums plan, still open. A `Clip.swing`
field applied when the snapshot is compiled (`compile_events` is the one place the
timeline mapping is written down), *not* baked into note positions, so it is adjustable
afterwards and undoable by degree. That is a schema bump (8), a migration writing `0`, and
a control in the rack's toolbar. Small, and cheap now; expensive after people have
projects with grooves in them.

**3. ~~Sends~~ — done (round 13, ADR 0015)**, with the remove-strip button, colours and
names in the strip's menu. What the mixer still lacks from FL, and what each waits on:
**external audio in / out** (an input path in the engine and the backend — it belongs to
the audio-clips block, step 8 of the playlist plan, not to the mixer); **sidechain sends**
(a send whose destination is an insert's second input — waits for the first effect that
has one, a compressor); **plugin delay compensation** (no insert has latency yet);
**compact / wide layouts and the meter-wave view** (pure UI, do them when the desk gets
crowded).

Then, in the order the playlist plan gives: **markers and time signatures** (clip chrome
and the header map landed in the visual pass).

The two plans below are kept as written, with each item marked as it landed, so the
reasoning stays next to the result.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0032 | done |
| `cantus-core`: time, model, commands, bus, snapshots, effects model, clip windows, pan law / width, pads and lanes, track height and icon, mixer strips and routing, **sends, strip solo, EQ, slot mix, submixes** | done; 39 tests incl. 40-seed undo/redo invariant fuzz (pads, instruments, strips, sends, routes the commands refuse) |
| **Native instruments** (ADR 0023): Unda, Calor and the shared `cantus_dsp::va`, Modulus, Exemplar, Gravis, Organum, Lamina, and Pulsus II; the shared modulation model, voice allocator and editor frame (Play · Edit · Matrix) | done; each has `docs/instruments/<name>.md` and its own DSP, engine and UI tests; `inst_contract.rs` holds every kind to block-size independence, headroom and exact silence |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, **Pulsus (kick, snare, hi-hat, sample pads, choke groups)**, stereo delay, reverb, **RBJ biquads and the three-band channel EQ**, null-test harness | done; 37 tests, fourteen of them the kit's (block-size independence, determinism, choke, headroom, exact silence, sample playback and retune) |
| `cantus-engine`: transport, processor with mixer strips in dependency order, **post-fader sends, strip EQ, per-slot dry/wet, swap, FX switch**, insert chains on strips, width / pan law / polarity, two instruments per slot, stereo instrument path, sample cache, hardware MIDI in, recording feedback, cpal backend, offline render | done; 23 null/behaviour tests (five for a kit, one golden render, one for buses, five for sends / EQ / slot mix / swap / solo) + 5 effect/MIDI tests + 6 unit tests + 1 real-device test |
| `cantus-project`: versioned JSON (**schema 12**), SHA-256, migrations 0→1→…→12, atomic save, autosave, recovery, fixture corpus | done; 10 tests; fixtures `v1`–`v12.cantus` must always load |
| `cantus-ai`: `AiProvider`, tool catalogue (**+ add_effect**), permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport, arrangement (clip editing, **time selection, per-track height, icons, reorder by drag**), piano roll (**lanes from the instrument**, 7 mouse tools, clipboard, nudge/transpose, slice, mute), **channel rack across every track**, mixer as a desk (master left, numbered strips, routing, `+`, **sends, solo, EQ bars, peak dB, strip menu, multi-selection with ganged faders, submixes**), inspector (channel, route, strip with inserts, **EQ with its response drawn, sends, a mix per slot**, instrument picker, pad editor with samples), widgets (fader, bar), keyboard piano, MIDI recording, design system, named layouts, editable keymap, settings dialog, welcome groove, **clip chrome, header map, colour tints, button steps**, i18n EN/ES | done; 122 tests (headless egui frames, edit/undo flows, clip and step gestures through published geometry, kit lanes as painted, save→open→export, recording take, effects delivery, fader taper, type scale / spacing grid / bundled fonts / theme upgrade) |

## Verified by running (Windows 11, Realtek WASAPI 48 kHz)

- Window opens maximized (or `--window W H`), audio device opens, MIDI ports are enumerated
  (`MIDI inputs: n` in the status bar; none were attached on the dev machine).
- Offline render is bit-identical to real-time processing for block sizes 1…8192, with
  and without effect chains.
- Real-device test: 200 blocks, 0 overruns, max callback 194 µs at 480-frame blocks.
- Fixture `v2.cantus` opens with a Delay on the Keys track; the effects rack edits it live.
- Round 12, fixture `v7.cantus`: the Drums track opens in the piano roll with seven named
  lanes filling the grid and the kick at the bottom; `--rack --track 1` opens the channel
  rack with the fixture's kick and closed-hat steps lit, the muted pad shown muted, and the
  inspector on *Bombo · Kick*; the header row is 40 px with the drum, keys and strings
  icons and the taller Drums / shorter Bass rows the fixture stores; the inspector's channel
  strip shows the Keys track's −4.5 dB and R25. Checked in English and Spanish.

## The design system (round 5)

`cantus-ui/src/design.rs` owns **form**; `theme.rs` owns **colour** (ADR 0009). The split
matters: colour is user data in `theme.json`, form is a product decision in code.

- **Typography.** IBM Plex Sans + IBM Plex Mono are bundled in
  `crates/cantus-ui/assets/fonts/` (SIL OFL 1.1, licence file alongside them; the faces are
  shipped unmodified because "IBM Plex" is a Reserved Font Name). Views ask for named text
  styles — `design::caption/strong/mono_small/readout/readout_small` — never a raw size.
  **Every number in the UI is monospaced** (`design::paint::ruler` for canvas text) so bar
  counters, dB values and timers do not shift horizontally as they count.
- **The 4-px grid.** `design::space::{S1,S2,S3,S4,S6,S8,ROW,ROW_DENSE}` plus `snap()`. A
  unit test fails if the installed spacing lands off the grid.
- **Elevation.** `design::Elevation::{Canvas,Panel,Card}` returns the `egui::Frame` for a
  surface. Panels are separated from the canvas by a hairline pulled flush with the panel
  edge (a −1 px outer margin), not by a drop shadow — shadows smear the dense grids. Only
  windows and menus cast one (`design::overlay_shadow`).
- **Focus rings.** egui returns the *active* visuals for a focused widget, so keyboard
  focus was invisible. `design::paint_focus_ring` runs once at the end of every frame,
  reads the focused id from egui memory and strokes a ring on a foreground layer — one
  implementation covering every widget in the app, with no call-site changes.
- **Interaction states.** `icons::button` paints rest / hover / press distinctly, and
  reserves the accent fill for *state* (playing, recording, tool selected), never for a
  press. Track colour swatches and mixer strip headers follow the same rule.

Two layout bugs the pass exposed and fixed: the transport and the status bar both let
left-hand content overrun the right-hand readouts at 1200 px. Both now nest a
`left_to_right` layout inside a `right_to_left` one, so the readouts claim their width
first and the flexible content truncates. The Loop / Metronome / Keys text toggles became
icon buttons, which is what freed the space.

## Round 18: a look drawn by hand, FL Studio first (requested 2026-09-25)

**The ask.** *The application looks super ugly and plain, and so do our plug-ins … the side
panel of the mixer looks very ugly, the letters run into each other … let me resize and move
the interface's elements … rethink the whole style at the level of the best DAWs — FL Studio
is beautiful … as excellent as FL, vectorized by hand, with incredible performance … as
configurable as possible.* The user also designed ten logos and asked for a splash like FL's.

**Research.** Seven reports in `research/ui/` (screenshots local and git-ignored): FL Studio
(the primary reference, measured, with a spec in §12), Ableton Live, Bitwig Studio,
Logic/Studio One, the best plug-in and instrument UIs, an audit of Cantus's own interface
(47 defects with file:line) and a measured study of how Cantus draws.

**Decision:** ADR 0024. Phase A built the kit (`crates/cantus-ui/src/kit/`) and a proof render
the user approved; Phase B migrated the app with five builders here and four in the
instruments session, merged file by file.

| Area | What changed |
|---|---|
| Kit | 18 hand-drawn parts on one lighting recipe, 61 named colour tokens, boxed text that elides or abbreviates and never collides, cached layers, one batched mesh for meters, 132 golden images, a lint against literal colours and stock widgets |
| Themes | **Slate is the default**; Paper, Parchment, Ink & Gold; FL-style knobs (hue, saturation, brightness, contrast, text) plus depth, roundness, density and text size; Dark / Light / Follow the system with a theme per mode |
| Workspace | `egui_dock`: tabs you drag, split, float and hide; splitters that reset on right-click; F5/F6/F7/F9, Alt+I, Alt+K; the hint bar and a value pill; UI zoom 75–200 %; zero repaints when idle |
| Appearance | searchable tokens with reset, A/B, save/import/export, **pick an element** (Ctrl+Shift+I), the logo picker |
| Splash and icons | the window starts as a splash with logo #2 while files, MIDI and plug-ins load on a thread; window, taskbar and `.exe` icons from the logo |
| Mixer | FL's strip with a legend column, Compact / Wide / XL, fit then scroll, the wheel moves a fader only with Ctrl, one-mesh meters, only visible strips laid out |
| Inspector | the channel's signal-flow sheet: insert tiles with mini views, the EQ you drag, sends, routing, the channel's fader pinned; resizable, collapsible |
| Editing views | FL-style arrangement, a real piano keyboard, the rack's step keycaps; scrolling both ways, Shift+wheel, zoom by the scrollbar's ends |
| Devices | every effect, the hosted-plug-in editor, every instrument and Pulsus II in one shell (preset well, A/B, device undo, bypass, help line); all seven effect editors also run in the CLAP/VST3 build |

**Numbers.** 64-strip whole window: 28 778 allocations and 27 ms a frame before, about 3 ms
and ≤ 3 500 allocations after (the desk alone: 19 allocations, 1.9 ms). The arrangement no
longer copies 18 MB of notes a frame. Each effect editor ≤ 0.83 ms. Held by
`tests/frame_budget.rs`, `views_budget.rs` and the kit's allocation test.

**Left.** Drag-to-select across strips and arrow keys between strips; FL's mini piano roll
in the rack's melodic channels; the File menu entries in `interop.rs` and rename fields still
stock egui; plug-in windows resizing from their host (CLAP `set_scale`) and the host's
transport into `FxHost`; hosted CLAP/VST3 instruments; whether CC-BY samples may be bundled
(ADR 0001 allows original or CC0 only — the user's call).

## Round 17, the other half: native instruments (requested 2026-09-24)

**The ask.** *Research the best virtual instruments on the market — synths, basses, drums,
samplers, keys, and how they look — then put agents on building ours, to equal and beat
them, with Cantus following the plug-in standards.* The user split the round: the standards
(hosting, export, interchange) went to the effects session (the section below), the
instruments to this one.

**Research.** Nine agents, nine reports in `research/instruments/` (about 10 400 lines;
140 official screenshots kept locally and git-ignored): wavetable (Serum 2, Pigments 7,
Phase Plant, Vital), analog (Diva, Repro, Arturia V), FM/additive/granular (FM8, Sytrus,
Harmor, Operator, Omnisphere 3), samplers and their formats (Kontakt 8, Falcon, HALion,
SFZ, Decent Sampler, SF2), drums (Battery 4, XO, Superior Drummer 3, Microtonic, 808/909),
bass (Trilian, MODO BASS 2, SubLab XL), keys (Pianoteq 9, Keyscape, Lounge Lizard, organs),
the standards (handed to the effects session for ADRs 0019–0020), and the DAWs' stock
instruments with the category's interface language. Their shared verdict: the leaders win
on sound and interaction and share structural gaps — their own undo, modulation that
cannot leave the plug-in, renders that do not repeat (unseeded drift, quality modes that
differ offline), knob boundaries at 1.0–2.2:1 contrast, no idea of the song.

**The contract first** (ADR 0023): `InstrumentSpec::Native` — a kind and its state as
numbers and texts by key, three commands with exact inverses; `InstrumentUnit`, delivered
by message and swapped on the audio thread (a large sampler is never preallocated per
slot); notes with ids and the seven CLAP/VST3 note expressions; one modulation model for
every instrument (8 macros, 32 connection slots stored in the spec) evaluated by
`cantus_dsp::modmatrix`, one voice allocator (`cantus_dsp::voices`); a shared editor frame
(Play · Edit · Matrix, macro bar, draggable modulation sources, a playable keyboard, a
footer with the voices); and the knob's edge raised to 3:1. Every kind's dispatch existed
from the first commit, so seven builders never edited the same line.

**Built** — each has `docs/instruments/<name>.md` with its parameters, what its tests hold
and its measured numbers:

| Instrument | What it is | Measured |
|---|---|---|
| **Unda** | wavetable flagship: 3 oscillators, 16-voice unison, two warps each, SVF + ZDF ladder, 4 envelopes, 8 drawable LFOs, 10 generated tables, Serum/Surge table import, a 3D table you scrub; additive (*Partes*) and granular (*Grana*) engines | aliasing −102.8 dB at the default 2×; pitch 0.00024 ct |
| **Calor** | analog polysynth on `cantus_dsp::va` (ladder, cascade, diode, SVF, Sallen-Key — all self-oscillating on pitch), Draft/Good/Best the same live and offline, 16 visible voice cards with seeded variance | self-oscillation within 4.9 ct (Good); aliasing −85 dB (Good) |
| **Modulus** | 8-operator FM, any-to-any with DX7 feedback, the 32 algorithms as templates, DX envelopes and scaling, DX7 SysEx in and out byte for byte, aliasing control saved with the patch | pitch 0.0002 ct; −80 dB aliasing at index 8 (Auto) |
| **Exemplar** | multisampler: groups/zones, seeded round robin, cross-fades, release triggers, loops; an alias-free resampler as the default; WAV/AIFF/FLAC/Ogg, SFZ, Decent Sampler, SF2/SF3 in, SFZ out; the loop seam measured in dB | aliasing −120 dBFS at +12/+24 st (High) |
| **Gravis** | bass: 808, synth (303 accent and slide) and a modelled string (waveguide, pickups in mm, fret collisions for slap) on one mono/legato voice | pitch < 0.05 ct on every engine; drive moves the fundamental < 0.1 dB |
| **Organum** | tonewheel organ: 91 gear-tuned wheels, three drawbar sets with splits, click, percussion, scanner, and a rotary speaker kept in its own module | tuning 0.0003 ct; CPU flat at 5 % of a core whatever the keys held |
| **Lamina** | modelled electric piano: tine and reed, the pickup nonlinearity that barks, geometry in mm, continuous half-pedalling | pitch 0.01 ct |
| **Pulsus II** | the kit grown in place: stable pad ids, sample layers with seeded round robin, a full section per pad, 16 choke groups, the hi-hat pedal, 808/909/modal voices; old kits load as model 1 and render bit-identically | new kick at F1 ±0.03 ct; a render from bar 5 equals bars 5–8 of a full render |

**Schema 16** (native instruments) and **17** (Pulsus II) — 14 and 15 are the effects
half's; fixture `v17.cantus` holds both.

**How it ran.** The contract on a side branch (`inst/contract`), a worktree per builder
(`inst/{unda,calor,modulus,exemplar,gravis,keys,pulsus}`), then an integration worktree
where everything was merged file by file over the effects half, conflicts resolved keeping
both sides. Two things went wrong on the way and were recovered: four builders stalled on a
stream watchdog early on, and all seven stopped at an account spend limit mid-round; each
was resumed with its context and lost nothing.

**What the integration fixed in the shared frame**, from the builders' screenshots: white
keys were invisible on a light theme (they are outlined now), the modulation strip ran off
the window instead of wrapping (rows are packed to the width), and every macro knob said
"Macro" instead of its name.

## Round 17: compatible with the market's DAWs (requested 2026-09-24)

**The ask.** *Keep our plug-ins native so they perform well, but give whoever wants to take
them out of our DAW the option, with the best performance possible — a separate build, or
whatever is excellent. The other session handles the digital instruments; on our side, let
us use external plug-ins — follow the standard. Attack the rest of the points with
different agents.*

**How it ran.** A snapshot commit of the working tree (no branch moved), a small scaffold
first — `crates/cantus-host` with only the contract (`backend.rs`: `Backend`, `MainThread`,
`AudioThread`, notes with ids and expressions, agreed with the instruments session) and a
locale file per area — then five builders in worktrees on `r17/*` branches, merged back
file by file. Coordinated throughout with the instruments session (`daw-b0`): a shared
base, ADR numbers (0019–0022 here, 0023+ there), schema numbers, and a message before every
merge that touched the engine or the format.

| Branch | ADR | What it built |
|---|---|---|
| `r17/host` | 0019 | The CLAP backend (`clap-sys`), the scanner as a separate process with a cache and a *blocked* list, `HostedUnit` behind `FxUnit`, the session (`PluginHost`): load, restart, gestures as commands, state saved with the project, offline render; the plug-in list in the insert menu, a generic editor in the shared frame, the plug-in's own window on Windows, a plug-in manager in Settings; **schema 14** (an external slot; a missing plug-in is a wire and keeps its data) |
| `r17/vst3` | 0019 | The VST3 backend (`vst3` 0.3, from the MIT 3.8 SDK): bundles and `moduleinfo.json`, separate and single-component controllers, lock-free parameter and event queues, controller edits as gestures, `.vstpreset` state, the HWND editor |
| `r17/export` | 0020 | `crates/cantus-plugins`: our seven effects as **CLAP and VST3**, thin wrappers of our own over the engine's own units; stable parameter ids (a golden file), state = our `EffectSpec` JSON, latency, sidechain, soft bypass, FTZ; `cargo run -p xtask -- bundle [--v3]`; Porta's real editor in a plug-in window through the new `FxHost` |
| `r17/interop` | 0021 | MIDI files in and out (types 0/1/2, drums on channel 10 as GM), **stems in one offline pass** (aligned whatever the latency), **DAWproject** in and out, validated against the official XSD; five File-menu entries, drop a `.mid` or `.dawproject` |
| `r17/engine` | 0022 | EQ memory on demand (a zero-latency EQ 153 KB, was 4.3 MB); **soft bypass** keeps latency; the playhead, recording, the click and the mix recorder follow **what is heard**; **MIDI into effects** (`EffectSlot::midi_from`, **schema 15**) and Porta's MIDI trigger; Spatium measures low octaves through 96 dB/oct filters |

**What the tests hold** (689 green): a hosted CLAP plug-in at 0 dB is bit-exact through the
whole engine; with latency, an inverted twin cancels exactly; a gesture in a plug-in's
window is one undo step; state round-trips; a plug-in that crashes or hangs while scanned
is blocked and Cantus survives; VST3 likewise, with zero host-side allocation in
`process` and 100 loads leaving nothing alive. Every exported plug-in, through CLAP and
through VST3, is identical to the bit to the DSP called directly, at blocks 1…4096 and
under automation; all seven pass `clap-validator` (built from source). MIDI export→import
is exact; stems sum to the master exactly; DAWproject validates against the XSD and
round-trips. A note heard on beat 2 is recorded on beat 2 at any latency; bypass toggles
keep a twin cancelled; the audio thread allocates nothing (a counting allocator — which
also found `eq_bands` building strings on the audio thread, fixed).

**The round trip.** `crates/cantus-plugins/tests/hosted_in_cantus.rs` builds our effects
as a `.clap` file, scans and loads it with Cantus's own host as it would a third party's,
and compares each with the native insert: **all seven are bit-identical** and report the
native latency. On the way it found a real defect: a state loaded after activation (what
many hosts do) made the plug-in glide from its defaults — a pre-delay sweep at the top of
the song. A state that arrives before the first sample is now where the effect starts.

**At the merge** (by hand): the whole-file conflicts some old CRLF checkouts caused (the
merge script now normalises line endings first); `effects.rs`, `processor.rs` (engine's
soft-bypass insert loop with host's song position and gesture forwarding), `gate.rs`
(export's `FxHost` editor with engine's MIDI-trigger row kept in the DAW-only part), the
two schema-14 bumps split into 14 and 15, `EffectSlot`/`EffectSpec` literals the new
fields broke, the exported EQ given room for linear phase at creation (inside another DAW
nobody delivers memory on demand), and a UI test that failed on a Spanish system — the
interface language is now per thread in tests (`i18n::default_thread_locale`), so tests
neither depend on `LANG` nor race each other.

**How to run it.** `cargo run --release -p cantus` (the DAW; `-- --locale es` in Spanish).
`cargo run -p xtask -- bundle` writes `target/bundled/Cantus <Name>.clap` and `.vst3`
(`--v3` for an x86-64-v3 build, 15–25 % faster on the heavy effects); copy them to
`%LOCALAPPDATA%\Programs\Common\CLAP` and `…\VST3`.

**Branches.** `r17/{host,vst3,export,interop,engine}` hold each part's history; everything is
merged into the working tree, which is still uncommitted.

## Round 16: seven plug-ins to FabFilter's bar, and delay compensation (requested 2026-09-24)

**The ask.** *Fix the master volume, then put one agent on each plug-in to finish them and
make sure they are of a quality equal to or better than FabFilter's.* Mid-round: *our DAW
should be compatible with the DAWs on the market too — let's look at that at the end* (it
is item 2 of the next session).

**The master.** A centred master balance went through the pan law and came out at
0.99999994, so a strip captured before the master and the master itself did not null.
The master now uses `balance_gains`, which is exact unity at centre; the capture null test
is exact (tolerance 0) and the golden render did not move.

**How the round ran.** Everything a plug-in needs was scaffolded first so seven builders
could work without touching each other (ADR 0018): the `FxUnit` contract, one module per
layer per kind, a shared editor frame (header, presets, host-kept A/B), per-plug-in locale
files, a software renderer for screenshots (`examples/shoot`). A snapshot of the working
tree was committed on a side commit (no branch moved) and each agent got its own git
worktree on an `fx/<name>` branch, a brief (the research report's *How we beat it*, null
tests first, iterate on screenshots in both themes) and a list of files not to touch. The
branches were merged back **file by file into the working tree** — `git merge-file` against
the snapshot — because the user's changes are unstaged and `git apply --3way` needs the
index. One conflict in the whole round (two telemetry arms in `effects.rs`) and one API
drift (Spatium built an `aequalis::BandSettings` literal the EQ branch had extended). Three
agents were cut off by a 600 s stream watchdog while writing very large files in one call;
resumed with *write in pieces under ~300 lines, commit WIP early*, all finished.

**The plug-ins** (each has `docs/plugins/<kind>.md`: decisions, what the tests hold, the
feature matrix against the FabFilter product):

| Plug-in | What it is | Numbers the tests hold |
|---|---|---|
| **Aequalis** (EQ) | 24 bands; zero-latency, natural-phase (64 samples) and linear-phase (four resolutions) modes; L/R and M/S bands together in one linear stage; dynamics in dB and ms above or below, keyed per band; auto gain measured on the signal; EQ match; spectrum grab, collisions against the key, piano axis, typed values (`2k`, `A4`) | natural phase within 0.036 dB / 1.1° of the analog prototype 20 Hz–20 kHz; linear phase within 0.004 dB (High) of the drawn curve; flat EQ is the input delayed by exactly its latency, every mode |
| **Comprimo** (compressor) | nine styles, each a visible, editable detector; ratio to ∞ with click stops; key EQ of six bands; auto gain by loudness; listen Output/Wet/Key/Delta/Compare; oversampling 2×/4×; history in bars; reduction statistics; *Duck this strip from…* on the desk | a wire at 0 dB of reduction in every style; ∞:1 holds the threshold ±0.1 dB; a 10 ms attack measures 10 ±1 ms |
| **Terminus** (limiter) | two stages with per-stage link; five styles; true peak; clipper; dither; loudness to BS.1770-4 (M, S, I, LRA, true peak); delivery profiles with a written verdict; a history in song time that seeks on click | true peak at most −0.019 dB re ceiling on inter-sample material pushed 18 dB (independent 32× check); EBU Tech 3341/3342 cases within tolerance |
| **Spatium** (reverb) | 16-line FDN with decay fitted per frequency; 12 rooms morphed by Space; Room/Plate/Vintage/Spring; decay in seconds with target, engine and measured side by side; tail and spectrogram views; ducking keyed from any strip; freeze | octave-band T20 within 3.7 % of target over 5 settings × 7 octaves; echo density 1.00–1.01; L/R correlation 0.021 |
| **Multiplex** (multiband) | up to 8 bands, each two-sided (compress and expand above and below); Dynamic, Split (Linkwitz–Riley) and Linear modes; thresholds dragged on the display; history lane per band; key from any strip with a free key range | the split at unity flat to 0.00023 dB with 8 bands; drawn = heard within 0.05 dB in every mode |
| **Sibilus** (de-esser) | *Single vocal* detection that tells an *s* from a bright sound; wide, split-high and band-only processing (the split is linear-phase, as a difference); M/S; listen Trigger/Removed; Learn | cuts a sibilant burst 6.00 dB and a bright sawtooth or an 8 kHz sine 0.00 dB; a wire in all 24 mode combinations |
| **Porta** (gate) | gate, expander, upward expander, ducker; six styles; adjustable hysteresis; lookahead at a fixed 10 ms; key filters on a spectrum; Learn; captured hits become a MIDI drum clip | hold exact to the sample; with 3 dB hysteresis one opening where none gives 20+; Clean opens 70 dB below a hard switch's click |

Every plug-in holds the house rules: a bit-exact wire at settings that do nothing (or an
exact delay of its reported latency), identical output at block sizes 1, 7, 64, 1000 and
4096, determinism, both themes legible, both languages complete.

**The host's part.**
- **Delay compensation** (ADR 0018, `cantus_engine::pdc`): per edge, planned once per
  snapshot on the audio thread; lines from a pool, kept while their delay is unchanged,
  each ring exactly as long as its delay. A linear-phase EQ needs 16 896 samples and makes
  every parallel path need as many, so **the pool grows on request**: the audio thread asks
  (`EngineEvent::PdcNeed`), the backend allocates (`EngineMessage::PdcLines`), the audio
  thread swaps them in for free short lines and returns those as garbage. An offline render
  settles this before its first block (`Processor::settle`). Tests: a track and its inverted
  twin through a latent strip cancel to exact silence (100 samples, and 16 884 — offline
  from the first sample, live after one latency); a slot mix at 50 % around a latent effect
  equals 100 %; a latent return lines up with the dry signal; a project with no latency is
  untouched.
- **Latency in the status bar** (`latency 2.1 ms`, the samples on hover, red when a path
  could not be compensated).
- **Every strip's gain reduction** in `Meters::strip_gr`, so the desk's reduction lane moves
  with no editor open (test: `every_strip_reports_its_compressors_reduction_to_the_desk`).
- Spatium's correlation meter turns red only below −0.5: a decorrelated tail swings either
  side of zero, and red at any negative reading made a healthy reverb look broken.

**Screenshots.** Each agent rendered its editor in both themes and iterated on it; the
final shots are not in the repository (they are build output). Regenerate any of them with
`cargo run -p cantus-ui --example shoot -- target/shots 1.0 <kind>` (`eq`, `compressor`,
`limiter`, `space`, `multiband`, `deesser`, `gate`).

**Branches.** The agents' branches `fx/eq`, `fx/compressor`, `fx/limiter`, `fx/space`,
`fx/multiband`, `fx/deesser` and `fx/gate` and their worktrees under `C:\projects\daw-fx\`
hold each plug-in's history; everything is merged into the working tree. They can be
removed once the round is committed.

## Round 15: the first plug-ins, the recorder, and a look at the interface (requested 2026-09-24)

**The ask.** *Carry on with the new plan, and meanwhile put agents to research every
product in FabFilter's Pro bundle, with screenshots of the official products, to make
something better — better-looking, with the same features or more.*

**Research.** Six agents ran in the background and wrote eight reports (`research/
fabfilter/`, indexed in its README); about 85 official screenshots are saved locally and
git-ignored, because they are FabFilter's images. The shared verdict: the benchmark wins on
interaction, not DSP; its displays are often read-only, its styles opaque, its dynamics in
percentages, its contrast low (knob bodies measured at 1.1–1.2:1), its undo local. Every
one of those is something a native plug-in in the host can do better. One process note: a
research agent reported that it scripted past a forum's bot check once before the
permission system stopped it. The reports mark which opinions come from search excerpts.

**Built** (ADR 0017):

- **The house knob** (`widgets::knob`): every inspector parameter — Firmus, the pads, the
  channel EQ, sends, slot mix, effect parameters — is a knob with an arc from its default,
  fine drag, reset, arrows, and a right-click entry box. The settings dialog keeps egui's
  sliders; it is a form, not an instrument.
- **Meter ballistics**: instant rise, 24 dB/s fall, the view keeps drawing until every
  meter has landed.
- **The mix recorder**: a ● on every strip and on the master; a take is the post-fader
  signal as 32-bit float WAV, written by a thread of its own from a lock-free ring; takes
  are listed in the mixer toolbar and revealed in the file browser. The master take is the
  output to the bit, never with the metronome (which now goes in after the master strip).
- **Aequalis**: 24 bands with stable numbers, nine shapes, cuts 6–96 dB/oct, stereo/L/R/
  M/S placement, dynamic bands in real units, output gain; matched filters (a 16 kHz bell
  0.61 dB from analog where the bilinear design is 4.27 dB out); an analyser of the input
  and output with 1/6-octave smoothing and a 4.5 dB/oct tilt; an editor where the display
  is the control (double-click to create, shape by where; drag, wheel for Q or slope, Alt
  to lock an axis or bypass, right-click menu, Delete), a cursor readout with frequency,
  note and curve gain, and the selected band's numbers always on screen.
- **Comprimo**: five styles that name their detector, soft knee, range, hold, auto and
  program-dependent release, stereo link, a key high-pass, listen to key or delta, and a
  **sidechain from any strip chosen inside the compressor** — which is also the mixer's
  missing sidechain. The editor's threshold is a line you drag on the level history and a
  point on the transfer curve; the ratio is the slope you drag; the knee is two marks.
- **The desk**: faders grow with the panel (144 to 440 px) as a console's do; the strip's
  switch row no longer overlaps at 96 px (the width's number moved to the bar's tooltip).
- **`--editor eq|comp` and `--play`** at launch, and **`cantus_ui::raster`** with the
  `shoot` example: the interface rendered to PNG on the CPU. This round's screenshots were
  made that way because the machine's display was locked — and they are how the four
  defects below were found.

**What the tests and the screenshots found:**

1. **The Butterworth Q formula used cosine.** Right for even orders, wrong for odd ones:
   18 dB/oct had its corner at −7.8 dB. The slope test caught it.
2. **"Matched is always better" was not true.** Measured against the same analog
   prototype, matching wins everywhere except a high shelf above 0.35·fs, where the
   bilinear design is closer (1.18 vs 1.81 dB). The first comparison was also unfair — the
   channel EQ's shelf reads `q` as the RBJ slope S, not as Q. The test now compares the same
   prototype both ways and records the numbers.
3. **A knob rewrote its value every frame.** Position → value is exact only to a rounding
   step on a log taper, so an untouched knob issued a command each frame. It now changes
   only on a gesture, and a test holds it.
4. **The screenshots**: the editor window came out translucent (the renderer's clock did
   not advance, so the opening fade never finished), the analyser was a staircase in the
   low end (interpolation added), a selected cut's fill drowned the display (fainter now),
   the strip's switch row overlapped, and the faders left half the desk empty.

Also found and deliberately left: the master's centred balance multiplies by 0.99999994
rather than 1; the strip-capture test notes it, and fixing it is its own decision because
it moves every existing render by one rounding step.

**Not done in this round, and why:** lookahead, linear-phase EQ and the limiter wait for
delay compensation (item 0a); spectral dynamics, EQ match and sketch wait for the fit
engine the Pro-Q report proposes; the reverb, multiband, de-esser and gate wait for the
dynamics-engine ADR; the instruments wait for their editor contract. None of these were
started.

## Round 14: docks and groups on the desk, and the plan for the next four blocks (requested 2026-09-23)

**The ask.** *Carry on with the rest of the features*; a separator in the mixer with three
channels pinned by default and more groups that can be made and coloured; and, as
pending work, **FabFilter-class effects**, **professional instruments with their own
interfaces**, **a recorder inside the mix**, and **an interface worthy of a first-rank
product**.

**Built** (ADR 0016): FL's three docks — left and right pinned, middle scrolling — with a
new desk pinning its last three strips on the right; **strip groups** with a name, a
colour and a fold, drawn as a separator column with the name running up it, members
gathered into one run; *Dock to*, *Group* and *Route "track" here* in the strip menu;
**density** (compact / normal / wide) and FL's **meter-wave** view (`Alt+W`), both
preferences. Three tests paint the desk and read it back: the right dock is painted at
900 px, a left-docked strip sits before strip 1, a folded group hides its members and
keeps its separator, compact drops the send rows for a count.

The one surprise: a folded group's separator paints its member count, and the first
version of the fold test looked for the number `3` to be gone. The count is also a 3. The
test now looks for the strips on either side of it.

**What the mixer still lacks from FL**, unchanged from round 13 and blocked on other
things: external audio in/out and record arm (an engine input path), sidechain sends (an
effect with a sidechain input), PDC (an insert with latency).

### Planned: the next four blocks

These were asked for as *pending*, and each is large enough to be a round of its own. They
are written as plans with the decisions to make first, in the style of the playlist and
drums plans below, so that the next session starts from a decision rather than a blank.

#### 1. A recorder in the mix

FL's answer is Edison on the *Current* track; Reaper's is arming the master. **Decision:
record the post-fader output of any strip, or of the master, to a WAV, from the strip
itself.** A `●` on each strip and on the master.

- Engine: a `Capture { strip: Option<StripId> }` message; the processor copies that
  buffer, after the fader, into an `rtrb` ring of stereo frames preallocated for about
  two seconds (ADR 0005: no allocation, no I/O on the audio thread). A disk thread drains
  it into `hound`. An overrun is counted and reported, never blocks.
- Offline equivalence: *render selection* and *record the master while playing* must
  produce the same file when nothing live is played — a null test holds it.
- UI: the take appears in a list under the mixer's toolbar (name, length, peak, *reveal*,
  *drag to arrangement* once audio clips exist). This is also the first half of audio
  recording: arm a strip that has an *input* and it becomes a recorder of the interface.

#### 2. Effects of FabFilter's class

The bar is not "more effects"; it is **the interface**: a large, direct-manipulation
display where the thing you drag *is* the parameter, with the analyser behind it. Order,
by how much a mix needs each:

| Plug-in | Modelled on | What makes it first-rank |
|---|---|---|
| **Aequalis** (EQ) | Pro-Q | Up to 24 bands, every RBJ shape plus 6–96 dB/oct cuts, drag a dot for freq/gain and the wheel for Q; a real-time FFT behind the curve, pre and post; **dynamic EQ** per band; M/S and L/R per band; *EQ match*; linear-phase mode (latency → needs PDC). |
| **Compressor** | Pro-C | Styles (clean, opto, bus, punch), knee, lookahead, **sidechain input with its own EQ** (this is the effect that unblocks sidechain sends), a scrolling level/gain-reduction graph. |
| **Limiter** | Pro-L | True-peak, oversampled, lookahead, LUFS meters (short-term, integrated, range) — the master's last slot. |
| **Reverb** | Pro-R | Decay by frequency drawn as a curve, space/character, a pre-delay synced to tempo. The current reverb becomes its engine. |
| **Multiband** | Pro-MB | Crossovers dragged on the spectrum, per-band dynamics. |
| **Saturation** | Saturn | Multiband drive with styles; a modulation matrix shared with the synth. |
| **De-esser, gate, Timeless-style delay** | Pro-DS/G, Timeless | After the above. |

Decisions to make first: **(a)** a generic *plug-in view* contract — an effect declares
its parameters (it already does, `EffectParamInfo`) *and* optionally a custom editor
drawn through one trait, so the inspector's sliders are the fallback and never the only
UI; **(b)** an analyser tap: the audio thread publishes a spectrum-ready block per
strip into a lock-free triple buffer, the FFT runs on the UI thread; **(c)** latency
reporting and **PDC**, because linear-phase EQ and lookahead limiting need it; **(d)**
oversampling as a shared DSP utility. Every one is null-tested at its defaults, as the
channel EQ is.

#### 3. Instruments with interfaces of their own

Firmus and Pulsus are sound engines with slider panels. The step up is **Serum / Vital /
Phase Plant for the synth and Battery / FPC for drums**:

- **Firmus II** — wavetable oscillators with a 3D table view you can scrub, two filters
  in series/parallel drawn as curves, drag-to-modulate (drag an LFO onto a knob), a
  modulation matrix, macros, unison with detune/spread shown, a preset browser with tags.
- **Pulsus II** — a pad grid with the waveform of each sample, layers and round-robin,
  per-pad envelopes drawn, choke drawn as links, drag a file from the OS onto a pad.
- **A sampler** (Kontakt-lite): zones on a keyboard × velocity map, loop points on the
  waveform, time-stretch.
- The keyboard at the bottom of every instrument shows what the modulation is doing.

Decision first: the same *plug-in view* contract as the effects, so the instrument UI is
a window or an inspector tab and nothing else in the app knows which.

#### 4. The interface, at the level of a first-rank product

Measured against Bitwig, Live 12 and FL 21 rather than described as "nicer":

1. **One visual language across the four views.** The mixer, the rack and the
   arrangement already share the plate, the tint and the fader; the piano roll and the
   inspector still use egui's default sliders. Replace every default slider with the
   house `Bar` / a new house **knob** (drag vertical, fine with Shift, double-click reset,
   value on hover), so no screen looks like a debug panel.
2. **Panels that dock, float and remember.** The mixer, rack and piano roll as tabs or
   floating windows (egui_dock), with layouts saved per project and per user — FL's
   window model without its overlap chaos.
3. **A browser** on the left: samples (with preview on click), presets, plug-ins, project
   files; drag onto a track, a pad or a strip.
4. **Motion with meaning**: meters at 60 fps with ballistics (not raw peaks), selection
   and drops animating over 120 ms, nothing else moving.
5. **Density at every zoom**, the round-11 rule, applied to the mixer (a 64-strip desk
   must be readable at 1366 px) and to the inspector.
6. **A first-run and empty states** that teach: an empty strip says how to feed it, an
   empty rack how to add a sound, the welcome groove as a real template picker.
7. **Accessibility and keyboard**: every control reachable and announced (AccessKit is
   in egui), the theme editor's AA check extended to the new surfaces.
8. **A design review with screenshots**: the `run` skill's headless frame dumps, compared
   per view before and after, so "better" is shown rather than claimed.

Order: **1 and 4 first** (they are what makes it *feel* professional and they touch no
model), then the recorder (small, and it opens audio recording), then Aequalis and the
compressor with the plug-in view contract, then Firmus II.

## Round 13: the rest of FL's mixer strip (requested 2026-09-20)

**The ask.** *Implement FL Studio's mixer features first*, with [FL's mixer
manual][fl-mixer] as the reference; then, narrowed: *sends and linking between channels,
channel groups, the per-channel effects rack, the base channel EQ, and the link with the
channel rack / instruments / external audio*. The decisions are ADR 0015; this is what was
built, measured against FL's strip diagram, and what was learned.

[fl-mixer]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm

| FL's strip has | Now | Verdict |
|---|---|---|
| Send switch + level per destination, post-fader | `MixerStrip::sends`, a row per send with a level bar and an on/off, `+` to add; the strip menu's *send selected strips here* | **take.** Post-fader, one per destination, a route for ordering and cycles like an output |
| Sidechain send | — | **blocked** on an effect with a sidechain input; the model needs nothing new for it |
| Route selected to this track / create submix | the strip menu, on a `Ctrl`+click multi-selection; a submix is a new strip fed by the members | **take** — a group *is* a bus (ADR 0014's tree) |
| Docks, separators, auto-colour group | — | **not this**: a grouping that changes what is seen but not what is heard is a second grouping |
| Mute · **Solo** (Alt = with connected routes) | `M` `S`; solo is always the connected-routes kind | **take, differently**: the plain solo that silences a channel's own bus is a solo you cannot hear |
| Invert phase · **Swap stereo** · stereo sep | `Ø` `⇄` width | **take** |
| **Disable all FX** lamp | `FX`, lit when the chain is *out* | **take** |
| Ten slots, each with **wet mix** and enable | ten, `EffectSlot::mix`, bypass | **take** |
| **3-band EQ** with a curve | shelf–bell–shelf after the inserts; three bars on the strip, the curve and the numbers in the inspector | **take, differently**: a console's shape rather than three bells |
| Latency comp / PDC | — | **blocked**: no insert has latency |
| Record arm · **Audio input / output menus** | — | **blocked** on an audio input path; belongs to the audio-clips block |
| Peak dB under the meter | yes, red once it touched 0 dBFS | **take** |
| Alt+←/→ move track · F2 rename · rename/colour menu | `Alt+←/→`, the strip menu (rename, palette, follow the track) | **take** |
| Ganged faders on a multi-selection | yes, one undo step | **take** |
| Channel rack → mixer routing, `TRACK` auto-assign | the rack's channel button and the route picker, now with *first free strip* | **take** |
| Compact / wide layouts, meter-wave view | — | later; pure UI |

**What the tests found.** Two things, both about the audio thread:

1. **A ringing EQ has to keep running over silence.** The strip loop skips a strip nothing
   fed and nothing is ringing in — but it decided "ringing" by *has inserts*. With an EQ
   engaged, a strip whose input fell silent mid-block was processed for the rest of that
   block at one block size and not at another, so the EQ's state froze at different
   samples and the next note met a different filter. Block 7 against block 512 disagreed
   by 1.3 µ. `Biquad::is_quiet` (state exactly zero, which the denormal flush guarantees is
   reached) is the fix: the strip runs while the EQ has anything in it, and skipping a
   quiet one changes no sample. The lesson is the standing one — *skip only what is
   provably the identity* — and it took a block-size test to make it precise.
2. **Two clips at the same start were kept in insertion order.** The undo fuzz, with the
   new commands changing its random walk, restored a project whose clip list compared
   unequal: same clips, swapped. `sort_by_key(start)` is stable, and stability is history.
   Clips sort by `(start, id)` now. This was a latent fault in round 11's clip code that
   the earlier seeds never reached.

Two smaller ones: a unity send into an empty strip is *not* exactly a second copy under a
−3 dB pan law, because the return is a channel strip and places its signal through the law
(centre = 0.707) — the test sets the 0 dB law so "exactly twice" is exact; and swap on a
bus against panning the source the other way differs by 2 × 10⁻⁸, the pan law's `cos(π/2)`
in `f32`.

**Not done, and why:** see the table's *blocked* rows and ADR 0015's last consequence.

## Round 12: Pulsus and the channel rack, the time selection, the shape of a track

Two blocks, in the order the last handoff asked for them. The decisions that turned out to
be architectural are in ADR 0013; this is what was built and what was learned.

### Block A — a channel rack of our own

**1. The piano roll asks the instrument.** `InstrumentSpec::lanes()` returns a `Lanes` —
`Chromatic` for Firmus, `Pads(&[DrumPad])` for a kit — and `lane_name(pitch)` answers
what a pitch is called. The piano roll builds an `Axis` from it every frame (lane → pitch,
pitch → lane, lane → name), plus a row for any pitch the clip holds that the kit has no pad
for, named as a keyboard would name it, so changing a track's instrument can never make
music invisible. The plan had asked for `Option<&[Pitch]>`; the reason it is `Lanes`
instead is written on the type and in ADR 0013 — a slice of pitches is one of the three
mappings an editor needs, and returning it would have put the other two in every caller.

On a kit the rows are the pads, kick at the bottom, **sized to fill the grid** (up to 44
px): seven pads at keyboard height were a strip along the top of an empty canvas.
Vertical moves are in **rows**, and the octave keys move four pads. A drum's note has a
fixed 1/16 length.

**2. Pulsus.** `cantus-dsp/src/pulsus.rs`: kick (sine with a fast pitch envelope, a
high-passed noise click, soft saturation), snare (two tuned bodies a fifth-and-a-bit
apart, band-passed noise with its **own** decay), hi-hat (the 808's six squares as ratios
of a base, band-pass then high-pass), and **choke groups** with a 4 ms fade rather than a
hard cut. Seven pads on the GM map: kick, rim, snare, closed hat, low tom, open hat, high
tom — the toms and the rim are the snare shape retuned, which is a preset and not a fourth
voice, and the kit says so. Everything is one-shot and parameters are captured at the
note-on, which is what makes the output block-size independent without a single smoother.

The null tests went in **before** the engine wiring, as the standing rule says, and two of
them were checked against deliberately broken code: disabling the choke fails exactly the
choke test, and resetting a filter per block fails exactly the block-size test. Two things
the tests then found in the *design*:

- **The kit was too hot.** The kick alone hit 0 dBFS at velocity 127, so a busy bar arrived
  at the fader already clipped. `voice_level` now puts the loudest pad around −6 dBFS — the
  same place Firmus's default gain puts it — and a headroom test holds the line.
- **The saturator was a hidden gain stage.** Peak-preserving normalisation lifts the body
  rather than lowering the top, which is right for a driven kick, but at `k = 1 + 7·drive`
  the default lifted the tail by 10 dB. It is `1 + 3·drive` now, defaulting to 0.2 (about
  +4 dB), with the reason on the function.

The engine holds **both** engines per track slot, preallocated, and switches with a flag
(ADR 0005: no allocation on the audio thread when a track changes instrument). Pulsus
renders **stereo** because its pads are panned; Firmus stays mono and is copied. A muted
pad is resolved in `Snapshot::compile`, like a muted track. `SetInstrumentParam` now
addresses an `InstrumentParam` — `Firmus(_)` or `Pulsus { pad, field }` — so a third
instrument will not need a third command; `SetPadName` renames a pad in the kit. Schema
**6** is the variant's arrival; the migration writes nothing and says why.

**3. The channel rack** (`views/channel_rack.rs`, `F6`). Pads down the side — mute, name
(double-click to rename), level and pan bars — and a step grid across, editing the
selected clip. **A step is a note**: switching one on or off is `InsertNotes` /
`RemoveNotes`, and there is a test that round-trips a project through both views and
asserts it is byte-identical. The steps follow the clip: a repeating clip shows one period.
Left-click on, right-click off, drag to paint with the mode decided at the press, Alt+wheel
for velocity — the same gesture as the piano roll's, because it is the same edit. Velocity
is the height of the lit part of a step, so the graph editor's first column is always on
screen. The pads × steps grid goes into **one `Mesh` with a level-of-detail rule** from
the first line: below four pixels the empty cells stop being drawn (the column shading
behind them says the same thing), below seven the lit steps become plain quads. The test
asserts the *shape count*, not a frame time, so it stays true on a loaded machine.

The rack uses the same `Axis` as the piano roll, which means it is a step view of *any*
clip: on a synth the rows are the pitches the clip uses.

The inspector shows the kit as a row of pad buttons and the selected pad's parameters —
only the fields its voice has, with the ranges its voice needs (a hi-hat's tone lives
around 9 kHz, a snare's around 2 kHz). The choke group is a named list, not a slider: it
is an identity, not a quantity.

### Block B — the playlist

**1. Time selection.** `CantusApp::time_selection: Option<(Ticks, Ticks)>`, a range across
every track. It lives on the app, beside `selected_clips`, and not in the project: a
selection is not part of the piece. The **ruler is two lanes** now — an 8-px loop lane on
top where dragging sets the loop brace (which is what a bare ruler drag used to do), and
the selection lane below where dragging selects time. Reaper splits the ruler the same way
and for the same reason: putting the new gesture on the old one would have cost the loop
its direct manipulation, and putting it behind FL's `Ctrl+drag` would have hidden the
thing everything else hangs off. A click in the selection lane still seeks, and clears the
selection. The selection is drawn as a translucent column across every track, brighter in
the ruler where the edges are grabbed, and its length in bars is read out in the toolbar.

What hangs off it today: `loop-to-selection` (`Shift+L`; sets the region **and** enables
looping, in one undo step) and `zoom-to-selection` (`Z`, Live's key; a view change, so not
a command). Live's `Ctrl+L` was not taken because it is the loop *switch* here and a switch
has to stay reachable when nothing is selected. `Esc` in the arrangement clears both
selections.

**2. Track shape.** `Track.height` (a multiple of the base row, in the **project** — it
does not change what the piece renders as, so ADR 0012 does not force it there, but a
height that did not travel with the file would have to be set again on every machine) and
`Track.icon: Option<TrackIcon>`, where `None` means *derive from the instrument* — a kit
gets a drum, a synth a waveform — so it keeps following the instrument until somebody
chooses. Schema **7**.

The header lost its fader, pan bar and meter to the **inspector's channel strip**, where
the fader has a scale and the pan has a readout. What is left is what a header is for:
colour, icon, name, M/S and a hairline meter. The base row went from 64 px to **40**, and
three tracks that used to fill the panel now leave room for four more. Drag the divider
under a header to resize the track; drag the header itself to reorder (`MoveTrack`, one
undo step); the context menu has height presets, *give every track this height*, and the
icon picker. `Ctrl+↑/↓` change track. `rows_of` is the one function that says where each
row is, and both the headers and the timeline ask it; `TimelineGeometry` publishes the
rows, so a gesture test asks rather than multiplies.

Six painted track icons (`icons::paint_track_icon`): drums, keys, strings, vocal, synth,
effects. The first drums glyph was a circle with a diagonal through it — which at 14 px
is the polarity Ø already on every channel strip. It is a drum with two crossed sticks now.
An icon must not be another icon's twin.

### Three defects found by running, and what they were about

**1. The key labels were painted in the canvas colour.** `paint_keys` wrote `C4` in
`theme::p().ink`, which is the *canvas* — dark on Ink & Gold, where it worked, and white on
Paper, where it had been invisible since round 10. The kit's lane names inherited the same
line and showed up only on every second row, which is what made it visible. It is
`theme::on_light_key` now: whichever of `ink` and `text` is darker, legible on a light key
in any theme. The test that asserted `"C4"` was *painted* had been passing throughout;
text that is painted invisibly is the round-11 lesson in a new shape.

**2. Every divider drag was a reorder.** The header rows were allocated flush, so the
divider line belonged to two rows' hit rectangles and egui gave the press to the later
one — for which the press was not on *its* grip. The interactive rect is shifted down by
the grip now: a row owns the divider *below* it. The gesture test caught this before it
shipped, which is what publishing the geometry is for.

**3. A pan test that measured the wrong thing.** The first stereo test panned the hi-hat
and summed `|x|` over the whole render; the kick dominates that sum, and the hat moved it
by 0.03 %. True, and no assertion can rest on it. The test pans the kick now, and its
companion asserts that with every pad centred the two channels are *bit-identical* —
which is the "a control at its default touches nothing" rule, again.

Also: a flaky guardrail did what round 9 said it would. `stress_project_renders_within_budget`
failed once at 266 ms against a 250 ms ceiling while another project was compiling, and
passed alone. It was left as it is; the note in round 9 already says what it means.

### Round 12, second half: the rack becomes FL's, on the user's direction

After the first half landed, the direction came back: *the channel rack should map several
channels and send them to different mixer channels or group them, use samples or generate
sounds, and open with a kick, snare and hi-hat that sound incredible.* Decision 1 of the
drums plan — one kit on one track, always — was what stood in the way, and it was wrong
about what FL's rack is for. Revised in ADR 0013, and this is what changed:

- **A channel is a track.** The rack lists every track, one row each, with the track's own
  mute, fader and pan (`SetTrackMute` / `SetTrackGain` / `SetTrackPan` — the mixer's
  commands) and the number of the strip it lands on. A kit with several pads expands into
  indented pad rows with the pad's own controls. A drum that needs its own strip is a
  track with a one-pad kit (`PulsusParams::single`); the whole kit on one strip is a
  track with seven. Both are the same type and the same rows.
- **The rack edits a pattern region** — the selected clip's window — on every track. A row
  shows what its track's clip plays inside it; a step on a channel with no clip there
  creates one, the pattern's size and repeat, in the same undo step. The rack **fits the
  pattern to its width** by default and only scrolls once you zoom. Clips stay the unit of
  reuse: still one timeline model. A melodic track is one row whose step adds the clip's
  lowest note, or middle C.
- **Samples per pad.** `PadVoice::Sample` and `DrumPad::sample`, schema **8**. A pad plays
  a WAV — any rate, mono or stereo, resampled and retuned by the pad's tune, choked and
  muted like the others — in the same kit as synthesized pads. `cantus_engine::samples::
  SampleCache` decodes with `hound` and **attaches** the audio to the snapshot
  (`TrackSnapshot::samples`) before it is sent, so real-time and offline render see one
  buffer; the cache never evicts, so an `Arc` dropped on the audio thread is never the
  last. A missing file makes the pad silent and the inspector say so; fixture `v8` points
  at a file that does not exist on purpose. Null tests: block-size independence and
  realtime-equals-offline through a real WAV in a tempdir; tuning up an octave halves the
  length; a stereo file keeps its channels apart; a missing file is exact silence.
- **Any track can become a kit** (`SetInstrument`, an *Instrument* combo in the inspector),
  **pads can be added and removed** (`InsertPad` / `RemovePad`, *+ Pad* and *Remove pad*),
  and *Load…* on a pad picks a WAV (`SetPadSample`, which also switches the voice, in one
  undo step). All four ran through the undo fuzz, which caught the first `PadField::Voice`
  resetting the decay alongside the voice — undo restored one and not the other. A
  parameter set changes exactly one field now.
- **The app opens on a groove.** `welcome_project()`: keys, kick, snare and hi-hat as four
  tracks — the drums each their own strip — playing two bars in a four-bar loop, pad names
  in the interface language. `starter_project()` stays as the tests' fixture, so a change
  to the groove cannot break a test about undo.
- **The voices.** Kick: a second, much faster pitch stage (the beater's knock, gone in
  8 ms) on top of the slow fall. Snare: a small pitch drop at the hit, two noise paths (a
  band-pass for the wires, a high-pass an octave below it for the crack), noise mix
  brighter with velocity. Hi-hat: two milliseconds of stick noise before the metal. The
  stick noise exposed a real non-locality — noise was seeded from a *global* hit counter,
  so adding a kick changed how the next hat sounded; the `kick under hats` residue test
  caught it and the seed is per pad now. I cannot listen; `cargo run -p cantus-ui
  --example render_kit -- out/` renders each drum and the groove to WAV, and those files
  went to the user for judgement. **Whether they sound incredible is not established** —
  the numbers (kick fundamental 47 Hz, ~110 ms to −20 dB; snare centroid brought down from
  10 kHz to ~6; hats −10 dB under the kick) say plausible, and only ears say good.

One mistake to own: mid-way, a `git checkout` on the test file restored the *staged*
round-11 copy and threw away the session's test additions. Every patch had been a script
in the scratchpad, so they were replayed in order and the count came back; the habit of
patching through scripts is what made that a ten-minute loss and not a day.

### Round 12, third part: the mixer becomes a desk, and the three views get their look

**Mixer strips (ADR 0014, schema 9).** On the user's direction — *many mixer channels,
numbered, present even when empty, more on demand, Master on the left* — a `MixerStrip`
is now a thing of its own. A project opens with sixteen; a track *feeds* a strip (the
first free one when it is created); a strip feeds the master or another strip, which is a
bus; the inserts, width and polarity live on the strip. The track keeps a level and a
*balance* of its own, at unity — FL's channel volume — and the pan law is applied once, at
the strip. Processing order is a topological sort computed in `Snapshot::compile` and read
by the audio thread, whose strip buffers are all preallocated. The 8 → 9 migration moves
each track's controls onto a strip of its own and **renders bit-identically**:
`a_pre_strip_project_renders_exactly_as_it_did` pins a SHA-256 of `v5.cantus`'s render,
recorded before the refactor. The undo fuzz runs every strip command, refused routes
included. The mixer draws the master first, then every strip by number with the colour and
name of what feeds it, `→ M`/`→ n` routing per strip, and `+` at the end; the inspector's
*Channel* section grew a *Mixer* route picker and a strip section with the inserts; the
rack's channel button shows the strip number and opens the picker.

**The visual pass**, measured against the list in *the interface gap* below:

- **Clip chrome**: every clip has a title bar in its colour with the name in whichever ink
  reads on it, over a body that is a wash of the same colour carrying the note preview.
  Below 22 px the bar goes and the name sits on the body; below 14 px wide the name goes
  rather than smears.
- **The header as a map**: a miniature of the track's clips across the whole piece, behind
  the name, one mesh. **Colour identity**: header and lane tinted with the track's colour —
  a few percent, more when selected — so a song is navigable by colour with nothing painted
  solid. The loop brace is a bracket with end caps; the playhead has a marker in the ruler.
- **The channel rack's steps are buttons**: rounded, a gloss over the upper half, a shadow
  under, a hairline rim, two tones by beat, more air between them; velocity is how strongly
  a button is lit, not its height. Above the level-of-detail threshold they are painted
  shapes; below it they are the mesh quads they were, so a 32-pad kit over 512 steps still
  costs what ADR 0011 says. The channel button is a tinted plate with the icon and the strip
  number; the grid sits one step down the elevation scale so the keys stand off it.
- **Mixer**: numbered, tinted header plates with the strip's colour along the top edge, the
  same plate on the master, and the strip's inputs and insert count under the name.

Two things this pass fixed that were bugs and not taste: the mixer's header text was in a
raw `FontId::proportional(12.0)` and the canvas colour; and `theme::on_color` /
`theme::tint` now exist so nobody guesses which ink reads on a colour again.

### What is not done, and why

- ~~**Sends**~~ — done in round 13 (ADR 0015).
- **Swing** (decision 4 of the drums plan) — not started; it is schema 10. See *The next
  session*.
- **What hangs off the selection beyond loop and zoom** — insert/delete space, ripple,
  quantize starts, render selection — is step 5 of the playlist plan and the next pull.
- **The rack has no horizontal scrollbar**; it scrolls with the wheel and Ctrl+wheel zooms
  the cells. A mini-map scrollbar is step 6 of the playlist plan and the rack should share
  it.
- **Track groups** are still one tree with the mixer's buses, still not started.

## Round 11: a fader of our own, clip editing, and the pan law

Three blocks, in the order the last handoff asked for them.

### A console fader (and a bipolar bar) — `cantus-ui/src/widgets/`

egui paints a slider's groove and its cap from the same `inactive.bg_fill`, so at rest the
two could only differ by an outline — the limit the round-10 review ran into. `Fader` is
ours: a **recessed groove**, a **raised cap with a read line**, a **dB scale** beside it and
a **unity datum** that crosses the whole lane, so "is this track above or below unity" is
answerable at a glance across twenty strips.

- **The taper is the interesting part.** Linear dB spends half the travel on −60…−30, a
  range nobody mixes in; linear amplitude crushes the useful range into the top centimetre.
  Cantus's fader is **6 dB per 10 % of travel above −24 dB, with unity at 80 %**, and
  steepens below that so silence is reachable without spending the fader on it. Silence is
  a real position (`MIN_GAIN_DB`), not a clamp: pulled all the way down, a fader is off.
  `db_at` and `pos_of` are exact inverses (there is a test over a thousand positions), or
  the cap would jump under the pointer on the frame after a drag.
- **Recessed, in every theme.** A slot is darker than its surface whether the theme is
  dark or light, so `theme::recess` takes whichever of the canvas and the strong grid line
  is actually darker — `ink` on a dark theme, `grid_strong` on a light one — and falls back
  to darkening the surface if a hand-edited theme leaves neither. The mixer's meters use it
  too; on Paper they used to be white slabs.
- **States are edges, never fills.** Rest, hover and grabbed differ by the cap's outline,
  with the accent reserved for the grabbed/focused state — the design-system rule that a
  light theme's "darker fill = pressed" gets exactly backwards.
- `Bar` is the same argument for bipolar controls: egui's slider fills from its left end,
  so a pan at dead centre looks like a pan at 3 %. `Bar` fills **from its detent outwards**,
  which makes the direction the reading and the length the amount, and it snaps to the
  detent within a pixel so centre is reachable by hand.
- Both are used by the mixer, the inspector and the arrangement track headers, and both are
  keyboard-operable.

**Widgets that own the arrow keys.** A focused fader moves with the arrows and so does the
note selection, and `global_shortcuts` runs before any panel is laid out, so it cannot see
that a widget is about to take the key. `widgets::claim_arrow_keys` /`arrow_keys_claimed`
close that: `Keymap::pressed` now takes `arrows_taken` beside `letters_taken`, and suppresses
by *the key pressed*, never by the action's identity — so rebinding Save to `ArrowUp` gets
the same rule the shipped nudge does. Without it, nudging a fader also transposed every
selected note. One egui detail the test pinned down: a widget's focus filter is only
installed on the frame *after* it gains focus, so an arrow pressed in the same frame as the
Tab moves the focus instead of the control.

### Clip editing — schema 4, `cantus-ui/src/clips.rs`

**The decision the last handoff said to make first: a clip is a window onto its content.**
`Clip` gained `offset` (which content tick sits at the left edge) and `content_length` (the
repeat period), plus `muted` and `color`. Notes now live in the clip's own content timeline,
and everything else falls out of that:

- **Splitting moves no notes at all.** Both halves carry the whole note list and differ
  only in `start`/`offset`. Which is why **split then glue gives back the clip, note for
  note** — there is a test that asserts exactly that, on the notes and on the snapshot the
  engine would play.
- **Trimming the left edge** moves `start` and `offset` together, so nothing slides. It is
  also lossless in both directions: notes outside the window are kept, as they already were.
- **Slipping** moves `offset` alone.
- Recording into a clip that has to grow leftwards now moves the offset instead of shifting
  every existing note, which is the same operation as a left-trim.

**Split shares note ids; copying allocates new ones.** Split has to share them, or glueing
the halves would not recognise them as the same notes. A *copy* must not, or glueing a clip
to its own duplicate would see every note as one it already had and throw the lot away.
That asymmetry is deliberate and both directions have a test.

**Repeat is opt-in, and that is a decision.** `content_length: None` — the default and what
every migrated clip gets — is a clip that plays its content once and reveals more of it as
it grows. `Some(period)` repeats. The two are different intents and must not be conflated:
dragging the right edge of a clip you shrank has to give back the notes you hid, and
dragging the right edge of a *looping* clip has to repeat it. Live draws the same
distinction with its Loop switch. The period is a brace around the content, not a modulus:
content outside it is not heard, rather than being piled onto the first bar.

**Drags preview and commit once.** The old code issued a merged `SetClipBounds` every frame
of a drag, which could not express a cross-track move at all and left undo depending on
gesture merging. Now the clips are *drawn* where they would land and one batch is committed
on release — one undo step whatever the pointer did on the way. Move, copy, trim and slip
are one code path.

**Modifiers in the arrangement are not the piano roll's**, and deliberately: there is no
pitch axis here, so Shift and Alt are free. **Alt+drag copies**, **Shift+drag slips**, Ctrl
ignores the grid, Ctrl+click extends the selection (egui reports a click only when the
pointer did not travel, so Ctrl+click and Ctrl+drag never meet). Right-click opens the
clip's menu instead of deleting it outright — deleting the wrong clip with no warning is a
hard thing to undo your way out of.

**Notes and clips share their shortcuts**, and which one runs is decided by `EditFocus` —
the half of the window you last edited in. Cut, copy, paste, duplicate, delete, select-all
and the arrow keys all exist twice over; deciding by "whatever you touched last" is a rule a
musician can predict without being told, and it is not decided by which selection is
non-empty, because both usually are.

New actions: `split-clip` (`Ctrl+E`), `glue-clips` (`Ctrl+J`), `toggle-clip-mute` (`0`),
`toggle-clip-loop` (unbound by default; it is in the clip's menu). WAV export moved to
`Ctrl+Shift+E` in **every** preset, where the Live preset already had it: splitting is
something a musician does a hundred times an hour and exporting a WAV a handful of times a
day, so where the two collide it is the export that moves.

Also: the arrangement's note miniatures now go into **one mesh for the whole view** with a
size check, per the ADR 0011 convention. They were one filled rect per note per clip.

### Pan law, width, mono and polarity — schema 5, ADR 0012

**The law is a field of `Project`,** not a preference. It changes the level of every panned
track in a piece, so two people opening the same file have to hear the same mix. It is
undoable like anything else and it is shown on the master strip, where somebody mixing
looks for it.

Four laws, named by how much a **centred** track is attenuated (the Logic/Pro Tools
convention); all four agree at the extremes:

| Law | Centre | Constant | For |
|---|---|---|---|
| `0` | 0 dB | neither | what Cantus did before — every migrated project gets this |
| `-3` | −3.01 dB | power | **the default**: music on two speakers |
| `-4.5` | −4.52 dB | the geometric mean | the hedge (Pro Tools's default) |
| `-6` | −6.02 dB | amplitude | anything that will be summed to mono |

**`0 dB` is a compatibility guarantee.** `PanLaw::ZeroDb::gains` is the old expression
verbatim, and a test sweeps two thousand pan positions asserting the two agree *exactly*.
Seeding old projects with the new default would have been one line and the worst kind of
bug: one you only hear, in a finished mix, months later.

**The master's pan does not take the law.** A pan law compensates for placing a *mono
source* in a stereo field; the master is already stereo, so the −6 dB law would take 6 dB
off every centred mix for nothing. Its pan is the balance formulation. The null test found
this — a centred mix came out 12 dB down instead of 6, because the law was applied twice.

**Mono is width zero**, not a switch: one value, one control, and no way for a boolean and a
knob to disagree. The mixer still shows a *Mono* button because that is what people look
for, but it is a shortcut to the end stop. Width sits after the inserts and before the pan,
which is a console strip's order — narrowing a stereo reverb has to happen before the
channel is placed. Polarity is an input-stage switch, before the inserts, as on a console;
on the master it is the last thing before the output.

**Null tests.** `width_at_unity_and_polarity_off_change_nothing` is the standing rule made
explicit: a control at its default must not touch a sample, so `apply_width` returns early
at unity rather than computing a mid/side round trip that is only *algebraically* the
identity. `a_track_against_its_inverted_copy_is_exactly_silence` is the classic one — a
track summed with a polarity-inverted copy of itself cancels to zero, not to a small number
— with the flip removed at the end so the test cannot pass by nothing playing.

### Three defects the round-11 review found, and what they were really about

All three shipped green. Worth writing down, because each one says something about what
the tests were and were not asserting.

**1. Resizing a clip moved it instead.** The drag hit-tested where the pointer *is*, but
egui only reports a drag once the pointer has travelled a few pixels — by which time it has
left the 8-px trim zone at the clip's edge, so every resize was read as a move. The fix is
to hit-test the **press origin**, which is exactly what the piano roll learned in round 3
and what its own note about the bug says. The arrangement was written without reading it.

**2. Moving a clip left its notes behind.** `move_to` was one line —
`set_window(track, clip, start, clip.length)` — and `set_window` is *trim* semantics: it
moves `offset` with `start` so the content stays anchored where it sounds. That is right for
an edge and precisely backwards for a move, so a dragged clip slid out from under its own
music. The two operations are opposites on the same two fields and now say so in their doc
comments.

The point is not the one-line fix. **The whole test suite passed with this bug in it**,
because the clip tests asserted on `start` and on the note list — and a clip that has slid
away from its notes still has exactly the `start` you asked for, and the note list of a
clip *never* changes when it moves, since it is the window that moves. The clip unit tests
in `clips.rs` had it right and asserted on `Snapshot::compile` — what the engine would
actually play — and the gesture tests in `app_flows.rs` did not. They do now, and the new
one was checked against the broken code before the fix went in.

**3. Clip editing was invisible.** It existed only on a right-click and on shortcuts
nobody had been told about, and the right-click menu closed the moment the pointer left the
clip — it was drawn inside `if let Some(hit)`, and a context menu appears *next to* the
pointer, so reaching for it took the pointer off its own target. Now: a **Clip** menu in
the menu bar, the context menu opened on the clip that was under the pointer when the
button went down and drawn every frame after, every entry showing the key bound to it
(`Keymap::display_for`, so the menu teaches the shortcut), and the status bar's hint
following `EditFocus` instead of always describing the piano roll.

The **Edit** menu was calling `cut_selection`/`paste_clipboard` directly, so it was
permanently about notes however the arrangement was being used. It goes through
`run_action` now — which is ADR 0010's rule, stated there and not followed here.

### A toolbar for clips, and a key for every one of them

Asked for after the fixes above: the operations to hand, as icons, the way the piano roll's
tools are — because a menu entry and a shortcut are not the same as a row of buttons above
the thing they act on.

`views::arrangement::toolbar` mirrors the piano roll's row exactly: title, icon buttons,
then the grid picker and the snap toggle, then a count on the right. Two new icons (`Glue`,
`Duplicate`); split reuses `Slice`, because it is the same idea applied to a clip and two
glyphs for one operation would be worse than one shared. `active` still means *state* —
mute and repeat light up because the clip is muted or repeating — never a press.

- **The colour button shows the colour**, not an icon of one. A monochrome glyph could only
  say "colour"; the swatch says *which*, and a diagonal bar says "the track's".
- **The grid is the same list the piano roll offers** (`views::GRIDS`), with the same names.
  Two pickers that disagreed about what "1/8" is called would be a small cruelty. The
  arrangement defaults to a bar; the piano roll to 1/16.
- Every tooltip carries the key bound **right now** (`Keymap::display_for`), so the toolbar
  teaches the shortcut instead of competing with it.

**Every clip action now has a key**, and a test asserts it for the whole `Clips` category
rather than for the ones that had one the day it was written: split `Ctrl+E`, glue
`Ctrl+J`, duplicate `Ctrl+B`/`Ctrl+D`, mute `0`, repeat `Ctrl+Shift+L`, colour
`Ctrl+Shift+C`, rename `F2`, delete `Del`.

Two of those are new operations, not just new keys:

- **Rename**, which `SetClipName` had supported since round 3 with no UI at all. `F2` opens
  an editor **on the clip itself** — the name is a label on a rectangle, and typing it in a
  dialog means looking away from the thing you are naming. Enter and clicking away commit;
  Escape and an empty name leave it alone, because a clip with no name is worse than the
  one it had.
- **Colour cycling**, which steps the selection through the palette and off the end back to
  the track's colour — so one key both sets a colour and clears it, and there is no state
  you can reach and not leave.

**`TimelineGeometry`.** The view now records where it drew the timeline each frame, and the
drag tests ask it instead of recomputing the layout from constants. They had mirrored it,
and adding this toolbar broke all four of them — a test that recomputes the layout is a
test that breaks the day the layout changes, which is the day it is least useful.

### What is not done, and why

- **Per-pad mixer routing** for a drum kit waits for bus tracks — see the drums plan.
- **The arrangement is roughly a third of FL's playlist**, and the gap is measured item by
  item in the plan above rather than left as an impression.
- **Repeat by dragging the right edge past the content** is not bound to a bare drag. That
  gesture already means "give me back the notes I shrank away", which is a promise Cantus
  makes and Live makes too. Repeat is the clip menu's *Repeat the content* and the
  `toggle-clip-loop` action.
- **`Ctrl+A` selects notes or clips by focus**, but there is no separate "select all clips
  on this track" yet.
- The **velocity lane at the widest zoom** still overlaps into a solid block (round 9).

## Planned: the arrangement, measured against FL Studio's playlist (requested 2026-09-10)

Read against [FL's playlist manual][fl-playlist] and its own screenshots. The point is not
to end up looking like FL — it is to know exactly what is missing and to decide, for each
thing, whether to take it, take it differently, or not take it at all. An unexamined "we
should have that too" is how a DAW ends up with two of everything.

[fl-playlist]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm

### Where we stand

The arrangement today: one clip type (notes), tracks of one fixed height, a bar ruler with
a loop region, a clip toolbar, and the round-11 clip editing — split, glue, duplicate,
trim, slip, repeat, mute, colour, name, marquee and `Ctrl+click` selection, drag between
tracks, Alt-drag to copy. That is a working arrangement and roughly Live's Session-less
core. It is perhaps a third of what FL's playlist does.

### The gap

**Verdicts:** *take* — do it as FL does. *differently* — the need is real, the mechanism
should not be copied. *not this* — a deliberate no, with the reason. *blocked* — wanted,
but something else has to exist first.

#### Clip types

| FL has | We have | Verdict |
|---|---|---|
| **Audio clips** — waveform, fades/gain envelope, time-stretch, beat-slicing, zero-cross slicing, reverse | nothing | **take**, and it is the single biggest hole. Needs an audio-file model, a sample cache, and waveform drawing (a `Mesh` with LOD from line one) |
| **Automation clips** — control-point envelopes, step/slide modes, articulator tools | nothing | **take** — already roadmap step 3. FL's *articulators* (flip, scale, normalise, smooth, "create sequence") are worth stealing wholesale; almost nobody else has them |
| Stereo / combined / **spectral** view per audio clip | — | **take, later**. A spectral view in the arrangement is rare and genuinely useful; it is also a lot of DSP for a view |
| Pattern clips carrying **notes *and* automation** in one clip | notes only | **differently** — with automation lanes on tracks, a clip carrying both is a modelling choice we should make once, deliberately |
| **Stem extraction / Deverb** (source separation) | — | **not this**, not for a long time. It is a machine-learning product inside a DAW; it belongs after the AI layer exists, if ever |

#### Tracks

| FL has | We have | Verdict |
|---|---|---|
| **Per-track height**, dragged on the divider, presets 33–200 %, lock to size | ~~one global height~~ **done (round 12)**: divider, four presets, *give every track this height*, stored in the project | **take**. This is small and it is felt every session |
| **Track groups** (drag to nest), group solo/mute, auto-colour a group | none | **take** — and it should be the same tree the mixer's buses use, not a second grouping |
| **Track icons** beside the name | **done (round 12)**: six painted icons, derived from the instrument until one is chosen | **take**. An icon is read faster than a name at 40 px |
| **Track modes** (unassigned / instrument / audio) linking track ↔ channel ↔ mixer track | every track is an instrument track | **differently.** FL needs modes because its playlist is *not* the instrument list; ours is. A track is an instrument or (later) an audio track, and that follows from what is on it |
| **Auto-name / auto-name clips** from the instrument | manual names | **take** — a new clip should be called what the track plays, not "Clip" |
| **Reorder tracks** by drag or Shift+wheel | **done (round 12)**: drag the header | **take**. The command is already there and reversible; it needs a drag handle |
| **Track selector** strip, `Ctrl+↑/↓` to change the selected track | **done (round 12)**: `Ctrl+↑/↓` | **take** |
| **Performance settings** per track (launch modes, trigger columns) | — | **blocked** on a session/clip-launch view, which is a whole feature (idea.md §5.4) |

#### Time: markers, signatures, arrangements

| FL has | We have | Verdict |
|---|---|---|
| **Named time markers** in the ruler | loop region only | **take.** Markers are how a song becomes navigable, and they cost a `Vec<Marker>` and a schema bump |
| Marker **actions**: Start, Loop, Marker loop, Marker skip, Marker pause, Start/Stop recording | — | **take the first three, differently.** Loop/skip/pause turn the ruler into a little program, which is powerful and unreadable. A marker should say *where* something is; a arrangement-level structure should say what plays. Take Start and Loop; leave the branching to a proper arranger track (idea.md §5.4) |
| **Time-signature changes** at markers, per region | one signature per project | **take.** It changes the ruler, the grid, the bar numbers and `ticks_per_bar` everywhere — the reason to do it early rather than late |
| **Multiple named arrangements** sharing channels and mixer | one | **take, differently.** The need is real (an edit, a radio version). FL's caveat — "be careful not to delete Channels used by other Arrangements" — is a warning about a model that lets you break the thing you are not looking at. Ours should make the shared part explicit rather than a footnote |
| **Insert / delete space**, slice-and-insert (`Ctrl+Ins`, `Ctrl+Del`, `Ctrl+Alt+Ins`) | — | **take.** This is ripple editing and it is how a song's structure actually gets changed |

#### Selection and editing

| FL has | We have | Verdict |
|---|---|---|
| **Timeline selection**: `Ctrl+drag` in the ruler selects a time range across all tracks | **done (round 12)**, on a bare drag in the ruler's lower lane; the loop brace moved to its own lane on top | **take.** It is the anchor for insert/delete space, loop-to-selection, render-selection, quantize and consolidate. Nearly every entry below depends on it |
| **Eight tools** (draw, paint, delete, mute, slip, slice, select, zoom) as modes | modifiers and a toolbar | **differently.** The piano roll has tools because a note is drawn; a clip is mostly moved. Add the two that are modes rather than gestures — **slice** and **mute** — and leave the rest as direct manipulation. Eight letter keys to memorise is FL's tax, not its feature |
| **Magic lasso** (loop the pointer around clips, no click) | marquee | **take.** It is genuinely faster and it is one of the few things nobody else copied |
| **Clip groups** (`Shift+G` / `Alt+G`) — move and delete together | — | **take** |
| **Make unique** (detach a clip from its shared source) | n/a — clips own their notes | **decision, see below** |
| **Ripple edit** on a clip edge (`Shift+S`) | — | **take** once timeline selection exists |
| **Swap clip source**, transpose clips, "select all similar" | — | **take** — cheap, and each saves a repetitive job |
| **Quantize clip start times** (`Ctrl+Q`) | note quantize only | **take** |
| Snap: Main / Line / **Cell** / None / steps / beats / bar / **Events** | grid list + snap toggle | **take Cell and Events.** "Cell" snaps to the grid cell rather than the line, which is what you want when placing; "Events" snaps to other clips' edges, which is what you want when butting two clips together |

#### Navigation

| FL has | We have | Verdict |
|---|---|---|
| **Zoom & scroll bar** doubling as a **mini playlist preview** of the whole song | plain scrolling | **take.** It is the single best orientation device in FL's playlist, and it is cheap once clips are already drawn as a mesh |
| Zoom presets `Shift+1..5`, zoom to selection, `Ctrl+RMB` zoom, middle-mouse pan | zoom to selection **done (round 12**, `Z`); the rest not yet | **take** |
| **Vertical zoom** / resize all tracks | — | **take** with per-track height |
| **Center on playhead** (`Shift+0`), follow playback | — | **take** |
| **Picker panel** — every clip in the project, filterable, drag into the playlist | — | **take, differently**, and only after the source decision below: without shared sources a picker is a list of things to copy, which is much less useful |

#### Rendering and performance

| FL has | We have | Verdict |
|---|---|---|
| **Consolidate** a track or selection to audio, muting the source (freeze) | — | **blocked** on audio clips; then **take**. It is how a CPU-heavy project stays playable |
| **Export all tracks / track groups** as stems | whole-mix WAV only | **take.** The render path exists and is already proven bit-identical; stems are a loop over it with solo applied |
| **Performance mode** — trigger and loop tracks and clips live | — | **not this, here.** Live triggering is a *session view* (idea.md §5.4), and bolting it onto the linear playlist is exactly the "two DAWs in one" trap. It should be its own view sharing the same clips |

### The three decisions to make first

Like the content offset was for clip editing: get these wrong and everything above has to
be redone.

**1. Does a clip own its notes, or reference a shared source?** In FL a Pattern Clip is an
*instance* of a pattern; twenty copies of a drum loop are twenty views of one thing, and
editing any of them edits all. "Make unique" is what detaches one. In Cantus a clip owns
its notes, so twenty copies are twenty independent note lists.

FL's model is the productive one for the music FL is used for, and the picker panel, "select
all similar", "make unique" and "swap source" all exist only because of it. But it also
means every edit is an action at a distance, which is why *make unique* has to be so
reachable.

The recommendation is to add it as an **opt-in link, not the default**: `Clip.source:
Option<PatternId>`, where `None` is exactly today's clip and `Some` means the notes live in
a project-level pool. Independent by default, shared when you ask; "make unique" writes the
pool's notes back into the clip. That keeps every existing project and every existing test
meaning what it meant, and it is a strict addition to the model rather than a replacement.
It is also a schema bump, a change to `Snapshot::compile`, and a change to every command
that edits notes — so it is the *first* thing to do, not a later one.

**2. Timeline selection, before anything that depends on it.** A time range across all
tracks is the anchor for insert/delete space, ripple, loop-to-selection, quantize starts,
render selection, consolidate, and zoom-to-selection. Building any of those first means
building it twice.

**3. Per-track height and track groups are the same feature as mixer buses.** A "group" in
the arrangement and a bus in the mixer must be one tree, decided once. Two grouping systems
that nearly agree is a bug that takes a year to surface and a rewrite to fix.

### The interface gap, specifically

The honest summary: **we are ahead of FL on discipline and well behind it on density and
richness.** Both halves of that are worth stating precisely, because "make it more polished"
is not a task and this list is.

**What FL's playlist does better than ours, and why it works:**

1. **Every clip has a title bar.** ~~Ours paints the name straight onto the body.~~
   **Done (round 12, visual pass)**: a bar in the clip's colour with the name in an ink
   that reads on it, over a wash body carrying the preview.
2. **The clip body *is* the preview.** Notes, automation curves as filled areas, waveforms,
   spectra — a clip is recognisable without reading it. Ours draws a note miniature and
   nothing else, because there is nothing else yet.
3. **The track header is a map, not a label.** **Done (round 12)**: a miniature of the
   track's clips across the whole piece behind the name.
4. **Colour identity is total.** **Done as a tint (round 12)**: header, lane, clip bar
   and body all carry the track's colour, none of them painted solid.
5. **Density.** **Done (round 12)**: 40-px headers, the strip controls moved to the
   inspector and the mixer.
6. **The scrollbar is a mini-map** of the entire song. It is the best orientation device in
   the playlist and it costs almost nothing once clips are already drawn as a mesh.
7. **Time markers are chips in the ruler** with names, and a time selection is a translucent
   column across every track. Both make the song's structure visible rather than remembered.

**What we already do better, and should not give up to get the above:**

- **Type.** Bundled IBM Plex with a named scale, and every number monospaced so nothing
  jitters as it counts (ADR 0009). FL's playlist sets everything in one small proportional
  face; its bar numbers shift as they scroll.
- **Colour is the user's**, editable to 22 tokens, with a test that every shipped theme
  clears WCAG AA. FL's is fixed per theme and several of its greys fail contrast outright.
- **Keyboard focus is visible** on every widget in the app, from one implementation. FL's
  playlist shows none at all.
- **The accent means state**, never decoration. FL colours everything, which is precisely
  why nothing in it stands out.
- **Every menu entry shows its shortcut**, so the menu teaches the keyboard.

**What "superior" would concretely mean.** Not FL's look — FL's *information*, at our
standard of craft:

| | |
|---|---|
| **Clip chrome** | Title bar with a type icon and a grab area; the name stays readable down to a 12-px-wide clip, and disappears rather than smearing below that |
| **Body as preview** | Every clip type draws its content, at every zoom, through a `Mesh` with an LOD rule (ADR 0011). FL degrades to mush when zoomed out; we already have the machinery not to |
| **Header as map** | A miniature of the track's content behind the name — and the mixer-strip controls moved out of the header into the inspector where they belong, which is what buys the density back |
| **Mini-map scrollbar** | The whole song in the scroll bar, with markers on it |
| **Structure visible** | Marker chips in the ruler; a time selection as a column across all tracks |
| **Motion** | State changes animate — a clip landing, a selection forming, a marker being dragged — and nothing else does. Never at the cost of a frame; the budget is the ADR 0011 one |
| **Every zoom readable** | The one thing no DAW does well. A 300-bar view should still tell you where the chorus is |

### Order of work

Grouped so that each block is shippable and nothing is built twice. The drums block
(previous section) is independent of all of this and can go before, after or between.

1. ~~**Timeline selection**~~ **done (round 12)** — with loop-to-selection and
   zoom-to-selection hanging off it; the rest of what hangs off it is step 5.
2. ~~**Track shape**~~ **done (round 12)** except auto-naming clips from the instrument,
   which is small and still open. The header is 40 px and the fader/pan/meter are in the
   inspector.
3. **Clip chrome and the header map** — the visual block above. No model changes, and it is
   the one that will make the arrangement *feel* like a finished product.
4. **Markers and time signatures**: named markers, Start and Loop actions, per-region time
   signature. A schema bump, and it touches the ruler, the grid and `ticks_per_bar`
   everywhere — which is the argument for doing it before there are many projects.
5. **Structural editing**: insert/delete space, ripple, clip groups, magic lasso, snap to
   cell and to events, quantize clip starts, swap source, select-all-similar.
6. **Navigation**: mini-map scrollbar, zoom presets, zoom to selection, centre on playhead,
   follow playback, middle-mouse pan.
7. **Shared clip sources** (decision 1) and then the **picker panel**, which is only worth
   much once sources are shared.
8. **Audio clips** — the biggest single block, and the one that needs a file model, a sample
   cache, waveform drawing and stretch. Then **consolidate/freeze** and **stem export**,
   which are cheap once audio clips exist and the render path is already proven.
9. **Automation clips and lanes** (roadmap step 3), with FL's articulator tools — flip,
   scale, normalise, smooth, create-sequence — which are nearly free once envelopes exist
   and which almost no other DAW offers.
10. **Multiple arrangements**, once there is enough in a project for a second version of it
    to be worth having.

**Deliberately not on this list:** performance mode (it is a session view, and it belongs in
its own view sharing the same clips — not bolted onto the linear timeline), stem separation
and de-verb (a machine-learning product inside a DAW), and FL's eight-tool model for the
arrangement (two of the eight are modes; the rest are gestures we already have).

## Planned: drums, a channel rack, and a piano roll that knows what a pad is (requested 2026-09-10)

**The ask.** A drum kit with a *good* kick, snare and hi-hat; a way to program it in the
piano roll with the keys adapted or renamed for percussion; and a **channel rack** like FL
Studio's ([its manual][fl-rack] is the reference for what that means).

[fl-rack]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm

### The decisions to make first

These are the equivalents of the content offset in the clip work: get them wrong and every
later piece has to be redone.

**1. A kit is one instrument on one track, and pads are pitches.** ~~Not one track per drum.~~
*Revised the same day (round 12, second half; ADR 0013): a channel is a track, so a drum
that needs its own strip is a track with a one-pad kit, and the rack lists every track.
Per-drum routing is therefore real today up to one strip per drum; grouping several into
one strip is buses, next.*
A kit is one instrument the way a piano is; a pad is a note on it. Everything already
built then applies unchanged — one clip, one note list, the same commands, the same undo,
the same snapshot. FL's model, where every drum is its own channel with its own mixer
track, buys per-drum inserts, and that is worth having — but it needs **bus tracks and
sends**, which are already next-but-one in the roadmap. So: pads carry their own level, pan
and choke inside the kit now, and per-pad *routing* arrives with the buses rather than
being faked before them.

**2. The step sequencer is a second editor for the same clip, not a second data model.**
A step is a note: pad pitch, position `i × step`, a short length, a velocity. Toggling a
step is `InsertNotes`/`RemoveNotes` like everything else, so it undoes, it saves, and it is
the same thing the piano roll is editing — switch views mid-edit and nothing is lost or
converted. FL keeps them as two representations that need converting between; we do not
have to, and should not.

**3. A "pattern" in Cantus is a clip.** FL's Channel Rack is a *global* pattern bank: every
channel shares one pattern, and patterns are placed in the playlist. That is a second
timeline model beside the one Cantus has, and running both is how a DAW becomes two DAWs.
Clips already do what patterns are for — duplicate, repeat (`content_length`, round 11),
colour, name — so the channel rack edits **the selected clip**, and reuse stays clips.
This is the one place where we deliberately do not copy FL, and it should be said out loud
rather than discovered.

**4. Swing is a clip property, applied when the snapshot is compiled.** Not baked into note
positions. FL's swing is a playback-time offset of the even steps and is adjustable
afterwards; if we wrote it into the notes it would be destructive and un-un-doable by
degree. That makes it a `Clip` field and therefore a schema bump — cheap now, expensive
after people have projects with grooves in them.

**5. Lane names come from the instrument.** The piano roll asks the track's instrument what
a pitch is called and which pitches are worth showing, instead of assuming a keyboard.
`InstrumentSpec` grows `lane_name(pitch) -> Option<&str>` and `lanes() -> Option<&[Pitch]>`;
`pitch_name` stays the fallback. That is what makes drum mode a property of the *kit*
rather than a mode the user has to remember to switch on, and it is what a sampler with
named zones will want later too.

### Pulsus — the kit

Named like Firmus: *pulsus* is the Latin for a beat, a striking. **Synthesized, not
sampled**, and that is a decision rather than a shortcut:

- ADR 0001 requires bundled samples to be original or CC0, so every sampled kit is a
  licensing question before it is a musical one. Synthesis has none.
- A synthesized drum is *tunable*: the kick's pitch envelope, the snare's body, the hat's
  decay are parameters, not a file you replace. That is the difference between a kit and
  a folder of wavs.
- It costs kilobytes, and the sampler (roadmap) is the right home for sampled kits anyway.

| Voice | Design | The parameters that matter |
|---|---|---|
| **Kick** | Sine body with a fast downward pitch envelope, plus a short click transient, plus soft saturation | start pitch (~110 Hz), end pitch (~45 Hz), pitch decay (~40 ms), amp decay, click level, drive |
| **Snare** | Two tuned bodies (≈180 Hz and ≈330 Hz) plus band-passed noise, decaying separately | body pitch, body/noise mix, body decay, noise decay, noise tone (band-pass centre) |
| **Hi-hat** | Six detuned square oscillators through a band-pass then a high-pass — the 808 method, which is what makes a hat metallic rather than just bright | tone (band-pass centre), closed decay, open decay, HP cutoff |

**Choke groups** are not a detail to leave for later: an open hat must be cut by a closed
hat, or the kit does not sound like a kit. A pad names a choke group; a pad triggering
silences any voice in the same group. It is a handful of lines in the voice allocator and
it is the difference between "hi-hats" and "two unrelated noises".

**The standing rule applies before any of it lands:** a null test per voice — block-size
independence and offline-equals-realtime — the same as every other DSP module.

### The channel rack

A panel listing the kit's pads, one row each, editing the selected clip. Following FL's
layout where it is right, which is most of it:

| Element | FL's | Ours |
|---|---|---|
| Mute | per channel | per pad |
| Pan / volume knobs | per channel, pre-mixer | per pad, inside the kit |
| Channel button | opens the plugin | opens the pad in the inspector |
| **Step grid** | 16th notes, left-click on, right-click off | the same, on the selected clip's notes |
| Steps per pattern | up to 512 | the clip's length ÷ the grid, so it follows the clip |
| **Graph editor** (`Ctrl+K`) | velocity, pan, pitch, shift, … per step | velocity first; the rest as the note model grows |
| Global + channel swing | both must be > 0 | one swing per clip (decision 4) — two controls that must agree is a trap |
| Mixer routing | per channel | one strip per track today (the rack shows the number); several channels into one strip needs buses — next |
| Channel groups/filters | named groups, Page Up/Down | later, and only if a kit ever gets big enough to need it |

The grid is dense — pads × steps, redrawn every frame — so it goes into a **`Mesh` with a
level-of-detail rule** from the first line, per ADR 0011. Not after it is slow.

### Drum mode in the piano roll

With decision 5 in place this is small, and it is what "adapting the keys" means:

- The key strip shows **pad names** — Kick, Snare, Closed hat — instead of C1/D1/F#1, and
  the black/white key colouring gives way to one lane per pad.
- Only the pitches the kit actually uses get a lane, so a 12-pad kit is 12 rows and not
  128 with 12 in use.
- Notes default to a fixed short length, because a drum has no note-off worth aiming at.
- A pad's name is editable, and travels in the kit rather than in the project, so a kit
  renamed once is renamed everywhere it is used.

The velocity lane, the tools, the clipboard, quantize and everything else keep working
unchanged — which is the whole reason for making a pad a pitch rather than a new kind of
thing.

### Order of work

1. ~~**`InstrumentSpec::lane_name` / `lanes`**~~ **done (round 12)** — as `Lanes`, not
   `Option<&[Pitch]>`; ADR 0013 says why.
2. ~~**Pulsus**~~ **done (round 12)**, schema 6. Seven pads: the three voices plus toms and
   a rim as presets of the snare shape.
3. ~~**The channel rack panel**~~ **done (round 12)**, `F6`.
4. **Swing** as a clip property — **still open**, and the next schema bump (8).
5. Per-pad mixer routing, when bus tracks arrive.

More voices — toms, clap, rim, ride, crash — are the same shapes with different numbers
once the three above are right, and are not worth planning separately.

## Appearance (round 10): white by default, and yours to change

**The default is now light.** `theme::paper_palette()` — white canvas, light grey panels,
ink text, the gold accent darkened until it is readable on white. Ink & Gold stays as the
dark preset, and Parchment as the warm light one; *Settings → Theme* switches between them.
An existing `theme.json` is left alone, so anyone already on a theme keeps it.

**Every colour is legible, and a test says so.** `every_shipped_theme_is_legible` computes
WCAG contrast for each preset: 4.5:1 for text and the accent, 3:1 for the focus ring, the
bright accent and the selection. Writing it immediately found two real defects in the
*existing* Parchment theme — its gold managed 2.88:1 against the panel, so every heading
was decorative rather than readable, and its dim text sat on exactly 4.50:1. Both fixed.

**The whole palette is editable in the app**, not just in the file. *Settings → Colours*
opens a window with a picker for each of the 22 tokens, grouped, applied as you drag and
written through to `theme.json` — which stays the source of truth and what `ThemeWatcher`
reloads. `Theme::tokens_mut` is what the dialog iterates, and a test asserts it exposes
every hex field the struct serializes, so a token cannot be added and silently left
unreachable.

**Interface size** is a setting too (`settings.ui_scale`, 70–200 %), applied through
egui's zoom factor so the spacing grid scales with the type instead of only the text
growing. Clamped, NaN included, because it comes from a file people edit.

Two defects the white ground exposed, both fixed:

- The piano roll dimmed everything past the end of the clip with a **black scrim**. On ink
  it was invisible; on white it was a heavy grey slab covering half the view. It now paints
  the panel colour — one step of the elevation scale, right in any theme.
- **Selected notes were painted white**, which was my own regression from the mesh
  batching: unmistakable on ink, invisible on paper. Selection now uses the palette's
  `selection` colour with a `text`-coloured outline, and the contrast test covers it.

**Faders**: egui defaults to a chunky rectangle on a thick rail — the "old Windows" look.
They are now a slim cap (`HandleShape::Rect { aspect_ratio: 0.38 }`) on a 4-px groove with
a defined edge. **Known limit:** egui paints a slider's groove and its cap from the same
`inactive.bg_fill`, so at rest they can only differ by that edge. A cap with its own fill,
a centre line and dB ticks needs a widget of our own — the next thing to do for the mixer.

## Piano roll performance (round 9) — and why we stayed on egui

Prompted by "would Slint be better?", ADR 0011. The honest answer needed a number, and the
number did not exist: the stress test ran at a zoom where culling left **139 notes** on
screen, so ADR 0003's "60 fps with 20 000 visible notes" was never being checked.

Measured properly, with tessellation included:

| | Frame | Triangles | Per note |
|---|---|---|---|
| Before | **74.7 ms** (13 fps) | 416 772 | 20.8 |
| After | **6.9 ms** (146 fps) | 96 772 | 4.8 |

Two causes, both ours. Notes were drawn as rounded rectangles with a stroked outline at
every zoom, and epaint anti-aliases every filled shape into a feathered ring — about twenty
triangles for a rectangle. About forty lines fixed it: `NoteLod` decides what a note is
worth at its size on screen, and anything below the thresholds joins one `egui::Mesh` at
two triangles each. Tessellation fell from 60.4 ms to 2.9 ms. At editing zoom nothing
changed visually — verified by screenshot.

**Convention this establishes:** anything drawn once per note, clip, sample or automation
point goes into a `Mesh` and gets a level-of-detail rule. Not an optimisation to remember —
a rule, because the convenience painters cost ~10x per item.

**What the speed bought:** the piano roll's zoom-out limit was 4 px/beat — about 80 bars —
and it was there because drawing more was ruinous. It is now 0.5 px/beat
(`MIN_PX_PER_BEAT`), so a 1 300-px grid shows some 650 bars and a whole piece fits on
screen. That needed the ruler to thin its bar numbers out, which it now does by doubling
(every bar, every 2, every 4 …) so the numbers shown stay ones a musician counts in, never
every 7th; a test asserts it. Verified by running: the 20 000-note stress project at the
widest zoom draws 6 540 notes in a 6.8 ms frame in a *debug* build, ruler legible, zero
audio overruns.

Known refinement: at that zoom the velocity lane's per-note bars overlap into a solid
block. Truthful about density, but it could show the peak per column instead.

Note on verification: driving the running app with synthetic clicks is unreliable on
Windows — twice now a stray event has landed in the app during a screenshot (once dragging
the autosave slider, once switching theme and selecting every note). When a capture
disagrees with the code, check `settings.json` / `theme.json` before believing the
screenshot, and prefer the headless render tests, which assert on the text actually
painted.

Also fixed: `looping_wraps_and_keeps_playing` asserted `stats().overruns == 0`, which
compares wall clock against the block budget. On a loaded machine a 1 000-block run
exceeds it and the test fails, in an otherwise fully deterministic null test. Real-time
behaviour belongs in `tests/hardware.rs`; a flaky guardrail teaches people to re-run
instead of read.

## Settings (round 8)

`cantus-ui/src/settings.rs` and the *View → Settings…* dialog. Preferences — the ones a
dialog can hold in a handful of rows — live in `settings.json` beside the other user files:
audio device, sample rate, buffer size, language, autosave interval. The theme deliberately
stays in `theme.json`: it is a whole document the user edits by hand and the app reloads
live, not a row in a dialog.

- **Audio changes are applied on *Apply*, never as you drag.** Reopening the device on
  every frame of a combo box would stutter the audio.
- **A settings change can never leave the user in silence.** `EngineHandle::start` falls
  back to the system default when the saved device is not there (unplugged since last
  time), and if nothing can be opened at all, the engine already running is left alone.
  Either way the status line names the device actually in use, so a fallback is visible
  rather than mysterious — there is a test for exactly that.
- **The buffer size is shown in milliseconds** next to the frame count, because that is
  what a musician is actually choosing between.
- **The autosave interval is clamped on the way out** (10 s–10 min): a `settings.json`
  hand-edited to `0` must not turn autosave into a per-frame write of the whole project.
- **Language and theme moved out of the *View* menu** into Settings. Two places to change
  one thing is how they drift apart.
- A test asserts that *opening* the dialog changes nothing and writes no file. A
  preferences window that rewrites preferences by being looked at is a nasty bug, and every
  widget in it is bound to a value that could report "changed" on its first frame.
- `--settings` and `--shortcuts` open those dialogs at startup, next to `--mixer`. They
  exist so that looking at a view does not depend on driving menus, which synthetic clicks
  do not do reliably on Windows.

## The keymap (round 7)

`cantus-ui/src/keymap.rs` and ADR 0010. An `Action` is a thing the app can do, a `Binding`
is a key plus exactly those modifiers, and `keymap.toml` joins them — next to the theme and
the layouts, and written on first run so it is there to be found. *Help → Shortcuts* is now
a real editor: pick a preset, click a shortcut to clear it, click *Add* and press a key to
bind it, with conflicts flagged in red.

- **`CantusApp::run_action` is the single dispatch.** Shortcuts, menus and any future
  script go through one `match`, so a binding cannot reach something a menu cannot. The old
  `if cmd && z` chain in `global_shortcuts` is gone.
- **Modifiers match exactly**, or `Ctrl+Z` would fire on `Ctrl+Shift+Z`.
- **Presets are conflict-free by construction**: a preset is a shared spine plus its own
  bindings, and a later binding takes the shortcut from whoever held it. Tests assert that
  no shipped preset conflicts with itself and that none leaves an essential action unbound
  — that is how the Live preset's `Ctrl+E` (split) colliding with WAV export was caught
  before it shipped; export moved to `Ctrl+Shift+E` there.
- **FL / Live / Bitwig presets are approximations**, following each program's conventions
  for the actions Cantus has. The dialog says so. Only the Cantus preset is authoritative —
  claiming parity we have not verified would be worse than saying nothing.
- **A damaged file never stops the app**: unknown ids and unreadable shortcuts are logged
  and skipped, and something that is not TOML falls back to the default map.
- Views no longer read keys. The piano-roll tooltips ask `Keymap::display_for`, so they
  show whatever is bound today instead of a typed-in "P / 1". `Tool::shortcut()` and
  `Tool::key()` are gone; `Tool::action()` replaces them.
- Mouse and wheel modifiers are not bindings and are not in the file; they are listed
  below the scrolling list — not inside it, where a newcomer would never scroll far enough
  to find them.
- `cantus --shortcuts` opens the editor at startup, next to the existing `--mixer`. Both
  exist so a look at a view does not depend on driving menus.

## Named layouts (round 6)

`cantus-ui/src/layout.rs`. A `Layout` is the shape of the window — the inspector's width,
the arrangement's height, which of the two are showing, and which view fills the centre.
`Layouts` holds the current one plus the user's named ones, in
`%LOCALAPPDATA%/Cantus/layouts.json`. *View → Layout* switches between them, toggles the
panels, saves the current shape under a name, deletes one, or resets to the built-in shape.

Two things about egui 0.36 shaped the design, both found by experiment and both recorded in
tests so a future upgrade tells us if they change:

- **`Panel::default_size` cannot impose a size** once a panel has stored state, and eframe
  is built here **without its `persistence` feature**, so egui does not carry panel sizes
  across restarts at all. The layouts file is what makes sizes survive: the app captures the
  layout on exit and forces it on the first frame.
- **Applying a layout uses `Panel::exact_size` for exactly one frame** (`Layouts::forced` /
  `end_frame`). `exact_size` pins the size range to a point, so it wins whether or not the
  panel has state; the frame after, the panel is resizable again from there. There is no
  setter for `PanelState`, and forging egui's internal state would break the first time it
  changed shape.

A bare `egui::Context` does not keep panel sizes between frames the way eframe's does, so
the tests assert what a *forced* frame produces and leave "a dragged size stays put" to be
checked by running the app. That is noted at the top of the module so the next person does
not read the gap as an oversight.

## Piano roll tools (round 3)

Bug fixed: drags hit-test the **press origin**, not the current pointer, so resizing from a
note's right edge no longer turns into a marquee. Tools use FL's letters — `P` Draw, `B`
Paint, `E` Select, `D` Delete, `T` Mute, `C` Slice, `Y` Play — and also `1`–`7`. The typing
keyboard piano starts **off** (as in FL) because it claims the letter keys; `Ctrl+T` or the
*Keys* toggle in the transport turns it on, and then only the number keys switch tools. Modifiers follow FL:
Shift locks pitch, Ctrl locks time, Alt bypasses snap, right-click deletes, Ctrl+drag
selects, Alt+wheel edits velocity. Shortcuts: Ctrl+B/Ctrl+D duplicate, Ctrl+C/X/V, arrows
nudge (Shift: bar) and transpose (Shift: octave), Esc clears, Ctrl+T toggles the typing
piano. `Note.muted` is schema 3 (fixture `v3.cantus`). All operations live on `CantusApp`
(`slice_at`, `toggle_mute_notes`, `duplicate_selection`, …) so they are unit-tested without
a window.

## How the new pieces fit

- **Effects** (`cantus_core::effect`): a generic `kind + key→f32` parameter model. The
  audio thread never allocates: `EngineHandle::set_snapshot` builds `EffectUnit`s on the UI
  thread for ids the engine has not seen and sends `AddEffect` before the snapshot; units
  dropped by a snapshot come back as `GarbageEffect`. `Processor::rebuild_chain` re-orders
  a track's chain with pre-sized Vecs only.
- **Recording** (`cantus_ui::recording::Recorder`): the engine stamps live notes
  (`EngineEvent::NoteInput`) with the transport tick while `SetRecording(true)`; the UI
  pairs on/off, handles loop wrap and retriggers, and commits each take as **one undo step**.
  `CommandBus` gained *composing* gesture merges for accumulating gestures
  (`Command::supersedes` decides replace-vs-compose).
- **Hardware MIDI** (`cantus_engine::midi`): every input port is opened at start; callbacks
  push into an SPSC ring the audio thread drains each block. The selected track is the
  live track (`SetLiveTrack`).
- Timing note: live notes are stamped at block start (≤ 10 ms at 480 frames). Sub-block
  timestamps from `midir`/cpal are a later refinement.

## Design & personalization plan (requested 2026-09-08)

Goal: a look that says "best DAW in the world" and a UI the user can make their own.
Ordered plan, each step shippable on its own:

1. ~~**Theme tokens as data**~~ **done (round 4)**: `theme::Theme` is serialized to
   `%LOCALAPPDATA%/Cantus/theme.json` (created from "Ink & Gold" on first run), reloaded
   live by `ThemeWatcher`; presets Ink & Gold (dark) and Parchment (light) under
   *View → Theme*, plus "Open theme file". Views read colours via `theme::p()` (a copied
   `Palette`), never constants. `icons.rs` paints all toolbar/transport icons as shapes;
   the piano roll tools are icon buttons with a tooltip "Name (letter / number)" and the
   long hint moved to the status bar.
2. ~~**Design pass**~~ **done (round 5)**: see "The design system" above — bundled IBM
   Plex Sans/Mono, the 4-px grid, `Elevation`, a global focus ring, distinct hover/active
   states, and three more painted icons (Loop, Metronome, Keyboard). ADR 0009 records the
   typography and licensing decision.
3. ~~**Layout personalization**~~ **done (round 6)** for panels: named layouts, saved panel
   sizes, panel visibility and *Reset layout* — see "Named layouts" above. Still open from
   this step: per-view zoom presets and configurable clip colours (track colours are
   already editable in the inspector).
4. ~~**Keyboard map file**~~ **done (round 7)** — see "The keymap" above. The editor is a
   self-contained window today; the settings dialog will host it rather than redefine it.
5. ~~**Settings dialog**~~ **done (round 8)** — see "Settings" above. It hosts the keymap
   editor rather than redefining it.
6. **Piano roll polish** — scale highlighting + snap-to-scale, ghost notes, chord stamps,
   note properties popup, glue/legato, strum/arpeggiate, magic lasso.

## Known gaps / next steps (in order)

1. **What hangs off the time selection**: insert/delete space, ripple, quantize clip
   starts, select clips in the selection, render the selection. See *The next session*.
2. **Swing as a clip property** (schema 10).
3. **Sends**, a *remove strip* button, strip colours in the UI.
4. The rest of the playlist plan, in the order it gives: clip chrome and the header map,
   markers and time signatures, the remaining structural editing, navigation (a mini-map
   scrollbar the channel rack should share), shared clip sources and the picker panel,
   audio clips, automation clips, multiple arrangements.
4. Piano roll polish, the last of the design plan: scale highlighting and snap-to-scale,
   ghost notes from other clips, chord stamps, a note properties popup, glue/legato, strum,
   arpeggiate.
5. **Automation lanes** (parameter curves, sample-accurate) and a **tempo track**.
6. **Sends/buses**: the mixer has strips but no routing yet; add bus tracks + sends.
7. More native devices: EQ, compressor, chorus; a sampler instrument.
8. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
9. **AI providers** (Anthropic/OpenAI/Ollama over `reqwest`), disclosure dialog, MCP server.
10. Recording polish: count-in, punch in/out, overdub vs replace, sub-block MIDI timestamps.
11. Accessibility pass (semantic nodes for the painted views).
12. `docs/adr/0001`: trademark search for the name before any public release.
13. Velocity lane at the widest zoom: per-note bars overlap into a solid block. Showing the
    peak per column would say more.

## Conventions

- Every mutation is a `Command`; never touch `Project` fields directly from the UI.
- Audio-thread code lives only in `cantus-engine::processor` and `cantus-dsp`; both are
  `#![forbid(unsafe_code)]` and must not allocate after construction.
- Add a null test before adding a DSP module; effects must be block-size independent.
- New UI strings go in `crates/cantus-ui/locales/en.txt` **and** `es.txt`; a unit test
  fails otherwise. The bundled fonts lack many symbol glyphs: paint icons or use words.
- Spacing comes from `design::space`, type from the named text styles, surfaces from
  `design::Elevation` — never a bare `12.0` or `.size(15.0)` at a call site. Numbers are
  monospaced.
- egui **panics** on a text style it cannot resolve, so any `egui::Context` that renders a
  view must have had `design::install_style` called on it (the test harness has a `ctx()`
  helper that does what `CantusApp::new` does).
- A new colour token goes in `Palette`, in `Theme` with `#[serde(default)]`, **and** in
  `Theme::fill_missing`, so older `theme.json` files keep loading and get the new token
  written back for the user to edit.
- No view reads keys. A view that wants a shortcut adds a `keymap::Action` and a
  `run_action` arm; a view that wants to *show* one asks `Keymap::display_for`. Adding an
  action also means an `action.<id>` string in both locales.
- A **widget** that moves with the arrow keys while focused calls
  `widgets::claim_arrow_keys`, or the same press also nudges the note selection. Widget
  keys are not shortcuts and do not belong in `keymap.toml`.
- **A drag hit-tests the press origin, not the pointer.** egui reports a drag only after
  the pointer has travelled, so by then it has left whatever narrow zone it was aimed at.
- **A menu item runs a `keymap::Action`**, never the method behind it: the same entry means
  different things in the arrangement and the piano roll, and ADR 0010 says a menu and a
  shortcut must reach the same code. `views::arrangement::action_item` also shows the
  binding, which is how a shortcut gets discovered.
- **A test of a clip edit asserts on `Snapshot::compile`** — what the engine would play —
  and not on the clip's fields. A clip is a window: its notes do not move when it does, so
  a note list that looks unchanged proves nothing.
- **A control at its default must not touch a sample.** Skip the maths, do not rely on it
  being algebraically the identity — `(l+r)/2 + (l−r)/2` is not bit-identical to `l`.
- Anything that changes **what a project renders as** belongs in the project file, not in
  `settings.json` (ADR 0012). Preferences follow the machine; a mix must not.
- The **arrangement** and the **piano roll** have different mouse modifiers on purpose, and
  the shortcuts dialog lists both. `EditFocus` — the half of the window last edited in —
  decides whether a shared shortcut acts on clips or on notes.
- **Anything drawn once per note, clip, step, sample or automation point goes into a
  `Mesh` with a level-of-detail rule** (ADR 0011). The convenience painters cost ~10x per
  item. This is a rule at the first line, not an optimisation after it is slow — the
  channel rack's pad × step grid is the next thing it applies to.
- **A view that is tested by pointer gesture publishes its geometry**
  (`arrangement::TimelineGeometry`) and the test asks it where things are. A test that
  mirrors the layout breaks the day the layout changes, which is the day it is least
  useful.
- Anything a test writes to disk must go through a `tempfile::tempdir()`. `CantusApp`'s
  `layouts_path`, `keymap_path` and `settings_path` are public so tests can redirect them;
  never let a test write to the real `%LOCALAPPDATA%/Cantus`.
- The locale is process-wide and tests run in parallel: a test must not leave it changed,
  or it races every test that asserts an English string.
- Bump `SCHEMA_VERSION`, append a migration and regenerate a fixture
  (`cargo run -p cantus-project --example write_fixture`) whenever `Project` changes shape.
- **An editor's vertical axis comes from the instrument** (`InstrumentSpec::lanes`, ADR
  0013), through `views::piano_roll::Axis`, which also gives a row to any pitch the clip
  holds that the instrument has no lane for. A view must never assume a keyboard, and
  every vertical move — drag, transpose — goes through `Axis::moved`, in rows.
- **A step is a note.** The channel rack issues `InsertNotes` / `RemoveNotes` and holds no
  data of its own. A second editor for a clip must not be a second data model.
- **A pad's mute, like a track's, is resolved in `Snapshot::compile`.** The audio thread
  is handed events with the pad already gone; it never consults a kit per note.
- **A snapshot that will be *played* is made by `EngineHandle::compile`**, never by
  `Snapshot::compile` alone: the engine attaches the sample files. Tests that only look at
  events may compile directly.
- **The sample cache never evicts.** That is what makes every `Arc` the audio thread
  holds safe to drop there. Do not add an eviction without adding a garbage path.
- **Noise is seeded per pad**, never from a global counter: editing one pad must not
  change another's render. The `kick under hats` residue test is the guard.
- **A parameter set changes exactly one field.** The command's inverse restores that
  field; anything else it touched is not undone. The fuzz catches this, but only after.
- **The channel rack is a view over every track.** A "channel" is a `Track`; a new
  per-channel feature goes on `Track` and the rack shows it. What belongs to a *channel of
  the mixer* — inserts, width, polarity, the fader a mix is balanced on — goes on
  `MixerStrip` (ADR 0014); a track has a level and a balance of its own and a route.
- **A snapshot carries its processing order.** `Snapshot::strip_order` is computed in
  `compile`; the audio thread never sorts, searches, or allocates for routing. Anything
  that adds a dependency between strips (sends) recomputes there.
- **Colour on a surface is a tint, never a fill** (`theme::tint`); **text on a colour asks
  `theme::on_color`**. The accent stays free to mean state.
- **The rack's cells are buttons above the level-of-detail threshold and quads below it.**
  The rounded, glossed, shadowed shape costs ~20 triangles; the rule is what keeps a
  512-step pattern under budget, and the test on shape counts is what keeps the rule.
- **A golden render guards a migration that claims to be identical.**
  `a_pre_strip_project_renders_exactly_as_it_did` pins a hash of `v5.cantus`'s render;
  a migration or a signal-path change that moves a sample fails it. Update the hash only
  with a written reason.
- **One instrument per track, both engines per slot.** `TrackSlot` holds a `Firmus` and a
  `Pulsus` preallocated and switches with a flag; a new instrument adds a field, never a
  `Box<dyn>` (ADR 0005). An instrument that pans inside itself renders stereo; everything
  from the polarity switch on is one signal path.
- A parameter of *any* instrument is addressed by `InstrumentParam`, never by a
  per-instrument command. Adding an instrument means a variant there, a `set_param` arm,
  and a fixture that uses it.
- **Text on a light key is `theme::on_light_key`**, never `ink` (the canvas colour, white
  on Paper) and never `text` (light on Ink & Gold). A test that asserts text was painted
  does not assert it was visible.
- **Rows come from `rows_of`.** Track heights vary; anything that needs to know where row
  *n* is asks `TimelineGeometry::rows`, never multiplies by a height.
- **A row's interactive rect owns the grip below it**, shifted down by the grip's width,
  or a press on the divider goes to the row underneath and means something else.
- `--rack` and `--track N` open the channel rack and select a track at startup, next to
  `--mixer`; synthetic input to the running app is unreliable here, and these exist so a
  view of a particular track can be looked at without it.
- Screenshots on this machine: PrintWindow from a DPI-aware process, enumerating the
  process's windows and taking the largest visible one — `MainWindowHandle` points at an
  18x18 helper window and gives a blank capture. Pick the newest `cantus` process, and
  never kill instances you did not start: if one holds the exe and blocks a build, **rename
  the exe** (Windows allows renaming a running binary) rather than killing it.
