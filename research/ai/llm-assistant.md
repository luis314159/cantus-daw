# Direct LLM integration: an assistant panel inside Cantus

*Researched 2026-10-05. Anthropic facts were read from the official docs
(platform.claude.com, fetched 2026-10-05). Other providers: the proxy blocked most official
pages, so many facts come from search extracts of the official URLs cited ("[search]");
Gemini prices were read from the official pricing page. Prices change monthly — the app
must never hard-code them (§7).*

## 1. What the panel is for

MCP (`mcp-server.md`) lets a user bring their own AI app. The **assistant panel** is the
built-in path for users who do not have one: a dockable *Assistant* tab (an `egui_dock`
tab like the others) where the user types — in English or Spanish — and the model answers
and acts through the same tool table as MCP. Use cases from `idea.md` §4.5, in order of
value and safety:

1. **Explain** — "why is my mix muddy?", "what does Culmen's *release* do?", "what key is
   this in?" — reads only.
2. **Do the boring thing** — "colour the drums red and route them to a bus", "add a
   sidechain from the kick to the bass", "bring the drum bus up 2 dB".
3. **Write a part** — "a bassline for bars 9–16 following the chords on *Keys*", "a trap
   hi-hat pattern"; notes appear ghosted in the piano roll until accepted.
4. **Mix and master suggestions** — numbers from analysis tools, proposed chains, applied
   only on approval.
5. **Teach** — onboarding inside the DAW ("show me how to make a 808 slide").

Rule (`idea.md` §4.6): **AI suggests, the human decides**; every edit is a previewable,
undoable command; Cantus remains complete with the panel switched off.

## 2. Providers and current models

### Anthropic (verified on the official docs, 2026-10-05)

| Model | API id (alias) | Price in / out per 1M tokens | Cache write 5 min / 1 h · cache read | Context / max out | Latency (Anthropic's own label) | Notes |
|---|---|---|---|---|---|---|
| **Claude Opus 5.5** | `claude-opus-5-5` | $4 / $20 | $5 / $8 · **$0.20** | 1M / 128K | Moderate | Adaptive thinking always on; default effort `medium`; retirement not before 2027-09-22 |
| **Claude Sonnet 5.5** | `claude-sonnet-5-5` | $2 / $10 | $2.50 / $4 · $0.20 | 1M / 128K | Fast | "Best combination of speed and intelligence"; default effort `high`; not before 2027-09-28 |
| **Claude Haiku 4.5** | `claude-haiku-4-5-20251001` (`claude-haiku-4-5`) | $1 / $5 | $1.25 / $2 · $0.10 | 200K / 64K | Fastest | Extended thinking with `budget_tokens`; no `effort`; **retirement "not sooner than October 15, 2026"** — ten days from this research; do not hard-code it |
| Claude Fable 5.1 | `claude-fable-5-1` | $10 / $50 | — · $0.25 | 1M / 128K | Slower | Top tier; overkill for a DAW panel |

Sources: models overview https://platform.claude.com/docs/en/about-claude/models/overview
and pricing https://platform.claude.com/docs/en/about-claude/pricing (both fetched
2026-10-05). Batch API is 50% off (useless for an interactive panel; useful for bulk
sample tagging). Tool use adds a hidden system prompt of 286 tokens on Opus/Sonnet 5.5 and
496 on Haiku 4.5 (pricing page).

API facts that shape the Cantus adapter (Messages API, `POST /v1/messages`; there is **no
official Rust SDK** — raw HTTP over `reqwest` is the documented route for unsupported
languages):

- **Tools**: `tools: [{name, description, input_schema}]` — Cantus's `ToolSpec` maps 1:1.
  `strict: true` (with `additionalProperties: false`) guarantees schema-valid arguments.
  Parallel tool calls are on by default: return *all* `tool_result` blocks in one user
  message; failures as `tool_result` with `is_error: true`
  (https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview).
- **Forced tool choice is gone on the 5.5 models**: `tool_choice` `any`/`tool` returns
  400 on Opus 5.5 and Sonnet 5.5 — use `auto` (+ `strict`) and steer from the prompt.
