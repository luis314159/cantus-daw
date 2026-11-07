# Cantus column (from the repository at c95ee8a9, 2026-11-04)

| ID | Cantus | Evidence |
|---|---|---|
| T1 | have — own stretcher (phase vocoder, PSOLA, Rhythm, Re-pitch) | ADR 0061, STATUS r29 |
| T2 | have — Complex/Melodic modes transpose with formants kept | ADR 0061 |
| T3 | have — tempo points with closed-form ramps | ADR 0060 |
| T4 | have — metre changes | ADR 0060 |
| T5 | have — ruler markers: jump, loop, select, export between | ADR 0060 |
| T6 | have — Quantise audio | ADR 0061 |
| T7 | have — tempo/transients/downbeat at import | ADR 0061 |
| A1 | have — Read/Write/Touch/Latch | ADR 0068 |
| A2 | have — automation clips and lanes, curves | ADR 0027 |
| A3 | have — bend, pressure, CC 0–119, velocity, release lanes | ADR 0068 |
| A4 | have — LFO, envelope, follower, steps, random (MIDI/math modulators open) | ADR 0057 |
| A5 | have — hosted parameters automatable and modulatable | ADR 0057 |
| P1 | have | ADR 0019 |
| P2 | have | ADR 0019 |
| P3 | missing | BACKLOG §2 |
| P4 | have — out-of-process helpers, crash cards, Reload | ADR 0063 |
| P5 | have — per-plug-in CPU share | ADR 0063 |
| P6 | have | ADR 0019 |
| P7 | have — whole graph, re-planned without a dropout | ADR 0018 |
| P8 | partial: out-of-process slice open; no commercial plug-in verified | ADR 0097 |
| P9 | have — 23 effects as CLAP/VST3, bit-identical to the engine | ADR 0020, `cantus-plugins/tests/clap.rs` |
| E1 | have | ADR 0070 |
| E2 | have | ADR 0070 |
| E3 | have | ADR 0096 |
| E4 | have | ADR 0096 |
| E5 | have — denoise by profile, declick | ADR 0096 |
| E6 | have — monophonic only | ADR 0054 |
| E7 | have — Tonus, 2.67 ms | ADR 0053 |
| E8 | have | ADR 0025, 0058 |
| E9 | have | ADR 0064 |
| E10 | have | ADR 0064 |
| E11 | missing | BACKLOG §3 Editing |
| E12 | partial: linked clips; no clip groups | ADR 0037 |
| M1 | partial: scale highlight and snap; no ghost notes | ADR 0069 |
| M2 | have | ADR 0069 |
| M3 | have | ADR 0069 |
| M4 | partial: channel rack; no mini piano roll per melodic row | ADR 0013 |
| M5 | have | ADR 0080 |
| M6 | partial: model and translation; no UMP transport | ADR 0080 |
| M7 | have | ADR 0062 |
| M8 | have — engraved score, MusicXML, PDF | ADR 0075 |
| M9 | missing | BACKLOG §0.2 |
| X1 | have | ADR 0015, 0041 |
| X2 | have | ADR 0093 |
| X3 | have — snapshots, A/B, cues | ADR 0093 |
| X4 | have | ADR 0052, 0056 |
| X5 | partial: reference only inside the mastering assistant | ADR 0084 |
| X6 | have — track templates carry the strip | ADR 0072 |
| X7 | missing | BACKLOG §0.2 |
| R1 | have | ADR 0025 |
| R2 | have | ADR 0064 |
| R3 | have — recording mode, 64 tracks 224–590 % → 2–4 % | ADR 0092 |
| R4 | have | ADR 0094 |
| R5 | have — virtual units; real hardware untested | ADR 0095 |
| L1 | have | ADR 0076–0077 |
| L2 | have | ADR 0078 |
| L3 | have | ADR 0079 |
| L4 | have — Launchpad, APC | ADR 0077 |
| W1 | have | ADR 0072 |
| W2 | have | ADR 0072 |
| W3 | have | ADR 0072 |
| W4 | missing | STATUS plan |
| W5 | partial: sample previews; no preset audition (round 37 library) | ADR 0030, 0029 |
| W6 | have | ADR 0006 |
| W7 | missing — MCP tools only | BACKLOG §0.2 |
| W8 | missing | BACKLOG §0.4 item 8 |
| W9 | missing | BACKLOG §0.2 |
| X8 | partial: no AAC | ADR 0033 |
| X9 | have | ADR 0033 |
| X10 | have | ADR 0021 |
| X11 | missing | ADR 0021 |
| I1 | 10 (Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Chorda, Firmus, Pulsus) | `inst/mod.rs` |
| I2 | have — SFZ, Decent Sampler, SF2 | ADR 0023 |
| I3 | missing (round 37 piano: Sectio) | BACKLOG §3e |
| I4 | partial (round 37 drums: Pulsus III) | BACKLOG §3e |
| I5 | have — Chorda; owner: does not sound good yet (round 37) | ADR 0059 |
| I6 | have | ADR 0023 |
| I7 | have | ADR 0023 |
| I8 | partial: Grana granular; Exemplar spectral open | BACKLOG §3e |
| I9 | partial: Chorda, Gravis, Lamina; resonators open | BACKLOG §3e |
| I10 | missing | BACKLOG §7a |
| F1 | 23 processors (+ legacy Reverb, Speculum analyser) | `effect.rs` |
| F2 | have | ADR 0017 |
| F3 | have | ADR 0026 |
| F4 | partial: algorithmic only | BACKLOG §3b |
| F5 | have | ADR 0026, 0084 |
| C1 | partial: 13 packs (11 CC0), per-unit presets | `content.json` |
| C2 | have | ADR 0030, 0042 |
| AI1 | have — HT-Demucs | ADR 0085 |
| AI2 | have | ADR 0086 |
| AI3 | have | ADR 0084 |
| AI4 | missing | BACKLOG §0.4 item 9 |
| AI5 | have — MCP server, 63 tools | ADR 0081 |
| O1 | have — GPL-3.0 app, MPL-2.0 engine | ADR 0047 |
| O2 | free | ADR 0047 |
| O3 | partial: Windows, Linux; macOS unsigned preview | ADR 0066–0067 |
| O4 | have — offline = real time to the bit, hashes equal on 3 systems | `null_tests.rs`, `null-hashes.yml` |

Unsure: P8 (docs disagree — followed ADR 0097); R5/L2/L4 untested on real hardware;
"better" rows need a measurement against another DAW before they are claimed.
