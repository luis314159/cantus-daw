# FabFilter Pro-G: product research for the Cantus gate / expander

*Researched 2026-09-23. Subject: **FabFilter Pro-G 1.42**, the current version. All facts are paraphrased from the sources tagged in brackets; the full list is at the end. Anything marked **(unverified)** is not in an official document and should be checked in the 30-day trial. Companion reports: [pro-mb.md](pro-mb.md) (which holds the full "one dynamics device or four?" argument, §10), [pro-ds.md](pro-ds.md) and [pro-c.md](pro-c.md).*

---

## 0. Summary

- **Pro-G is still version 1.** It was announced on **9 May 2011** with five styles; **Ducking** was added in 1.01 (October 2011). The current build is **1.42, 16 April 2026**. The only feature of the last decade is **control from Pro-Q 4's Instance List** (January 2026). [press-2011] [press-2011b] [download] [news-instlist]
- **Price:** EUR 169 today; it was EUR 129 at launch. [shop] [press-2011]
- **What it is known for:**
  - **Six tuned styles**: Classic, Clean, Vocal, Guitar, Upward, Ducking.
  - **Lookahead that preserves attacks.** Tape Op calls it the most powerful feature, one others got wrong.
  - A real-time **input and output level display with the transfer curve drawn over it**.
  - An **Expert** panel with a side-chain filter and a free L/R or M/S detector matrix.
  - Gating from **MIDI notes**.

  [help-time] [tapeop] [help-metering] [help-expert]
- **Weak spots we can exploit.**
  - The display is **read-only**: you cannot drag the threshold or the curve.
  - You **cannot hear what the gate removes**.
  - MIDI can open the gate but **cannot hold it closed**.
  - The stereo detector knobs are **awkward**.
  - Hysteresis is fixed per style.
  - There is no hit and bleed classification; Oxford Drum Gate stays ahead on bleed.
  - There is no frequency-selective gating, no surround, and it is smaller than its siblings.

  [musicradar] [plap] [gs-search]

---

## 1. Screenshots saved

Official FabFilter images from fabfilter.com. Each file was checked as a valid JPEG larger than 5 KB, and the main ones were opened and looked at.

| File | What it shows | Source |
|---|---|---|
| `screenshots/pro-g-01-main-interface-expert.jpg` | The current UI (1280×978), "Classic Beats" preset, with **Expert open**: the level display with its transfer curve, Threshold, Ratio, Range, Style, Attack, Release, Hold, Knee and Lookahead, then the side-chain gain/mix pairs, the SC filter slider, Audition, and Wet and Dry with pan. | [help-about] |
| `screenshots/pro-g-02-annotated-overview.jpg` | The UI with hand-written labels on each area. | [help-overview] |
| `screenshots/pro-g-03-level-display-transfer-curve.jpg` | Close-up of the display: input (dark blue) behind output (light blue) on a fixed 60 dB scale, the white transfer curve with its dashed threshold crosshair, and the meter. | [help-metering] |
| `screenshots/pro-g-04-expert-sidechain-routing.jpg` | The Expert strip: L and R **Gain / Mix** knobs, In/Ext, the 48 dB/oct HP/LP slider with its triangular handles, Audition, and **Wet / Dry** levels with pan. | [help-expert] |
| `screenshots/pro-g-05-style-and-time-controls.jpg` | The Style button, Attack (0 ms–1 s), Release (0 ms–5 s), Hold (0–250 ms), Knee (0–30 dB) and Lookahead (0–10 ms) with its on/off button. | [help-time] |
| `screenshots/pro-g-06-pro-q4-instance-list.jpg` | **Newest feature (Jan 2026):** a Pro-G instance tile ("Guitar" style) edited inside Pro-Q 4's Instance List. | [help-instlist] |

---

## 2. Version history (only what matters for design)

