# AI in Cantus — MCP, LLM providers, generative audio

*Research for the backlog, round 20 (requested 2026-10-05). No code. Researched
2026-10-05 at `403ca06`.*

## Resumen (español)

**Qué pidió el dueño:** funciones de IA en el backlog — MCP para que un LLM haga cosas en
el DAW, integración de audio generativo, e integración directa con las APIs de los
proveedores más conocidos (LLM y generadores de audio).

**Lo que ya tenemos.** Cantus nació preparado: toda edición es un `Command` reversible
(ADR 0004) con origen (`User`, `Ai`, `Mcp`…), deshacer y auditoría; `cantus-ai` ya tiene
el trait `AiProvider`, un catálogo de 7 herramientas en JSON Schema (el mismo formato que
usan Claude, OpenAI, Gemini, Ollama y MCP), permisos y un puente que convierte llamadas
del modelo en comandos. Falta: un proveedor real, streaming, resultados de herramientas,
costo, la interfaz.

**Recomendación, por fases:**
1. **Base** — crecer el catálogo a ~30 herramientas, permisos por origen, "una acción de
   IA = un paso de deshacer", vista de actividad de IA.
2. **Servidor MCP primero** — dentro de Cantus (HTTP en `127.0.0.1` con token) más un
   pequeño puente stdio para Claude Desktop/Claude Code/Cursor/VS Code/LM Studio. Barato
   para el proyecto (el usuario trae su modelo y paga su cuenta), y ningún otro DAW lo
   tiene con deshacer, auditoría y confirmación dentro del programa.
3. **Panel asistente** — Anthropic (Sonnet 5.5 por defecto) y un adaptador
   "compatible con OpenAI" que cubre modelos locales (Ollama, LM Studio, llama.cpp) y
   muchas nubes; llaves en el almacén del sistema, aviso de qué se envía, medidor de
   costo y límite de gasto.
4. **Inteligencia de audio local** — análisis (LUFS, espectro, tonalidad), asistente de
   mastering sobre nuestros propios efectos, audio→MIDI (Basic Pitch), separación de
   stems (HT-Demucs). Todo licencias permisivas, sin nube.
5. **Audio generativo** — en la nube con la llave del usuario (Stable Audio, ElevenLabs
   efectos, Google Lyria) y luego modelos locales descargables (ACE-Step, Stable Audio
   Open, Magenta RealTime), siempre con procedencia y marca de "generado por IA".

**Decisiones legales para el dueño:** solo llave propia del usuario; ningún peso de modelo
dentro del instalador; excluir modelos no comerciales y servicios en litigio sin licencia
(Suno, Udio); marcar y declarar el audio generado (EU AI Act art. 50 desde 2026-08-02,
DDEX/DistroKid); Cantus no reclama derechos sobre la música del usuario; nada de
clonación de voz por defecto. Detalle en `legal.md`.

## Summary

The owner asked for three things: **MCP** so an LLM can do things in Cantus, **generative
audio**, and **direct integration** with the main LLM and audio-generation APIs.

Cantus is unusually well placed. Because every mutation is a reversible, serialisable,
origin-tagged `Command` (ADR 0004), an AI edit in Cantus is — by construction — typed,
validated, undoable as one step and audited. Every DAW MCP server found (Ableton, REAPER,
Logic, FL, Bitwig, Ardour, Studio One, Pro Tools; none official) bolts onto its host
through a scripting or MIDI side door, and the most popular one has no undo at all.

The research's conclusions:

- **MCP first.** The current spec is **2026-07-28** (stateless, multi-round-trip
  requests); the official Rust SDK **`rmcp` 3.5.1 (Apache-2.0)** supports it and the
  older revisions clients still speak. One tool table in `cantus-ai` should feed MCP and
  the panel. The server lives in the app (Streamable HTTP on `127.0.0.1`, token, Origin
  check) with a tiny stdio bridge for clients that launch servers. Confirmation happens
  in the Cantus window; one tool call (or one panel turn) = one undo step.
