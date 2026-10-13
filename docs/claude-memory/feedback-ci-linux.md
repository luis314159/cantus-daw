---
name: feedback-ci-linux
description: GitHub Actions runs Linux with -D warnings and fmt --check; Windows-only cfg code breaks it — check in WSL (tools/wsl/check.sh) before pushing
metadata:
  node_type: memory
  type: feedback
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-13T17:29:31.205Z
---

The owner reported (2026-10-13) a red GitHub Actions run: Linux jobs build with
`RUSTFLAGS=-D warnings`, so code used only under `#[cfg(windows)]` (VST3 timer, editor
windows, ASIO example, drop() of a placeholder window) fails on Ubuntu; `cargo fmt --check`
had also been failing since earlier rounds.

**Why:** Windows-only local builds never show these; CI was red for several rounds unnoticed.

**How to apply:** before any push, `cargo fmt --all` and run the Linux check in WSL Ubuntu
24.04 without sudo: `tools/wsl/fetch-deps.sh` once, then `tools/wsl/check.sh <branch>
clippy` and `test` (clones /mnt/f/projects/cantus into ~/cantus-ci). Fix
Linux-only dead code with `#[cfg_attr(not(windows), allow(dead_code))]` or cfg-gating. See
[[machine-f-drive-build-quirks]].