| Date | Version | Change | Source |
|---|---|---|---|
| 9 May 2011 | 1.0 | Launch: five styles (Classic, Clean, Vocal, Guitar, Upward); threshold, ratio, range, knee, attack, release, hold and lookahead; mono, stereo or M/S; up to 4× linear-phase oversampling; zero-latency mode; external side chain; MIDI triggering; separate wet and dry; an Expert mode that **resizes the window** to show the side-chain controls. EUR 129 / USD 174 / GBP 114. | [press-2011] |
| 7 Oct 2011 | 1.01 | **Ducking** style: inverted gating, for voice-over ducking and pumping. | [press-2011b] |
| Dec 2015 | — | **Circular side-chain level meter around Threshold**; higher resolution in the level display; Retina and High DPI. | [press-2015] |
| Dec 2016 | — | The Threshold meter now shows the level **after** the Expert side-chain gains. | [press-2016] |
| Feb 2018 | — | Fixed a crash on Windows at 150 % scaling. | [press-2018] |
| **15 Jan 2026** | 1.4x | **Instance List compatibility** with Pro-Q 4.10. | [news-instlist] |
| 19 Mar 2026 | — | Fixed a crash on closing the UI with a text field open (Windows). For all plug-ins: preset drag-and-drop, **Copy / Paste**, and sizing fixes. | [news-mar] |
| **16 Apr 2026** | **1.42** | Small bug fixes. The macOS installer lets you pick formats. | [news-apr] [download] |

**What changed in the latest version:** nothing specific to Pro-G. The last real feature is the **Instance List** tile.

---

## 3. Complete feature list with parameters

Ranges come from the manual and from the labels printed on the official screenshots (marked *[shot]*). FabFilter does not publish defaults.

### 3.1 Dynamic controls (left)

| Parameter | Range | Notes |
|---|---|---|
| Threshold | **−60 dB … 0 dB** *[shot]* | Where the gate opens. Since 2015 a circular SC meter surrounds the knob. **Upward style uses its own Threshold and Ratio parameters with smaller ranges** (the numbers are not published). [help-dyn] [help-time] |
| Ratio | **1:1 … ∞:1** *[shot]* | Expansion below the threshold. 4:1 means 4 dB of cut per dB below. Above about 5:1 it behaves more and more like a gate. [help-dyn] |
| Range | **0 … 100 dB** *[shot]* | The floor, or maximum cut. At 100 dB there is no limit: a full gate. [help-dyn] [feature hint image] |

### 3.2 Style, time and knee (right)

| Parameter | Range / values | Notes |
|---|---|---|
| Style | **Classic, Clean, Vocal, Guitar, Upward, Ducking** | Classic: vintage console-strip character, good for drums. Clean: minimal flutter and distortion. Vocal: opens gently on breaths and releases gently. Guitar: follows the natural decay before distortion, best at 2:1–5:1. Upward: amplifies above the threshold. Ducking: an inverted gate for voice-over and pumping. [help-time] |
| Hysteresis | **Built in per style; not adjustable** | The gate closes a little below the opening threshold to avoid chatter. [help-time] |
| Attack | **0 ms … 1 s** *[shot]* | Programme-dependent; very fast times are possible. [help-time] |
| Release | **0 ms … 5 s** *[shot]* | Time to reach full cut; programme-dependent. [help-time] |
| Hold | **0 … 250 ms** *[shot]* | The minimum time the gate stays fully open. [help-time] |
| Knee | **0 … 30 dB** *[shot]* | A soft knee below the threshold; drawn on the transfer curve. [help-time] |
| Lookahead | **0 … 10 ms**, with an on/off button | "Pre-open". **When on, latency is 10 ms**, plus a little for oversampling. Zero latency when off and without oversampling. [help-time] |

### 3.3 Expert mode (the window grows downward)

| Parameter | Range / values | Notes |
|---|---|---|
| Side chain | **In / Ext**, plus **Audition** | Audition plays the **side-chain (key) signal**, not the removed signal. [help-expert] |
| SC filters | **HP and LP, 48 dB/oct** | Drag the triangles, or double-click to type. **At the far ends they are bypassed.** Alt-drag, or dragging the lit area between them, moves both. *[shot]* shows 56.48 Hz to 5 kHz. [help-expert] |
| SC Gain L / R | **−36 … +36 dB** *[shot]* | One knob per detector channel. |
| SC Mix rings L / R | **L ↔ R** (or **M ↔ S** in Mid/Side) | Sets which input each detector hears. Both centred means fully linked; hard left and right means unlinked; crossing them cross-links (for example Mid keys the Side). Alt links the two rings. [help-expert] |
| Wet level / pan | **OFF … +36 dB** *[shot]* | [help-expert] |
| Dry level / pan | **OFF … +36 dB** *[shot]* | Parallel gating: "dilute" the effect. [help-expert] |
| Expert off | — | With Expert off, internal key, fully linked, no filter, 100 % wet and 0 % dry. **Hiding the section means it is really disabled.** [help-expert] |