- **Then the panel**, with two adapters that cover most of the market: native
  **Anthropic** (Claude Opus 5.5 `claude-opus-5-5` $4/$20, Sonnet 5.5 `claude-sonnet-5-5`
  $2/$10, Haiku 4.5 `claude-haiku-4-5-20251001` $1/$5 per 1M tokens — Haiku's retirement
  is "not sooner than 2026-10-15", so model ids must be discovered, not hard-coded) and
  **OpenAI-compatible** (Ollama, LM Studio, llama.cpp locally; OpenRouter, Groq, DeepSeek,
  Mistral, xAI in the cloud). OpenAI Responses and Gemini native next. A typical cached
  chat turn costs about a cent on Sonnet 5.5; local is free.
- **Audio intelligence before audio generation.** Deterministic local tools with
  permissive licences — loudness/spectrum analysis, a mastering assistant that sets
  Cantus's own devices, Basic Pitch (Apache-2.0) audio-to-MIDI, HT-Demucs (MIT) stem
  separation — serve the musician's own material and raise no output-rights question.
- **Generation with care.** Cloud, bring-your-own-key: Stable Audio 2.5/3 (licensed
  training data, $0.20/generation), ElevenLabs Sound Effects, Google Lyria 3.5 ($0.08 per
  song, SynthID). Local, downloadable: ACE-Step 1.5 (MIT), Stable Audio Open (Community
  License: < $1M revenue, "Powered by Stability AI"), Magenta RealTime 2 (CC-BY). Excluded:
  non-commercial weights (MusicGen, AudioLDM 2…) and Suno/Udio (no API, litigation).

## Files

| File | Topic |
|---|---|
| [`cantus-today.md`](cantus-today.md) | What exists in `cantus-ai`, the command bus and `idea.md` §4; what is reusable as is and what must grow |
| [`mcp-server.md`](mcp-server.md) | The MCP spec today, transports, the tool/resource/prompt catalogue for a DAW, permissions, one undo step per AI action, how each client connects, Rust SDKs, prior-art DAW servers, security |
| [`llm-assistant.md`](llm-assistant.md) | Anthropic, OpenAI, Gemini, Mistral, local models; cost, latency, privacy, key storage; the evolved `AiProvider` design and adapters |
| [`generative-audio.md`](generative-audio.md) | Stability, ElevenLabs, Google Lyria, Suno/Udio, open models; stem separation; audio-to-MIDI; AI mastering; how each lands in Cantus |
| [`legal.md`](legal.md) | GPL fit, output rights, disclosure and marking, training-data provenance, privacy, voices, ethics; the owner's checklist |

## The phased plan

Each phase is shippable on its own; Cantus stays complete with AI switched off.

