# FabFilter Pro-L 2: product research for the Cantus mastering limiter

**Researched:** 2026-09-23 · **Subject:** FabFilter Pro-L 2, version 2.26 (latest) · **Purpose:** to understand
the market-leading limiter in depth so that Cantus's own limiter can match its feature set and beat it on
workflow and interface. This is not a clone spec. Everything here is paraphrased from the cited sources. Where a
value is our own inference from an official screenshot, the text says so.

---

## 0. Screenshots

All images are official FabFilter material: the press kit, the online help and the product page.

| File | What it shows |
|---|---|
| [`screenshots/pro-l-01-main-interface.png`](screenshots/pro-l-01-main-interface.png) | The full interface (press kit, 1900×1184): the real-time display with peak labels, the Advanced panel open, the output and GR meters, the loudness panel set to Short Term and a −14 LUFS "Strm" target, TP limiting on, 8x oversampling and 16-bit dither |
| [`screenshots/pro-l-02-overview-annotated.jpg`](screenshots/pro-l-02-overview-annotated.jpg) | The help file's annotated map of every region: display, gain slider, advanced controls, MIDI Learn, TP limiting, oversampling, dither, in/out options, resize, presets, undo/A-B, help and full screen |
| [`screenshots/pro-l-03-level-display-and-meters.jpg`](screenshots/pro-l-03-level-display-and-meters.jpg) | The output and GR meters, annotated: max peak, current peak, overall and per-channel momentary loudness, current and peak GR, and readouts you click to reset |
| [`screenshots/pro-l-04-display-modes.jpg`](screenshots/pro-l-04-display-modes.jpg) | The Display Mode pop-up: Slow Down, Fast, Slow, Infinite and Off |
| [`screenshots/pro-l-05-loudness-metering.jpg`](screenshots/pro-l-05-loudness-metering.jpg) | The loudness panel, annotated: max TP, max M/S, meter scale "EBU +9", target "−23 (EBU)", LRA, the M/S/I meters, the LUFS reading, the time-scale button and the loudness curve on the display |
| [`screenshots/pro-l-06-true-peak-limiting.jpg`](screenshots/pro-l-06-true-peak-limiting.jpg) | The bottom bar: the True Peak Limiting toggle (green light), the TP metering button, the display mode, the Loudness scale, and Oversampling and Dither both Off |
| [`screenshots/pro-l-07-advanced-panel-style.jpg`](screenshots/pro-l-07-advanced-panel-style.jpg) | The Advanced panel: the Style selector button (Modern), Lookahead 0–5 ms, Attack 0–10 s, Release 0–10 s, and Channel Linking for Transients and Release at 0–100 % |
| [`screenshots/pro-l-08-compact-view.png`](screenshots/pro-l-08-compact-view.png) | Compact mode (press kit): Gain 0–30 dB, Output −30…0 dBTP, horizontal meters, a large loudness bar, and the DC / side-chain / 1:1 / audition icons in the bottom bar |
| [`screenshots/pro-l-09-output-options.jpg`](screenshots/pro-l-09-output-options.jpg) | The Output pop-up: the Output Level knob plus the DC, side-chain trigger, 1:1 (Unity Gain) and headphones (Audition) buttons |
| [`screenshots/pro-l-10-oversampling-menu.jpg`](screenshots/pro-l-10-oversampling-menu.jpg) | The oversampling menu: Off, 2x, 4x, 8x, 16x (CPU-intensive) and 32x (very CPU-intensive) |
| [`screenshots/pro-l-11-dither-noise-shaping.jpg`](screenshots/pro-l-11-dither-noise-shaping.jpg) | The Dither pop-up: bit depth plus noise shaping, shown as "16 Bits (O)" |
| [`screenshots/pro-l-12-surround-channel-linking.jpg`](screenshots/pro-l-12-surround-channel-linking.jpg) | Surround channel linking, with the extended "ALL" range and the +C and +LFE toggles |

FabFilter publishes no image of the Style menu open. The Style button in #07 opens a plain list of the eight
styles.

---

## 1. Version, release and what changed

### Lineage

| Version | Date | Notes |
|---|---|---|
| Pro-L (v1) | announced 21 Oct 2010 | Four styles (Transparent, Punchy, Dynamic, Allround), up to 4x oversampling, the scrolling level display. Still downloadable as a legacy plug-in. |
| **Pro-L 2.00** | **5 Dec 2017** | A major update. Launch price EUR 169 / USD 199 / GBP 149. It installs alongside v1 and opens v1 presets, but **v1 automation does not carry over**. |
| 2.01-ish | 15 Feb 2018 | The TP detector now matches the ITU BS.1770-4 example algorithm, so it can read slightly high but under-reads less. This eases MFiT round-trip tools. It also fixed the integrated loudness resetting after a Logic bounce. |
| (2.1x) | Mar 2023 | Loudness **Auto-Reset** on playback start, added after a forum request (see §6). |
| (2.1x) | 6 Jul 2023 | Atmos 5.1.2, 5.1.4, 7.1.4, 9.1.4 and 9.1.6 added to the AAX plug-in. |
| (2.2x) | 19 Dec 2023 | **CLAP** format; more Atmos layouts (7.0.4, 7.0.6, 7.1.6, 9.0.4, 9.0.6); ITU and Dolby 9.x.x layouts in VST3. A third-party tracker lists iOS 2.22 on 20 Dec 2023 with "Export Preset Folder" and Auto-Reset. |
| (2.2x) | 17 Jun 2024 | The 5.1.2 layout in VST3. |
| (2.25) | 19 Mar 2026 | For all plug-ins: sizing and scaling fixes (Logic especially), **drag-and-drop preset files**, and **Copy/Paste** in the preset right-click menu. |
| **2.26** | **16 Apr 2026** | The latest build. It fixes the red GR curve in the display drawing without its fill, and lets Copy/Paste work from earlier plug-in versions. |

The version numbers in parentheses are inferred from the order of releases. FabFilter's news posts announce
updates for "all plug-ins" and do not give each product's version number.

