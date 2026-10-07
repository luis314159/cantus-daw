---
name: round-19-recording
description: "Round 19 (2026-10-04/05): pro audio recording, audio clips, master FX chain, Fervor/Latitudo/Culmen, six demos; integrated r19/integration 1422cdc, applied UNCOMMITTED to C:\\projects\\daw; what is left"
metadata:
  node_type: memory
  type: project
  originSessionId: a47a4c80-267b-403d-a74f-bdab0153945d
  modified: 2026-10-05T19:32:18.347Z
---

On 2026-10-04 the user asked (Spanish) to attack what is pending with focus on
**recording from an audio interface professionally, with every feature of the top
recording DAWs**, then polish everything so whole songs can be made; plus **demo songs**
and a **better startup project** ("tema default" = the song that opens). Mid-turn:
**effects on the master channel** ("bien bien importante") and mastering effects
(Maximus-class maximizer, Blood Overdrive/WaveShaper saturation, Patcher mid/side).
Mission they state: the best DAW in the world, done with excellence (Colossians 3:23).

Done: contract 6cba3f7 (schema 18, ADR 0025 recording, ADR 0026 master chain + Fervor /
Latitudo / Culmen), nine builders + fixes-rec + fixes-mix, all merged on branch
`r19/integration` (1422cdc): 1422 tests green, clippy clean. On 2026-10-05 the user
asked to push: committed as 7305d6e on master and pushed to origin
(github.com/luis314159/cantus). Release builds: target\release\cantus.exe (WASAPI) and
target\asio\release\cantus.exe (`--features asio`, CPAL_ASIO_DIR =
C:\projects\daw-r19\asio\ASIOSDK, LIBCLANG_PATH = C:\Program Files\LLVM\bin). Zscaler
quarantines the SDK zip for ~10 min on first download: wait and retry, never bypass it. Worktrees C:\projects\daw-r19\* (targets deleted) await
the user's OK to remove (they asked if they should make repos for them: no, they are worktrees); branches r19/* kept. I ran `cargo clean` in C:\projects\daw, so
the next build there is a full one.

Left (STATUS "The next session: what round 19 left"): test ASIO duplex on the user's
Behringer UMC (Realtek ASIO stalls in duplex — likely cpal's ASIO host); warping/tempo
track; crossfades, clip gain envelopes, MP3, disk streaming; Lamina and Exemplar ~8–9 LU
quiet at unity (+6 dB recommended with a migration — user's call); Culmen's TP margin now
stacks with Terminus's 0.15 dB.

**How to apply:** session daw-11 (effects/UI) stays out of crates/ until the user decides
the commit. See [[cantus-parallel-builders]], [[cantus-workflow-habits]], [[ui-quality-bar]].