### 3.4 Global

- **Channel mode:** Left/Right or **Mid/Side**. In M/S, the internal left channel processes Mid and the right processes Side, and the in/out pan rings pan between Mid and Side. [help-ms] [help-output]
- **Oversampling:** Off / 2× / 4× (linear phase); useful for very short attack or release times at high ratio and range. [help-os]
- **MIDI triggering:** while any note is held, the detector acts as if a 0 dB signal were present, so the gate is **fully open**. It is turned off with *Enable MIDI* in the MIDI Learn menu. **There is no mode to hold the gate closed.** [help-expert] [musicradar]
- **Input / Output** level and pan, and a soft, latency-compensated **global bypass**. [help-output]
- **Sizes:** Small, Medium (default), Large, Extra Large, plus Scaling, Full Screen, and free resizing in VST3. Expert adds height. [help-resize]
- **Formats:** VST 2/3, AU, AAX Native, AudioSuite, CLAP, iPad AUv3. Mono and stereo only. [product]
- **Instance List tile** in Pro-Q 4.10+, with Threshold, Range, Ratio, Style, Attack, Release, Hold, Knee and Lookahead at high zoom. [help-instlist] [shot 06]
- **The usual FabFilter features:** MIDI Learn, undo/redo, A/B, Smart Parameter Interpolation, help hints, control-surface support, and factory presets for common jobs. [product]

---

## 4. The display, in detail

### 4.1 What is drawn

- **A scrolling level history.** The **output (light blue) is drawn over the input (dark blue)** on a **fixed 60 dB scale**. Where the two differ, the gate is cutting, so open and closed are visible at a glance, as are drum hits against bleed (Tape Op). [help-metering] [tapeop]
- **The transfer curve** is drawn over the history as a white line: input level across the bottom, output up the right. It reflects Threshold, Ratio, Range and Knee, and a **dashed crosshair marks the threshold**. A thin grey line from the Threshold knob into the display ties the knob to the curve. [help-metering] [shot 03]
- **A meter** next to the display shows **output against input** in one bar, with a peak readout that resets on click and a clip light. [help-metering]
- **A toggle** at the bottom right hides the moving display if it is distracting. [help-metering]
- **The Threshold ring meter:** the side-chain level after the Expert gains. [press-2016]

### 4.2 What can be dragged, and how

- **Knobs and the SC filter handles only.** The standard FabFilter gestures apply: vertical drag, arrow-rotate, wheel, double-click text entry (`1k`, `A4`, `2x`, `50%`), Ctrl/Cmd-click to reset, Shift for fine steps, and Alt for linked pairs such as in/out or the two mix rings. [help-knobs] [help-expert]
- **The display and transfer curve cannot be dragged.** The help documents no threshold dragging, no clicking into the history, and no freezing. [help-metering]

---

## 5. Workflow

- **Presets.**
  - Factory presets by task (for example "Classic Beats"). Previous/next arrows; the name dims when edited.
  - Presets are `.ffp` files in `Documents/FabFilter/Presets/Pro-G`.
  - Options menu: Save As Default, Open Other Preset, Restore Factory Presets, and MIDI Program Change / Bank Select.
  - The right-click menu has Save, Save As, and Copy / Paste (since March 2026). Preset files can be dragged onto the UI.
  - There is no search or tags.

  [help-presets] [news-mar]
- **Undo / Redo / A/B / Copy.** Changes from automation or MIDI are not recorded. [help-undo]
- **MIDI Learn** follows the standard FabFilter flow. The *same* MIDI input is used for triggering, so a host that sends all MIDI to effects can trigger the gate by accident; *Enable MIDI* turns both off. [help-midi] [help-expert]
- **Resizing and Full Screen:** see §3.4.

---

## 6. What reviewers and users say

### Praise

