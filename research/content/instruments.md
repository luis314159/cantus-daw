# Free instruments Cantus could preload, bundle, or link to

Research note, round 20. Accessed 2026-10-05. Research only, no code.

Scope: which free instruments can fill Cantus's gaps (acoustic piano, orchestral, guitar, acoustic drums, world, vocal) and which free plug-ins can prove that CLAP/VST3 hosting works. "Verified" means checked this session on the official site, the upstream repository, or the GitHub API. Anything marked **not verified** was not checked; confirm it before shipping.

## 0. Summary

1. **The gaps are best filled with sample content, not plug-ins.** Cantus already ships Exemplar, which imports SFZ, Decent Sampler and SF2/SF3. There are enough CC0 and CC-BY sample libraries to cover piano, orchestra, guitar, bass, acoustic drums and several world instruments, and they load in Exemplar with no third-party code (section 3). This is the only option that is completely free of rights problems and works offline.
2. **Open-source plug-ins to bundle (GPL-3/MIT, with Windows CLAP or VST3 binaries):** Surge XT, Dexed, OB-Xf, Odin 2, Firefly Synth 2, Six Sines, JC303, RipplerX. All of them are synths or percussion models, which overlap with Cantus's own instruments. They are worth bundling for **compatibility proof**, not to fill a gap. Bundling a GPL binary means shipping (or hosting next to it) the exact corresponding source.
3. **Freeware is link-only.** No freeware EULA checked this session gives a third party the right to redistribute it. Most of these products also need a vendor account or app: Vital (account), BBC SO Discover (sign-in), Spitfire LABS (now Splice INSTRUMENT, Splice account), Komplete Start (NI account), Decent Sampler (email). Cantus can deep-link to the vendor's product page from an in-app catalogue.
4. **Trademarks are a separate question from copyright.** The GPL gives no trademark licence. Vital's README forbids using "Vital" to name a distribution of binaries, which is why DISTRHO's fork is called **Vitalium**. VCV branding was removed from Cardinal for the same reason. OB-Xd and OB-Xf carry an "inspired by Oberheim, not affiliated" disclaimer. "VST" may be used only under Steinberg's usage guidelines.
5. **Two facts changed since older write-ups.** First, **sfizz is archived:** sfizz-ui was archived in 2025 and the library repository on 2026-06-21, and the last release is 1.2.3 (2024-01). Second, the **VST 3 SDK has been MIT since 3.8 (2025-10-29)**, so hosting VST3 raises no licence questions for Cantus. Also, JUCE 8 is AGPL-3, so newer JUCE-based OSS binaries (plugdata, Shortcircuit XT) ship under AGPL-3.

## 1. Open-source instruments (CLAP / VST3)

Versions, dates and sizes come from the GitHub releases API on 2026-10-05. Licences come from the repository LICENSE file or README.

