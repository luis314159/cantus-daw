# Legal and ethical points the owner must decide

*Research, not legal advice. Researched 2026-10-05; sources as cited, "[search]" /
"[secondary]" as in the other files. Before a public release with generation features,
have a lawyer read §1–§4.*

## 1. GPL fit (ADR 0001)

| Question | Finding | Decision needed |
|---|---|---|
| Can GPL-3.0 Cantus call cloud APIs (Anthropic, OpenAI, Google, Stability, ElevenLabs)? | Yes. Talking HTTP to a service is not linking; no provider code enters the binary. Provider terms bind the **user** who holds the key, not Cantus. | Cantus never ships or proxies its own key (cost, abuse, and it would make the project a party to every provider's terms). **Confirm: bring-your-own-key only.** |
| Rust dependencies for AI | `rmcp` (Apache-2.0), `keyring` (MIT/Apache), `reqwest`, `tokio`, `ort`/`tract`/`candle` (MIT/Apache) are GPL-3.0-compatible. Apache-2.0 is compatible with GPL**v3** (not v2) — fine for `cantus-ai` (GPL-3.0-or-later). | Keep all AI deps inside GPL crates; the MIT/Apache core crates never depend on them (dependency direction = licence direction). |
| ONNX Runtime native library (via `ort`) | MIT (Microsoft) — compatible. | Ship it only with the AI feature, downloaded on demand if large. |
| Model **weights** | Weights are data, not GPL "Program" code; their own licences govern them. A GPL app may *download* weights the user chooses (an aggregate, not a derivative). Shipping weights inside the installer makes Cantus a distributor bound by their terms. | **Never bundle weights.** Download on demand with the licence shown and accepted (content manager). Allow-list only commercially usable licences by default (MIT, Apache-2.0, CC-BY, Stability Community with its notice). |
| Stability AI Community License | Free for commercial use under US$1M annual revenue; must display **"Powered by Stability AI"**, keep NOTICE, follow the AUP; outputs may not train foundation models [search] (https://stability.ai/license). The licensee is whoever uses the model — the user. | Show the notice in the generator UI and the model's about box; tell users with > $1M revenue they need Stability's enterprise licence. |
| Non-commercial weights (MusicGen, AudioLDM 2, RoFormer community weights, UMX-L) | CC-BY-NC(-SA): outputs may not be used commercially per most readings. | **Exclude** from the catalogue, or allow only as "advanced, non-commercial" with an explicit warning. Recommendation: exclude. |
| Trademarks | Naming providers ("Claude", "Gemini", "GPT") to say what a setting connects to is nominative use; logos need each brand's guidelines. | Names in text, no logos, unless permitted. |
| GPL and outputs | GPL covers Cantus's code, never the user's music, and nothing an AI makes for the user. | Say so in the docs (as every DAW does). |

## 2. Output rights — who owns what the AI makes?

- **United States.** The Copyright Office's Part 2 report (2025-01-29): prompts alone do
  not make the user the author; human selection, arrangement and modification of AI
  output can be protected (https://www.copyright.gov/ai/Copyright-and-Artificial-Intelligence-Part-2-Copyrightability-Report.pdf).
  *Thaler v. Perlmutter*: the Supreme Court denied certiorari on 2026-03-02, leaving the
  human-authorship rule in place [secondary]
  (https://www.mayerbrown.com/en/insights/publications/2026/03/supreme-court-denies-review-in-ai-authorship-case).
  So a vendor's "you own the output" gives contractual rights, not necessarily copyright.
- **Vendor terms** (by plan, see `generative-audio.md` §2): Stability assigns its rights
  in outputs to the user; ElevenLabs paid plans are commercial (music excludes film, TV,
  radio and games on self-serve plans); Lyria: no explicit ownership clause found; Suno:
  paid plans own, free tier does not; open models: the weights' licence governs.
- **What an LLM writes** (notes, chords, mix settings) is the same question at smaller
  scale: a model's bassline is probably not protectable on its own; the user's edits,
  selection and arrangement are.

**Decisions:** (a) Cantus claims **no rights** in anything made with it, AI or not
(write it in the docs and the licence FAQ). (b) Each generator in the UI shows a one-line
**output-terms summary** with a link to the vendor's terms and the date checked. (c)
Cantus does not promise users that AI outputs are copyrightable; the help page explains
the human-authorship rule in plain English and Spanish.

## 3. Disclosure and marking

- **EU AI Act, Article 50** (transparency obligations, applicable from **2026-08-02**):
  providers of AI systems that generate synthetic audio must mark outputs in a
  machine-readable, detectable way; a Code of Practice on marking and labelling was
  published 2026-06-10; systems already on the market got until 2026-12-02 under the
  Digital Omnibus [secondary] (https://www.cooley.com/news/insight/2026/2026-08-03-eu-ai-act-transparency-obligations-take-effect-2-august-2026;
  https://www.jonesday.com/de/insights/2026/06/european-commission-publishes-final-code-of-practice-on-marking-and-labelling-aigenerated-content).
  The Act's exemption for free and open-source AI (Art. 2(12)) **does not extend to
  Article 50 systems**. If Cantus runs a generative model locally, Cantus may be the
  "provider" of that AI system; with a cloud model the vendor marks its own output
  (Lyria uses SynthID). **Legal review needed.**
- **Distributors** [secondary]: Spotify (2025-09-25) — impersonation policy, spam filter,
  AI credits through **DDEX**; Deezer tags AI tracks and keeps them out of recommendations
  (AI > 50% of daily uploads by 2026-06); DistroKid added an AI-disclosure toggle
  (2026-05-14).
  (https://newsroom.spotify.com/2025-09-25/spotify-strengthens-ai-protections/;
  https://newsroom-deezer.com/2026/07/ai-music-exceeds-50-percent-daily-uploads-deezer/)

**Decisions:** (a) every AI-generated or AI-separated source carries **provenance**
(model, provider, prompt, date, licence) in the project; (b) clips made from it show a
small "AI" badge; (c) *Export* can write an AI flag and credits into metadata (BWF `bext`
/ iXML, ID3 `TXXX`, and a sidecar compatible with DDEX AI-credit fields), default **on**
when the mix contains AI-generated audio; (d) for local generators, add a
machine-readable mark (metadata at minimum; a watermark if the Code of Practice requires
it) — decide after legal review; (e) LLM-written MIDI and AI mix suggestions are **not**
"synthetic audio" and need no marking, but the history/audit already records them.

## 4. Training-data provenance

| Source | Claim | Status |
|---|---|---|
| Stable Audio 2/2.5/3 | licensed AudioSparx (+ Freesound CC for open models); UMG and WMG alliances | cleanest public claim |
| ElevenLabs Music | Merlin and Kobalt licences | licensed (press) |
| Google Lyria | "data YouTube and Google has a right to use" | indie-artist lawsuit pending |
| Suno | — | UMG and Sony suit ongoing; GEMA won in Munich (2026-07-31) |
| Udio | — | settled with UMG/WMG; no downloads |
| ACE-Step 1.5 | licensed, royalty-free and synthetic data (claim) | verify |
| Magenta RealTime | ~190K h of mostly instrumental stock music | stated |
| HT-Demucs | MUSDB + 800 internal songs | undisclosed |
| LLMs (all) | web-scale corpora; US Copyright Office Part 3 (pre-publication, 2025-05-09) says training fair use is fact-specific | open legal question across the industry |

**Decision:** a **provenance policy** for what Cantus lists by default. Recommendation:
default catalogue = models and services with an explicit licensed / permissive training
claim and commercial output terms (Stability, ElevenLabs, ACE-Step, Magenta RT; Lyria with
a note); exclude services in active infringement litigation without a licence (Suno, and
Udio until it offers exports); each entry shows its provenance line.

## 5. Privacy

- Bring-your-own-key; **default provider none**; per-request disclosure (`Disclosure`);
  verifiable local-only mode; no telemetry (idea.md §7).
- Provider data statements in settings: Anthropic and OpenAI APIs do not train on API
  data by default and keep it ~30 days; Gemini **free** tier may be human-reviewed and
  used to improve products; Mistral's default to verify; local = nothing leaves.
- Uploading audio to separation/mastering services sends the user's (or a client's)
  unreleased music to a third party — warn on first use, per service.
- Keys in the OS credential store, never in projects (a shared `.cantus` must never leak
  a key — add a test).

## 6. Voices and likeness

Voice cloning and "in the style of <named artist>" prompts raise right-of-publicity and
impersonation issues (e.g. Tennessee's ELVIS Act, 2024; Spotify's impersonation policy).
**Decisions:** no voice-cloning feature in the default build; if added, require the
user's attestation that they own or have consent for the voice; pass prompts through
unchanged but do not offer artist-name presets.

## 7. Ethics and product principles

- **AI suggests, the human decides** — no AI action without preview and undo; no feature
  that needs AI to finish a song (idea.md §4.6).
- **Credit the musicians**: native, deterministic tools first (analysis, separation,
  transcription, mastering assist) — they serve the musician's own material; generative
  features second, clearly labelled.
- **Honesty in the UI**: say when a model is local or cloud, what it costs, what it was
  trained on (as far as known), and what the user may do with the output.
- **One switch** turns the entire AI area off and hides it.
- **Agents are guests**: an external MCP agent gets the permissions the user grants, is
  visible while connected, and its actions are labelled in the history.

## 8. Checklist for the owner

1. Bring-your-own-key only; Cantus ships no key. (Recommended: yes.)
2. No model weights in the installer; download on demand with licence acceptance. (Yes.)
3. Default catalogue limited to commercially usable licences and licensed-data claims;
   non-commercial weights excluded. (Yes.)
4. Suno and Udio not integrated. (Yes, until official APIs with clear terms exist.)
5. Provenance per AI source, an "AI" badge, and AI flags on export (default on when
   present). (Yes.)
6. EU Art. 50 marking for local generators — obtain legal review before shipping local
   generation in the EU.
7. Cantus claims no rights in user output; docs explain human authorship (EN/ES).
8. No voice cloning by default.
9. External agents (MCP) off by default, localhost only, confirmation inside Cantus.
