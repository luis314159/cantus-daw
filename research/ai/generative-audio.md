# Generative audio, separation, transcription and mastering

*Researched 2026-10-05. The proxy refused most vendor pages, so many facts come from
search extracts of the cited official pages ("[search]") or from secondary sources
("[secondary]"). Verified directly on 2026-10-05: Gemini API Lyria prices, ACE-Step 1.5
`LICENSE` (MIT), `facebookresearch/demucs` (archived, MIT), Magenta RealTime 2 page.
Prices and terms in this field change monthly — re-check before building.*

## 1. How any of this lands in Cantus

Every generator is a **job**, never a stream into the audio thread:

```
prompt / selection ──► job (AI runtime: cloud call or local model on a worker thread)
                         │ progress, cancel, cost shown
                         ▼
                 a WAV in <project>/Audio/Generated/  (+ provenance sidecar)
                         │
       ┌─────────────────┼──────────────────┬──────────────────────┐
       ▼                 ▼                  ▼                      ▼
 audio-pool clip at   sample on a pad    new tracks (stems,      MIDI clip on a
 the playhead/        (Pulsus / Pulsus   one per stem, grouped   track (audio-to-
 selection            II / Exemplar)     on a bus)               MIDI)
```

- **Commands, as always.** Import uses the round-19 path: `InsertAudioSource` +
  `InsertClip`/`SetClipAudio`, or `SetPadSample`; stems = `InsertTrack` × n + clips + a
  bus, all in one `Batch` → **one undo step**. Undo removes the clips; the file stays in
  the pool folder until *Collect and save* cleans unused files (as for any import).
- **Tempo and key.** Pass the project's BPM/key to models that accept them (Stable Audio,
  Lyria, ElevenLabs music, ACE-Step); otherwise time-stretch later (Cantus has no
  stretcher yet — a dependency to note).
- **Provenance.** Each generated source records model, version, provider, prompt, seed,
  date, licence id and an "AI-generated" flag (a new optional field on the audio source —
  a schema bump with a migration). This feeds the disclosure duties in `legal.md` (EU AI
  Act marking, DDEX/DistroKid AI credits) and lets *Export* write the flag into BWF/ID3
  metadata.
