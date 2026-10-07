---
name: instruments-round-17
description: "Round 17 instruments (2026-09-24/25): researched, built by 7 agents, integrated and merged into c:\\projects\\daw as UNCOMMITTED changes on branch rounds-13-17; schemas 16-17; what is left"
metadata:
  node_type: memory
  type: project
  originSessionId: b0b2654a-292c-4c32-83d9-5788c7201752
  modified: 2026-09-25T19:44:22.911Z
---

On 2026-09-24 the user asked (in Spanish) to research the best VST instruments (synths,
bass, drums, samplers, keys, and how they look), then build Cantus's own to equal and beat
them, with Cantus following the plug-in standards. Session daw-b0 owned instruments; the
effects session (daw-7f) owned hosting/export/interchange (ADRs 0019–0022).

State on 2026-09-25:
- Research: `research/instruments/` (9 reports + README). Screenshots local, git-ignored.
- ADR 0023 (instrument contract, shared modulation model, voice allocator, editor frame)
  and `docs/instruments/*.md` (BUILDING brief + one doc per instrument).
- Built and integrated: Unda (wavetable), Calor (analog + cantus_dsp::va), Modulus (FM),
  Exemplar (sampler), Gravis (bass), Organum (organ), Lamina (EP), Pulsus II (in place,
  model version keeps old kits bit-identical). Schema 16 = native instruments, 17 = Pulsus
  II; fixture v17. 1010 tests green in the integration worktree.
- Merged into c:\projects\daw file by file as **uncommitted** changes on top of
  `rounds-13-17` (af84319, committed by daw-7f at the user's request); the user has not
  asked daw-b0 to commit. Branches kept: inst/contract, inst/<name>, inst/integration
  (0db57d9) in C:\projects\daw-inst\.

Round 18 (2026-09-27/28): all 8 editors + Pulsus II moved onto the kit shell (ADR 0024),
opened as DeviceWindow workspace tabs, Alt+K toggles the keyboard card; tests host editors
with `inst::window_in` / `pulsus_editor::window_in`. Kit LedButton/Segmented/MenuField got
`show_at(&Ui, rect)`, `.id .caps .enabled .momentary .value`, and every instrument's
led/segmented/field glue was folded onto them. Merged via inst18/integration (fb79086);
1154 workspace tests green. Committed by daw-86 as e25c393 on rounds-13-17 (2026-09-28, 608 files: rounds 18 +
instruments + research + docs); pushed to origin/master on 2026-10-06 as part of be216de (rounds 19-20 by session daw-af). Builder worktrees
C:\projects\daw-inst\* and daw-inst18\* (~54 GB of target/) are clean and redundant;
removal awaits the user's yes.

**Why:** a later session must know where the instrument work and its history live.

**How to apply:** next steps are in STATUS "The next session" item 1: host hooks (tempo /
SongPosition to units, Gravis kick duck + song key, Pulsus per-pad outputs), hosted
CLAP/VST3 instruments via cantus-host, each instrument's P1 list, a shared preset library.
Open question for the user: bundling CC-BY samples (ADR 0001 says original or CC0).
Merging worktree files into a tree whose files are CRLF gives false whole-file conflicts:
compare ignoring CR, and take the branch's version when only line endings differ.
See [[cantus-parallel-builders]], [[daw-interop-later]].