| Phase | Deliverable | Depends on | Why this order |
|---|---|---|---|
| **A. Foundation** (ADR "the AI layer") | One tool table (≈ 30 tools: read, tracks, instruments, notes/clips, mixer, effects, transport, undo) with titles, annotations and output schemas; dry-run; per-origin permissions with classes (read / edit / destructive / files-export / network); turn = one undo step; *AI activity* view over the audit log; `AiProvider` grown (multi-part messages, tool results, streaming, usage, cancel, capabilities); `locales/areas/ai.*` | nothing new | Everything else stands on it; small and testable with a fake provider |
| **B. MCP server** | In-app Streamable HTTP (`rmcp`), `127.0.0.1` + token + Origin check, UI-thread marshalling, confirmation card, connected-agent chip; `cantus-mcp` stdio bridge; `.mcpb` for Claude Desktop; *Copy setup for…* Claude Code / Cursor / VS Code / Gemini CLI / LM Studio; resources and prompts | A | Highest value per effort: no provider code, no key handling, no cost to the project; works with the user's Claude/ChatGPT-class app or a local model in LM Studio; proves the tool table in the wild |
| **C. Assistant panel** | Dockable *Assistant* tab; Anthropic + OpenAI-compatible adapters (then OpenAI Responses, Gemini); keys in the OS store (`keyring`); disclosure line, cost meter, spend cap, local-only mode; ghosted previews in the piano roll | A (B shares the gate) | For users without an AI app; same tools, same safety |
| **D. Audio intelligence (local)** | `analyse_loudness` / spectrum / key-tempo tools; mastering assistant proposing settings for Fervor/Latitudo/Culmen and a Matchering-style reference match (GPL-compatible port); Basic Pitch audio-to-MIDI (`tract`/`ort`); HT-Demucs stems (`ort` or demucs-rs); model download through the content manager | A; content manager (`research/content/`) | Serves the musician's own audio; permissive licences; deterministic and null-testable; can run in parallel with C |
| **E. Generative audio** | *Generate* tab and jobs; cloud BYO-key: Stable Audio, ElevenLabs SFX (Music with its exclusions shown), Lyria 3.5; provenance per source (schema bump), "AI" badge, export metadata flags; then local ACE-Step / Stable Audio Open / Magenta RT as downloadable models | A, D's runtime and downloads; legal review of §3 in `legal.md` | Most legal surface and most churn in the market; build it on settled foundations |
| **F. Further** | Cantus as an MCP *client* (external tool servers in the panel); MCP tasks for long jobs; Magenta RealTime as a live AI instrument; per-task provider routing and fallback | B, C, E | Nice to have |

Rough size: A ≈ one round; B ≈ one round; C ≈ one round; D ≈ one to two rounds; E ≈ two
rounds (cloud first, local second).

## Legal and ethical decisions for the owner (detail in `legal.md`)

1. **Bring-your-own-key only** — Cantus ships no provider key and is not a party to any
   provider's terms.
2. **No model weights in the installer or the repo** — download on demand with the
   licence shown and accepted.
3. **Default catalogue = commercially usable licences with licensed/permissive
   training-data claims**; exclude non-commercial weights; do not integrate Suno or Udio.
4. **Output rights** — Cantus claims nothing in what users make; the docs explain that
   AI output alone may not be copyrightable (US Copyright Office, 2025; *Thaler* cert
   denied 2026-03-02) and each generator shows its vendor's output terms.
5. **Disclosure** — provenance per AI source, an "AI" badge, AI flags in exported
   metadata (on by default when AI audio is present); **EU AI Act Art. 50** marking
   (applicable since 2026-08-02; the open-source exemption does not cover Art. 50) needs
   legal review before shipping *local* generation in the EU.
6. **GPL fit** — all candidate Rust dependencies (rmcp, keyring, reqwest, ort, tract,
   candle) are GPL-3.0-compatible; keep them in GPL crates, never in the MIT/Apache core;
   API calls are not linking; weights are separate content under their own licences.
7. **Privacy** — default provider none, per-request disclosure, verifiable local-only
   mode, warnings before uploading audio, Gemini free-tier data-use warning.
8. **Voices** — no voice cloning by default; consent attestation if ever added.
9. **External agents** — off by default, localhost only, visible while connected,
   confirmation inside Cantus, labelled in the history.

## Method and caveats

Sources are cited inline with dates. Anthropic facts were read from the official docs
(platform.claude.com, 2026-10-05). The corporate proxy blocked many other official pages;
those facts come from search-engine extracts of the cited official URLs and are marked
"[search]" (or "[secondary]" for press/blogs). Directly verified besides Anthropic: the
MCP spec's latest revision (2026-07-28), `rmcp` 3.5.1 / Apache-2.0 on crates.io, Gemini
and Lyria prices on ai.google.dev, ACE-Step 1.5's MIT licence, Demucs archived/MIT, the
Magenta RealTime 2 page, Ollama's OpenAI-compatibility page. Re-check prices and model ids
when a phase starts: four OpenAI and three Gemini releases happened in September 2026
alone.