**Is there a Pro-L 3?** No, not as of 23 Sep 2026. The products page and the download page both list Pro-L 2
2.26. FabFilter's recent major releases went to other products: Pro-Q 4 (Dec 2024), Pro-C 3 (15 Jan 2026) and
the Pro-Q 4.10 multi-plug-in instance list (Jan 2026), which covers Pro-Q 4, Pro-C 3, Pro-G and Pro-DS but not
Pro-L. On the forum in Dec 2024, co-founder Frederik asked users what they would want in a Pro-L update. In Sep
2025 staff said they never pre-announce releases. **Expect a Pro-L 3 at some point.** Its likely themes are the
ones users keep asking for: multiband, M/S, loudness history and low latency.

### What v2 added over v1 (from the manual and the press release)

- **Four new styles:** Modern (the new default), Aggressive, Bus and Safe. v1's Transparent, Punchy, Dynamic and Allround remain.
- **True Peak Limiting**, a new mode that holds the output at or below the ceiling in dBTP.
- **Loudness metering** to ITU-R BS.1770-4, EBU R128, ATSC A/85 and TR-B32: momentary, short-term and integrated loudness, LRA, TP, and selectable targets and scales.
- **Oversampling up to 32x**, linear-phase (v1 stopped at 4x). v2 also fixed v1's habit of slightly overshooting the ceiling when oversampling.
- **New display modes** (Slow Down, Slow, Infinite, Off) and **peak labels** on the largest GR events.
- **Surround and Atmos support** (7.1.2 at launch, 9.1.6 today) with surround-aware channel linking.
- A **DC offset filter**, **external side-chain triggering** (for stem mastering), **Unity Gain** and **Audition Limiting** (a delta signal).
- An **AudioSuite "Analyze"** option in Pro Tools, which measures material at the current settings without rendering.
- A new factory preset library and a redesigned interface.