| Name | Developer | Licence (verified) | Formats | Platforms | Latest version + date | Win size | Quality notes | May Cantus bundle? | Notes |
|---|---|---|---|---|---|---|---|---|---|
| **Surge XT** | Surge Synth Team | GPL-3.0 | CLAP, VST3, AU, LV2, standalone | Win x64/x86 (arm64 nightly), macOS universal, Linux x64/arm64 | 1.3.4, 2024-08-11 (nightlies to 2026-10-01) | plugins-only zip 49 MB; setup 183 MB; portable with content 302 MB | Flagship OSS hybrid synth, 2,800+ patches, very well maintained | **Yes, with source** | The plugins-only zip probably lacks the factory data (patches, wavetables); bundle the portable content too (**not verified** which zip holds what). Source tarball `surge-src-1.3.4.tar.gz` (319 MB) is published. |
| **OB-Xf** | Surge Synth Team (continuation of OB-Xd ≤2.11) | GPL-3.0 | CLAP, VST3, AU, LV2, standalone (the README does not list desktop formats; **check the zip**) | Win x64/arm64/arm64ec, macOS, Linux | v1.0.3, 2026-03-27; nightly 2026-09-30 | zip 9.2 MB + assets 24 MB | Virtual analog OB-X style; imports OB-Xd banks | **Yes, with source** | README: "free in all contexts and for all uses". No App Store build because of GPL-3. Do not use "Oberheim" in Cantus UI. |
| OB-Xd 2.x | discoDSP (orig. 2DaT) | GPL-3 **only up to 2.11**; later versions are freeware binaries, "free for any use" | AAX, AU, LV2, VST2, VST3 | Win, macOS, Linux | 2.20, 2026-08-18 | 39 MB | Good | **Link only** | Current binaries are not OSS. Prefer OB-Xf. |
| **Dexed** | asb2m10 / Digital Suburban | GPL-3.0 (msfa core Apache-2.0) | CLAP, VST3, AU (LV2 **not verified**) | Win, macOS, Linux | v1.0.1, 2025-11-29 (1.1.0 nightlies) | zip 9.7 MB | DX7 emulation, loads DX7 sysex | **Yes, with source** | README notes a known CLAP scaling limit ("scaling is not available for now"). Overlaps with Modulus, but a good compatibility case. |
| **Odin 2** | TheWaveWarden | GPL-3.0 (font OFL-1.1) | VST3, CLAP, AU, LV2 | Win, macOS, Linux | 2.4.1 (from asset names; date **not verified**; repo last push 2025-09-07) | installer 41 MB | Polished semi-modular VA | **Yes, with source** | GitHub reports the licence as "NOASSERTION" because the LICENSE file has a font preamble; the text is GPL-3. |
| **Firefly Synth 2** | Sjoerd van Kreel | MIT | CLAP, VST3 (separate zips; instrument + FX builds) | Win, macOS, Linux | v2.1.6, 2026-09-21 | ~9.4 MB per format | Very active, CLAP-first, own framework (not JUCE) | **Yes** (MIT: keep notice) | Firefly Synth 1 (GPL-3) repository is archived. README carries a correct VST trademark notice. |
| **Six Sines** | Paul Walker (baconpaul) | MIT (repo) | CLAP via clap-wrapper → VST3, AU, standalone (**formats not verified in zip**) | Win, macOS, Linux | v1.2.1, 2026-07-20 | zip 22 MB / exe 9.9 MB | Small FM/PM/RM synth, CLAP-native | **Yes** (licence of the binary **not verified**; depends on sst libs) | Good CLAP-first test case. |
| Shortcircuit XT | Surge Synth Team | Source MIT; **binaries AGPL-3** (GPL3/AGPL3 deps) | CLAP-first + VST3/AU via clap-wrapper | Win, macOS, Linux | nightlies only, 2026-10-04 | zip 22 MB | Sampler, still pre-1.0 (**status not verified**) | Later; yes with source | Overlaps with Exemplar. Useful test of a big CLAP-first plug-in. |
| **JC303** | midilab | GPL-3.0 (Open303 engine also MIT) | CLAP, VST3, LV2, AU | Win, macOS, Linux | v0.13.0, 2026-09-20 | zip 26 MB | TB-303 style bass line | **Yes, with source** | Overlaps with Gravis/Calor. |
| **RipplerX** | tiagolr | GPL-3.0 | VST3, LV2, AU (CLAP "planned when JUCE supports it") | Win, macOS, Linux | v1.5.19, 2026-06-13 | zip 6.9 MB | Physical-modelling percussion (mallets, membranes) | **Yes, with source** | Partly fills "world/percussion". VST3 only on Windows. |
| Monique | Thomas Arndt / Surge Synth Team | dual GPL-3 or MIT | **not verified** (Surge team usually builds VST3/CLAP/AU/LV2) | Win, macOS, Linux | nightly 2024-08-07 | setup 6.4 MB | Mono synth | Yes (pick MIT or GPL) | Little maintenance since 2024. |
| Vaporizer2 | VAST Dynamics | GPL-3.0 (FFTW option GPL-2+) | VST2, VST3, AU, AAX, LV2, CLAP (CLAP named in the README tagline only) | Win, macOS, Linux | v3.5.0, 2024-03-02 | installer 421 MB | Large wavetable/additive workstation | Possible; heavy | Size makes it a catalogue item, not a bundle. |
| Cardinal | DISTRHO (falkTX) | GPL-3.0 (modules must be GPL-3+ compatible) | CLAP (partial), VST2, VST3, AU, LV2, standalone | Win, macOS, Linux, FreeBSD, Web | 26.02, 2026-02-28 | installer 369 MB | VCV Rack in a plug-in, 1,000+ modules | Possible; heavy | "All VCV branding has been removed ... to avoid any trademark issues." Best DPF test case. |
| Geonkick | Iurie Nistor (quamplex) | GPL-3.0 | LV2, VST3, standalone (Linux) | Linux; Windows LV2/VST3 per README | v3.7.0 on GitHub (Jan 2026); README says 3.9.0 | **not verified** | Percussion synth (kicks, snares) | Only if Windows VST3 binaries exist (**not verified**) | Repository moved from Geonkick-Synthesizer to quamplex. |
| ZynAddSubFX / Zyn-Fusion | Mark McCurry et al. | GPL-2.0-or-later | VST, LV2, standalone | Win, Linux (macOS "coming soon") | Zyn-Fusion binaries sold for $45+ on Gumroad; free demo with a 10-minute silence limit | n/a | Deep additive/subtractive synth | No (official binaries are paid; we would have to build our own) | GPL-2+ can be used under GPL-3, so there is no licence conflict, only work. |
| Vital (source) / Vitalium | Matt Tytel / DISTRHO | GPL-3.0 | Vitalium: LV2 on Win/macOS via PawPaw v1.1 (2021-03-16); Linux distros (VST3/LV2 via distro packages, **not verified**) | | DISTRHO-Ports 2021-03-15 | PawPaw win64 95 MB | Excellent synth, but the Vitalium fork is from 2021 (JUCE 6) | Only as a self-built, renamed binary, without Vital's presets | See section 2.1. |
| sfizz | SFZ Tools | BSD-2-Clause | VST3, LV2, AU, Pd (no CLAP) | Win, macOS, Linux | 1.2.3, 2024-01-14 | win64 installer 3.6 MB | Reference SFZ player | Yes (permissive) | **Both repositories are archived** (sfizz-ui 2025; library 2026-06-21). Exemplar already covers SFZ. Use only for cross-checking. |
| ChowKick | Chowdhury DSP | BSD-3-Clause | VST3, AU (CLAP **not verified**) | Win, macOS | v1.2.0, 2023-01-16 | 5.8 MB | Kick drum synth | Yes | Dormant since 2023. |
| ADLplug / OPNplug | Jean Pierre Cimalando | BSL-1.0 | **not verified** (VST2/VST3/LV2) | Win, macOS, Linux | v1.0.2, 2020-08-05 | 18.6 MB (7z) | OPL3 / OPN2 FM chip emulation | Yes (BSL is permissive) | Retro/game sound, low priority. |
| plugdata | plugdata team | Source GPL-3; **binaries AGPL-3** (JUCE 8) | CLAP, VST3, LV2, AU, standalone | Win, macOS, Linux | v0.9.3-2, 2026-03-06 | msi 269 MB | Pure Data in a plug-in | Catalogue | Mainly a test case. |
| Helm | Matt Tytel | GPL-3.0 | VST2, LV2, AU (no VST3) | | 0.9.0 pre-release, 2017; repository archived | | Superseded by Vital | **No** | Cantus does not host VST2. |
| Tunefish 4 | Brain Control | GPL-3.0 | VST2-centric build (needs VST SDK) | | last push 2023-09 | | Small demoscene synth | No | No VST3 or CLAP binary verified. |
| DrumGizmo | DrumGizmo team | LGPL/GPL (**not verified**: COPYING fetch failed) | LV2, VST2 (Windows: VSTi 32/64-bit only) | Linux; Windows/macOS VST2 | 0.9.20, 2022-09-08 | | Multi-mic acoustic kit engine | **No** (VST2 only on Windows) | Its kits (DRSKit, MuldjordKit, CC-BY-4.0) exist as SFZ ports, so use them through Exemplar. |
| sfizioso | jpcima | **not verified** (repository not found at github.com/jpcima/sfizioso) | | | | | | Not verified | |

