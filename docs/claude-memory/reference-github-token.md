---
name: reference-github-token
description: "Owner's fine-grained GitHub token for cantus-daw (2026-11-20): Windows Credential Manager target cantus-daw-github; use via C:\\Users\\luis1\\.cantus-tools\\gh-api.ps1 — logs, re-runs, variables; never print it"
metadata:
  node_type: memory
  type: reference
  originSessionId: 02467d0a-aeb1-4d1b-bb55-92c52af32cb9
  modified: 2026-11-20T14:22:06.262Z
---

The owner (remote, 2026-11-20) gave a fine-grained GitHub token scoped to cantus-daw only so I
can do release chores myself: read job logs (no more pasting), re-run/cancel runs, set Actions
variables (e.g. `CANTUS_RELEASE_MACOS`), and edit releases if Contents write was granted. It
lifts the 60/h unauthenticated rate limit to 5 000/h. Expiry ~90 days from 2026-11-20.

Stored only in Windows Credential Manager: generic target `cantus-daw-github`, user `release`.
Use it through `C:\Users\luis1\.cantus-tools\gh-api.ps1 <METHOD> <path> [json body] [-OutFile f]`
(reads the credential via CredRead at call time, sends it only in the header). From Git Bash:
`powershell.exe -NoProfile -File 'C:\Users\luis1\.cantus-tools\gh-api.ps1' GET repos/...`.

**How to apply:** never echo, log, commit or put the token in memory, a builder's prompt or
CI. It is a capability, not an authorisation: decisions (publish without a system, move a
public tag, change a release) still need the owner's say-so as before. The token was pasted
in chat once — suggest rotating it when convenient. Replaces the old "owner pastes CI logs"
habit. See [[public-repo-cantus-daw]], [[fx-side-round]].