Sources: [product page](https://www.fabfilter.com/products/pro-l-2-limiter-plug-in), [manual PDF](https://www.fabfilter.com/downloads/pdf/help/ffprol2-manual.pdf), [press release (Dec 2017)](https://www.fabfilter.com/press/1512462600/fabfilter-releases-fabfilter-pro-l-2-true-peak-limiter-plug-in), [Feb 2018 update](https://www.fabfilter.com/press/1518681600/fabfilter-updates-all-plug-ins), [Jul 2023 update](https://www.fabfilter.com/news/1688634000/all-plug-ins-updated-extended-dolby-atmos-aax-support-for-pro-q-3-and-pro-l-2), [Dec 2023 update](https://www.fabfilter.com/news/1702983600/all-plug-ins-updated-with-clap-support-bug-fixes-and-more), [Jun 2024 update](https://www.fabfilter.com/news/1718618400/all-plug-ins-updated-with-bug-fixes), [Mar 2026 update](https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes), [Apr 2026 update](https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins), [download page (2.26)](https://www.fabfilter.com/download), [Pro-Q 4.10 instance list](https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410), [forum "When Pro-L3???"](https://www.fabfilter.com/forum/topic/7874/when-pro-l3), [forum "Pro L3 soon?"](https://www.fabfilter.com/forum/topic/8248/pro-l3-soon).

---

## 2. Feature list and parameters

FabFilter publishes almost no numeric defaults. The ranges below come from the knob legends in the official
screenshots (#07 and #08) and from the manual. "Default (inferred)" means the value can be read from the official
*Default Setting* screenshot (#05).

### 2.1 Main controls

| Parameter | Range / values | Unit | Default | Behaviour |
|---|---|---|---|---|
| **Gain** | 0 … +30 | dB | 0 dB presumably (not documented) | Drives the signal into the ceiling: the threshold and the make-up gain move together. It is a large vertical slider over the left edge of the display (Compact view: a knob). **Alt-drag** (Shift in Pro Tools) moves Output the opposite way, for level-matched listening. |
| **Output Level** (ceiling) | −30 … 0 | dB, or dBTP when TPL is on | 0.0 (factory presets are saved at 0.0) | The maximum output. It sits behind the "Out: x dB" button in the bottom bar: drag the button vertically, double-click it to type, or click it to open a pop-up with a large knob. |
| **Style** | Transparent, Punchy, Dynamic, Allround, Aggressive, Modern, Bus, Safe | — | **Modern** | The limiting algorithm (§3). |
| **Lookahead** | 0 … 5 | ms | not documented; low (visible in #01) | Look-ahead for the fast *transient* stage. Below about 0.1 ms the limiter approaches hard clipping. |
| **Attack** | 0 … 10 | s | not documented | How soon the slower *release* stage engages. Shorter attack brings the release stage in sooner. |
| **Release** | 0 … 10 | s | not documented | How long the release stage keeps acting. The knob taper looks non-linear. |
| **Channel Link – Transients** | 0 … 100 | % | not documented (the manual suggests below 100 %) | Stereo linking for the transient stage, where short one-channel peaks are best removed from that channel only. |
| **Channel Link – Release** | 0 … 100 | % | not documented (the manual suggests starting at 100 %) | Stereo linking for the release stage. |
| Surround link extension | "0 % Stereo" … "100 % Stereo, 100 % All" | % | C and LFE **off** | In surround, the second half of each knob's travel blends pair-linking (L/R, Ls/Rs …) into all-channel linking. **+C** links C to L/R (and Cs to Ls/Rs); **+LFE** includes the LFE in all-channel linking. |

### 2.2 Bottom bar and output options

| Option | Values | Notes |
|---|---|---|
| **True Peak Limiting** | on / off | On in the *Default Setting* (inferred: the out readout says dBTP). It catches true peaks already in the input, then smooths away any TP overshoot the fast limiter itself creates. It adds about **5 ms** of latency. |
| **Oversampling** | Off, 2x, 4x, 8x, 16x, 32x | Linear-phase, so it can add slight pre-ringing. The manual recommends 4x or 8x; 16x and 32x are "for offline". 4x plus a lookahead of at least 0.1 ms keeps inter-sample peaks within about 0.1 dB. Off in the *Default Setting* (inferred). |
| **Dither** | Off, 24, 22, 20, 18, 16 bits | TPDF, 2-bit peak-to-peak. Off in the *Default Setting*. |
| **Noise Shaping** | Basic, Optimized, Weighted | Basic lowers the floor a few dB but raises noise above 6 kHz. Optimized goes further, pushing noise above 10 kHz. Weighted is shaped to the ear at low listening level and is tuned for 44.1 kHz. The button shows it as a letter, e.g. "16 Bits (O)". |
| **Filter DC Offset** | on / off | A gentle DC filter, so a DC bias does not cause asymmetric limiting. |
| **Side Chain Triggering** | on / off | The external side chain drives detection. Use it for **stem mastering**: put Pro-L on each stem and feed the full unlimited mix to the key input, so every stem receives the master's exact GR. |
| **Unity Gain** (1:1) | on / off | Sets the output to the inverse of Gain, so you hear the limiting without the loudness increase. The meters turn **yellow** but keep showing levels *as if* the normal output applied. Meant to be temporary. |
| **Audition Limiting** (headphones) | on / off | Plays the delta: input minus output, i.e. what the limiter removes. |
| **Global Bypass** | on / off | Latency-compensated and soft (no clicks). The display dims and a red light shows. |
| **Lock Output** | on / off (top bar) | When loading presets, keeps the current Gain, Output, DC, Unity Gain, TPL, Oversampling, Dither and Noise Shaping. |
| MIDI Learn | toggle + menu | Enable MIDI, Clear (single or all), Revert, Save. |
| Resize | Compact, Small, Medium (default), Large, plus a Scaling sub-menu | VST3 and CLAP resize freely. Full Screen is a separate button at the top right. |

### 2.3 Metering options

| Option | Values |
|---|---|
| **Meter Scale** | 16 dB, 32 dB, 48 dB (linear in the top part); K-12, K-14, K-20; **Loudness** (opens the loudness panel) |
| **TP metering** (TP button) | on / off. On shows the output meter and readout as true peak (the readout turns **green**), **orange** above the ceiling and **red** above 0 dBTP |
| **Display Mode** | Slow Down, Fast (v1 behaviour), Slow, Infinite, Off |
| **Loudness Target** | −9 (CD), −14 (streaming: Spotify, iTunes, YouTube), −23 (EBU R128), −24 (ATSC A/85 / TR-B32), Custom |
| **Loudness Time Scale** | Momentary, Short Term, Integrated. The chosen one is drawn large; the other two as thin bars |
| **Loudness Meter Scale** | +9 or +18 LU above the target (the EBU mode scales), absolute or relative (target shown as 0) |
| Max M / Max S | Chooses which maximum shows above the loudness meter in Integrated mode |
| Pause / Reset / **Auto-Reset** | Reset also clears the peak and GR readouts. Auto-Reset resets on host playback start |

Metering settings are **not** changed by preset loads. They are saved with the session, and the last one used
becomes the default for new instances.

### 2.4 Formats, platforms and "goodies"

VST2, VST3, CLAP, AU, AAX Native and AudioSuite, 64-bit and 32-bit, on Windows (Vista to 11) and macOS 10.13+
(Intel and Apple Silicon), plus an **iPad AUv3** with touch gestures: double-tap to reset, second finger for
fine control, long-press to type. Also GPU-accelerated graphics, **Smart Parameter Interpolation** (no zipper
noise), interactive help hints on hover, text entry by double-click with shortcuts (typing `2x` gives +6 dB,
`50%` puts a knob at mid-travel), Pro Tools hardware control surfaces, MIDI Program Change preset loading, and
mono, stereo and surround (up to 9.1.6) adapting automatically.

Sources: [manual PDF](https://www.fabfilter.com/downloads/pdf/help/ffprol2-manual.pdf), [online help](https://www.fabfilter.com/help/pro-l), [advanced settings](https://www.fabfilter.com/help/pro-l/using/advancedsettings), [output options](https://www.fabfilter.com/help/pro-l/using/outputoptions), [dithering](https://www.fabfilter.com/help/pro-l/using/dithering), [oversampling](https://www.fabfilter.com/help/pro-l/using/oversampling), [true peak limiting](https://www.fabfilter.com/help/pro-l/using/truepeaklimiting), [surround](https://www.fabfilter.com/help/pro-l/using/surround), [iOS](https://www.fabfilter.com/help/pro-l/using/ios), [press kit](https://www.fabfilter.com/downloads/press/fabfilter-pro-l.zip).

---

## 3. The eight limiting styles

Pro-L is a **two-stage limiter**:

1. A fast *transient* stage, governed by Lookahead, that catches short peaks and lets go of them almost at once.
2. A slower *release* stage whose envelope is shaped by Attack and Release.

A third-party analysis puts it this way: peaks shorter than the Attack time are handled by the transient stage,
and longer events engage the release stage. The same analysis says Safe uses only the slower stage. Channel
linking is set **per stage**, and that is a key idea to keep.

| Style | Since | Character, per the manual and reviews | Intended use |
|---|---|---|---|
| **Transparent** | v1 | Stays faithful and avoids pumping and colour | Most material, especially rock and pop. The **lowest-latency** choice |
| **Punchy** | v1 | The most audible style. Pushed, it pumps pleasantly while staying fairly clean | Single tracks (vocals, bass, guitar), beat-driven music |
| **Dynamic** | v1 | Enhances transients *before* limiting, to keep punch and clarity | Rock, and more besides |
| **Allround** | v1 | A balance of loudness and transparency | Almost anything |
| **Aggressive** | v2 | Smart, near-clipping limiting ("light clipping", per MusicTech) | EDM and trance; also rock, metal and pop |
| **Modern** | v2 (default) | Clean at very high loudness, and tolerates near-zero lookahead | The all-purpose default |
| **Bus** | v2 | Deliberately not transparent: glue, pump, squash | Drum buses, individual tracks |
| **Safe** | v2 | Never distorts; does not chase loudness | Acoustic and classical music, delicate sources |

**Latency:** the four v2 styles need considerably more latency than the v1 styles. To minimise latency the manual
says to pick a v1 style and turn off TPL and oversampling. Forum users reported the Bus style at **more than
65 ms** in Logic, and Transparent at **about 12 ms**, against Pro-L 1's 8 ms. In 2024 a user put Transparent with
no lookahead at **496 samples** and asked for a "super low latency mode". FabFilter replied it would look at this
"when Pro-L is getting an update".

Sources: [advanced settings](https://www.fabfilter.com/help/pro-l/using/advancedsettings), [Jonathan Jetter: time constants](https://www.jonathanjetter.com/blog/fabfilter-prol2-timeconstants), [MusicTech review](https://musictech.com/reviews/fabfilter-pro-l-2-review/), [forum: Pro-L2 latency](https://www.fabfilter.com/forum/topic/3116/pro-l2-latency).

---

## 4. The display in detail

See #01, #02, #03, #04 and #05. This display is the core of Pro-L's reputation since v1; a 2014 KVR user review of v1 likened it
to "blood dripping over icebergs".

### What is drawn

- **Input level:** a grey or light-blue filled waveform envelope, scrolling right to left. The newest audio is at the right edge, next to the meters.
- **Output level:** a darker blue fill, inside the input shape.
- **Gain reduction:** a **red** trace hanging down from the 0 dB line. Its depth is the GR at that moment, so the space between input and output reads directly as "what was removed".
- **Loudness:** a thin line (orange in Short Term, red-pink in the Integrated screenshot) plotting the chosen loudness time scale over time. The loudness meter uses its own scale, so **curved connector lines** between the display's right edge and the loudness meter map one scale onto the other. It is clever, but it takes learning.
- **Peak labels:** yellow tags such as "−1.6 dB" pinned to the largest GR events, so you can see *when* and *how hard* the limiter worked without watching live.
- **Gain slider overlay:** a translucent vertical strip at the far left with a red gradient whose height follows Gain, plus a value tag ("+10.5").
- **Vertical axis:** dB labels on the right edge of the display follow the selected meter scale, which is linear and expanded in the top few dB where limiting happens.
- **Bypass** dims the whole display. **Unity Gain** turns the output meters yellow.

### Meters to the right of the display

- **Output meter:** peak and **RMS (momentary loudness)** at once, per channel and combined, with a max-peak hold line. The readout above it (e.g. "−1.0") is green when it shows true peak. Click it to reset.
- **GR meter:** current GR bars in red, with a peak-GR marker. Click the readout above it (e.g. "−6.5") to reset.
- **Loudness panel** (Loudness scale): a large bar for the chosen time scale, with thin bars for the other two (S, M, I). It shows a big LUFS readout that turns red over the target, max M or S above, **LRA** with a bracket (Integrated mode only), the target and scale buttons, and pause, time-scale and reset buttons.
- In **surround**, one meter per channel in logical order, labelled (L C R Ls Rs LFE …), for both output and GR.

### Speeds and modes

**Fast** is a steady scroll, as in v1. **Slow** scrolls slower, with more history on screen, and pairs with
Short Term. **Slow Down** fills left to right and then quickly scrolls back to make room, which makes peak labels
easy to read. **Infinite** keeps everything since the interface opened, compressing the time axis every time it
fills, and pairs with Integrated. **Off** is for listening without watching.

### What you can drag and click

| Target | Interaction |
|---|---|
| Gain slider | Click anywhere and drag vertically; the mouse wheel; slow drags on the background for precision; double-click to type; **Alt-drag** to move Output inversely |
| Knobs | Four modes: vertical drag (speed-sensitive), rotate by grabbing the arrow (moving further out gives finer control), mouse wheel, double-click to type. Ctrl/Cmd-click resets; Shift gives fine control; Alt drags linked knobs. Hovering lights the knob and pops up its value |
| Out button | Drag vertically to set the ceiling without opening the panel; double-click to type |
| Compact view | Drag the value readouts under the Gain and Output knobs |
| Readouts | Click a peak or GR readout to reset it |
| Loudness buttons | Pop-up menus for target, scale and time scale |

**Not possible**, and each is an opportunity for us: scrolling back or zooming the history, freezing the display,
hovering to read a value at a past moment, clicking a peak label to go to that moment, showing time labels on
the axis, or showing a spectrum (Pro-L has no analyser at all, although its help page on full screen mentions a
"spectrum analyzer", apparently copied from Pro-Q). Frozen, scrollable loudness history is the top metering
request on FabFilter's own forum (§6).

Sources: [metering](https://www.fabfilter.com/help/pro-l/using/metering), [loudness metering](https://www.fabfilter.com/help/pro-l/using/loudnessmetering), [knobs](https://www.fabfilter.com/help/pro-l/using/knobs), [overview](https://www.fabfilter.com/help/pro-l/using/overview), [full screen and resize](https://www.fabfilter.com/help/pro-l/using/fullscreenandresize), [KVR user reviews](https://www.kvraudio.com/product/pro-l-2-by-fabfilter/reviews).

---

## 5. Workflow

- **Presets.** A factory library sorted by genre with descriptive names. Prev/next arrows step through them. The name dims once you edit a preset. *Options* offers Save As Default (you can change the default preset), Open Other Preset, Restore Factory Presets, Change Preset Folder, Export Preset Folder, and Enable MIDI Program Changes (presets are numbered by bank and program). Presets are `.ffp` files, identical across platforms, stored in `Documents/FabFilter/Presets/Pro-L 2`, with subfolders as categories. Right-clicking the preset button gives Save, Save As, **Copy** and **Paste** (Copy/Paste reached every plug-in in 2026, and also pastes v1 settings). Preset files can be **dragged onto the interface** (2026).
- **Lock Output** (§2.2) lets you audition presets without the loudness jumping. This is the correct design: choosing a sound and choosing the delivery level are separate decisions.
- **Recommended workflow** (manual): (1) pick a preset or the default, raise Gain; (2) refine Style, Lookahead, Attack, Release and Link in the Advanced panel; (3) set TPL, oversampling, dither and Output last. For delivery, turn on TP metering and TPL and set Output to the spec, e.g. −1 dBTP for EBU R128 or streaming, −2 dBTP for ATSC A/85, and about −0.1 dBTP for CD or iTunes.
- **A/B and undo:** Undo and Redo buttons (every UI change is a step, but automation and MIDI changes are not); **A/B** swaps two states; **Copy** copies the active state to the inactive one and greys out when they match.
- **MIDI Learn:** click the button, touch a control, move a controller. The menu offers enable/disable, clear, revert and save; mappings save automatically.
- **Resizing:** Compact, Small, Medium and Large, plus scaling (e.g. 150–300 % on Retina), remembered per monitor type and separately for full screen. Full screen exits with Esc. VST3 and CLAP resize freely.
- **Compact mode** drops the display in favour of large horizontal meters and a big Output knob, for people who "judge limiting by ear" (#08).
- **Help:** a hover help-hint system (see the product page image) plus a very good manual that reviewers single out.

Sources: [presets](https://www.fabfilter.com/help/pro-l/presets/loadingpresets), [saving presets](https://www.fabfilter.com/help/pro-l/presets/savingpresets), [recommended workflow](https://www.fabfilter.com/help/pro-l/using/recommendedworkflow), [undo/redo](https://www.fabfilter.com/help/pro-l/using/undoredo), [MIDI learn](https://www.fabfilter.com/help/pro-l/using/midilearn), [full screen and resize](https://www.fabfilter.com/help/pro-l/using/fullscreenandresize).

---

## 6. Reception: praise and complaints

### Praised

- **Sound:** transparent and loud, with punch and low end intact. In a shootout at 4 dB of GR on a soul track (against Pro-L 1, UAD Precision Limiter, Ozone 7 and Waves L2), testers found it "deeper" in the sound stage, with better lows and better transient retention. Computer Music gave it 10/10.
- **Workflow and interface:** *Sound On Sound* (Eric James, Apr 2019) found it "highly informative and bling-free" and one of the most efficient limiters to set up, listing **no cons**. *Tape Op* (Garrett Haines) called the loudness display "a godsend". *BPB* (Aug 2018) scored it 92 %.
- **Metering:** a forum thread titled "the metering in Pro-L 2 is the best I've ever seen" asks FabFilter to sell it as a standalone meter, and several users agree.
- **Unity Gain and Audition** are repeatedly cited as learning and honesty tools. The **per-stage channel link** is admired: SOS found 75–80 % on transients a small free improvement.
- **Stem mastering** through the side-chain trigger was praised by MusicTech.

### Complained about, or missing

| Complaint | Source |
|---|---|
| **No knee control** | Computer Music / MusicRadar review (the only con listed) |
| **Latency**: v2 styles are much higher (Bus > 65 ms reported; Transparent about 12 ms against v1's 8 ms), no true low-latency mode, and the latency is **not shown in the UI** | FabFilter forum, 2018 and 2024 |
| **Not multiband, no M/S.** Users ask for a 3-band split with per-band M/S, like DMG Limitless. SOS notes the missing split-band option for very demanding material | FabFilter forum Dec 2024; SOS |
| **TP limiting and oversampling can dull the sound.** One experienced user found it often sounds best with TPL and oversampling *off* and more headroom left instead. Linear-phase oversampling has pre-ringing, which the manual admits | Gearspace thread (via search excerpt); manual |
| **Loudness history** you can **scroll back** through, as in Nugen VisLM | FabFilter forum, Jul 2026 |
| **A meter freeze** (and freeze linked to transport), because Infinite mode rescales while you read it, and an **automatable loudness reset** | FabFilter forum, Jan 2019 (staff: "will look into it") |
| **Transport-based, elapsed metering** for post and broadcast. Auto-Reset shipped in Mar 2023, but only as a reset on playback start | FabFilter forum, 2023 |
| **Dither auto-blanking** on silence, and setting dither on **all instances** at once | FabFilter forum, 2022 |
| A **zero-latency meter-only mode**, or a separate metering plug-in | FabFilter forum, 2020–2023 |
| The RMS/loudness line cannot be made to look like v1's | FabFilter forum, 2020 |
| Pro Tools side-chain limited to mono; the Bus style is too much for mastering; it is **easy to over-limit** because it hides damage so well | MusicTech 2018; Musco 2026 |
| Ambisonics (SOA/TOA) channel counts | [FabFilter forum](https://www.fabfilter.com/forum/topic/3168/pro-l2-soa-toa-support) |
| No spectrum or frequency view of *where* GR happens (our inference; competitors all have one) | — |
| **Ageing:** no major version in almost nine years, while Ozone moved to IRC 5 multiband (2025) | This research |

Sources: [MusicRadar/CM review](https://www.musicradar.com/reviews/fabfilter-pro-l-2), [Sound On Sound](https://www.soundonsound.com/reviews/fabfilter-pro-l-2), [MusicTech](https://musictech.com/reviews/fabfilter-pro-l-2-review/), [Tape Op](https://tapeop.com/reviews/gear/127/pro-l-2-limiter-plug-in), [BPB](https://bedroomproducersblog.com/2018/08/06/fabfilter-pro-l2-review/), [Musco 2026](https://www.michaelmusco.com/2026/03/fabfilter-pro-l-2-review.html), [GearShoot shootout](https://gearshoot.com/reviews/fabfilter-pro-l-2-shootout-review/), [Gearspace: experiences and tests](https://gearspace.com/board/mastering-forum/1193163-fabfilter-pro-l-2-experiences-tests.html), [forum: loudness requests](https://www.fabfilter.com/forum/topic/3441/pro-l-2-loudness-metering-feature-requests), [forum: transport metering](https://www.fabfilter.com/forum/topic/7254/pro-l2-feature-requests-transport-based-lufs-metering-elapsed-metering-auto-metering), [forum: auto-blank dither](https://www.fabfilter.com/forum/topic/6962/feature-request-pro-l2-auto-blanking-dither), [forum: standalone meter](https://www.fabfilter.com/forum/topic/6116/the-metering-in-pro-l-2-is-the-best-ive-ever-seen-fabfilter-should-make-it-into-a-standalone-plugin), [forum: RMS line](https://www.fabfilter.com/forum/topic/6401/pro-l2), [forum: latency](https://www.fabfilter.com/forum/topic/3116/pro-l2-latency), [forum: When Pro-L3](https://www.fabfilter.com/forum/topic/7874/when-pro-l3).

---

## 7. Competitors, and what they have that Pro-L 2 lacks

### iZotope Ozone 12 Maximizer (released 3 Sep 2025)

- **Modes:** IRC Low Latency, IRC 1, 2, 3 (with Clipping, Crisp, Balanced and Pumping styles), IRC 4 (spectral shaping over dozens of psychoacoustic bands; Classic, Modern and Transient styles), and **IRC 5**, its first **multiband** limiter: four bands, each with its own attack and release.
- A continuous **Character** control (0 = clipping … 10 = very slow) instead of separate attack and release.
- **Learn Input Gain**: listens for five seconds and sets Gain to a **Target LUFS**. iZotope warns it is not for compliance.
- **Upward Compress**: level-matched parallel upward compression before the limiter.
- **Soft Clip**: a 4x-oversampled pre-clipper with Light, Moderate and Heavy knees (−3, −9 and −30 dBFS) and an amount control.
- **Transient Emphasis**, applied before limiting.
- **Stereo Independence** split into **Transient** and **Sustain**, the same idea as Pro-L's per-stage link.
- **Gain/Output link**, **Delta**, and views for Spectrum, Gain Trace (limiter GR from the top, upward boost from the bottom) and GR meter.
- **What Pro-L lacks here:** multiband, a clipper, upward compression, transient emphasis, learn-to-target and a spectrum view.

### DMG Audio Limitless

- **Up to six bands** with a linear-phase, variable-slope crossover, each band with its own gain, mute/solo and release.
- **Multi-stage** processing: two limiter stages (macro dynamics, then transients) plus a **64x-oversampled pre-limiter clipper** with several shapes.
- **Loudness weighting**, to optimise for bass or perceived loudness.
- Integrated R128 metering with **loudness history and a histogram**; zoomable PPM and GR meters.
- Views: frequency (FFT with per-band GR), time and loudness.
- **Gain-lock** for auditioning presets, **eight A/B banks**, adjustable noise-shaped dither, and a freely resizable UI.
- **What Pro-L lacks here:** multiband, a clipper stage, loudness history and histogram, per-band views, and more than two A/B slots.

### Weiss MM-1 Mastering Maximizer (Softube; algorithms from the DS1-MK3)

- **One Amount knob** over five **styles** (Transparent, Loud, Punch, Wide, De-ess), each a tuned compressor chain (ratio, times, knee) that progresses as Amount rises, feeding a brick-wall limiter.
- **Parallel Mix**, and a delivery-oriented output menu (−0.1, −0.3, −1 dB).
- M/S processing in Wide and De-ess, and GR metering in L/R or M/S.
- Its **simplicity** is the selling point; many engineers rate the Weiss sound above all digital limiters.
- **What Pro-L lacks here:** a macro "amount" control that moves several parameters musically, compression before limiting, M/S, and parallel mix.

### Also relevant

- **Sonnox Oxford Limiter v3**: an **Enhance** control for perceived loudness, a soft **knee**, a reconstruction ("Recon") TP meter, and **Auto Comp**, a separate stage that dynamically corrects true peaks.
- **Nugen VisLM**: scrollable loudness history, the reference users point to.
- **Newfangled Audio Elevate**: many-band, auditory-scale limiting with a spectral clipper.

Sources: [Ozone 12 Maximizer docs](https://docs.izotope.com/ozone12/en/maximizer.html), [Ozone 12 launch (Synthtopia)](https://www.synthtopia.com/content/2025/09/06/izotope-ozone-12-now-available/), [Inside Ozone 12](https://www.izotope.com/en/learn/inside-ozone-12), [DMG Limitless](https://www.dmgaudio.com/products_limitless.php), [SOS on Limitless](https://www.soundonsound.com/reviews/dmg-audio-limitless), [Weiss MM-1 manual](https://www.softube.com/us/user-manuals/weiss-mm-1-mastering-maximizer), [Sonnox Oxford Limiter](https://sonnox.com/products/oxford-limiter), [Medium: Ozone vs Pro-L 2 at high loudness](https://medium.com/@LukeNew/compare-sound-quality-izotope-ozone-maximizer-vs-fabfilter-pro-l-2-for-very-high-loudness-59963363de6b).

---

## 8. How we beat it

Pro-L 2 wins on three things: a **two-stage limiter with sensible styles**, **one honest picture** (input, output,
GR and loudness on one time axis with peak labels), and **delivery discipline** (TPL, targets, Lock Output, Unity
Gain, Audition). Everything below keeps those three and attacks what it cannot do.

Pro-L's weaknesses split into two groups:
- **Algorithms:** no knee, no clipper, not multiband, no M/S, high and hidden latency.
- **Time:** the display forgets, cannot be scrolled, frozen or queried, and knows nothing of the song.

Cantus has one structural advantage no plug-in vendor has: **we are the host.** The limiter can know the
playhead, the arrangement, the time selection, the render path and its own position in the chain.

Working name: **Limes**, Latin for *boundary* (as in the Roman frontier), in the family of Firmus, Pulsus and
Aequalis.

### P0: parity, and done better (the first release; the limiter is not shippable without these)

1. **A two-stage core with per-stage linking.**
   - Parameters: Gain 0…+30 dB; Ceiling −30…0 dB (dBTP with TP on); Lookahead 0…5 ms; Attack and Release 0…10 s on a log taper; Link *Transients* and Link *Release* 0…100 %.
   - The Alt-drag Gain↔Ceiling link is part of the core.
   - Ship **four clearly different styles** rather than eight overlapping ones: *Clean* (Transparent/Modern), *Punch*, *Loud* (near-clip), and *Safe*. Add *Bus* once the compressor exists.
   - Declare parameters through `EffectParamInfo`, so the AI bridge and automation get them free.
2. **True-peak limiting that is verified, not just claimed.**
   - A TP detector that is ≥4x oversampled and BS.1770-conformant: **BS.1770-5 (Nov 2023)** is the current revision, while Pro-L still cites -4.
   - A post-limiter TP stage, and property tests that sample-peak output never exceeds the ceiling, and TP never exceeds it with TP on, across random and adversarial signals, at 44.1–192 kHz and at every oversampling factor.
   - Publish the test results in the manual; nobody else does.
3. **Oversampling 1x–16x (32x offline) with a phase choice.**
   - **Linear or minimum phase.** This answers the pre-ring complaint, and minimum phase lowers latency.
   - Build it as the shared oversampling utility that STATUS.md's plan (d) already calls for.
4. **Latency as a first-class citizen.**
   - Engine **PDC** first (plan item (c)), with latency reported per style, shown **in the style menu and live in the header** (e.g. "Clean · 1.2 ms").
   - Add a **Zero-latency / Tracking** style (no lookahead, TP off, with a clear quality warning) so Limes can sit on the master while recording. Pro-L has no such mode, and users have asked for one since 2018.
5. **Loudness metering, measured correctly.**
   - Momentary, Short-term, Integrated (gated), LRA, max M, max S and max TP.
   - Conformance-tested against **EBU Tech 3341 and 3342** vectors and the ITU TP test signals (null-test culture: fixtures in `tests/`).
   - Show **all three time scales plus LRA at once**; Pro-L hides LRA outside Integrated mode.
6. **Delivery options.** A DC filter; **Unity Gain** (1:1); **Delta** audition; a latency-compensated soft bypass; **Lock Output** on preset loads; and TPDF dither at 16–24 bits with three noise shapes plus **auto-blank on digital silence** (a forum request Pro-L never shipped).
7. **The time display at Pro-L's level.**
   - Input fill, output fill, a GR curve from the top, a loudness line, and **peak labels** at the largest GR events.
   - Modes: *Follow* (scroll), *Page* (Pro-L's Slow Down) and *Whole song*.
   - Draw it with the analyser-tap and triple-buffer design already planned (plan (b)); the audio thread publishes min/max/GR per pixel-column block and allocates nothing (ADR 0005).
8. **Presets, A/B and undo through the Cantus command bus.** Every edit is host-undoable, grouped per drag. **A/B/C/D** snapshots (Limitless has eight banks; Pro-L has two). Save As Default. Copy/Paste between instances.

### P1: where we beat it (the reasons to choose Limes)

1. **Loudness and GR history keyed to *song time*, not wall-clock time.** This is the single biggest win.
   - The history is stored per project position, so replaying bars 33–48 **overwrites those bars** instead of appending.
   - Scroll and zoom it like the arrangement, with a ruler in **bars and beats**.
   - **Freeze** it; hover to read input, output, GR, M, S and TP at any moment.
   - **Click a peak label and the playhead jumps there.**
   - **Measure the time selection**: select a chorus in the arrangement and get its integrated loudness, LRA and max TP.
   - This covers every open request in FabFilter's forum (scroll-back, freeze, freeze-on-transport, transport-based metering, section metering) and no plug-in can do it, because plug-ins do not own the timeline.
2. **An offline Analyze and a compliance report.**
   - "Analyze song" renders the master chain faster than real time and reports integrated loudness, LRA, max TP, PLR and PSR, plus the **Gain needed to hit the target**. One click applies it.
   - Pro-L offers this only as a Pro Tools AudioSuite option.
   - The same numbers appear in the **export dialog** as a pass/fail badge for the chosen delivery profile, with an optional JSON/CSV report beside the WAV. This ties in idea.md's "export with streaming targets".
3. **Delivery profiles instead of bare numbers.**
   - Profiles such as *Spotify / YouTube / Tidal*, *Apple Music*, *EBU R128*, *ATSC A/85*, *Podcast*, *CD / club*, and *Custom*.
   - Each sets **target LUFS, ceiling in dBTP, TP on, and the meter scale** at once, with the rule stated in plain words. The platform figures must be checked at implementation time, since the standards change, as Pro-L's own manual warns.
   - A **"Hear it on…"** switch plays the master at the level the platform's normalisation would give it (turn-down only, where that is the rule). This shows at once that pushing past target only buys distortion. The screen shows the penalty in dB.
4. **Loudness-matched comparison that does not lie.**
   - Pro-L's Unity Gain only subtracts Gain. Our **Match** button level-matches bypass and each A/B slot using **measured** short-term loudness.
   - **Style shoot-out**: loop the last 8–16 bars through every style, loudness-matched, and show a small table of GR, max TP, crest and LRA per style. You pick by ear with numbers beside you.
5. **A knee and a clipper: the two things reviewers and the competition point at.**
   - A **Knee** control (0 = hard … soft).
   - An optional **pre-limiter clip stage** (soft or hard, oversampled, with a *Clip ↔ Limit* balance so the user decides where the peaks go).
   - Draw clipped regions in a distinct hue on the display, so the user sees them.
6. **Stereo that is modern.**
   - Link in **L/R or M/S**, keeping Pro-L's *per-stage* link (transients vs release, which maps to Ozone's transient vs sustain).
   - A **width-safety readout** that warns when unlinked limiting narrows the image, measured as a correlation change between input and output.
7. **An honest distortion indicator.**
   - When lookahead drops below about 0.1 ms, or the transient stage behaves like a clipper, tint the GR curve and raise a small "clipping-like" badge.
   - Show **the added TP overshoot** the limiter created, which Pro-L's manual explains but never shows.
8. **Chain awareness, possible only because we are the host.**
   - A warning chip if **dither is on but Limes is not the last thing before the output**, or if something after it changes gain.
   - One-click *"make this the master's last slot"*.
   - Dither settings shared across instances (a forum request).
9. **Frequency insight.** A thin spectrogram strip, or GR by band, under the time display (the analyser tap is already planned), so the user sees *which* frequencies trigger the limiting. Pro-L has no analyser.

### P2: later

- **Multiband mode** (2–4 bands, linear-phase crossover, per-band link and release): the most requested Pro-L 3 feature.
- Lossy-codec preview with **post-codec TP**.
- A **Metering-only mode** at zero latency (FabFilter users asked for this for years) as a separate "Metrum" meter plug-in sharing the same code.
- Surround and Atmos linking when Cantus has multichannel buses.
- MIDI Learn once host-wide MIDI mapping exists.

### Interface and UX principles (where we look and feel better)

1. **One canvas with nothing covering it.**
   - Pro-L's Advanced panel slides *over* the display, and its most important delivery options (DC, SC, 1:1, audition) hide in a pop-up behind tiny text in the bottom bar.
   - Ours: the display on top; a **single control row** below it (Gain, Ceiling, Style, Lookahead, Attack, Release, Knee, Link T/R); a **delivery strip** (Profile, TP, OS, Dither, 1:1, Δ, Match) always visible and in legible type; meters on the right.
   - Compact mode is the same layout with the display collapsed, not a different UI.
2. **The thing you drag is the thing you change.**
   - Drag the **ceiling line** in the display up and down.
   - Drag the **gain handle** on the display's left edge (a Pro-L idea worth keeping).
   - Drag the **target line** on the loudness meter.
   - Hover shows value and unit; Shift gives fine control; double-click types (support Pro-L's clever `2x` = +6 dB and `50%` shortcuts); Ctrl/Cmd-click resets. Use the house knob and bar from STATUS.md item 1.
3. **Separate the traces.** GR in a mirrored lane with its own numeric scale, so it is never lost behind the waveform, and loudness drawn **on the same dB axis** where possible. This avoids Pro-L's curved connector lines, which are beautiful but take learning.
4. **Themes and accessibility.**
   - Cantus defaults to light with an editable palette; Pro-L is dark-only.
   - Pick GR and over-target colours that pass the theme editor's AA and colour-blind checks, and never encode pass/fail with red/green alone: add a shape or a text label.
   - Every control reachable by keyboard and announced through AccessKit.
5. **Motion with meaning** (STATUS.md item 4). Meters at 60 fps with correct ballistics, peak labels that fade in over 120 ms, and nothing else moving. An "Off / Frozen" state that is obvious.
6. **Explain inline.** Hover hints (FabFilter's help hints are loved) in **English and Spanish** from the start, per ADR 0007. Every delivery profile shows its rule in one sentence.

### Engineering checklist (for the ADR that will follow)

- **Null tests:** with Gain 0 dB and the ceiling above the signal, the output equals the input delayed by the reported latency, bit-exact without dither. Bypass is sample-aligned.
- **Property tests:** the ceiling is never exceeded (sample peak, and TP with TP on); the reported latency equals the measured latency for every style and oversampling combination; zero allocations on the audio thread.
- **Conformance:** EBU Tech 3341 and 3342, and the ITU TP test vectors, stored as fixtures.
- **Schema:** the new effect kind bumps the project format with a fixture (per the workflow habits); undo fuzzing covers its parameters.

---

## 9. Sources

**FabFilter (official):** https://www.fabfilter.com/products/pro-l-2-limiter-plug-in · https://www.fabfilter.com/help/pro-l · https://www.fabfilter.com/downloads/pdf/help/ffprol2-manual.pdf · https://www.fabfilter.com/downloads/press/fabfilter-pro-l.zip · https://www.fabfilter.com/press/1512462600/fabfilter-releases-fabfilter-pro-l-2-true-peak-limiter-plug-in · https://www.fabfilter.com/press/1518681600/fabfilter-updates-all-plug-ins · https://www.fabfilter.com/press/1287648000/fabfilter-announces-pro-l-limiter-plug-in · https://www.fabfilter.com/download · https://www.fabfilter.com/news · the news posts linked in §1 · the online help pages linked in §2–5.

**FabFilter forum:** topics 7874, 8248, 3441, 3116, 6401, 7254, 6962 and 6116 (links in §1 and §6).

**Reviews:** Sound On Sound (Apr 2019) https://www.soundonsound.com/reviews/fabfilter-pro-l-2 · MusicRadar / Computer Music (May 2018) https://www.musicradar.com/reviews/fabfilter-pro-l-2 · MusicTech (Aug 2018) https://musictech.com/reviews/fabfilter-pro-l-2-review/ · Tape Op https://tapeop.com/reviews/gear/127/pro-l-2-limiter-plug-in · Bedroom Producers Blog (Aug 2018) https://bedroomproducersblog.com/2018/08/06/fabfilter-pro-l2-review/ · Musco (Mar 2026) https://www.michaelmusco.com/2026/03/fabfilter-pro-l-2-review.html · GearShoot https://gearshoot.com/reviews/fabfilter-pro-l-2-shootout-review/ · KVR https://www.kvraudio.com/product/pro-l-2-by-fabfilter/reviews · Jonathan Jetter https://www.jonathanjetter.com/blog/fabfilter-prol2-timeconstants · Gearspace (behind Cloudflare, so seen through search excerpts only) https://gearspace.com/board/mastering-forum/1193163-fabfilter-pro-l-2-experiences-tests.html.

**Competitors and standards:** https://docs.izotope.com/ozone12/en/maximizer.html · https://www.synthtopia.com/content/2025/09/06/izotope-ozone-12-now-available/ · https://www.dmgaudio.com/products_limitless.php · https://www.soundonsound.com/reviews/dmg-audio-limitless · https://www.softube.com/us/user-manuals/weiss-mm-1-mastering-maximizer · https://sonnox.com/products/oxford-limiter · https://www.itu.int/rec/R-REC-BS.1770 (BS.1770-5, 11/2023).
