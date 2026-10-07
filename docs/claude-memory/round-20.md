---
name: round-20
description: "Round 20 (started 2026-10-05): automation clips, device-window chrome + swap instrument/effect, visual device browser, hosted CLAP/VST3 instruments, research on free samples/instruments and AI (MCP, LLM APIs, generative audio)"
metadata:
  type: project
---

On 2026-10-05, right after round 19 was pushed (7305d6e), the user asked (Spanish) for:
automation clips; easier, graphical close/minimise of instrument windows; easy swapping
of instruments and of mixer effects; a "see every instrument on screen" browser ("se ve
muy muy pro"); maybe preloaded free instruments (also proves compatibility); and as
backlog research only: rights-free WAVs (kits) and where to store them ("en el repo no
creo"). Then added: backlog research on AI — an MCP server for Cantus, direct LLM provider
APIs, generative audio APIs. They said "levanta múltiples agentes".

Contract 403ca06 on `r20/contract` (`cantus-ui/src/picker.rs`: PickPurpose/PickChoice,
open_picker/apply_pick; locale areas automation/picker/swap/hosted). Six agents in
C:\projects\daw-r20\<name>: automation (ADR 0027, schema 19), device-ux, browser
(ADR 0029), hosted-inst (ADR 0028, also schema 19 on its branch — renumber one to 20 at
merge), research (`research/content/`), research-ai (`research/ai/`), and samples (content manager downloading CC0 packs into the user data dir + sample browser, ADR 0030 — the user said "pon un agente para lo de los wavs de una vez").

**How to apply:** integrate like round 19 ([[round-19-recording]],
[[cantus-parallel-builders]]); the user asked to push to master at the end of round 19, so
ask before committing/pushing round 20 unless they say so again.

Second wave (2026-10-05, the user agreed to my advice: only low-conflict backlog items now,
heavy engine/model items after round 20 merges): verse (RV1909/WEB verse at start, ON by
default with a switch), packaging (installer, signing pipeline, auto-update, CI release
workflow — never tag/publish), audio-setup (always-visible audio chip, first-run wizard,
plug-and-play), bench (benchmarks, callback histograms, dropout blame, perf gates). Base
805a720 (r20/contract also holds docs/BACKLOG-excellence.md). The user owes real-use
testing of the r19 build with their Behringer UMC — the top backlog item.

Owner's priorities (2026-10-05): after round 20, the **next work is performance**
(BACKLOG-excellence §4b: anticipative processing, multicore graph, render cache/auto-freeze,
SIMD, RT priority…), not other features. Verses: RVR1960 chosen (≤500 verses, acknowledgement
in About), RV1909 selectable fallback, WEB for English; the verse follows the live language
switch (Settings ▸ Language already switches EN/ES live).

2026-10-06: the user installed ASIO4ALL (registered fine: ASIO4ALL v2, Realtek ASIO, UMC ASIO
Driver under HKLM\SOFTWARE\ASIO) but didn't see it — likely ran the non-ASIO build or didn't
pick Driver: ASIO / rescan. Asked: FlexASIO (MIT, latest from GitHub API at install time,
never a fixed copy) offered by the first-run wizard and the installer; ASIO4ALL and vendor
drivers as links only. A perf-design agent (r20/perf-design, ADR 0031, design only) runs now;
performance coding waits for the round-20 merge (automation + hosted-inst edit processor.rs).

2026-10-06 (user going to sleep): finish round 20 incl. driver auto-detect + FlexASIO,
then launch the performance round (multicore first) — work in order. Don't touch
C:\projects\daw or push without asking. ADR 0031 + docs/perf/plan.md on r20/perf-design;
r20/engine-research reviews it against market engines and papers (DAFx, ICMC, LAC, ADC;
Ardour, Supernova, JACK2, tracktion_graph, REAPER, ASIO-Guard, Bitwig) — the user wants
the engine to be the best ("toma las mejores ideas ... incluso estudios y papers").

2026-10-06 integration state: r20/integration @74c26e6 holds everything but bench (contract,
research, research-ai, browser, samples, engine-research+perf-design, verse, automation
(schema 19), device-ux, audio-setup, hosted-inst (renumbered schema 20, fixture v20),
packaging (ADR renumbered 0032; samples kept 0030)). Full suite: 1597 passed + 2 fixed
(device_ux expectation; a Windows file-lock flake in cantus-host). Fixes I made: browser
split_left share, keymap v3 fills round-19 keys, automation follows a swapped effect,
one instrument-window path for native+hosted. Open for the owner: in-app FlexASIO
download+run (refused by permission policy; installer has it), code-signing purchase,
CC-BY packs, content hosting, RVR1960 written confirmation.
