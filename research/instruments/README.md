# Instruments research (September 2026)

The user's ask (2026-09-24): research the best virtual instruments on the market (synths,
basses, drums, samplers, keys), study them deeply (including how they look), and build
Cantus's own to equal and surpass them. Cantus must also speak the plug-in standards: host
third-party instruments and effects, and make its own loadable in other DAWs.

This folder has the first-pass shortlist (below) and one report per category, written by
research agents in the style of `research/fabfilter/`. Screenshots are saved in
`screenshots/`, which is **git-ignored**: they are the manufacturers' images, kept locally
for reference and never published with the source. Nothing here is copied into the
product; the reports are paraphrase and analysis with their sources.

## First pass: the benchmarks, by category

Chosen from 2026 round-ups (MusicRadar's soft-synth showdown, LANDR, Output, Plugin Grab,
Bedroom Producers Blog, KVR) and from what the major DAWs ship as stock instruments.

| Category | Benchmarks (what the market measures itself against) | Open / free references | Cantus today |
|---|---|---|---|
| Wavetable / hybrid flagship | **Xfer Serum 2** (sample, multisample, granular, spectral engines), **Arturia Pigments** (6/7), **Kilohearts Phase Plant**, NI Massive X, u-he Hive 2 | **Vital** (GPL), Surge XT (GPL) | Firmus: 2 PolyBLEP oscillators, SVF, 2 ADSR, 16 voices |
| Analog modelling / virtual analog | **u-he Diva**, u-he Repro, Arturia V Collection (Mini V, Jup-8 V, Prophet V), Lennar Digital Sylenth1, Reveal Sound Spire | OB-Xd, TAL-U-NO-LX, Surge XT | — |
| FM, additive, spectral, granular | NI FM8, Image-Line **Sytrus** and **Harmor**, Ableton Operator, **Spectrasonics Omnisphere 3** (late 2025), u-he Zebra 3, NI Absynth 6 | Dexed (DX7), Surge XT | — |
| Sampler / multisample | **NI Kontakt 8**, **UVI Falcon 3**, Steinberg HALion 7, Ableton Sampler, Logic Sampler | **Decent Sampler**, TX16Wx, sforzando (SFZ) | pads can play a WAV |
| Drums | **NI Battery 4**, **XLN XO**, **Toontrack Superior Drummer 3**, XLN Addictive Drums 2, FL FPC, Sonic Charge Microtonic, Roland Cloud TR-808/909 | Ableton-style drum racks | Pulsus: synthesized kick/snare/hat + sample pads, choke groups |
| Bass and 808 | **Spectrasonics Trilian**, **IK MODO BASS 2** (physical model), Future Audio Workshop **SubLab XL**, FL Transistor Bass | — | — |
| Keys and piano | **Modartt Pianoteq 9** (physical model), **Spectrasonics Keyscape**, AAS Lounge Lizard (modelled EP), Arturia Piano V / Stage-73 V | Salamander Grand (CC-BY) | — |
| Plug-in standards | **VST3** (SDK under MIT since 3.8, Oct 2025), **CLAP** (≈15 hosts, ≈400 plug-ins; FL 2024+, Bitwig, Reaper), AU/AUv3 on macOS | nih-plug / nice-plug (Rust export), clack (Rust CLAP host) | ADR 0001 made CLAP primary; nothing built |

Deliberately **not** in this round: orchestral libraries (Spitfire BBC SO, EastWest) and
recorded acoustic instruments at library scale — they are recordings, not engines, and a
product cannot ship them without recording its own. A physical-modelling path (SWAM-style)
is noted in the keys and bass reports instead.

## The reports

Nine agents, about 10 400 lines, 140 official screenshots (local only).

| Report | Scope | The headline | Builds |
|---|---|---|---|
| [wavetable.md](wavetable.md) | Serum 2 (2.1.5), Pigments 7, Phase Plant, Massive X, Hive 2, Vital, Surge XT | The ecosystem shares one table format (2048-sample frames, Serum's `clm ` WAV); mipmaps + 2× oversampling is alias-free and flat; none of them is host-native | **Unda** |
| [analog.md](analog.md) | Diva, Repro, Arturia V Collection, Sylenth1, Spire, OB-Xd, TAL-U-NO-LX | "Alive" is six behaviours, not a component count; nobody seeds drift, so no render repeats; replica panels at 1.3–1.8:1 contrast | **Calor** and `cantus_dsp::va` |
| [fm-additive-granular.md](fm-additive-granular.md) | FM8, Sytrus, Harmor, Operator, Dexed, Omnisphere 3, Zebra 3, Absynth 6 | One dedicated FM instrument; additive and granular become engines of the flagship; nobody handles FM aliasing openly | **Modulus** |
| [sampler.md](sampler.md) | Kontakt 8, Falcon 3, HALion 7, Decent Sampler, TX16Wx, SFZ/SF2, open libraries | Kontakt leads on its catalogue, not its engine; everyone interpolates badly when pitching; SFZ + Decent Sampler + SF2 import, SFZ export | **Exemplar** |
| [drums.md](drums.md) | Battery 4, XO, Superior Drummer 3, Addictive Drums 2, FPC, Microtonic, Kick 3, 808/909 models | Nobody owns more than one of pads, recorded kits, drum synths; Pulsus's noise seed resets on seek and its kick is 52 cents sharp | **Pulsus II** |
| [bass.md](bass.md) | Trilian, MODO BASS 2, SubLab XL, 808 Studio, Transistor Bass | No bass knows the kick or the key; the modelled bass is cheap and well documented; drive can keep the fundamental exactly | **Gravis** |
| [keys.md](keys.md) | Pianoteq 9, Keyscape, Lounge Lizard, Arturia keys, organs | Modelling is the only route open to us; the organ is the cheapest big win; Modartt's patent expires 2027-10-20 | **Organum**, **Lamina** |
| [standards.md](standards.md) | VST3 (MIT since 3.8), CLAP 1.2, AU, hosting and exporting from Rust | clack + `vst3` crates (MIT/Apache); CLAP-first export; ADR 0001's GPL reason is gone | handed to the effects session (ADR 0019/0020) |
| [family.md](family.md) | Stock instruments of Live 12, Logic 12, FL 2026, Bitwig 6.1, Cubase 15, Studio Pro 8, Reaper; the shared instrument UI | Clearing the stock bar takes ~8 engines; knob boundaries fail 3:1 almost everywhere — and so did ours | the frame (ADR 0023) |

**Decisions taken from them** are in ADR 0023 (the instrument contract, the shared
modulation model and frame, the house rules). **Open, for the owner:** bundled content —
ADR 0001 allows only original or CC0 samples, idea.md §8 says "permissive/CC0"; a CC-BY
set such as Salamander Grand needs that ruling (sampler.md, keys.md).
