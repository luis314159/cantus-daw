---
name: reference-anthropic-key
description: "The owner's Claude API key (given 2026-10-30, \"for later\") lives in Windows Credential Manager as generic target \"cantus-daw\", user \"anthropic-api\" — never in the repo, memory or logs"
metadata:
  node_type: memory
  type: reference
  originSessionId: 3b24da2c-539b-4b94-8f79-b015bae6f859
  modified: 2026-10-30T20:46:38.405Z
---

The owner gave a Claude API key on 2026-10-30 to use later (likely the assistant panel,
BACKLOG §7 phase C, or tests against the real API). It is stored only in Windows Credential
Manager: generic credential, target `cantus-daw`, user `anthropic-api` (`cmdkey /list:cantus-daw`).

**How to apply:** read it at the moment of use (PowerShell: CredRead via Add-Type, or a
`keyring`-style call — Cantus's own design keeps keys in the OS store, `research/ai/
llm-assistant.md`), pass it only through an environment variable to the one process that
needs it, never write it to a file, the repository, `docs/claude-memory/`, a commit, a test
fixture, CI or a builder's prompt. Spending on it needs the owner's go-ahead for each use.
See [[round-34]].