Not evaluated (out of scope or not instruments): Dragonfly Reverb (effect), Hydrogen (application, not a plug-in), Stochas (sequencer, GPL-3, Surge team), Wolpertinger and Aether (Aether is a reverb; neither checked), FluidSynth plug-ins (none with Windows CLAP/VST3 binaries found), b-step (sequencer), "Nektarine" (Nektar's proprietary host, not an instrument).

### 1.1 Notes on the OSS list

- **JUCE licensing drift.** JUCE 7 offered GPL-3. JUCE 8 offers AGPL-3, which is why plugdata and Shortcircuit XT state that their binaries are AGPL-3. GPLv3 section 13 explicitly allows combining with AGPLv3 works, and the AGPL's extra network clause does not affect a local plug-in. Bundling AGPL binaries carries the same source obligation as GPL.
- **Surge Synth Team family:** Surge XT, OB-Xf, Monique, Shortcircuit XT and Stochas, plus Paul Walker's Six Sines. All publish source tarballs next to the binaries, which makes complying with the source obligation trivial (mirror the tarball).
- **Clap-first projects** (Firefly Synth 2, Six Sines, Shortcircuit XT, free-audio's clap-plugins, Surge's Conduit) are the most valuable for testing Cantus's CLAP host, because they use CLAP features JUCE wrappers do not.

## 2. Vital, freeware, and why it is link-only

### 2.1 Vital and Vitalium, precisely

- **Source:** `github.com/mtytel/vital` is GPL-3.0. Its README adds conditions that are not copyright terms but trademark and service terms:
  - "Do not use the name 'Vital', 'Vital Audio', 'Tytel' or 'Matt Tytel' for marketing or to name any distribution of binaries built with this source."
  - "Do not connect to any web service at vital.audio, account.vital.audio or store.vital.audio from your own builds."
  - "Do not distribute the presets that come with the free version of Vital. They're under a separate license that does not allow redistribution."
  - No iOS App Store builds ("The app store is not compatible with GPLv3").
  The GitHub repository has no binary releases and was last pushed in 2023-05.
- **Vitalium:** DISTRHO-Ports release 2021-03-15 ("Add Vitalium") plus PawPaw v1.1. The KXStudio announcement says: "We cannot use the Vital brand or connect to its vital.audio online services, so this is completely removed from the Vitalium source code, with an alternative name and icon." The code lives in `ports-juce6.0/vitalium` and is JUCE 6 based. PawPaw ships it as **LV2** for Windows and macOS. Cantus does not host LV2, so Windows CLAP/VST3 Vitalium binaries are **not verified to exist**. Cantus would have to build them itself, under the name Vitalium and without Vital's factory presets.
- **Vital Audio's binary** (vital.audio): Basic tier is free ("the full synth with all features", 75 presets, 25 wavetables). Plus and Pro are paid, and there is a subscription. It supports Windows 10+, macOS 10.15+ and Ubuntu 18.04+, 64-bit only, and needs OpenGL 3. The EULA applies "by installing, creating an account, and/or otherwise using". It says the software is "licensed, not sold", forbids redistributing unaltered Vital content (presets, wavetables, skins), and gives **no right to redistribute the software**. Third-party reports say CLAP is offered (installer option); the official page does not list formats, so CLAP is **not verified**.
- **Verdict:** link to vital.audio. Do not bundle the official binary. Bundling a self-built Vitalium is legal (GPL-3 with source, renamed, no presets), but it is a 2021 fork with no maintained Windows CLAP/VST3 build. Not worth doing now.

### 2.2 Freeware (proprietary, free of charge)

For every item below, "redistribution" was checked against the vendor's own terms where the page could be read. None grants a third party the right to redistribute the installer. Where a product sits behind an account or email gate, Cantus can only link to the vendor's product page (deep-linking to the file itself would bypass the gate and break as soon as the vendor rotates URLs).

| Name | Developer | Licence / terms (verified?) | Formats | Platforms | Version / date | Size | Quality / gap | May Cantus bundle? | Notes |
|---|---|---|---|---|---|---|---|---|---|
| Vital Basic | Vital Audio | Proprietary EULA (verified, see 2.1) | VST3, AU, CLAP (CLAP **not verified** officially) | Win, macOS, Linux | **not verified** (1.6.x reported) | | Top-tier wavetable synth | **Link only** | Account required. |
| BBC Symphony Orchestra Discover | Spitfire Audio | Free licence via sign-in (verified page: "Sign in to access"); EULA allows 2 machines | VST3, AU, AAX | Win 10/11, macOS 11–26 | **not verified** | 360 MB disk | **Orchestral gap**: full orchestra (strings, brass, woodwinds, percussion), now with BBCSO piano | **Link only** | Spitfire account and Spitfire app. |
| Spitfire LABS → **Splice INSTRUMENT** | Spitfire / Splice | labs.spitfireaudio.com now redirects to Splice INSTRUMENT, a freemium product needing a Splice account (verified). Spitfire support page: "not permitted to use the sounds to create another sample library" | **not verified** (LABS was VST/VST3/AU/AAX) | Win, macOS | | per pack | Soft Piano, Strings and others; LABS Wurli and Sleigh Bells discontinued | **Link only** | The ecosystem is moving to a subscription; it may stop being a reliable free option. |
| Decent Sampler | Decent Samples (Decidedly LLC) | Freeware; EULA's redistribution clause covers samples ("You may not redistribute any of the samples or sample libraries..."); no grant to redistribute the plug-in (verified) | VST, VST3, AU, AAX, standalone; **no CLAP** | Win, macOS, Linux, iOS | v1.36.1 | | Huge free library ecosystem (Pianobook) | **Link only** | Email required to download. Exemplar already reads `.dspreset`, so the player matters less than the libraries. |
| Pianobook libraries | Pianobook community | FAQ: free for commercial and non-commercial composition; users may not "sell or redistribute the sample libraries" (verified) | Decent Sampler / Kontakt | any | per library | per library | Pianos, strings, oddities, many vocal and world items | **Link only** | Good catalogue content for Exemplar, but the user must download each library. |
| Komplete Start | Native Instruments | Free with NI account (verified page); EULA **not read** | Kontakt Player, Reaktor Player (formats **not verified**) | Win, macOS | | **not verified** | Kontakt Player instruments, synths, effects | **Link only** | Needs Native Access. |
| sforzando | Plogue | Free (verified); redistribution terms **not found** | **CLAP**, VST3, AU (macOS), AAX, standalone | Win, macOS, Linux | **not verified** | | Reference SFZ/ARIA player; also imports SF2/DLS | **Link only** | A rare **freeware CLAP** instrument on all three OSes: an excellent compatibility test. |
| TX16Wx | CWITEC | Free version ("100% free... no limitation"); Professional is paid (verified). Redistribution **not verified** | VST2, VST3, AU, AAX | Win, macOS | **not verified** | | Full sampler | **Link only** | Overlaps with Exemplar. |
| TyrellN6 | u-he / Amazona.de | Free, "does not require a serial number" (verified). Redistribution **not verified** | **CLAP**, VST3, AU, AAX | Win, macOS, Linux (beta) | 3.0.0 beta rev 16976, 2025-03-18 | **not verified** | Classic VA (Juno-ish) | **Link only** | Proprietary CLAP by a CLAP co-author: high-value test. |
| Podolski | u-he | Free, no serial (verified) | VST3, AU, AAX (Win); CLAP **not verified** | Win, macOS, Linux | 1.2.3 rev 12092, 2021-08-10 | | Simple VA | Link only | |
| Synth1 | Ichiro Toda (Daichi) | Freeware; commercial use of music OK (verified) | **VST2.4 only** (Win), AU (macOS) | Win, macOS | 1.13 beta3 (Win, 2014); AU beta11 (2021) | <1 MB | Nord Lead 2 style | **No** | Cantus cannot host VST2. |
| Ample Guitar M Lite (AGML) v4 / Ample Bass P Lite | Ample Sound | Free (verified listing); EULA **not read** | VST2, VST3, AU, AAX, standalone | Win 7–11 x64, macOS 10.15+ | v4 (2026, per review links on vendor page) | library 877 MB (16-bit), disk >10 GB stated for the family | **Guitar gap**: Martin D-41 fingerstyle; Riffer limited to 64 bars | **Link only** | Account/registration **not verified**. |
| Orchestral Tools SINE free libraries | Orchestral Tools | **not verified** (product page 404) | SINE player (VST3/AU/AAX) | Win, macOS | | | Orchestral | Link only | |
| Keyzone Classic | Bitsonic | **not verified** | **not verified** | | | | Piano | Link only | Could not check this session. |
| Piano One | Sound Magic | **not verified** | **not verified** | | | | Piano | Link only | Could not check this session. |
| MT Power Drum Kit 2 | Manda Audio | **not verified** (site did not respond) | **not verified** (historically VST2/VST3/AU) | Win, macOS | | | **Acoustic drum gap**; popular | Link only | |
| Steven Slate Drums 5.5 | Steven Slate Drums | Vendor page shows "Try it Free"; licence via PACE/iLok License Manager (verified). Whether a permanent free tier exists is **not verified** | **not verified** | Win, macOS | | | Acoustic drums | **Link only** | iLok/PACE: a poor fit for "free". |
| Sennheiser DrumMic'a, Ugritone free, Arturia Analog Lab (free edition) | various | **not verified** | | | | | | Link only | Not checked. |

**Rule for the catalogue:** a freeware entry is a card with the vendor's product page URL, what the vendor requires (account, app, email), the formats, and whether the plug-in is CLAP or VST3. Cantus never downloads freeware itself.

## 3. Free sample libraries Exemplar can load (the real gap-fillers)

The licence verdicts here follow samples.md, which checks each library in more depth; where they differ, samples.md wins. These are SFZ (some also ship as Decent Sampler or ARIA banks). Licences come from each repository (GitHub API spdx id plus README). Sizes are git repository sizes, a rough upper bound on what the samples weigh.

| Library | Gap | Licence (verified) | Size | Bundle? | Notes |
|---|---|---|---|---|---|
| **Osiris Piano** (Versilian Studios + Karoryfer) | Piano | **CC0-1.0** | ~448 MB | **Yes** | Upright/compact grand character. Ships SFZ programs and a manual. |
| **Salamander Grand Piano V3** (Alexander Holm; SFZ by sfzinstruments) | Piano | **CC-BY 3.0** (README) | ~731 MB (48 kHz/24-bit) | **Only if the owner admits CC-BY** (ADR 0001 today: CC0 only); then with attribution | Yamaha C5, 16 velocity layers, string resonance, hammer and pedal noise. The best-known free grand. |
| Splendid Grand Piano (Akai, public domain) | Piano | "Public domain" per the README ("released as public domain in early 2000 by Akai"); no licence file and no primary source for Akai's dedication (samples.md §4) | ~73 MB repo; README says 256 MB version | **Unclear** until a primary source is found | Steinway, 4 velocity layers (+1 synthetic). Small "lite" piano. |
| Headroom Piano (Bengt Nilsson) | Piano | CC-BY-4.0 | ~137 MB | Owner's CC-BY ruling; then with attribution | Yamaha C3. |
| Maestro Concert Grand (Mats Helgesson) | Piano | **All rights reserved**: "You may not modify and spread this soundfont without the author's written permission" (mcgreadme.txt, verified in samples.md §4) | ~256 MB | **No** | Yamaha CF-3, 5 layers. The sfzinstruments port has the author's e-mail permission; it does not extend to Cantus. |
| **VSCO 2 Community Edition** (Versilian) | Orchestral: strings, brass, woodwinds, keys, percussion | **CC0-1.0** | ~2.3 GB repo | **Yes** (subset) | SFZ 1.1.0 download. The default free orchestra. |
| **VCSL** (Versilian Community Sample Library) | General: orchestral, world, percussion, keys | **CC0-1.0** ("do whatever you want... no credit") | 20–75 MB per instrument | **Yes** | Lightweight; README emphasises pitch fidelity over many velocity layers. |
| Sonatina Symphonic Orchestra (peastman/sso) | Orchestral | **CC Sampling Plus 1.0** | ~1.3 GB | **No** | Sampling+ was retired by Creative Commons in 2011 (Freesound treats it as BY-NC), and its author says some sources' licensing is unknown (samples.md §1.5, §4). Prefer VSCO 2 CE/VCSL. |
| **Virtuosity Drums** (Versilian + Karoryfer, KVRDC'21) | Acoustic drums | **CC0-1.0** | ~1.2 GB | **Yes** | Kit plus percussion (tumba, timbales). |
| DrumGizmo DRSKit / MuldjordKit (SFZ ports) | Acoustic drums | CC-BY-4.0 plus a request for album credits | 736 MB / 337 MB | Owner's CC-BY ruling; then with attribution | Multi-mic kits. DrumGizmo asks for "Drum samples provided by DrumGizmo.org" in album credits (samples.md §1.3). |
| Naked Drums (Wilkinson Audio) | Acoustic drums | CC-BY-4.0 | ~1.25 GB | Owner's CC-BY ruling; then with attribution | 10 round robins, multi-mic. |
| Karoryfer Frankensnare, Unruly Drums, Big Rusty Drums, Swirly Drums, The Hat With The Phat | Drums | CC0-1.0 (The Hat With The Phat: repo says CC0, shop page says GPL 3; ask Karoryfer) | 336–840 MB each | Yes (the Hat once cleared) | Characterful kits. |
| **Karoryfer Shinyguitar / Emilyguitar / Black And Green Guitars** | Guitar (electric) | **CC0-1.0** | 360 / 101 / 454 MB | **Yes** | The best free CC0 guitars. |
| Karoryfer basses (Meatbass, Pastabass, Growlybass, Swagbass, Fashionbass, Black And Blue Basses, Big Little Bass, Sneakybass), D. Smolken double bass | Bass, double bass | CC0-1.0 | 140–985 MB | Yes | |
| Karoryfer Bear Sax, Weresax, War Tuba; MTG Solo Sax | Wind / brass | CC0-1.0; MTG is CC-BY-4.0 | 106–193 MB | Yes | |
| Karoryfer–Bigcat Cello; String Cyborgs | Strings | CC0-1.0 | 129 / 62 MB | Yes | Solo cello. |
| World: Gogodze Phu I/II, Cithara barbarica (10-string lyre), Hungarian zither, AliExpress erhu, Ganjo, Horse Pulse (bass tagelharpa), steel drum (jlearman, **Unlicense**), Body percussion | World / ethnic | CC0-1.0 (steel drum Unlicense) | 23–677 MB | Yes | Thin coverage, but rights-free. |
| legato_vocal_tutorial | Vocal | CC0-1.0 | ~160 MB | Maybe | No real free choir or vocal library was verified. **The vocal/choir gap stays open.** |
| jRhodes3c/3d, Greg Sullivan E-Pianos | Electric piano | jRhodes: BY-NC-SA-4.0 / BY-NC-4.0 (**no**, NC); Greg Sullivan: CC-BY-3.0 (samples.md §4) | 18–163 MB | jRhodes no; Greg Sullivan only under a CC-BY ruling | Lamina already covers e-piano. |
| Discord SFZ GM Bank | General MIDI | **not verified** (no licence field) | ~180 MB | Not until verified | Would be useful for MIDI import. |

No acoustic guitar with a verified free licence was found in the sfzinstruments set. The "Ovation Guitar" and "Damien's Funky Guitar" entries have no licence field. So the **acoustic guitar gap** is filled only by link-only freeware (Ample Guitar M Lite).

Practical consequence: a "Cantus Essentials" content pack needs **no plug-ins at all**. For example: Osiris Piano (+ Salamander if CC-BY is admitted) + a VSCO 2 CE subset + Virtuosity Drums + Shinyguitar + one Karoryfer bass + a few VCSL world instruments. That is roughly 2–4 GB at full size. It can be cut down (fewer velocity layers, FLAC) or delivered as an optional download from Cantus's own release server, which is legal for CC0 and (with attribution) CC-BY content; ADR 0001 decides which of the two Cantus accepts.

## 4. Legal analysis

This is not legal advice. It sets out what the primary sources say and which decisions belong to the owner.

### 4.1 Is a hosted plug-in "part of" Cantus? (GNU GPL FAQ)

- **#GPLPlugins** ("When is a program and its plug-ins considered a single combined program?"): "If the main program dynamically links plug-ins, and they make function calls to each other and share data structures, we believe they form a single combined program, which must be treated as an extension of both the main program and the plug-ins." Only simple fork/exec with no intimate communication makes them clearly separate. CLAP and VST3 plug-ins are shared libraries loaded into the host's address space that exchange structures (`clap_process`, `ProcessData`). On the FSF's reading, Cantus plus a loaded plug-in is a combined program **at run time**.
- **#MereAggregation**: "An 'aggregate' consists of a number of separate programs, distributed together on the same CD-ROM or other media. The GPL permits you to create and distribute an aggregate, even when the licenses of the other software are nonfree or GPL-incompatible." The FAQ adds that the line between separate programs and one program is "a legal question, which ultimately judges will decide". It also warns that "exchanging complex internal data structures" can make two parts one program.
- **#GPLAndPlugins / #NFUseGPLPlugins**: if host and plug-in form a single combined program, "the main program must be released under the GPL or a GPL-compatible free software license". If they are separate works, the plug-in's licence "makes no requirements about the main program".
- **The GPL does not restrict running.** Combining at run time on the user's own machine is not distribution. Obligations attach only to what **Cantus distributes**.

What follows for Cantus:

1. **Cantus app is GPL-3.0-or-later**, so bundling GPL-3 or GPL-2-or-later plug-ins is fine on either reading (aggregation or combined work). The same holds for MIT, BSD, ISC and BSL plug-ins: all are GPL-compatible.
2. **The core crates are MIT OR Apache-2.0** and do not link any plug-in at build time. Bundling a GPL plug-in in the installer does not change their licence. Anyone who takes only the crates gets no GPL code. Apache-2.0 is compatible with GPL-3 (not GPL-2-only), which matters only if crates were ever combined with GPL-2-only code.
3. **GPL-2-only plug-ins: avoid bundling.** FAQ **#v2v3Compatibility**: "Is GPLv3 compatible with GPLv2? No... if you tried to combine code released under GPLv2 with code under GPLv3, you would violate section 6 of GPLv2. However, if code is released under GPL 'version 2 or later,' that is compatible with GPLv3." Under the FSF's dynamic-loading view, shipping a GPL-2-**only** plug-in inside the Cantus installer, "designed to be used with" a GPL-3 host, is arguable. The aggregation argument (separate programs that talk through a public, third-party, MIT-licensed API) is reasonable but untested. None of the recommended plug-ins is GPL-2-only. ZynAddSubFX is GPL-2.0-**or-later** and Vaporizer2's optional FFTW is GPL-2-or-later, so both are fine.
4. **Proprietary freeware loaded by a GPL host.** Cantus does not distribute these, and users run them privately, so there is no GPL issue in practice. **Owner decision:** consider adding a GPL-3 section 7 "additional permission" to Cantus's licence that explicitly allows loading non-free CLAP/VST3 plug-ins through the plug-in interfaces. Only copyright holders can grant this: fine for Cantus's own code, not for third-party GPL code it includes. This removes any doubt for redistributors, such as Linux distributions. Whether peer GPL DAWs do this was **not verified** this session.
5. **AGPL-3 binaries** (JUCE 8 builds such as plugdata and Shortcircuit XT): GPLv3 section 13 permits combining with AGPLv3 works. The AGPL's network-interaction clause does not apply to a local plug-in. The source obligation is the same as for GPL.

### 4.2 Source obligations when Cantus ships GPL binaries

- GPL-3 section 6. FAQ **#AnonFTPAndSendSources**: "If you make object code available on a network server, you have to provide the Corresponding Source on a network server as well... The sources you provide must correspond exactly to the binaries." FAQ **#SourceAndBinaryOnDifferentSites**: source may live on another site, but "you must take care to make sure that the source remains available for as long as you distribute the object code."
- **Recommendation:** for every bundled GPL/AGPL plug-in, mirror the upstream source tarball of the exact version (Surge, OB-Xf, Dexed, Six Sines and Shortcircuit XT all publish `*-source-*.tar.gz` or `surge-src-*.tar.gz`) as an asset of the same Cantus release. Also list each one in a `THIRD-PARTY-PLUGINS` notice shipped in the installer (name, version, licence, upstream URL, source URL). Do not rely on an upstream link alone; upstream can delete releases. A "written offer" (GPL-3 6(b)) is the alternative for physical media, and it is more work than mirroring.
- Do **not modify** bundled plug-ins. If Cantus patches one, the obligation becomes "your modified source", and the trademark rules below apply to the changed build.
- MIT/BSD/ISC/BSL plug-ins only need their licence text and copyright notice shipped.
- CC-BY sample content needs attribution (title, author, licence, link) in the notice and ideally in Exemplar's instrument info panel. CC0 and public domain need nothing, but attribution is good practice.

### 4.3 Trademarks (the GPL grants none)

- **Vital:** the README forbids using "Vital", "Vital Audio", "Tytel" or "Matt Tytel" for marketing or to name a binary distribution. DISTRHO's fork is therefore **Vitalium**, with a new icon and the online services removed (KXStudio, 2021). Cantus may say "Vital" only when linking to vital.audio. It must never label a self-built binary "Vital".
- **VCV Rack → Cardinal:** "All VCV branding has been removed ... in order to avoid any trademark issues." This is a precedent that trademark caution applies even between GPL projects.
- **OB-Xd / OB-Xf:** discoDSP's disclaimer reads "independent project inspired by the Oberheim OB-X... not affiliated with, endorsed by, or connected to Oberheim". Cantus should present them as "OB-Xf" and never use "Oberheim" as a category or label. The same applies to JC303 (do not write "Roland TB-303" as a product name; "303-style" is safer) and Dexed (do not use "Yamaha DX7" as a product name; descriptive use only).
- **Surge XT / Surge Synth Team:** no published trademark policy was found (**not verified**). Distributing the **unmodified** upstream binary under its own name is ordinary nominative use. If Cantus ever patches it, ask the team first.
- **VST:** since VST 3.8 (2025-10-29) the SDK is **MIT**; the GPL-3 and proprietary licence options are gone. The repository README says the name and the "VST Compatible" logo are optional. If Cantus uses them, it must follow the *Steinberg VST Usage Guidelines* shipped in the SDK: logos unaltered, used in product contexts (splash screen, about box, website, manual), "VST" never inside a company or product name, never on non-compatible products. Writing "VST3" in Cantus's plug-in browser to name a format is descriptive. Recommended: add "VST is a trademark of Steinberg Media Technologies GmbH, registered in Europe and other countries" to the About box and docs (Firefly Synth 2 does this). Use the logo only by following the guidelines (see steinberg.net/sdklicenses_vst3).
- **CLAP:** the SDK is MIT. The repository offers a "CLAP Logo Pack" with no restrictive policy found (**not verified** that none exists).
- **Freeware names** (Spitfire, BBC, Native Instruments, u-he): use them only to describe and link to the vendor's product. Never use their logos as Cantus UI art without permission.

### 4.4 Bundle vs in-app catalogue

| | Bundle in installer | In-app "Get free instruments" catalogue |
|---|---|---|
| Legal | OSS only; must ship or mirror exact source; attribution notices | No redistribution by Cantus. For freeware: open the vendor page. For OSS: may download from the upstream GitHub release URL |
| Offline | Works offline on first run | Needs network |
| Compatibility proof | Strong: Cantus tests the exact versions it ships | Weaker: upstream updates can change behaviour; pin version + sha256 |
| Credit and updates | Cantus must track upstream updates itself | Upstream gets the download, credit and its own updates |
| Installer size | +60 MB (small synths) to +700 MB (Surge with content) | None |
| Freeware | Not allowed | Allowed (link only) |

**Recommended hybrid:**
1. Bundle a small CC0 sample pack (piano + drums + guitar) that Exemplar loads, so the first run fills the gaps offline.
2. Bundle 2–3 small OSS plug-ins as an optional installer component: OB-Xf (CLAP), Dexed (CLAP), Firefly Synth 2 (CLAP + VST3). Mirror their sources. This proves hosting on the first run.
3. Put everything else in a catalogue. OSS entries download from upstream with pinned version and sha256, and the downloaded file is checked before installation. Freeware entries are cards that open the vendor page.

## 5. Ranked shortlists

### 5.1 By gap

| Gap | Rights-free (bundle/download via Exemplar) | Link-only freeware |
|---|---|---|
| Acoustic piano | 1. Osiris Piano (CC0; bundle-safe today), 2. Salamander V3 (CC-BY: the better general grand, if the owner admits CC-BY), 3. Splendid Grand ("PD", small; unclear until a primary source is found) | BBCSO Discover piano, LABS/INSTRUMENT Soft Piano, Keyzone Classic, Piano One (last two not verified) |
| Orchestral | 1. VSCO 2 CE (CC0), 2. VCSL (CC0). Sonatina excluded (Sampling+) | 1. BBC SO Discover, 2. Splice INSTRUMENT (LABS strings), 3. SINE free (not verified) |
| Guitar | Electric: Karoryfer Shinyguitar, Black And Green Guitars, Emilyguitar (CC0). Acoustic: **none verified** | Ample Guitar M Lite v4 (acoustic), Ample Bass P Lite |
| Acoustic drums | 1. Virtuosity Drums (CC0), 2. Naked Drums (CC-BY), 3. DrumGizmo kits as SFZ (CC-BY) | MT Power Drum Kit 2 (not verified), SSD 5.5 (trial/iLok) |
| World / ethnic | VCSL world items, Karoryfer (Gogodze Phu, lyre, zither, erhu, tagelharpa), steel drum; RipplerX (GPL-3) for modelled mallets/membranes | Pianobook libraries |
| Vocal / choir | **Gap remains**: nothing verified | Pianobook vocal libraries (not individually verified) |

### 5.2 (a) What Cantus may BUNDLE

Ranked by value × ease. All have Windows binaries and are verified OSS:

1. **Sample content via Exemplar:** Osiris Piano, Virtuosity Drums, Shinyguitar, VSCO 2 CE subset, VCSL subset (all CC0, bundle-safe under ADR 0001 today); Salamander V3 only if the owner admits CC-BY. Fills real gaps; no code.
2. **OB-Xf** v1.0.3: GPL-3, CLAP, 9 MB + 24 MB assets, maintained by Surge Synth Team.
3. **Dexed** v1.0.1: GPL-3, CLAP, 10 MB.
4. **Firefly Synth 2** v2.1.6: MIT, CLAP + VST3, non-JUCE.
5. **Surge XT** 1.3.4: GPL-3, CLAP. Best synth, but needs its content folder (~250 MB+); also a strong catalogue candidate.
6. **Six Sines** v1.2.1: MIT, CLAP-first.
7. **Odin 2** 2.4.1: GPL-3, CLAP.
8. **JC303** v0.13.0: GPL-3, CLAP.
9. **RipplerX** v1.5.19: GPL-3, VST3 only. Physical modelling; partly fills world/percussion.

Not recommended for the bundle: Vitalium (2021, LV2 on Windows, renaming needed), Cardinal/plugdata/Vaporizer2 (300–420 MB each; catalogue), sfizz (archived; Exemplar covers SFZ), Helm/Synth1/Tunefish/DrumGizmo (no VST3/CLAP on Windows).

### 5.3 (b) LINK only

1. BBC Symphony Orchestra Discover. 2. Vital (Basic). 3. Ample Guitar M Lite v4 + Ample Bass P Lite. 4. Splice INSTRUMENT (ex-LABS). 5. Decent Sampler + Pianobook libraries. 6. sforzando. 7. TyrellN6. 8. Komplete Start. 9. TX16Wx. 10. MT Power Drum Kit 2, Keyzone Classic, Piano One, SINE free (each **not verified**). 11. OB-Xd 2.20 (freeware binaries; prefer OB-Xf).

### 5.4 (c) Compatibility test set (CI and smoke tests)

The aim is one plug-in per framework and format path. Prefer redistributable OSS for CI (it can be cached in CI artifacts); use freeware only in manual smoke tests.

| Framework / path | CLAP | VST3 | Licence | CI-safe? | Why |
|---|---|---|---|---|---|
| CLAP reference: free-audio **clap-plugins** | yes | n/a | MIT | Yes (build from source; no binary releases) | Canonical CLAP behaviour, example synths + effects |
| Clap-first + clap-wrapper: **Six Sines** | yes | yes (wrapped) | MIT | Yes | Same plug-in in both formats, so behaviour can be compared |
| Own framework (non-JUCE): **Firefly Synth 2** | yes | yes | MIT | Yes | Separate CLAP and VST3 builds; heavy modulation |
| JUCE 7 + clap-juce-extensions: **Surge XT** or **OB-Xf** | yes | yes | GPL-3 | Yes | The most common real-world JUCE path |
| JUCE (VST3 only): **RipplerX** | no | yes | GPL-3 | Yes | VST3-only JUCE plug-in |
| DPF: **Cardinal** (or a small DPF example) | partial | yes | GPL-3 / ISC (DPF) | Yes (large) | DPF's own CLAP/VST3 wrappers |
| Rust **nih-plug**: Diopser / Spectral Compressor / Crisp (effects; `latest-builds`, Windows zip 30 MB) | yes | yes | plug-ins GPL-3 (**not verified**); framework ISC | Yes | Same language as Cantus. No nih-plug **instrument** with releases was verified |
| VSTGUI / VST3 SDK direct: **sfizz** 1.2.3, Steinberg SDK samples (e.g. "again", note-expression synth) | no | yes | BSD-2 / MIT | Yes | Plain VST3 SDK path |
| iPlug2 | | | | **not verified** | No maintained OSS iPlug2 instrument with Windows binaries was found; build iPlug2's example instrument in CI if needed |
| Proprietary CLAP: **TyrellN6**, **sforzando** | yes | yes | freeware | No (manual smoke only) | Commercial-grade CLAP implementations |
| Proprietary VST3 big sampler: **BBC SO Discover**, **Decent Sampler** | no | yes | freeware | No (manual) | Large streaming sample engines, account-gated content |

Minimum CI set: clap-plugins + Six Sines + Firefly Synth 2 + OB-Xf + RipplerX + one nih-plug effect + sfizz. Together that is under 150 MB and covers CLAP-native, clap-wrapper, JUCE CLAP/VST3, VST3-only JUCE, Rust and the plain VST3 SDK.

## 6. Decisions for the owner

1. Ship rights-free **sample content** (CC0 today; CC-BY if ruled in) through Exemplar as the main gap-filler? (recommended)
2. Bundle OSS plug-ins in the installer (with mirrored source and notices), or only offer them in a catalogue that downloads from upstream? (recommended: 2–3 small CLAP synths bundled, the rest in the catalogue)
3. Add a GPL-3 section 7 additional permission for loading non-free plug-ins?
4. Allow CC-BY content (needs an attribution UI)? Sonatina's Sampling Plus 1.0 is recommended out (retired, NC-like, unclear sources).
5. Policy on GPL-2-only plug-ins: none in the bundle (recommended).
6. Use the VST logo (follow Steinberg's guidelines) or only the word "VST3" plus the trademark notice? (recommended: word plus notice)
7. Accept that **vocal/choir** and **acoustic guitar** have no verified rights-free option yet.

## 7. Not verified

Vital's official format list (CLAP) and current version; Spitfire/Splice EULA text and LABS formats; Komplete Start EULA and formats; Keyzone Classic, Piano One, MT Power Drum Kit 2, SSD 5.5 free tier, DrumMic'a, Ugritone, Arturia free edition, Orchestral Tools SINE free (pages failed or were not reached); Ample Sound EULA and registration requirement; u-he and Plogue redistribution terms; Odin 2's 2.4.1 release date; Geonkick Windows binaries; Monique, Dexed (LV2), OB-Xf and Six Sines exact format lists inside the zips; DrumGizmo licence; whether Surge's plugins-only zip includes factory data; Maestro Concert Grand, jRhodes, Discord GM Bank licences; Sonatina Sampling+ terms (from memory); sfizioso's location; Surge Synth Team and CLAP trademark policies; Windows VST3/CLAP builds of Vitalium; whether other GPL DAWs add a plug-in exception. The GPL FAQ was read from the Internet Archive copy, because gnu.org did not answer through the proxy.

## Sources

All accessed 2026-10-05.

- GitHub API (licence, releases, sizes): https://api.github.com/repos/{surge-synthesizer/releases-xt, surge-synthesizer/surge, surge-synthesizer/OB-Xf, reales/OB-Xd, baconpaul/six-sines, surge-synthesizer/shortcircuit-xt, TheWaveWarden/odin2, asb2m10/dexed, sfztools/sfizz, sfztools/sfizz-ui, DISTRHO/DISTRHO-Ports, DISTRHO/Cardinal, DISTRHO/PawPaw, DISTRHO/DPF, mtytel/vital, mtytel/helm, tiagolr/ripplerx, sjoerdvankreel/firefly-synth, sjoerdvankreel/firefly-synth-2, zynaddsubfx/zynaddsubfx, surge-synthesizer/monique-monosynth, jpcima/ADLplug, robbert-vdh/nih-plug, free-audio/clap-plugins, surge-synthesizer/conduit, iPlug2/iPlug2, plugdata-team/plugdata, midilab/jc303, paynebc/tunefish, VASTDynamics/Vaporizer2, Chowdhury-DSP/ChowKick, surge-synthesizer/stochas}
- https://api.github.com/orgs/sfzinstruments/repos (sample library licences and sizes)
- https://surge-synthesizer.github.io/
- https://github.com/surge-synthesizer/surge
- https://github.com/surge-synthesizer/OB-Xf
- https://github.com/reales/OB-Xd
- https://github.com/mtytel/vital
- https://github.com/DISTRHO/DISTRHO-Ports (README, `ports-juce6.0/vitalium`, release 2021-03-15 notes)
- https://kx.studio/News/?page=11 and https://kx.studio/News/?page=12 (Vitalium announcement)
- https://github.com/quamplex/geonkick and https://github.com/quamplex/geonkick/releases
- https://raw.githubusercontent.com/TheWaveWarden/odin2/master/LICENSE
- https://raw.githubusercontent.com/surge-synthesizer/monique-monosynth/main/LICENSE and README
- READMEs: baconpaul/six-sines, surge-synthesizer/shortcircuit-xt, asb2m10/dexed, tiagolr/ripplerx, midilab/jc303, DISTRHO/Cardinal, paynebc/tunefish, VASTDynamics/Vaporizer2, sfztools/sfizz-ui, sjoerdvankreel/firefly-synth-2, free-audio/clap-plugins, plugdata-team/plugdata, robbert-vdh/nih-plug, zynaddsubfx/zynaddsubfx (README.adoc)
- https://github.com/sfztools/sfizz (archived notice)
- https://zynaddsubfx.sourceforge.io/zyn-fusion.html
- https://www.drumgizmo.org/wiki/doku.php?id=start, https://www.drumgizmo.org/wiki/doku.php?id=about, https://www.drumgizmo.org/wiki/doku.php?id=getting_drumgizmo
- https://vital.audio/ and https://vital.audio/eula
- https://www.decentsamples.com/product/decent-sampler-plugin/ and https://www.decentsamples.com/decent-samples-end-user-license-agreement/
- https://www.pianobook.co.uk/faq/
- https://splice.com/instrument/labs-instrument (redirect target of https://labs.spitfireaudio.com/)
- https://support.spitfireaudio.com/en/articles/11815239-are-spitfire-audio-sample-libraries-royalty-free-and-can-i-use-them-on-commercial-recordings
- https://www.spitfireaudio.com/bbc-symphony-orchestra-discover
- https://www.native-instruments.com/en/products/komplete/bundles/komplete-start/
- https://www.plogue.com/products/sforzando.html
- https://www.tx16wx.com/
- https://u-he.com/products/tyrelln6/ and https://u-he.com/products/podolski/
- https://daichilab.sakura.ne.jp/softsynth/index.html (Synth1)
- https://www.amplesound.net/en/pro-pd.asp?id=7 (Ample Guitar M Lite)
- https://stevenslatedrums.com/ssd5/
- https://www.orchestraltools.com/store/collections/sine-factory (404)
- https://www.mtpowerdrumkit.com/ (no response)
- READMEs and LICENSE files: sfzinstruments/SalamanderGrandPiano, sfzinstruments/SplendidGrandPiano, sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano (mcgreadme.txt), sfzinstruments/Osiris_Piano, sfzinstruments/virtuosity_drums, sgossner/VCSL, sgossner/VSCO-2-CE, peastman/sso
- GNU GPL FAQ, entries #GPLPlugins, #GPLAndPlugins, #NFUseGPLPlugins, #MereAggregation, #v2v3Compatibility, #AllCompatibility, #AnonFTPAndSendSources, #SourceAndBinaryOnDifferentSites, #WhatDoesWrittenOfferValid, #GPLStaticVsDynamic: https://www.gnu.org/licenses/gpl-faq.html (read via https://web.archive.org/web/2025id_/https://www.gnu.org/licenses/gpl-faq.html)
- https://github.com/steinbergmedia/vst3sdk (MIT licence; VST usage guidelines; https://www.steinberg.net/sdklicenses_vst3)
- https://sonicstate.com/news/2025/10/30/vst-3-now-available-under-mit-license
- https://github.com/free-audio/clap (MIT; logo pack)
