---
name: machine-f-drive-build-quirks
description: "The owner's second machine (repo at F:\\projects\\cantus): cargo not on PATH, incremental builds fail to link, UAC blocks test exes named *setup*/*update*"
metadata:
  node_type: memory
  type: reference
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-09T23:45:18.011Z
---

Seen in round 23 (2026-10-09) on the machine where the repository lives at
`F:\projects\cantus` (worktrees in `F:\projects\daw-r23\`):

- **cargo is not on PATH** in either shell: prefix Git Bash commands with
  `export PATH="$HOME/.cargo/bin:$PATH"`.
- **Incremental builds break the link** (MSVC LNK2019/LNK2001 on `anon.<hash>.N.llvm.<n>`
  symbols in cantus_engine / cantus_ui / cantus_core rlibs) after a few edit-build cycles
  — the dev profile's opt-level 1 (3 for engine/dsp) with incremental on. `cargo clean -p`
  does not help; turn incremental off in the worktree's untracked `.cargo/config.toml`
  (`[profile.dev] incremental = false`, same for `[profile.test]`) or `CARGO_INCREMENTAL=0`.
- **UAC installer detection**: test executables whose names contain "setup" or "update"
  (`audio_setup*`, `cantus_update`) fail with "requires elevation (os error 740)" and
  never run. Copy the exe to a neutral name (`audio_stp-…exe`) and run it from the crate
  folder to check them; they pass.
- Python reads a Bash heredoc's *source* as UTF-8, but the console prints em dashes as
  `�` — not a file problem.

**How to apply:** set these up before the first build of a round on this machine; count
the four UAC-blocked binaries by running them renamed when reporting the test total.
See [[cantus-workflow-habits]].