- **The styles and lookahead are the killer features.** Computer Music (July 2011) came in doubting that a gate needs styles and left convinced. Tape Op (James Neil, #158, Nov/Dec 2023) calls lookahead the most powerful feature, "figured out" where others fumbled it, and names Pro-G the **new benchmark**. [musicradar] [tapeop]
- **The display shows hits against bleed at once;** usually only the threshold needs adjusting. The SC EQ helps zero in on the source. [tapeop]
- **Pros and beginners alike.** MusicTech (July 2011) calls it a joy to use. Studio users (Drumsound & Bassline Smith) put it on every project and love the animated display. [product]
- **The Produce Like A Pro drum-gate shootout** (Warren Huart, 27 Mar 2026) placed Pro-G **a close second to Oxford Drum Gate**: clean and precise on kick and snare, flexible beyond its presets. [plap]

### Complaints

- **You cannot monitor the gated-out signal.** Audition only plays the key. [musicradar]
- **You cannot force the gate closed via MIDI.** [musicradar]
- **Awkward stereo routing knobs**, and **M/S controls that are not intuitive.** [musicradar]
- **Bleed quality.** In the 2026 shootout some upper-mid and high bleed stayed audible once compression and EQ were added; Oxford's suppression was more controlled. [plap]
- **Size.** A Gearspace user wishes it were as large as the other FabFilter plug-ins. [gs-search]

---

## 7. Competitors: what they have that Pro-G lacks

| Product | What it has that Pro-G does not |
|---|---|
| **Sonnox Oxford Drum Gate 2** (2026) | **Detection by timbre, not level**: a classifier trained on kicks, snares, toms and hi-hats that tells ghost notes from bleed. **Exports the hits as MIDI** with their dynamics, sample-accurate. **Time, polarity and phase alignment** of multi-mic kits. **Adaptive Resonant Decay** separates the drum's ring from spill. A **Leveller** for loud and soft hits, and **Hit Clip**. Audio and MIDI external keys. [sonnox-dg] |
| **sonible smart:gate** | **AI target detection**: pick "vocal", "speech", "snare" and so on, and it gates on that source inside a mix. **Gates only specific frequency regions** based on its spectral analysis. A Ducking mode. [pe-gates] [plap] |
| **Boz Digital Gatey Watey** | **Frequency-selective gating** with HP and LP modes: only the part of the spectrum where the bleed lives is gated. [pe-gates] |
| **Unfiltered Audio G8** | **Sends the rejected signal to separate outputs**; **Cycle and One-Shot** envelope modes; MIDI triggering. [pe-gates] |

---

## 8. How we beat it: proposals for the Cantus gate / expander

**Principle.** Pro-G's sound, styles and lookahead are the bar to meet. Its **interface is the weakest of the FabFilter dynamics line**: a read-only display, a hidden detector matrix, and no way to hear what is removed. We keep the sound model, **make the display the control**, and use the fact that **Cantus owns the timeline and the drum instrument**. It is built as the **Gate face of the shared Dynamics device** (pro-mb.md §10). Cantus habits apply: a null test first, an ADR per decision, schema plus fixture, undo fuzz, and EN and ES strings.

### P0: parity, plus the fixes reviewers asked for

1. **The parameter set, at Pro-G's level or above [M].**
   - Threshold −80…0 dB; **a separate Close threshold** (hysteresis) that follows the style by default and can be unlocked and dragged.
   - Ratio 1:1…∞:1 as a number; Range 0 dB…∞ (floor).
   - Attack 0.01 ms–1 s, **Hold 0–500 ms**, Release 1 ms–5 s, Knee 0–30 dB, Lookahead 0–10 ms under the shared max-lookahead latency cap.
   - Styles as **named, documented envelope presets**: Classic, Clean, Vocal, Guitar, Upward and Ducking. Each style says in one line what it changes.
   - Null test: bit-identical at Range 0 dB, and at the threshold floor with lookahead off.
2. **A display you can touch [M].**
   - The scrolling history keeps Pro-G's clear "output over input" idea.
   - Below it, add an **open / hold / release / closed state lane** as a thin coloured strip, so chatter is visible as flicker.
   - **Drag the open and close threshold lines** directly on the history.
   - The **transfer-curve inset is editable**: drag the threshold point, the ratio slope, the range floor and the knee, with the same gestures as the compressor knee graph (pro-c.md P0-3).
   - Hover shows the level and the gate state at that moment. Freeze, and scroll back with the wheel.
3. **Hear every part [S]:** **Key** (the side chain), **Removed** (the delta: what the gate cuts, bleed included) and **Wet**. Missing "Removed" is Computer Music's first complaint.
4. **A simple stereo detector [S].**
   - A **Detect** switch: *Linked* / *Per channel* / *Mid* / *Side* / *Max(L,R)*, plus one **Link %**.
   - Pro-G's four-knob gain and mix matrix moves into an "Advanced routing" drawer for the rare cross-link case. This answers "awkward stereo knobs".
5. **Keys and MIDI without routing [S].**
   - Choose the key from **any mixer strip** in a menu.
   - The SC filter is the **Aequalis side-chain EQ** (HP/LP 6–96 dB/oct plus bells), with the filtered spectrum drawn.
   - **MIDI mode**: *Open while held* (Pro-G's), **Close while held**, or *Retrigger*. The notes come from any Cantus MIDI track chosen in the same menu.
   - **Force open / force closed** are automatable switches.

### P1: the "better than Pro-G" layer

6. **A drum-aware detector (optional) [L].**
   - An onset detector plus timbre features (spectral centroid, band-energy ratios) to **separate hits from bleed** and tell kick, snare and tom apart.
   - It is exposed as a detector stage in the shared core, alongside the de-esser's voice detector.
   - **Learn bleed:** select a stretch of the clip that holds only bleed, and the gate sets the close threshold above it and proposes an SC filter.
   - This goes after Oxford's lead without claiming its ML model.
7. **Frequency-selective gating [M].** Cut only above or below a split point, for example gate the cymbal bleed on a snare mic above 4 kHz and leave the body alone. It reuses the multiband split engine (Gatey Watey and smart:gate territory).
8. **Hits to MIDI, natively [M].** "Make MIDI from gate": create a **MIDI clip of the gate openings with velocities** on a new track routed to **Pulsus** (the Cantus drum instrument) for drum replacement or layering. Oxford sells this as an export. For us it is a command and one undo step.
9. **Gate events on the timeline, and commit [M].** Show where the gate opened as a faint overlay on the clip waveform in the arrangement. **"Commit as clip edits"** splits and fades the clip at the openings (non-destructive strip silence), for people who prefer edits to live gating.
10. **Rejected signal to a strip [S].** Send the "Removed" signal to any mixer strip. Example: reverb only on the bleed and ghost notes (G8's idea, without host multi-out plumbing).
11. **Better ducking [S].** The Ducking style keyed from any strip, with a **tempo-synced release** (1/8, 1/16.) and the **host-tempo pulse trigger** planned for the compressor. "Duck this strip from…" in the strip menu builds the whole thing in one undoable command (pro-c.md P1-12).

### P2: breadth

12. **Spectral gate / denoiser [L]:** gating per FFT bin below a learned noise profile. It shares the FFT engine with the spectral modes of the de-esser and multiband.
13. **Transient emphasis on opening [S]:** an optional "punch" boost for the first few ms after the gate opens, a classic drum-gate trick.
14. **Surround** once Cantus has multichannel buses.

### Interface and UX: look and feel better

- **The display is the largest element**, so one continuous layout replaces Pro-G's grow-when-Expert window. Controls sit in three labelled groups: **Detect** (Threshold, Ratio, Range, Knee), **Time** (Attack, Hold, Release, Lookahead) and **Key** (source, SC EQ, detect mode). Style is a segmented control with a one-line description.
- **Shared colour meaning across the dynamics family:** input is neutral, output is the accent colour, and **what is removed uses the same colour as in the de-esser and compressor**. The state lane uses a colour plus a pattern, so it stays readable for colour-blind users. The theme is light by default with an editable palette, and passes AA.
- **Compact face** for the inspector and mixer: a threshold bar with the key-level meter and a state LED. Every setting is reachable by keyboard and announced through AccessKit.

### One device or a separate gate?

A gate is a downward expander with a high ratio, hold and hysteresis. In one two-sided transfer curve it is simply the **lower** segment. So in Cantus it should be the **Gate face of the shared Dynamics device**: the browser lists "Gate / Expander", it opens on a gate-first layout with gate defaults, and **Expand** shows the rest. That way the drum-aware and voice detectors, the SC EQ, the listening modes and the timeline tools are built once. The general argument, for and against, is in [pro-mb.md §10](pro-mb.md).

### Suggested build order

1. The shared dynamics core (with the compressor) and PDC.
2. P0-1 and P0-2 (parameters and the touchable display), then P0-3, 4 and 5.
3. P1-8, 9 and 11 (MIDI clip, timeline overlay and commit, ducking): cheap for a DAW, impossible for a plug-in.
4. P1-6 and 7 (drum-aware detector, frequency-selective gating), then P2.

---

## Sources

Official FabFilter (all read 2026-09-23):

- [product] Product page: https://www.fabfilter.com/products/pro-g-gate-expander-plug-in
- [shop] Shop: https://www.fabfilter.com/shop/pro-g-gate-expander-plug-in
- [download] Download page with version numbers and dates: https://www.fabfilter.com/download
- [manual] PDF manual: https://www.fabfilter.com/downloads/pdf/help/ffprog-manual.pdf
- [help-about] https://www.fabfilter.com/help/pro-g/intro/fabfilterprog
- [help-overview] https://www.fabfilter.com/help/pro-g/using/overview
- [help-knobs] https://www.fabfilter.com/help/pro-g/using/knobs
- [help-dyn] https://www.fabfilter.com/help/pro-g/using/dynamiccontrols
- [help-time] https://www.fabfilter.com/help/pro-g/using/timecontrols
- [help-metering] https://www.fabfilter.com/help/pro-g/using/metering
- [help-expert] https://www.fabfilter.com/help/pro-g/using/expertmode
- [help-os] https://www.fabfilter.com/help/pro-g/using/oversampling
- [help-ms] https://www.fabfilter.com/help/pro-g/using/midsideprocessing
- [help-resize] https://www.fabfilter.com/help/pro-g/using/fullscreenandresize
- [help-instlist] https://www.fabfilter.com/help/pro-g/using/instancelist
- [help-output] https://www.fabfilter.com/help/pro-g/using/outputoptions
- [help-midi] https://www.fabfilter.com/help/pro-g/using/midilearn
- [help-undo] https://www.fabfilter.com/help/pro-g/using/undoredo
- [help-presets] https://www.fabfilter.com/help/pro-g/presets/loadingpresets, …/savingpresets, …/howpresetsarestored
- [press-2011] https://www.fabfilter.com/press/1304928000/fabfilter-announces-pro-g-gate-expander-plug-in
- [press-2011b] https://www.fabfilter.com/press/1317981639/fabfilter-releases-pro-g-101-update-with-ducking-style
- [press-2015] https://www.fabfilter.com/press/1450083600/fabfilter-updates-all-plug-ins-including-retina-support-announces-holiday-sale
- [press-2016] https://www.fabfilter.com/press/1481533200/fabfilter-updates-all-plug-ins-announces-holiday-sale
- [press-2018] https://www.fabfilter.com/press/1518681600/fabfilter-updates-all-plug-ins
- [news-instlist] https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410
- [news-mar] https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
- [news-apr] https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
- [feature hint image] Product-page help-hint image for Range: https://cdn-b.fabfilter.com/assets/img/products/desktop/pro-g-feature-3-2x.jpg

Reviews and user discussion:

- [musicradar] Computer Music / MusicRadar review (8 Jul 2011): https://www.musicradar.com/reviews/tech/fabfilter-pro-g-474771
- [tapeop] Tape Op #158, James Neil (Nov/Dec 2023): https://tapeop.com/reviews/gear/158/pro-g-gate-plug-in
- [plap] Produce Like A Pro, "5 Drum Gates Shootout", Warren Huart (27 Mar 2026): https://producelikeapro.com/blog/5-drum-gates-shootout-which-one-actually-wins/
- [pe-gates] Production Expert, "5 Innovative Gate Plugins" (28 Oct 2023): https://www.production-expert.com/production-expert-1/5-innovative-gate-plugins
- [gs-search] Gearspace Pro-G gear page and user comments (search summary; Gearspace blocks direct fetch): https://gearspace.com/gear/fabfilter/pro-g

Competitors:

- [sonnox-dg] Sonnox Oxford Drum Gate 2: https://sonnox.com/products/oxford-drum-gate-2 and https://sonnox.com/articles/oxford-drum-gate-2-is-here/

**Access notes.** Reddit and Gearspace refused direct fetches from this environment, and FabFilter forum threads sit behind a bot check. KVR has no user reviews of Pro-G (https://www.kvraudio.com/product/pro-g-by-fabfilter/reviews). Community views therefore rely on the published reviews above plus search-result summaries, marked as such.

### Open items to verify in the trial

- The Upward style's Threshold and Ratio ranges.
- The hysteresis amount per style (in dB).
- The effective attack at 0 ms with and without lookahead.
- Whether Range at 100 dB is truly −∞.
- Whether MIDI triggering respects velocity.
