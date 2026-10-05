# Round 20 — research-ai (report)

**Branch:** `r20/research-ai` · **Scope:** research only, no code (`research/ai/`).

## Done

- `research/ai/README.md` — Spanish summary, summary, the phased plan (A foundation → B MCP
  server → C assistant panel → D local audio intelligence → E generative audio → F
  further), the owner's legal/ethical decisions, method and caveats.
- `research/ai/cantus-today.md` — `cantus-ai` and the command bus read line by line; what
  is reusable as is (bus, `Origin::{Ai, Mcp}`, `Batch`, audit, `ToolSpec`, `Disclosure`,
  `Permissions`) and what must grow (the trait: streaming, tool results, usage, cancel;
  the catalogue: 7 → ~30 tools).
- `research/ai/mcp-server.md` — MCP 2026-07-28, transports, catalogue of tools /
  resources / prompts, permissions and confirmation, one undo step per AI action, client
  setup (Claude Desktop, Claude Code, Cursor, VS Code, Gemini CLI, LM Studio), Rust SDKs
  (`rmcp` 3.5.1, Apache-2.0), prior-art DAW servers, security, size.
- `research/ai/llm-assistant.md` — Anthropic (verified on the official docs), OpenAI,
  Gemini, Mistral, local models; cost per turn, latency, privacy, key storage; the evolved
  `AiProvider` and three adapters.
- `research/ai/generative-audio.md` — cloud and local generators, stems, audio-to-MIDI,
  AI mastering; licences of models and outputs; how each lands in Cantus.
- `research/ai/legal.md` — GPL fit, output rights, EU AI Act Art. 50, distributors,
  training-data provenance, privacy, voices, ethics; the owner's checklist.

## Tests

None (no code).

## Files touched outside the area

This report only.

## Left

- Many non-Anthropic facts come from search extracts (the proxy blocked the pages):
  marked "[search]" / "[secondary]". Re-check prices, ids and terms when a phase starts.
- Unverified: ElevenLabs per-unit prices, Stable Audio 3 API price, Lyria output-ownership
  clause, Mistral's training default on paid plans, `.mcpb` support for a native binary
  server, MCP Registry GA, `ort`'s exact licence wording.

## Notes for others

- **research (content):** model weights are content — the download-on-demand content
  manager you design (manifest, checksums, licence acceptance) should also carry AI
  models (sizes 0.2–20 GB, VRAM requirements shown before download).
- **device-ux:** `ReplaceEffect` is also what an AI `replace_effect` tool would call;
  keep it one command with an inverse.
- **automation:** an `add_automation` tool would be the AI's way to write curves — stable
  target ids across saves make that possible.