- **UI.** A *Generate* tab in the browser/picker area: prompt, duration, BPM-sync toggle,
  provider (cloud/local), cost estimate, licence line ("Output: you may use commercially
  under …"), results as auditionable tiles you drag to the playlist or a pad. Right-click
  an audio clip → *Separate stems…*, *Convert to MIDI…*; master chain → *Match a
  reference…*.

## 2. Cloud generators

| Service | What | Access and price | Output terms | Training data | Fit |
|---|---|---|---|---|---|
| **Stability — Stable Audio 2.5** (2025-09-10) | text-to-audio, audio-to-audio, inpainting; up to 3 min; < 2 s on GPU | REST API; **20 credits = $0.20 per generation** at $0.01/credit [search] (https://platform.stability.ai/pricing) | Stability assigns its rights in outputs to the user "to the extent permitted by law" [search] (https://stability.ai/terms-of-service) | "trained exclusively on licensed data" (AudioSparx) (https://stability.ai/news/stability-ai-introduces-stable-audio-25-the-first-audio-model-built-for-enterprise-sound-production-at-scale) | loops, textures, one-shots, SFX; the cleanest provenance among cloud music models |
| **Stable Audio 3.0** (2026-05-20) | Small/Small-SFX (459M), Medium (1.4B, ~6 min), Large (2.7B, closed) | Large via API/partners; price **unverified** | as above | AudioSparx licensed + Freesound CC [secondary] (https://the-decoder.com/stability-ai-launches-stable-audio-3-0-with-up-to-six-minute-tracks-and-open-weights/) | Stability also ships its own DAW plug-in (beta, 2026-08) [secondary] |
| **ElevenLabs — Sound Effects** | 0.1–30 s, loop option; WAV 48 kHz (non-loop), MP3 | REST; price conflicting (credits per second vs ~$0.12/min) — **verify** [search] (https://elevenlabs.io/docs/overview/capabilities/sound-effects) | Paid plans: commercial licence; free plan: non-commercial and must credit (https://elevenlabs.io/docs/help-center/legal/can-i-publish-the-content-i-generate-on-the-platform) [search] | not disclosed for SFX | one-shots on pads, foley, risers |
| **ElevenLabs — Eleven Music** (2025-08-05) | songs with or without vocals, 3 s to minutes; composition plans; stems | REST, paid plans; ~$0.15–0.30/min (conflicting) [search] | self-serve plans: commercial use **except film, TV, radio and games**; Enterprise covers all (https://elevenlabs.io/eleven-music-model-specific-terms) [search] | licensing deals with **Merlin and Kobalt** [secondary] | song sketches, beds; check the media exclusions |
| **ElevenLabs — TTS / voice** | speech, voice design, cloning | ~$0.05–0.10 per 1K characters [secondary] | paid plans commercial | — | vocal chops, spoken intros; voice cloning raises consent issues (`legal.md`) |
| **Google — Lyria 3.5 / Lyria 3** (Gemini API) | full songs with vocals and timed lyrics (3.5, Pro); 30 s clips (Clip); 44.1 kHz stereo, MP3 / WAV | **Lyria 3.5 $0.08 per song; Lyria 3 Clip Preview $0.04; Lyria 3 Pro Preview $0.08**; paid tier only (https://ai.google.dev/gemini-api/docs/pricing, read 2026-10-05) | explicit output-ownership clause **not found**; Google's Gen AI prohibited-use policy applies | "data YouTube and Google has a right to use"; indie artists are suing [secondary] | cheap, good; every output carries **SynthID** watermark (https://deepmind.google/models/synthid/) |
| **Lyria 2** (Vertex AI) | instrumental, 30 s, 48 kHz WAV | **$0.06 per 30 s** [search] (https://cloud.google.com/vertex-ai/generative-ai/pricing) | Google Cloud terms | as above | instrumental beds |
| **Lyria RealTime** | streaming 48 kHz PCM, BPM 60–200, density/brightness, mute drums/bass; ~2 s control latency | experimental, allow-listed [search] (https://ai.google.dev/gemini-api/docs/realtime-music-generation) | — | — | a "jam" instrument; experimental only |
| **Suno** | songs | **no public API**; a curated partner programme announced 2026-07 [secondary] | paid plans own outputs; free tier non-commercial [secondary] | sued by UMG and Sony (ongoing); GEMA won in Munich 2026-07-31 [secondary] | **Do not integrate.** Unofficial wrappers drive the web UI against Suno's terms. |
| **Udio** | songs | **no API**; downloads disabled after the UMG settlement (2025-10-29); a licensed walled-garden platform [secondary] | — | settled with UMG and WMG | **Do not integrate.** |
| Google MusicFX / MusicFX DJ | consumer | shut down 2026-07-31, replaced by Flow Music [secondary] | — | — | not an API |

## 3. Local (open-weights) generators

Run on the user's machine through a downloaded model; nothing leaves; cost = GPU time.

| Model | Licence: code / **weights** | Can the user sell the output? | Quality, speed, hardware | Fit |
|---|---|---|---|---|
| **ACE-Step 1.5** (2026-01; XL 4B 2026-04) | **MIT** (LICENSE read 2026-10-05; v1 was Apache-2.0) | **Yes** (model card says commercially usable) | full songs; < 10 s on an RTX 3090, < 4 GB VRAM claimed [secondary] (https://github.com/ace-step/ACE-Step-1.5) | **best licence/quality fit for local song sketches**; training data claimed licensed/royalty-free/synthetic — verify |
| **Magenta RealTime 2** (2026-06) / v1 (2025-06) | code Apache-2.0 / weights **CC-BY 4.0** [search] | **Yes, with attribution** | 2.4B base + 230M small; ~200 ms control latency; MIDI/audio/text steering; C++ MLX engine on Apple Silicon (https://magenta.tensorflow.org/magenta-realtime-2, read 2026-10-05) | a **live AI instrument** that follows MIDI — closest to how a DAW wants to use AI; Windows/CUDA path to check |
| **Stable Audio Open 1.0 / Small / SA3 Small, Medium** | Stability AI **Community License** | **Yes while the user's (and Cantus's?) annual revenue < US$1M**; must show "Powered by Stability AI"; outputs may not train foundation models [search] (https://stability.ai/license) | Open 1.0: ≤ 47 s, 44.1 kHz stereo; Small: 11 s in < 8 s on a phone CPU; ONNX exports exist | one-shots, loops, textures; licence obligations must be displayed; T5Gemma encoder may add Gemma terms [unverified] |
| **YuE** | Apache-2.0 | Yes (credit requested) | slow: ~6 min per 30 s on an RTX 4090 | not practical in a DAW |
| **DiffRhythm** | Apache-2.0 | Yes | full songs up to 4:45 | provenance unclear |
| **Meta AudioCraft** — MusicGen, AudioGen, MAGNeT | MIT / **CC-BY-NC 4.0** | **No** (non-commercial weights) | MusicGen Small ~6 s per 10 s on an RTX 3060 Ti | **exclude by default**; at most an opt-in "non-commercial" model with a warning |
| AudioLDM 2 | **CC-BY-NC-SA 4.0** | No | — | exclude |
| Tencent SongGeneration / LeVo | custom licence, research-only reading | treat as **No** | — | exclude |
| Riffusion v1 | CreativeML OpenRAIL-M | Yes with use restrictions | dated | skip |

**Rust runtimes for local models** [search]: **`ort`** (ONNX Runtime bindings; CUDA,
DirectML, CoreML, ROCm execution providers; MIT/Apache — verify), **`tract`** (Sonos;
pure Rust; ONNX/NNEF; MIT/Apache; best for small models with no native dependency),
**`candle`** (Hugging Face; MIT/Apache), **Burn** (demucs-rs proves HT-Demucs runs natively
in Rust). Big generators (ACE-Step, Stable Audio Medium) are realistically run either
through ONNX exports in `ort` or as an **optional helper process** (a separate downloadable
"Cantus AI runtime", possibly Python-based, speaking a local protocol — or MCP) so the main
binary stays small and GPL-clean. Decide per model in an ADR.

**Model files are content.** Never in the repo or the installer; download on demand
through the content manager proposed in `research/content/` (manifest, checksums,
versions, licence text shown and accepted, size and VRAM shown before download).

## 4. Stem separation

| Option | Licence | Quality (MUSDB18-HQ SDR) | How it would run | Notes |
|---|---|---|---|---|
| **HT-Demucs** (v4) | **MIT** (code and weights); `facebookresearch/demucs` **archived** (verified via GitHub API 2026-10-05), a fork takes fixes only | 9.0 dB (9.2 fine-tuned) (https://arxiv.org/abs/2211.08553) | **ONNX export** (~316 MB, 166 MB fp16) through `ort` with STFT/iSTFT in host code; or **demucs-rs** (native Rust on Burn, already a CLAP/VST3 on macOS) [search] | **The local default.** 4 stems (drums, bass, other, vocals) or 6 (+ guitar, piano). Minutes per song on CPU, seconds on GPU. Trained with 800 extra internal songs of undisclosed provenance — note it. |
| BS-RoFormer / Mel-RoFormer | papers open; **community weights mostly CC-BY-NC-SA** (check each) | ~9.8 dB without extra data, ~12 with | ONNX re-exports exist | best quality, but weights' licences block a default; optional download with the licence shown |
| UVR / MDX-Net | MIT (GUI and models; credit UVR) | between Spleeter and Demucs | ONNX | a lighter vocal/instrumental split |
| Spleeter | MIT | older, lower | TF | superseded |
| Open-Unmix | code MIT; **UMX-HQ weights MIT**, UMX-L CC-BY-NC-SA | ~6 dB | small | a tiny fallback |
| Cloud: AudioShake, LALAL.AI, Music.ai | commercial | top tier | REST, per-minute credits (LALAL ≈ $50 per 750 min [secondary]) | only as a BYO-key option; uploads the user's audio |

Lands as: right-click an audio clip → *Separate stems…* → n new tracks under a bus, each
clip aligned to the source sample, the original muted (not deleted) — one undo step. A
null check is free: the stems' sum minus the original shows the separation residual.

## 5. Audio-to-MIDI

| Option | Licence | What | Run from Rust |
|---|---|---|---|
| **Spotify Basic Pitch** | **Apache-2.0** | polyphonic, instrument-agnostic notes + pitch bends; ~17K parameters | ships an **ONNX** model (`nmp.onnx`) → `tract` or `ort`; the note post-processing (Python) must be ported [search] (https://github.com/spotify/basic-pitch) |
| ByteDance piano transcription | Apache-2.0 | high-resolution piano, pedals | ONNX export possible |
| MT3 (Magenta) / **YourMT3+** | Apache-2.0 / **GPL-3.0** | multi-instrument transcription | heavy (T5/JAX/PyTorch) — helper process; YourMT3+ is GPL-compatible |
| Omnizart | MIT | multi-task | Python |
| CREPE / torchcrepe | MIT | monophonic f0 | small; vocal or bass to MIDI |

Lands as: *Convert to MIDI…* on an audio clip → a new instrument track (Unda or Exemplar
by default) with a MIDI clip aligned to the audio; drums → Pulsus with GM mapping (the
round-17 MIDI code already maps channel 10). Basic Pitch is the obvious first: tiny,
permissive, runnable in pure Rust, and genuinely useful (hum a melody, get notes).

## 6. "AI mastering"

| Option | Access | Licence | Fit |
|---|---|---|---|
| **Matchering 2.0** | Python library | **GPL-3.0** | Matches a target to a reference (RMS, frequency response, peak, stereo width) and limits. GPL-compatible, so the algorithm can be **ported to Rust inside Cantus legally**; deterministic, offline, testable with null tests [search] (https://pypi.org/project/matchering/) |
| LANDR | partner API (pricing by agreement); a $299 plug-in | proprietary | BYO account; uploads audio |
| iZotope Ozone 12 (Master Assistant) | plug-in, no API | proprietary | users host it as VST3 in Cantus already |
| eMastered, BandLab | web services, no public API | proprietary | none |

**The native route is better than any API.** Cantus already has mastering devices —
Fervor, Latitudo, Culmen on the master chain (ADR 0026) — and loudness meters. An
*assistant* that (a) analyses the mix (LUFS, true peak, LRA, spectral balance against a
genre target or a user reference) and (b) **proposes parameter settings for those
devices** as ordinary `SetEffectParam` commands, previewable and undoable, is "AI
mastering" with no cloud, no black box and no output-rights question. A ported Matchering
reference-match becomes one of its strategies; an LLM can explain the result in words.

## 7. Recommendation for generative audio, in order

1. **Local and deterministic first** (no output-rights questions, no cost, offline):
   *Match a reference* / mastering assistant over Cantus's own devices; **Basic Pitch**
   audio-to-MIDI; **HT-Demucs** stem separation (ONNX via `ort`, or demucs-rs). All
   permissive or GPL-compatible, all testable.
2. **Cloud generation with the user's own key**, outputs into the pool with provenance:
   **Stable Audio 2.5/3** (licensed training data, clear output assignment, $0.20 per
   generation), **ElevenLabs Sound Effects** (one-shots/foley), **Google Lyria 3.5**
   ($0.08 per song, SynthID-marked). ElevenLabs Music only with its media exclusions shown.
3. **Local generation as downloadable models**: **ACE-Step 1.5** (MIT) for song sketches,
   **Stable Audio Open Small/SA3** (Community License; attribution and revenue cap shown)
   for one-shots/loops, **Magenta RealTime 2** (CC-BY) as a live AI instrument once a
   Windows path is confirmed.
4. **Never by default**: MusicGen/AudioGen/MAGNeT, AudioLDM 2, RoFormer community weights,
   SongGeneration (non-commercial weights); Suno and Udio (no API, litigation, terms).
5. **Disclosure built in** from the first generator: provenance per source, an
   "AI-generated" export flag, and machine-readable marking where Cantus itself is the
   generator (`legal.md` §3).
