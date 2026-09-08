# ADR 0004 — One command bus

**Status:** Accepted · **Date:** 2026-09-07

Every mutation of a project — from the mouse, the keyboard, a script, an MCP agent or an AI
provider — is a `cantus_core::Command`. `CommandBus::execute` applies it to the `Project`,
records the inverse for undo, bumps the project revision and returns the outcome.

Consequences:
- Undo/redo, permissions and audit logging are implemented once.
- The engine never sees commands. It sees **playback snapshots** (`Arc<Snapshot>`) compiled
  from the project after a command applies. Snapshots are immutable and shared, so the audio
  thread reads them without locks.
- `Command` is `serde`-serializable, so a session can be replayed, diffed and sent over MCP.
- Commands carry an `Origin` (User, Script, Ai, Mcp) so the audit log and the permission
  layer can treat them differently without changing their semantics.