- **Thinking and effort**: Opus 5.5 thinking cannot be disabled (control cost with
  `output_config.effort`: `low`…`max`); Sonnet 5.5 rejects `{type:"disabled"}` and offers
  `{type:"between_tools"}`. For a chat panel: `low` effort for chat, `medium` for writing
  parts. Thinking tokens bill as output.
- **Streaming**: SSE events `message_start`, `content_block_start`,
  `content_block_delta` (`text_delta`, `input_json_delta` for tool arguments,
  `thinking_delta`), `content_block_stop`, `message_delta` (stop reason, usage),
  `message_stop`, `error`; `eager_input_streaming: true` on a tool streams large tool
  inputs (a 64-note `add_notes`) as they are generated — validate before executing
  (https://platform.claude.com/docs/en/build-with-claude/streaming).
- **Prompt caching**: prefix order `tools → system → messages`; up to 4 breakpoints or a
  top-level automatic `cache_control`; 5-minute TTL by default (1 h optional); minimum
  cacheable prefix **512 tokens** on Opus/Sonnet 5.5, **4 096 on Haiku 4.5**; reads cost
  0.1× input (0.05× on Opus 5.5)
  (https://platform.claude.com/docs/en/build-with-claude/prompt-caching). Design: the tool
  table and a frozen system prompt first (cached), the volatile project snapshot after the
  breakpoint.
- **Stop reasons** to handle: `end_turn`, `tool_use`, `max_tokens`, `pause_turn`, and
  `refusal` (with `stop_details.category`) — show a plain message, never crash.
- **Data**: "Retained data is never used for model training without your express
  permission"; standard API retention deletes inputs/outputs within 30 days; zero data
  retention by agreement
  (https://platform.claude.com/docs/en/build-with-claude/api-and-data-retention;
  https://privacy.claude.com/en/articles/7996866, fetched 2026-10-05).

### OpenAI [search, official URLs]

- Current: **`gpt-6-astra`** (flagship, ~1M context; $10 in / $50 out, cached $1),
  **`gpt-6-sol`** / **`gpt-6.1-sol`** ($2 / $10), **`gpt-6-luna`** ($0.10 / $0.50); older
  GPT-5.5/5.6 still listed (https://developers.openai.com/api/docs/changelog; model
  pages; https://openai.com/index/introducing-gpt-6-sol-and-luna/).
- **Responses API** is recommended for new projects; Chat Completions remains supported;
  Assistants API shut down 2026-08-26. On Chat Completions, Luna's function calling works
  only with `reasoning_effort: none` — a reason to target Responses.
- Tools `{type:"function", name, description, parameters, strict:true}`; streaming SSE
  (`response.output_text.delta`, `response.function_call_arguments.delta`/`.done`,
  `response.completed`).
- Caching automatic (writes 1.25×, reads 0.1×; explicit mode from GPT-5.6, 30-min TTL).
- Data: API data not used for training unless the customer opts in; abuse logs 30 days;
  ZDR by approval (https://developers.openai.com/api/docs/guides/your-data).

### Google Gemini

- **`gemini-3.8-flash`** (2026-09): **$0.75 in / $3.75 out per 1M until 2026-12-31, then
  $1.50 / $7.50**; context-cache reads $0.075; a free tier exists
  (https://ai.google.dev/gemini-api/docs/pricing, read directly 2026-10-05).
  **`gemini-3.1-pro-preview`**: $2 / $12 (≤ 200K prompt), $4 / $18 above (same page). Also
  current: 3.7/3.6/3.5 Flash, 3.5 and 3.1 Flash-Lite (model list read 2026-10-05).
- The **Interactions API** is GA and default since June 2026; `generateContent` is legacy
  [search] (https://ai.google.dev/gemini-api/docs/migrate-to-interactions).
- Tools via `functionDeclarations` (`parameters` / `parametersJsonSchema`); Gemini 3
  requires returning **thought signatures** with function-call history, or the next call
  fails with 400 [search]. An OpenAI-compatible endpoint exists
  (`…/v1beta/openai/`) but thought signatures have broken it at times [search].
- **Privacy trap**: on the free tier, "human reviewers may read, annotate, and process API
  input and output" and the pricing page marks free-tier data "Used to improve our
  products: Yes"; paid tier "No" (https://ai.google.dev/gemini-api/terms; pricing page).
  Cantus must say this in the provider settings.

### Mistral [search]

- Mistral Medium 3.5 (open weights, modified MIT; $1.50 / $7.50), Mistral Large 3 (Apache
  2.0; $0.50 / $1.50), **Mistral Small 4** (Apache 2.0, 119B MoE / 6.5B active; $0.15 /
  $0.60), Ministral 3 (3B/8B/14B, Apache 2.0) (https://docs.mistral.ai/models/overview;
  https://mistral.ai/pricing/api/). OpenAI-style tools, `tool_choice` auto/any/none.
- EU hosting by default — the European-sovereignty option. Help-centre text suggests
  inputs may be used for training unless the user opts out (Admin ▸ Privacy) — **verify
  for paid API plans before recommending it** (https://help.mistral.ai/en/articles/455207).
- Apache-2.0 weights also run locally (Ollama), which makes Mistral a bridge between cloud
  and local.

### Local models — the private, offline path

| Runtime | Licence | API | Tool calling | Notes |
|---|---|---|---|---|
| **Ollama** | MIT | native `/api/chat` (NDJSON stream) and OpenAI-compatible `/v1/chat/completions` on `localhost:11434` | yes, also while streaming (since 2025-05); `/v1` lacks `tool_choice` | read https://docs.ollama.com/api/openai-compatibility 2026-10-05; a reported bug gives every streamed tool call `index: 0` on `/v1` — prefer native `/api/chat` [search] |
| **llama.cpp** `llama-server` | MIT | OpenAI-compatible | yes, with `--jinja` and a tool-aware template | Rust bindings `llama-cpp-2` 0.1.158 (MIT/Apache) to embed it — heavier; talk HTTP instead |
| **LM Studio** | proprietary freeware | OpenAI-compatible `localhost:1234/v1`, also an MCP host | yes | easiest for non-technical users |

Models that call tools well and are commercially usable [search]: **Qwen3 / Qwen3.6**
(Apache 2.0), **gpt-oss-20b/120b** (Apache 2.0; 20b runs in 16 GB), **Mistral Small**
(Apache 2.0), **Gemma 4** (Gemma terms). Rough memory at Q4: 8B ≈ 5 GB (8 GB GPU), 14B ≈
9 GB (12 GB GPU), 30B-A3B ≈ 19 GB (24 GB GPU) (https://ollama.com/library/qwen3). Expect
reliable single-tool edits from 8–14B models and weaker multi-step planning than cloud
frontier models; say so in the UI ("local models are private and free but less capable").

### OpenAI-compatible clouds

OpenRouter (one key, hundreds of models, tool support filterable), Groq (very low latency
on open models), Together, DeepSeek (`deepseek-v4-*`), xAI (Grok) all accept Chat
Completions with `tools` [search]. One "OpenAI-compatible" adapter with a configurable
base URL therefore covers them and every local runtime.

## 3. Cost — what a user would actually pay

A panel turn sends: the tool table (~30 tools ≈ 3–4K tokens), a frozen system prompt (~1K),
a compact project summary (1–5K, depending on size), the conversation, and gets back text
and tool calls (~300–1 000 tokens, plus thinking). An *agentic* request ("write a verse")
loops 4–10 times. With the stable prefix cached (prices from §2):

| Model | One chat turn (≈ 8K in, 6K of it cached; 600 out) | One agentic request (≈ 8 calls) | Notes |
|---|---|---|---|
| Claude Haiku 4.5 | ≈ $0.006 | ≈ $0.05 | cheapest Anthropic; 4 096-token cache minimum is met by the tool table |
| Claude Sonnet 5.5 | ≈ $0.011 | ≈ $0.09 | recommended default cloud model |
| Claude Opus 5.5 | ≈ $0.021 | ≈ $0.17 | for hard composition/arrangement requests |
| Gemini 3.8 Flash (promo price) | ≈ $0.004 | ≈ $0.03 | free tier: private data used to improve products |
| GPT-6 Luna | ≈ $0.001 | ≈ $0.01 | very cheap, smaller model |
| Local (Ollama / LM Studio) | $0 | $0 | the user's GPU and electricity |

*Arithmetic for Sonnet 5.5: 6K × $0.20/M + 2K × $2/M + 600 × $10/M ≈ $0.0012 + $0.004 +
$0.006. Thinking tokens add to output; effort `low` keeps them small. These are estimates
for planning, not quotes.* A heavy user (200 requests a month) lands in the $2–20/month
range on cloud models — but it is the **user's** key and the **user's** bill, so Cantus
must show cost per request and to date, and enforce a **local spend limit** per provider
(`idea.md` §4.2) computed from the `usage` each response returns.

## 4. Latency

- Anthropic labels Haiku 4.5 *fastest*, Sonnet 5.5 *fast*, Opus 5.5 *moderate* (models
  overview). Artificial Analysis, read via search 2026-10-05: GPT-6 Luna non-reasoning
  ≈ 0.7 s to first token; reasoning variants 4–13 s
  (https://artificialanalysis.ai/models/gpt-6-luna-non-reasoning,
  https://artificialanalysis.ai/models/gemini-3-8-flash) [search].
- **Reasoning effort, not the provider, dominates time to first token.** Use low effort
  for chat; stream everything; show the model's progress (tool calls appear as chips as
  they stream); let the user cancel.
- Never on the audio thread, never blocking the UI: the request runs on the AI `tokio`
  runtime; edits arrive as commands at frame boundaries (ADR 0005). Promise "assistance",
  never "real time" (`idea.md` §12).

## 5. Privacy and keys

**What leaves the machine** is decided by the user and shown before sending —
`Disclosure` already computes provider, local/cloud, bytes, whether project state is
included and the tool count. Extend it with audio seconds, estimated cost and host. Rules:

- Default provider: **none** (`NullProvider`). The AI area is opt-in.
- Project state sent as a summary (names, structure, notes on request) — never audio
  unless the user attaches it explicitly; never file paths beyond names.
- Per provider a one-line data statement in the settings page, drawn from §2 (Anthropic
  and OpenAI: not used for training by default, ~30-day retention; Gemini free tier:
  reviewed and used to improve products; local: nothing leaves).
- A **verifiable offline mode** (`idea.md` §4.3): with only local providers enabled, the
  AI layer refuses any non-loopback host and the settings page says so.

**API keys**: in the OS credential store, never in a project, never in plain-text
settings (`idea.md` §7). The `keyring` crate — 4.2.0 (2026-08-29), MIT OR Apache-2.0 — is
now modular: `keyring-core` plus per-platform stores (`windows-native-keyring-store` →
Windows Credential Manager, `apple-native-keyring-store` → Keychain,
`dbus-`/`zbus-secret-service-keyring-store` and `linux-keyutils-keyring-store` on Linux)
[search: crates.io and docs.rs]. Fallback: an environment variable
(`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, …) read at startup, never written. The key field
shows only the last four characters; a *Test* button makes a 1-token request.

## 6. A provider-agnostic design behind `AiProvider`

Today's trait (`cantus-today.md` §3) cannot carry a tool result back, stream, or report
usage. A sketch of what it should become (names indicative, for the ADR that will decide):

```rust
pub struct AiRequest {
    pub system: String,                 // frozen, cacheable
    pub messages: Vec<AiMessage>,       // role + Vec<Part>
    pub tools: Vec<ToolSpec>,           // + title, annotations, output_schema
    pub max_tokens: u32,
    pub effort: Effort,                 // Low | Medium | High — mapped per provider
    pub cache_hint: CacheHint,          // where the stable prefix ends
}
pub enum Part { Text(String), ToolUse { id, name, input }, ToolResult { id, content, is_error },
                Image { mime, bytes }, Audio { mime, bytes } }
pub enum StreamEvent { TextDelta(String), ToolUseStart { id, name }, ToolInputDelta { id, json },
                       ToolUseDone { id, input }, Usage(Usage), Stop(StopReason) }
pub struct Usage { input: u32, cached_read: u32, cache_write: u32, output: u32 }
pub enum StopReason { EndTurn, ToolUse, MaxTokens, Refusal { category: Option<String> }, Paused }

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn is_local(&self) -> bool;
    fn capabilities(&self) -> Capabilities;   // tools, vision, audio_in, context, caching, prices
    async fn stream(&self, req: AiRequest, cancel: CancelToken)
        -> Result<BoxStream<'static, Result<StreamEvent, AiError>>, AiError>;
    async fn complete(&self, req: AiRequest) -> Result<AiTurn, AiError> { /* collect stream */ }
}
```

**Adapters** (each a few hundred lines over `reqwest` + an SSE/NDJSON parser):

1. `anthropic` — native Messages API (tools, streaming, `cache_control`, effort, refusal).
2. `openai-compatible` — Chat Completions with a configurable base URL: covers Ollama,
   LM Studio, llama-server, OpenRouter, Groq, Together, DeepSeek, xAI, Mistral, and
   Gemini's compat endpoint. One adapter, most of the market, all of local.
3. `openai-responses` — OpenAI's recommended API (reasoning models' tool calling).
4. Later, if needed: `gemini` native (thought signatures, explicit caching) and `ollama`
   native `/api/chat` (reliable streamed tool calls).

**Build or borrow?** Candidate crates [search: crates.io 2026-10-05]: `genai` 0.6.5 /
0.7.0-rc (MIT OR Apache; 25+ providers incl. Anthropic, OpenAI Responses, Gemini, Ollama;
streaming and tools), `rig-core` 0.43 (MIT; agent framework), `async-openai` 0.42
(MIT; OpenAI only, base URL configurable), `llm` 1.3.8 (MIT). All pre-1.0 except `llm`.
**Recommendation: own the three adapters** (as `idea.md` §11–12 planned): the surface is
small, the wire formats are stable and documented, and the Cantus trait — not a 0.x crate
— stays the contract. Keep `genai` as a fallback if adapter upkeep becomes a burden.

**The loop** (in `cantus-ai`, UI-agnostic, testable with a scripted fake provider):
user turn → `stream` → tool calls collected → `Permissions::decide` per call → dry-run →
confirmation card (or auto) → execute on the UI thread with `Origin::Ai` → `ToolResult`
parts back → repeat until `EndTurn`, a step cap (e.g. 12), the spend cap, or Cancel.
The whole turn merges into **one undo step** labelled with the user's request.

**Per-task routing and fallback** (`idea.md` §4.2): settings map tasks (chat, write
notes, mix advice, tagging) to provider+model; on `RateLimited`/`Unavailable` try the next
provider in the user's list, telling the user. Model ids come from each provider's models
endpoint (Anthropic `GET /v1/models` returns context window, max output and capabilities)
rather than a hard-coded list, so retirements such as Haiku 4.5's do not break releases.

## 7. Context: what the model sees

- **Small and stable first** — tool table and system prompt (cached); then a compact
  project summary (`describe_project` grown: tracks, instruments, strips, inserts, clip
  windows, selection, playhead, key/tempo); details on demand through read tools.
- **Device knowledge** from the catalogue (`cantus://devices`): parameter keys, ranges,
  units and one-line meanings, plus the docs in `docs/instruments/*.md` and
  `research/fabfilter/*` as retrievable text, so the model can set *Culmen* sensibly
  without guessing.
- **Audio as numbers**: loudness, spectrum bands, peaks, key/tempo estimates — reliable
  across all providers. Raw audio input (Gemini and OpenAI accept audio; Claude accepts
  images, not audio, per the models overview) is optional later; a spectrogram image is a
  portable alternative.
- **Language**: answer in the UI language (EN/ES) unless the user writes otherwise.

## 8. Recommendation for the panel

1. Ship after the MCP server (same tools, same permission gate, same undo — the panel is
   "an MCP client that happens to live inside Cantus").
2. First adapters: **Anthropic** (Sonnet 5.5 default, Opus 5.5 for composition, Haiku for
   cheap chat while it lives) and **OpenAI-compatible** (which gives Ollama/LM Studio
   local models on day one, plus OpenRouter/Groq/DeepSeek/Mistral). OpenAI Responses and
   Gemini native next.
3. Default provider none; keys in the OS store; disclosure line before every send; cost
   meter and spend cap; local-only mode.
4. One locale area `locales/areas/ai.*` (EN/ES) for every string.
