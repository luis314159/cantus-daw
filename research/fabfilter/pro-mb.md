# FabFilter Pro-MB: product research for the Cantus multiband dynamics

*Researched 2026-09-23. Subject: **FabFilter Pro-MB 1.33**, the current version. All facts are paraphrased from the sources tagged in brackets; the full list is at the end. Anything marked **(unverified)** is not in an official document and should be checked in the 30-day trial before we copy a number. Companion reports: [pro-ds.md](pro-ds.md), [pro-g.md](pro-g.md), and the compressor study [pro-c.md](pro-c.md), whose proposals this report builds on.*

---

## 0. Summary

- **Pro-MB is still version 1.** It was released on **17 October 2013**. The current build is **1.33, 16 April 2026**. There has never been a Pro-MB 2. Since 2014 it has only had maintenance updates. Its neighbours were modernised in the meantime: Pro-Q 4 got spectral dynamics, and Pro-C 3 (January 2026) got a multiband "TTM" style, auto threshold and Atmos support. [download] [press-2013] [news-proc3]
- **Price:** EUR 169 (the shop's EUR price as of today). [shop]
- **What it is known for:** "think bands, not crossovers". You click anywhere on the spectrum and a band appears there; the rest of the spectrum is left alone. The other signature is **Dynamic Phase**: zero latency, no pre-ringing, and no static phase shift while no gain is changing. Reviewers praise both. [product] [sos] [musictech]
- **Weak spots we can exploit.** Its own users list them:
  - The threshold is not drawn on the display, and the band dots look like thresholds but set gain.
  - Attack and release are percentages, not milliseconds.
  - A band cannot compress downward and upward at the same time.
  - There is no input meter and no per-band gain-reduction history.
  - The ratio control does not show a number.
  - The floating band panel covers the analyser.
  - There are 6 bands at most, bands cannot overlap, and slopes stop at 48 dB/oct.
  - There is no surround support, and it is not in Pro-Q 4's Instance List.

  [forum-8257] [forum-7482] [sos] [product]

---

## 1. Screenshots saved

All images are official FabFilter images from fabfilter.com. Each file was checked as a valid JPEG larger than 5 KB, and the key ones were opened and looked at.

| File | What it shows | Source |
|---|---|---|
| `screenshots/pro-mb-01-main-interface.jpg` | Full plug-in, 1801×1212. Three bands: magenta low shelf, blue mid dip, green high boost. It also shows the yellow overall curve, the spectrum, the floating band panel with Expert open, and the bottom bar. | product page screenshot [product] |
| `screenshots/pro-mb-02-annotated-overview.jpg` | The same UI with hand-written labels on every area: display, band controls, meter, processing mode, oversampling, analyser, in/out/mix, resize. | [help-overview] |
| `screenshots/pro-mb-03-display-band-handles.jpg` | Close-up of a selected band with its labelled handles: solo/mute, low/high crossover, low/high slope buttons, peak dot (gain/centre) and range line. | [help-display] |
| `screenshots/pro-mb-04-display-hover-and-band-panel.jpg` | Hovering a band shows a readout ("Band 5, −4.16 dB, 1840.4 Hz"). Slope buttons sit on the crossovers, the Free side-chain range bar sits above the panel, and the panel is in Expand mode. | product feature image [product] |
| `screenshots/pro-mb-05-snapped-crossover-split-unsnap.jpg` | Two bands snapped to one shared crossover, with the **Split** (+) and **Unsnap** buttons that appear on hover. | [help-display] |
| `screenshots/pro-mb-06-basic-band-controls.jpg` | The basic band panel: Threshold with its circular side-chain meter, Range, Attack, Release, Output with the M/S ring, Compress/Expand, Ratio, Knee and Lookahead. | [help-basic] |
| `screenshots/pro-mb-07-expert-sidechain-controls.jpg` | The Expert column: Band/Free, In/Ext, Audition, Stereo Link with the MID/SIDE mode button, and the Free trigger-range bar with triangular handles. | [help-expert] |

---

## 2. Version history (only what matters for design)

| Date | Version | Change | Source |
|---|---|---|---|
| 17 Oct 2013 | 1.0 | Launch: 6 free bands, three processing modes (Dynamic, Linear, Minimum Phase), up to 4× oversampling, 20 ms lookahead, per-band M/S, external side chain and Free trigger range, 0–200 % mix. Launch price EUR 169 / USD 229 / GBP 139. | [press-2013] |
| 12 Dec 2014 | 1.10 | **Full Screen mode, a Large size, horizontal zoom**, a **side-chain analyser**, and moving a band while holding its solo button. Adds Retina and High-DPI support, and reports gain reduction to Pro Tools. These answer SOS's launch criticism that there was no zoom. | [press-2014] [sos] |
| 11 Feb 2015 | 1.11 | Sizes renamed to Medium, Large and Extra Large. Fixes: a muted band stayed audible after deletion in Dynamic Phase; zoomed display edge bands; gain-change reporting when Stereo Link was below 100 %. | [press-2015a] |
| Dec 2015 | — | Bug fix: automating Audition Side Chain crashed the plug-in. | [press-2015b] |
| Dec 2016 | — | Fixes: a band's gain was drawn wrongly after a drag while the display range was animating; MIDI Learn of Band State parameters crashed. | [press-2016] |
| Feb 2018 | — | Fix: output could go silent with 4× oversampling. | [press-2018] |
| Dec 2023 | — | CLAP format, for all FabFilter plug-ins. | [news-2023] |
| 19 Mar 2026 | — | For all plug-ins: interface sizing and scaling fixes (Logic especially), **drag-and-drop preset files**, and **Copy / Paste** in the preset right-click menu. | [news-mar] |
| **16 Apr 2026** | **1.33** | **Fixed: dragging a slope button horizontally did not move the crossover.** macOS installer lets you choose formats. Minor fixes. | [news-apr] [download] |

**What changed in the latest version:** only the slope-button drag fix and installer work. Pro-MB is **not** in the Pro-Q 4.10 Instance List, which covers Pro-Q 4, Pro-C 3, Pro-DS and Pro-G; FabFilter only says "more plug-ins will follow". [news-instlist]

---

## 3. Complete feature list with parameters

Ranges come from the manual text and from the labels printed around each control in the official screenshots (marked *[shot]*). FabFilter publishes almost no defaults. The `Default Setting` preset loads a plug-in **with no bands**, and new bands get their default from the band preset called **Default**, which you can overwrite. [help-basic] [review-paf]

### 3.1 Bands and crossovers

| Parameter | Range / values | Notes |
|---|---|---|
| Band count | 1–**6** | Placed freely. Bands **do not overlap**: moving one squashes and pushes its neighbours. [help-display] [product] |
| Band position | Centre frequency, plus low and high crossover | The peak dot moves the centre, which moves both crossovers. Each crossover line can also be dragged on its own. [help-display] |
| Crossover slope | **6–48 dB/oct**, per crossover, low and high set independently | The feature list says this applies in Dynamic and Linear Phase. The Minimum Phase help says only 6 dB/oct everywhere avoids static phase shift. [manual-pdf] [help-mode] |
| Snap | Automatic when a band nears a neighbour or the display edge | Snapped bands share one crossover. On hover, **Split** puts a new band between them and **Unsnap** leaves a small gap. [help-display] |
| Solo / Mute | Per band | Hold the button for a momentary solo or mute. Ctrl/Cmd-click makes it exclusive. [help-display] |
| Bypass | Per band (or per selection) | The band dims and the button glows red. Hold for a momentary bypass. [help-basic] |

### 3.2 Per-band dynamics (basic panel)

| Parameter | Range | Notes |
|---|---|---|
| Threshold | **−90 dB … 0 dB** *[shot]* | A circular **side-chain level meter** around the knob shows the filtered and linked detector level. [help-basic] |
| Range | **−30 dB … +30 dB** *[shot]* | Limits the maximum gain change. **Its sign chooses downward (−) or upward (+)** action. [help-basic] |
| Dynamics mode | **Compress / Expand** | Combined with the sign of Range this gives four behaviours: downward compression, upward compression, downward expansion (a gate at high ratio and range), and upward expansion (transient emphasis). [help-basic] |
| Attack | **0–100 %** | Shown as a percentage because the real time depends on the programme and on the band's frequency. [help-basic] |
| Release | **0–100 %** | Same reason as Attack. [help-basic] |
| Ratio | **1:1 … ∞:1** slider *[shot]* | Scales the effect; Range caps it. The slider shows no numeric label. [help-basic] [forum-8257] |
| Knee | **Hard … Soft** slider *[shot]* | Not documented in dB. |
| Lookahead | **0–20 ms** slider *[shot]* | Global on/off switch in the bottom bar. **When on, latency is a fixed 20 ms** whatever the per-band value. [help-basic] |
| Output level | **−30 … +30 dB** *[shot]* | Equals the peak dot's gain. |
| Output pan (ring) | **Mid ↔ Side** balance | Useful for narrowing the lows or widening the highs. Alt-drag on Output also sets the ring, so that make-up gain only affects the processed Mid or Side part. [help-basic] |
| Band presets | Save / load per band | Overwrite "Default" to change what new bands get. [help-basic] |

### 3.3 Per-band expert panel (side chain and stereo)

| Parameter | Values | Notes |
|---|---|---|
| Trigger range | **Band / Free** | **Free** shows a draggable range bar above the panel. FabFilter describes its filtering as "intelligent and adaptive", which makes narrow ranges easier to trigger on. [help-expert] |
| Trigger source | **In / Ext** | External side chain, per band. Routing has to be set up in the host, and the help needs a full page of per-DAW steps. [help-expert] [help-extsc] |
| Audition | toggle, or hold for momentary | Plays the filtered, stereo-linked **trigger** signal. It does **not** play the removed signal. [help-expert] |
| Stereo Link | **0 % → 100 % → Mid-only or Side-only** | The first half of the slider sets linking. The second half moves towards processing only Mid or only Side; a small mode button chooses which. Stereo instances only. [help-expert] |

With Expert off, each band triggers on its own band, from the internal input. [help-expert]

### 3.4 Global

| Setting | Values | Notes |
|---|---|---|
| Processing mode | **Dynamic Phase** (default), **Linear Phase**, **Minimum Phase** | See §4.4. You can change the default with *Save As Default*. [help-mode] |
| Oversampling | **Off / 2× / 4×** (linear-phase) | Reduces aliasing from fast, deep gain changes and improves the top end near Nyquist in Dynamic and Minimum Phase. Adds latency. [help-os] |
| Lookahead | On / Off (global) | Latency is 0 with Dynamic or Minimum Phase, no lookahead and no oversampling. [help-basic] [help-os] |
| Input level / pan | knob with pan ring | Can stand in for moving every threshold. [help-output] |
| Output level / pan | knob with pan ring | Alt-drag links input and output in opposite directions. [help-output] |
| Mix | **0–200 %** | Scales every dynamic and static gain change. Above 100 % exaggerates the effect. [help-output] |
| Global bypass | toggle | Soft, latency-compensated bypass. The UI dims and the output button top glows red. [help-output] |
| Analyser | Pre / Post / **SC**; Resolution Low / Medium / High / Maximum (1024 / 2048 / 4096 / 8192 points); Speed; **Tilt** (default **4.5 dB/oct**, pivoting around 1 kHz); **Freeze** (hold for a momentary freeze) | Grey scale −100…0 dB. The panel opens on hover and can be made sticky. Analyser settings are not changed by presets but are saved in songs. [help-analyzer] |
| Display range | **±3 / 6 / 12 / 30 dB** | Grows automatically while you drag past the edge. This can be turned off in the Help menu. [help-display] |
| Size | Medium / Large / Extra Large, plus Scaling, plus **Full Screen**; free resizing in VST3 | The size and scaling you choose are remembered per monitor type. [help-resize] |
| Formats | VST 2/3, AU, AAX Native, AudioSuite, **CLAP**; **iPad AUv3** | Mono and stereo only. **No surround layouts are listed** (Pro-C 3, Pro-Q 4, Pro-L 2 and Pro-R 2 have them). [product] [help-ios] [news-proc3] |
| Other | MIDI Learn, Smart Parameter Interpolation, sample-accurate automation, GPU graphics, gain-change reporting to Pro Tools | [product] [press-2014] |

---

## 4. The interactive display, in detail

### 4.1 What is drawn

- **Band regions.** Each band is a translucent coloured column between its two crossover lines. Each band gets its own colour (magenta, blue, green, red, cyan…). Inside the column, a filled "lens" shows the band's **potential dynamic range**: the area between the static gain curve and the curve at full Range. It opens upward or downward depending on the sign of Range. [help-display] [shot 01, 03]
- **The yellow overall curve** is thick and shows the **actual combined gain of all bands at this moment**. It animates with the programme, so you watch the dynamics happen. [help-display]
- **Peak dot.** One dot per band at its static gain and centre frequency, in the band's colour, with a white ring when selected. [shot 03]
- **Range line.** A short horizontal line inside the band at the Range limit. It can be dragged. [help-display]
- **Crossover lines** are dashed verticals. Each carries a **slope button** drawn as a tiny filter-shape icon. [shot 03, 04]
- **Solo and Mute buttons** appear at the top of the band you hover. [shot 03]
- **Spectrum analyser** in grey: Pre and Post overlaid, with SC optional. It uses its own grey dB scale on the right (−100…0), next to the yellow gain scale for the curves. [help-display] [help-analyzer]
- **Output meter** at the far right, with a peak readout and a clip light. [help-output]
- **Free trigger range.** When a band is in Free mode, a coloured horizontal bar with two triangular handles is drawn above the band panel and connected to it by a stem. [shot 04, 07]
- **Floating band panel.** The panel sits **under the selected band(s)**. An arrow on the panel glows in the band's colour, or yellow when several bands are selected. [help-basic]
- **Hover readout.** A tooltip shows the band name, gain and frequency. [shot 04]

### 4.2 What can be dragged, and how

| Target | Gesture | Effect |
|---|---|---|
| Empty display | **click** | Creates the first band at a default width. The **mouse wheel while hovering** sets the width before you commit. [help-display] |
| Empty area or existing band | **+ button** on hover, **double-click** an empty area, or **drag the yellow curve** | Creates another band. You can keep dragging straight away. [help-display] |
| Peak dot | drag vertically / horizontally | Band gain / centre frequency (both crossovers move). **All selected bands move together.** Neighbours are squashed, then pushed. Dragging is locked to one axis; **Alt** frees it; **Shift** gives fine steps. [help-display] |
| Band width | **wheel**, or **Ctrl/Cmd plus vertical drag** | Symmetric width change. [help-display] |
| Crossover line | horizontal drag | Moves one edge on its own. [help-display] |
| Slope button | click (menu), **drag a rectangle around several** to edit them together; horizontal drag moves the crossover (fixed in 1.33) | Slope 6–48 dB/oct. [help-display] [news-apr] |
| Range line | vertical drag | Sets Range. [help-display] |
| Crossover / slope / range / dot | **double-click** | Type a value. On the dot you type gain first; **Tab** switches to frequency. [help-display] |
| Background | click-drag a rectangle | Selects adjacent bands. **Ctrl/Cmd-click** adds a band; **Shift-click** selects a range; a click on empty space deselects all. [help-display] |
| Snapped crossover | hover, then **Split** or **Unsnap** | Inserts a band, or separates the two. [help-display] |
| Solo button | hold, and drag horizontally while held | Momentary solo while sweeping the band across the spectrum (added in 1.10). [press-2014] |
| Frequency scale | drag up/down to zoom, left/right to pan, double-click to reset | Horizontal zoom (1.10). [help-display] |
| Free range bar | drag the triangles or the bar | Sets the trigger range. [shot 07] |
| Any knob | vertical drag, rotate by the arrow, wheel, double-click for text, Ctrl/Cmd-click to reset, Shift for fine, Alt for linked controls | Text entry understands `1k`, `A4`, `C#3+13`, `2x` (+6 dB) and `50%`. [help-knobs] |

**What you cannot do on the display:** you cannot drag a **threshold** there (it exists only on the knob), set attack or release, or see gain reduction over **time**. The yellow curve shows only the current instant. [forum-8257] [help-display]

### 4.3 State cues

- A bypassed band is dimmed and its bypass button glows red.
- With global bypass the whole UI dims.
- An edited preset shows its name dimmed.
- While Freeze is on, a blue line shows on the Analyzer button.
- The arrow glow on the band panel shows which band or bands it controls.

[help-basic] [help-output] [help-analyzer] [help-presets]

### 4.4 The three processing modes (the DSP story behind the display)

- **Dynamic Phase** (default). The signal is **not actually split**. Each band is realised as **dynamic filtering**, which is why latency is zero and the phase response is flat while no gain changes. FabFilter admits it acts slightly differently from true band splitting in some cases, such as muting every band or placing bands side by side. [help-mode] Users on KVR note it is really dynamic EQ, so it cannot serve as a band **splitter** for further processing. One KVR summary says equal cuts on adjacent bands give a bump at the shared crossover **(unverified: seen only in a search-result summary; measure it in the trial)**. [kvr-splitter] [kvr-search]
- **Linear Phase.** Bands are split and summed with a flat phase response. There are **no zipper artefacts when crossovers move**, which FabFilter calls unusual for linear phase. The cost is latency (the amount is not documented) and possible pre-ringing. [help-mode]
- **Minimum Phase.** Classic IIR crossovers: no latency, but static phase shift at the crossovers that gets worse with steep slopes. FabFilter's own advice is to avoid it for mastering, except with 6 dB/oct everywhere, which keeps the sum phase-flat. [help-mode]

---

## 5. Metering

- An output meter with peak hold and a clip indicator. **There is no input meter.**
- The circular side-chain meter around each band's Threshold knob.
- The animated yellow curve, which is the only gain-reduction display: a snapshot across frequency, with **no history over time** and **no per-band GR meter**.
- Gain change is reported to Pro Tools and Avid surfaces.

[help-output] [help-basic] [press-2014] [forum-8257]

---

## 6. Workflow

- **Presets.**
  - A factory library, previous/next arrows, and a dimmed name when the preset has been edited.
  - Presets are `.ffp` files in `Documents/FabFilter/Presets/Pro-MB`, organised in sub-folders.
  - Options menu: Save As Default, Open Other Preset, Restore Factory Presets, and MIDI Program Change / Bank Select.
  - The right-click menu has Save, Save As, and (since March 2026) **Copy / Paste**. Preset files can be **dragged onto the UI**.
  - Band presets are separate from plug-in presets.
  - The older Pro-MB still has the old menu-style browser. **Pro-C 3's new browser with search, tags and metadata has not been ported to it.**

  [help-presets] [news-mar] [news-proc3]
- **Undo / Redo / A/B / Copy** buttons sit at the top. Every UI change adds a step. **Changes from automation or MIDI are not recorded.** Copy puts A into B and then greys out while the two are equal. [help-undo]
- **MIDI Learn.**
  - A mode in the bottom bar. The UI dims and each parameter shows a balloon with its CC; touch a control, then move a hardware controller.
  - A menu offers Enable MIDI, Clear (one or all), Revert and Save.
  - Routing MIDI to an effect is host-specific; the help gives a page of per-DAW steps.

  [help-midi]
- **Resizing:** three fixed sizes plus scaling, Full Screen (Esc exits), and free resizing in VST3. On iPad the host decides the size. [help-resize] [help-ios]
- **Help:** interactive hints on hover and a full online and PDF manual. [product]

---

## 7. What reviewers and users say

### Praise

- **Transparency.** Sound On Sound (Nov/Dec 2013) found it inaudible until it actually compresses or expands, and smoother and quicker than rivals when it does. Gearspace mastering-forum posters call Dynamic Phase the most transparent multiband mode they know. [sos] [gs-877090]
- **Workflow and visuals.** Computer Music, on MusicRadar, calls it "a pleasure to use" and innovative; FabFilter quotes its verdict as 9/10. MusicTech (Excellence award) praises the fluid workflow and clear gain-reduction view. Pro Audio Files likes the opt-in approach: a new instance does nothing until you add a band. [musicradar] [product] [musictech] [review-paf]
- **Range per band** is singled out by SOS as missing from rivals. [sos]
- **Versatility.** It works as a multiband compressor, dynamic EQ, transient shaper (upward expansion) and M/S width tool in one. Reviewers use it for sibilance and guitar-body resonances. [musictech] [sos]
- **Free trigger range and external side chain per band** are flagged as unusually flexible. [review-paf]
- Low CPU. [musicradar]

### Complaints

- **The threshold is invisible on the display, and the peak dots are mistaken for thresholds.** [forum-8257]
- **Attack and release are percentages**, "a guessing game". Users ask for milliseconds, at least in an expert mode, and for faster recovery. [forum-8257] [forum-7482]
- **A band cannot compress downward and upward at the same time.** The user who raised it moved to another compressor for this. [forum-8257]
- **No input meter** to compare against the output. **The ratio shows no number.** [forum-8257]
- **The floating panel covers the analyser** and cannot be moved. SOS also missed horizontal zoom, which was added in 1.10. [sos]
- **Learning curve.** Reviewers mention a steep curve and needing notes on the four up/down modes. MusicTech warns the power can go badly wrong in careless hands. [kvr-review] [musictech]
- **Analyser** could be faster and more precise, and in and out are hard to tell apart. [kvr-review]
- Computer Music found the **narrowest bands too wide** for surgical work. [musicradar]
- **Age.** Forum threads ask for "Pro-MB 2", for example with bands routed to separate outputs. [forum-7353] [forum-3223] (search summaries)
- **Dynamic Phase is not a true split**, so it is no use as a crossover for band-wise effects. [kvr-splitter]

---

## 8. Competitors: what they have that Pro-MB lacks

| Product | What it has that Pro-MB does not |
|---|---|
| **DMG Audio Multiplicity** | **Compression and expansion both above and below the threshold, independently, per band** (ratio above and below, with floor and ceiling). Dual-stage release with RMS and crest-factor detection. Per-band dry gain for parallel processing. Per-band "character". Slopes **6–96 dB/oct**. [dmg] |
| **MeldaProduction MDynamicsMB** | **Draw your own transfer curve** per band (compressor, expander, gate, or all at once). Splits by frequency, but also by **level, panorama, mid/side, transients, tonal/noise**, or in parallel. Crossovers up to **120 dB/oct**. Per-band side-chaining. [melda] |
| **Ableton Live Multiband Dynamics** (and Xfer OTT, which copies its idea) | **Above and Below thresholds per band at the same time**: upward and downward compression or expansion in one band. That is exactly the Pro-MB forum request. [ableton] |
| **FabFilter's own Pro-Q 4 and Pro-C 3** | Pro-Q 4 **spectral dynamics** acts only on the bins that exceed the threshold inside a band. Pro-C 3 **TTM** does multiband up and down compression in a compressor, and adds **Auto Threshold**, a **6-band side-chain EQ**, **host-tempo triggering**, **Atmos** and a **search and tag preset browser**. Pro-MB is now the oldest dynamics UI in the FabFilter line. [pq4-spectral] [news-proc3] |

---

## 9. How we beat it: proposals for the Cantus multiband dynamics

**Principle.** Keep Pro-MB's two great ideas: bands you place where the problem is, and a transparent zero-latency mode. Then fix what its users keep asking for: a visible **threshold**, a visible **history**, **real time units**, and **two-sided dynamics**. Add what only a native DAW can do. It shares one dynamics core with the Cantus compressor planned in [pro-c.md §9](pro-c.md). Every proposal follows the Cantus habits: a null test first, an ADR per decision, a schema bump plus fixture, an undo fuzz test, and EN and ES strings.

Priorities: **P0** = the first release must have it. **P1** = what makes people say "better than Pro-MB". **P2** = later. Rough size: S, M or L.

### P0: parity, plus the fixes users ask for most

1. **Bands anywhere, up to 8 [M].**
   - Parity with Pro-MB: create on click or double-click, pick the width with the wheel, snap, Split / Unsnap, solo / mute / bypass per band (hold for momentary), multi-select with a rubber band plus Ctrl and Shift, and edit the whole selection at once.
   - Ours goes further: **8 bands**, and **slopes 6–96 dB/oct**, matching Multiplicity.
2. **Two split engines, honestly named [L].**
   - *Dynamic EQ (zero latency)*, our Dynamic Phase. It is **bit-identical to the input while no band acts**; a null test holds this.
   - *Linear-phase split*, with its latency shown in a badge and reported to Cantus **PDC** (PDC is a prerequisite: STATUS §2(c)).
   - *Minimum phase*, as Linkwitz-Riley with allpass compensation.
   - Drop the "dynamic phase" marketing name. Say what the engine does, and document the sum error for adjacent-band cuts in each mode, measured and shown in the ADR, since the KVR claim about a bump suggests FabFilter never published it.
3. **The threshold is on the display [M].**
   - Draw each band's **detector level**, a thin trace of its filtered side-chain level, and its **threshold as a draggable horizontal line** inside the band column.
   - Draw the Range line and the static gain dot **with different shapes** (for example, a dot for gain, a notch for threshold, a bracket for range), so they can never be confused. This fixes the #1 forum complaint.
4. **Attack and release in milliseconds [S].** Show real times (0.02–250 ms attack, 5 ms–5 s release). An **Auto** toggle adds programme dependence, and shows the **effective** time it is using at that moment next to the knob.
5. **Two-sided transfer per band [M].**
   - An **upper** threshold and ratio (downward compression or upward expansion) and a **lower** threshold and ratio (upward compression or downward expansion) in the same band, each with its own range. This covers the Ableton, OTT and Multiplicity model.
   - A small **transfer-curve inset** per band with the thresholds, ratios and knee all draggable, the same gestures as the compressor knee graph in pro-c.md P0-3.
   - Ratio is always **shown as a number**, with click-stops.
6. **Gain reduction over time [M].**
   - Under the spectrum, a strip of **one GR lane per band** (scrolling, the band's colour), plus **input and output meters** side by side.
   - The yellow "current response" curve stays, and gains a **peak-hold ghost** of the deepest cut in the last few seconds.
7. **Listening that tells the truth [S].** Per band: **Solo**, **Trigger** (hear the side chain) and **Delta** (hear only what the band removes, or adds in upward modes). Pro-MB has only the first two.
8. **The panel never hides the analyser [S].** The band editor is **docked below the display** by default, the pattern of Cantus's inspector and docks, with an option to float it. The display keeps its full height.
9. **Tests [S].**
   - Null at defaults and with every band at 0 dB range.
   - Block-size independence.
   - The flat sum of each split mode within a stated tolerance.
   - Automation of crossovers without clicks, which FabFilter claims even for linear phase.

### P1: the "better than Pro-MB" layer

10. **Mid and Side as real per-band layers [M].** Pro-MB offers Mid-only or Side-only per band, but its bands cannot overlap. Ours lets a band be **L/R, Stereo-linked, Mid, Side, or "M+S pair"**. A pair has separate thresholds and ranges for M and S at the same frequencies, drawn as two stacked lenses.
11. **Side chain from any Cantus strip, per band [M].** You pick it from a menu of mixer strips; no host routing dance. Pro-MB needs a page of per-DAW instructions. This also delivers the "sidechain sends" STATUS is waiting for. The Free trigger range becomes a small **side-chain EQ** (reusing Aequalis), with the filtered SC spectrum drawn behind it.
12. **Auto threshold and target GR [S].** Per band, "hold about N dB of GR" moves the threshold with the level (like Pro-C 3's Auto Threshold), and the moving threshold is drawn on the display. Add **loudness-matched bypass** so A/B is not a loudness contest.
13. **Learn from the clip [M]; only a DAW can do it.** Run the detector offline over the selected clip or region to propose a threshold per band (for example the 90th percentile of the band level) and **suggest band positions** where the spectrum's variance is highest. A plug-in only sees the present plus lookahead.
14. **Integration no plug-in can match [M].**
    - A **GR meter on every mixer strip** that hosts a dynamics device.
    - A project-wide **dynamics overview**: our answer to the Instance List, and one that includes the multiband device, which FabFilter's list still lacks.
    - **Project undo/redo through the command bus**, including changes made while automation is armed. Pro-MB ignores automation in its undo.
15. **A latency budget [S].** One **max lookahead** control that sets the latency (as in Pro-C 3). Pro-MB's lookahead always costs 20 ms, even when every band uses 2 ms.

### P2: breadth

16. **Bands as real splits for per-band effects [L].** In linear or minimum split mode, a band can host an **insert chain** (saturation, width), in the style of Kilohearts Multipass. This is Saturn's territory, so plan it with the Saturation row of STATUS §2.
17. **Drawn transfer curves [M].** A multi-segment curve per band (Melda's idea), with the two-sided curve of P0-5 as its default shape.
18. **Spectral mode [L].** Act only on the bins above threshold inside a band, like Pro-Q 4 spectral dynamics. It shares an FFT engine with the de-esser's spectral option (pro-ds.md).
19. **Surround [L].** A non-goal until Cantus has multichannel buses. Pro-MB has no surround either, so this is not a gap at launch.

### Interface and UX: look and feel better

- **One language across the dynamics family.** The same knob and `Bar`, the same threshold notch, the same GR colour and the same delta button in the compressor, gate, de-esser and multiband. **Theme-aware**: light by default, editable palette, passes AA. Pro-MB is dark-only.
- **Band colours that also carry a pattern or shape** (dot, notch, bracket), so a colour-blind user can tell them apart. Every value is readable on hover **without** covering the curve.
- **Motion with meaning:** the band lens breathes with its GR, meters run at 60 fps with ballistics, and **nothing else animates**. The display range auto-expands the way Pro-MB's does, since it is a good idea.
- **Sizes:** continuous resize, full screen from a dock, and a **compact strip view** (band GR lanes only) for the mixer.

### Suggested build order

1. The shared dynamics core (detector, gain computer, envelopes, oversampling utility), together with the compressor from pro-c.md, plus PDC.
2. P0-1, 2, 3 and 4, with their null tests.
3. P0-5, 6, 7 and 8.
4. P1-11 and 14 (side chain from strips, strip GR, overview, undo): cheap for us and impossible for FabFilter.
5. P1-10, 12 and 13, then P2 as users ask.

---

## 10. One dynamics device, or four?

**The question:** could one well-designed dynamics device cover compressor, gate, expander, de-esser and multiband better than four separate ones?

**Arguments for one device**

- **They are the same machine.** Every one is *detector → gain computer (transfer curve) → envelope → gain applied to a region*:
  - A **gate** is a downward expander with a high ratio, hold and hysteresis.
  - A **de-esser** is a compressor whose detector is filtered to the sibilant band, applied wideband or to a split band.
  - **Ducking** is a compressor or inverted gate keyed from another strip.
  - A **multiband** is N of these on frequency regions.

  One tested core means one set of null tests, one oversampling path, one PDC contract and one place to fix bugs.
- **The user learns one set of gestures.** Drag the threshold notch, read GR lanes, press Delta. Pro-MB's four up/down modes confuse people precisely because they are a different vocabulary from Pro-C. A two-sided transfer curve covers compressor, upward compressor, expander and gate in one picture.
- **Growing without rebuilding.** "This compressor needs to leave the lows alone" becomes *add a band* instead of *swap plug-ins and redo the settings*. FabFilter is converging this way itself: Pro-C 3 got multiband TTM, Pro-Q 4 got spectral dynamics, and the Instance List ties the plug-ins together. [news-proc3] [pq4-spectral] [news-instlist]
- **Cantus has no host boundary.** Effects are native, so there is no need to ship four binaries or register four plug-in IDs. A mode is just a parameter.

**Arguments against**

- **Speed to result.** A de-esser needs two knobs and a waveform. A gate needs a waveform with open and closed shading. A multiband needs a spectrum. Pro-DS and Pro-G are loved because they are **small and obvious**, while Pro-MB's top complaint is complexity. A single "does everything" screen risks being Pro-MB for every task.
- **The specialised detectors are the product.** Pro-DS's *Single Vocal* sibilance detector and Oxford Drum Gate's hit classifier do not generalise. A generic engine that does not reach their quality would lose on the thing users actually judge.
- **The displays differ by task.** Time-domain views suit the gate and de-esser; frequency-domain views suit the multiband. One view cannot be best at both.
- **Discovery, presets and automation.** Users look for "de-esser" in a browser. Presets are organised by task. An automation list of 200 parameters is worse than one of 20.

**Verdict: one engine and one data model, with several faces.**

- Build **one `Dynamics` effect kind** in `cantus-core`, with a `mode` or `face` parameter (Compressor, Gate, De-esser, Multiband) and **stable parameter IDs** shared across faces. It runs on one DSP core in `cantus-dsp`.
- The effect browser lists **four entries**, each opening the same device in its face, with task defaults and a **reduced surface**: the de-esser face shows Threshold, Range, detection band, a waveform and Delta.
- An **"Expand"** control on any face reveals the full device, so a de-esser can grow a second band or a gate can gain a side-chain EQ **without losing its settings**. Switching faces is one undoable command.
- Automation lanes list only the parameters the current face uses.
- The specialised detectors (sibilance, drum hit) are **pluggable detector stages** inside the core, not separate plug-ins.

This beats FabFilter structurally: four products with four UIs and four undo stacks, which they are now stitching together through Pro-Q's Instance List. Record the decision in an ADR before writing the compressor, because the compressor is the first face.

---

## Sources

Official FabFilter (all read 2026-09-23):

- [product] Product page: https://www.fabfilter.com/products/pro-mb-multiband-compressor-plug-in
- [shop] Shop: https://www.fabfilter.com/shop/pro-mb-multiband-compressor-plug-in
- [download] Download page with version numbers and dates: https://www.fabfilter.com/download
- [manual-pdf] PDF manual: https://www.fabfilter.com/downloads/pdf/help/ffpromb-manual.pdf
- [help-overview] https://www.fabfilter.com/help/pro-mb/using/overview
- [help-knobs] https://www.fabfilter.com/help/pro-mb/using/knobs
- [help-display] https://www.fabfilter.com/help/pro-mb/using/display
- [help-basic] https://www.fabfilter.com/help/pro-mb/using/basicbandcontrols
- [help-expert] https://www.fabfilter.com/help/pro-mb/using/expertbandcontrols
- [help-mode] https://www.fabfilter.com/help/pro-mb/using/processingmode
- [help-os] https://www.fabfilter.com/help/pro-mb/using/oversampling
- [help-resize] https://www.fabfilter.com/help/pro-mb/using/fullscreenandresize
- [help-analyzer] https://www.fabfilter.com/help/pro-mb/using/analyzer
- [help-output] https://www.fabfilter.com/help/pro-mb/using/outputoptions
- [help-midi] https://www.fabfilter.com/help/pro-mb/using/midilearn
- [help-undo] https://www.fabfilter.com/help/pro-mb/using/undoredo
- [help-ios] https://www.fabfilter.com/help/pro-mb/using/ios
- [help-presets] https://www.fabfilter.com/help/pro-mb/presets/loadingpresets, …/savingpresets, …/howpresetsarestored
- [help-extsc] https://www.fabfilter.com/help/pro-mb/support/externalsidechaining
- [press-2013] https://www.fabfilter.com/press/1382002200/fabfilter-releases-fabfilter-pro-mb-multiband-dynamics-plug-in
- [press-2014] https://www.fabfilter.com/press/1418392800/fabfilter-updates-pro-mb-and-pro-q-2-including-retina-support-announces-holiday-sale
- [press-2015a] https://www.fabfilter.com/press/1423609200/fabfilter-releases-bug-fix-update-for-fabfilter-pro-mb-and-pro-q-2
- [press-2015b] https://www.fabfilter.com/press/1450083600/fabfilter-updates-all-plug-ins-including-retina-support-announces-holiday-sale
- [press-2016] https://www.fabfilter.com/press/1481533200/fabfilter-updates-all-plug-ins-announces-holiday-sale
- [press-2018] https://www.fabfilter.com/press/1518681600/fabfilter-updates-all-plug-ins
- [news-2023] https://www.fabfilter.com/news/1702983600/all-plug-ins-updated-with-clap-support-bug-fixes-and-more
- [news-proc3] https://www.fabfilter.com/news/1768435200/fabfilter-releases-pro-c-3-compressor-plug-in
- [news-instlist] https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410
- [news-mar] https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
- [news-apr] https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
- [pq4-spectral] https://www.fabfilter.com/help/pro-q/using/spectral-dynamics

Reviews and user discussion:

- [sos] Sound On Sound review (2013): https://www.soundonsound.com/reviews/fabfilter-pro-mb
- [musicradar] Computer Music / MusicRadar review (Jan 2014): https://www.musicradar.com/reviews/tech/fabfilter-pro-mb-591162
- [musictech] MusicTech review, Alex Holmes (Feb 2014): https://musictech.com/reviews/fabfilter-pro-mb-review/
- [review-paf] Pro Audio Files review, Rob Schlette (Dec 2013): https://theproaudiofiles.com/review-fabfilter-pro-mb-multiband-compressor/
- [kvr-review] KVR user review (2015): https://www.kvraudio.com/product/pro-mb-by-fabfilter/reviews
- [kvr-splitter] KVR forum, "Most Transparent Multiband Splitter" (2021): https://www.kvraudio.com/forum/viewtopic.php?t=570992
- [kvr-search] Search-result summary citing KVR on adjacent-band response in Dynamic Phase (not opened; unverified)
- [forum-8257] FabFilter forum, "Fabfilter Pro-MB Improvements" (dated Sep 2025): https://www.fabfilter.com/forum/topic/8257/fabfilter-pro-mb-improvements
- [forum-7482] FabFilter forum, "Feature Request: Pro-MB tighter Attack/Release times" (search summary): https://www.fabfilter.com/forum/topic/7482/feature-request-pro-mb-tighter-attackrelease-times
- [forum-7353] FabFilter forum, "Pro-MB2 pls?" (search summary): https://www.fabfilter.com/forum/topic/7353/pro-mb2-pls
- [forum-3223] FabFilter forum, "Pro-MB 2 update???" (search summary): https://www.fabfilter.com/forum/topic/3223/pro-mb-2-update
- [gs-877090] Gearspace mastering forum thread (search summary; Gearspace blocks direct fetch): https://gearspace.com/board/mastering-forum/877090-fabfilter-pro-mb-multiband-compression-expansion-plug.html

Competitors:

- [dmg] DMG Audio Multiplicity: https://dmgaudio.com/products_multiplicity.php
- [melda] MeldaProduction MDynamicsMB features: https://www.meldaproduction.com/MDynamicsMB/features
- [ableton] Ableton Live audio effect reference (Multiband Dynamics): https://www.ableton.com/en/manual/live-audio-effect-reference/

**Access notes.** Reddit and Gearspace refused direct fetches from this environment. Their views appear here only through search-result summaries, which are marked as such. Other FabFilter forum threads are behind a bot check and were not opened.

### Open items to verify in the trial

- Linear Phase latency in samples at 44.1 and 48 kHz.
- The Knee range in dB.
- The real attack and release times at 0 %, 50 % and 100 % for a band at 100 Hz and at 5 kHz.
- The sum response in Dynamic Phase with two adjacent bands at equal cuts.
- Whether any per-band parameter is missing from automation.
