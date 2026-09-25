# FabFilter Pro-DS: product research for the Cantus de-esser

*Researched 2026-09-23. Subject: **FabFilter Pro-DS 1.32**, the current version. All facts are paraphrased from the sources tagged in brackets; the full list is at the end. Anything marked **(unverified)** is not in an official document and should be checked in the 30-day trial. Companion reports: [pro-mb.md](pro-mb.md) (which holds the full "one dynamics device or four?" argument, §10), [pro-g.md](pro-g.md) and [pro-c.md](pro-c.md).*

---

## 0. Summary

- **Pro-DS is still version 1.** It was released on **5 October 2012**. The current build is **1.32, 16 April 2026**. Recent updates only added **control from Pro-Q 4's Instance List** (January 2026) plus bug fixes. [download] [press-2012] [news-instlist]
- **Price:** EUR 169 today; it was EUR 149 at launch. [shop] [press-2012]
- **What it is known for:**
  - **Single Vocal** detection, which "separates sibilance from non-sibilance" regardless of level, so it usually works with just Threshold and Range.
  - A scrolling **waveform that paints the de-essed parts green**.
  - A circular side-chain meter around Threshold.
  - A spectrum drawn inside the detection-range slider.

  Reviewers call it transparent and uniquely easy. [sos] [musicradar] [product]
- **Weak spots we can exploit.**
  - The detector is a black box: no ratio, attack or release, and nothing shows *why* it triggered.
  - The display is **read-only**: nothing on it can be dragged.
  - Split band is a fixed "everything above the HP frequency" band. Users find it lispy, and it can get "grabby" and "choppy" on full mixes.
  - There is no level-tracking threshold option, which Sonnox has.
  - There is no statistic or warning for over-de-essing: "transparent to a fault".
  - It is stereo only, and nothing has changed in 14 years beyond maintenance.

  [forum-2298] [modwheel] [gs-search]

---

## 1. Screenshots saved

Official FabFilter images from fabfilter.com. Each file was checked as a valid JPEG larger than 5 KB, and the main ones were opened and looked at.

| File | What it shows | Source |
|---|---|---|
| `screenshots/pro-ds-01-main-interface.jpg` | The current UI (1500×956) with the **Default Setting** preset: waveform display, yellow Threshold and Range knobs, the 7–14 kHz detection slider, Single Vocal and Split Band selected, side chain In, Stereo Link 100 %, Lookahead, and the output and GR meters. | [help-about] |
| `screenshots/pro-ds-02-annotated-overview.jpg` | The UI with hand-written labels: real-time level display, basic controls, advanced controls, metering, MIDI Learn, oversampling, in/out options, resize. | [help-overview] |
| `screenshots/pro-ds-03-realtime-deessing-display.jpg` | Close-up of the scrolling waveform. The unaffected audio is grey and translucent; the de-essed moments are green (light green = the amount removed, darker green = what is left). | [help-metering] |
| `screenshots/pro-ds-04-threshold-range-audition.jpg` | The Threshold knob with its circular side-chain meter, the round **Audition Triggering** (headphone) button, and Range. | [help-basic] |
| `screenshots/pro-ds-05-sidechain-filter-analyzer.jpg` | The HP/LP detection slider (5–18 kHz here), with the live spectrum glowing inside it, and the **Audition** (side-chain) button. | [help-basic] |
| `screenshots/pro-ds-06-pro-q4-instance-list.jpg` | **Newest feature (Jan 2026):** a Pro-DS instance edited inside Pro-Q 4's Instance List, shown as a zoomed tile next to Pro-C 3 and Pro-Q 4 instances. | [help-instlist] |

---

## 2. Version history (only what matters for design)

| Date | Version | Change | Source |
|---|---|---|---|
| 5 Oct 2012 | 1.0 | Launch: Single Vocal and Allround detection, Wide Band or linear-phase Split Band, 15 ms lookahead, stereo link with Mid-only or Side-only, up to 4× linear-phase oversampling, waveform display, SC meter, spectrum in the filter slider, large and compact sizes. Launch price EUR 149 / USD 189 / GBP 124. | [press-2012] |
| Dec 2015 | — | Retina and High DPI support; gain change reported to Pro Tools and Studio One. Fix: the value display said "Mid-only" while set to Side-only. | [press-2015] |
| (2014–2018) | 1.1x | Full Screen, sizes from Small to Extra Large, and scaling reached Pro-DS in these years. The exact release date is not in the press archive **(unverified)**. | [help-resize] |
| Jun 2024 | — | Undo and full-screen buttons are no longer cut off in the Small layout. | [news-jun24] |
| **15 Jan 2026** | 1.3x | **Instance List compatibility:** Pro-Q 4.10 can show and edit Pro-DS instances. | [news-instlist] |
| 19 Mar 2026 | — | Fixed: the Mode, Processing and Stereo Link buttons in the instance tile showed the wrong option. Fixed a crash on closing the UI with a text field open (Windows). For all plug-ins: preset **drag-and-drop** and **Copy / Paste**, and sizing and scaling fixes. | [news-mar] |
| **16 Apr 2026** | **1.32** | Small bug fixes. The macOS installer lets you pick formats. | [news-apr] [download] |

**What changed in the latest version:** nothing specific to Pro-DS. The last real feature is the **Instance List** tile (January 2026).

---

## 3. Complete feature list with parameters

Ranges come from the manual and from the labels printed on the official screenshots (marked *[shot]*). The *Default Setting* preset in the official screenshot shows **Single Vocal, Split Band, side chain In, HP 7 kHz / LP 14 kHz, Stereo Link 100 %, Lookahead on, Oversampling Off, In 0 dB, Out 0 dB**. [shot 01]

### 3.1 Basic controls

| Parameter | Range | Notes |
|---|---|---|
| Threshold | **−∞ dB … 0 dB** *[shot]* | The level of the **filtered side chain** above which gain reduction starts. A circular SC meter around the knob. In Single Vocal you can go to −∞: then every sibilant is cut by roughly the same amount, set by Range. [help-basic] |
| Range | **0 … 24 dB** *[shot]* | Scales the detected gain reduction so it stays within the chosen range. [help-basic] |
| Detection HP / LP | **2 kHz … 20 kHz** | Two handles on one slider. Typical sibilance is 8–10 kHz. The **live spectrum of the filtered side chain** is drawn inside the slider. [help-basic] [help-metering] |
| Mode | **Single Vocal / Allround** | Single Vocal is an intelligent sibilance and non-sibilance classifier for one voice. Allround triggers only on level inside the HP/LP range; it is meant for mixes, drums and "HF limiting". [help-basic] |
| Audition Triggering | round headphone button at the top-left of Threshold | Plays **where and how much** de-essing happens: in effect the removed signal. [help-basic] |
| Audition (side chain) | button under the slider | Plays the filtered, linked **trigger** signal. [help-basic] |

### 3.2 Advanced controls

| Parameter | Values | Notes |
|---|---|---|
| Processing | **Wide Band / Split Band** | Wide Band turns the whole signal down during sibilance. Split Band (linear phase) turns down **only the highs above a split point that follows the HP setting automatically**. Split adds some latency. FabFilter suggests Wide for single vocals; users agree it sounds more natural. [help-adv] [gs-search] |
| Side chain | **In / Ext** | For example, key from the clean vocal when de-essing a distorted copy. Host routing is required. [help-adv] [help-extsc] |
| Stereo Link | **0 % → 100 % → Mid-only or Side-only** *[shot]* | The first half of the knob sets linking. The second half moves to processing only Mid or only Side; a small MID/SIDE button chooses which. Example: side-only for panned backing vocals. [help-adv] |
| Lookahead | **0 … 15 ms** *[shot]*, with an on/off button | About 10 ms is often best; leaving some of the onset untouched keeps a natural "s". **When on, latency is 15 ms** plus a little for oversampling or split band. [help-adv] |
| Oversampling | **Off / 2× / 4×** (linear phase) | Worth it when triggering is frequent or Range is high. Adds latency. [help-os] |
| Input / Output | level and pan knobs | Drag the button text to adjust; double-click to type. [help-output] |
| Global bypass | toggle | Soft, latency-compensated bypass. [help-output] |

**Zero latency** needs Lookahead off, Wide Band, and Oversampling off. [help-adv]

**Not present:** no Ratio, Attack, Release, Knee or Hold. The action is described only as "transparent programme-dependent compression/limiting". There is also no wet/dry Mix, no split-band shape choice, and no surround. [product] [help-basic]

### 3.3 Global and interface

- **Sizes:** Small, Medium (the default), Large, Extra Large, plus Scaling and Full Screen. Free resizing in VST3. The **DISPLAY** button collapses the waveform for a compact layout. [help-resize] [press-2012]
- **Formats:** VST 2/3, AU, AAX Native, AudioSuite, CLAP, and an iPad AUv3. Mono and stereo only. [product]
- **Instance List (Pro-Q 4.10+).** A zoomable tile shows the essential controls at low zoom and almost everything at high zoom. It has bypass, a preset menu, and copy/paste of settings. [help-instlist]
- **The usual FabFilter features:** MIDI Learn, undo/redo, A/B, Smart Parameter Interpolation, sample-accurate automation, interactive help hints, and support for Pro Tools control surfaces. [product]

---

## 4. The display, in detail

### 4.1 What is drawn

- **A scrolling waveform** of the input, after input gain, drawn **mirrored around a centre line**. Unaffected audio is dim grey and translucent. **Only the moments being de-essed are coloured:** light green shows the amount removed and dark green the level that remains, so the green area *is* the gain reduction. [help-metering] [shot 03]
- **A circular side-chain meter** around Threshold: the filtered, linked detector level, so you set the threshold against the thing that actually triggers. [help-metering]
- **A spectrum inside the HP/LP slider:** the strong frequencies of the **filtered** side chain light up in yellow between the two handles. [help-metering] [shot 05]
- **Meters at the right:** Output (green) and **gain reduction** (red, falling from the top), each with a peak readout that resets on click, and a clip indicator. The scale runs 0 to −48 dB. [help-metering] [shot 01]

### 4.2 What can be dragged, and how

- **The HP/LP handles**, on the slider only. Double-click types a value. Both knobs and slider use the standard FabFilter knob gestures: vertical drag, arrow-rotate, wheel, double-click text entry, Ctrl/Cmd-click to reset, Shift for fine steps. [help-knobs]
- **Nothing on the waveform display can be dragged.** The help documents no threshold line, no clicking a green event to inspect it, and no scrubbing or freezing of the history. The display is output only and can be hidden. [help-metering]
- The **Instance List tile** in Pro-Q 4 repeats the Threshold, Range and filter slider in miniature. [shot 06]

---

## 5. Workflow

- **Presets.**
  - Factory presets are grouped by task (for example "Female Split Band"). Previous/next arrows; the name dims when edited.
  - Presets are `.ffp` files in `Documents/FabFilter/Presets/Pro-DS`.
  - Options menu: Save As Default, Open Other Preset, Restore Factory Presets, and MIDI Program Change / Bank Select.
  - The right-click menu has Save and Save As, and **Copy / Paste** since March 2026. Preset files can be dragged onto the UI.
  - There is **no search, tags or metadata**. Pro-C 3's new browser has these and Pro-DS does not.

  [help-presets] [news-mar] [news-proc3]
- **Undo / Redo / A/B / Copy** at the top. Changes made by automation or MIDI are not added to undo. [help-undo]
- **MIDI Learn:** the standard FabFilter flow (touch a control, then move the controller), with a menu for Enable, Clear, Revert and Save. Host-specific MIDI routing is still needed. [help-midi]
- **Resizing:** see §3.3. On iPad the host provides the size. [help-resize]

---

## 6. What reviewers and users say

### Praise

- **Detection that just works.** SOS (Sam Inglis, Jan 2013) found it detects sibilants in a way that does not depend on level, gives the most natural result on subtle jobs, and is "set and forget" on easy material. A Gearspace user quoted by FabFilter calls it the first de-esser without a "de-esser sound". [sos] [product]
- **The feedback.** SOS and Computer Music call the green waveform and the spectrum in the slider the best visual feedback of any de-esser they had used. MusicTech gave it 10/10 for ease of use and visual feedback. [sos] [musicradar] [product]
- **Range of uses:** full mixes, splashy cymbals and harsh guitars (Allround with Split Band as an HF limiter), and even controlling drum spill on a sax microphone through an external key. [musicradar] [sos]
- **Transparency and low CPU** (ModWheel, 2026). [modwheel]

### Complaints

- **Grabby and choppy on finished mixes.** A strong "S" can be broken into fragments whatever the settings; one user misses the smoothness of the discontinued *Spitfish*. [forum-2298]
- **Split band gets lispy.** Gearspace users prefer Wide Band for vocals. [gs-search]
- **"Transparent to a fault."** It will happily take 4–5 dB off without sounding broken, so it is easy to lose consonant definition. There is no warning and no statistic. [modwheel]
- **No character option, and it is expensive for a utility** (USD 199 per ModWheel; Computer Music also mentions the price). [modwheel] [musicradar]
- **Missing control:** a Pro-DS 2 request asks for **EQ and compression that act only on the sibilant part**. [forum-2880]
- **Automation drawn by hand still wins sometimes.** SOS notes this; it is a limit of plug-ins in general. [sos]

---

## 7. Competitors: what they have that Pro-DS lacks

| Product | What it has that Pro-DS does not |
|---|---|
| **Sonnox Oxford SuprEsser** | **Three listen modes**: Mix, **Inside** (the band-pass part) and **Outside** (everything but the band). **Auto level tracking**: the threshold follows the signal level, so the same GR is applied at any loudness. **The detection band and threshold are drawn on an FFT display** with peak hold and the peak frequency marked. The cut is a **tuneable band-reject** using linear-phase crossovers, not "everything above HP". [sonnox] |
| **Waves Sibilance** | Resynthesis: it **separates the sibilant component from the voice** instead of turning a band down, which keeps timbre and the length of the consonant. A **Monitor** that solos exactly what is removed. Lookahead. [waves] |
| **oeksound soothe2 / soothe3** | **Spectral** resonance suppression that cuts only the bins that stick out, with Sharpness and Selectivity (merged into *Detail* in soothe3), a **delta** preview, M/S, side chain and mix. **soothe3** (19 May 2026) adds an adaptive-threshold Soft mode, a **low-latency mode for tracking**, and **surround up to 9.1.6**. [soothe2] [soothe3] |
| **FabFilter Pro-Q 4 (internal)** | **Spectral dynamics** inside an EQ band, used for de-essing. It shows the cut on the EQ curve, which Pro-DS cannot. [pq4-spectral] |

---

## 8. How we beat it: proposals for the Cantus de-esser

**Principle.** Pro-DS wins on the **first 30 seconds**: two knobs, and it works. It loses on the **next 30 minutes**: you cannot see *why* it triggered, you cannot touch the display, and you cannot shape the cut. We keep the two-knob face and add an **explainable detector**, **direct manipulation**, and **DAW-native tools** a plug-in cannot have. It is built as the **De-esser face of the shared Dynamics device** (see pro-mb.md §10). Cantus habits apply: a null test first, an ADR per decision, schema plus fixture, undo fuzz, and EN and ES strings.

### P0: parity, plus the obvious wins

1. **The two-knob face [M].**
   - Threshold (−∞…0 dB) and Range (0–**30** dB).
   - A detection band **1–20 kHz**. Pro-DS stops at 2 kHz, too high for dark "sh" sounds.
   - A Voice / Allround detector, Wide or Split, Lookahead 0–15 ms under a latency cap, Link with M or S, and 2× / 4× oversampling.
   - Null test: bit-identical output when no sibilance is detected in Wide mode with lookahead off.
2. **Keep the green waveform, and make it touchable [M].**
   - Under the waveform, add a **detector lane**: the sibilance level over time, with the **threshold as a draggable line** across it. You set the threshold *where the events are*, not on a knob.
   - Hover a green event to see its peak GR and frequency.
   - Freeze the history and scroll it back with the wheel.
3. **The detection band on a spectrum [S].** A wider analyser strip in which the HP/LP handles are dragged **on the spectrum itself**, with the peak frequency of the last event marked. This is SuprEsser's strongest idea; it replaces Pro-DS's thin slider.
4. **Listening modes with clear names [S]:**
   - **Trigger**: the side chain.
   - **Removed**: the delta.
   - **Band only / All but band**: SuprEsser's Inside and Outside.
5. **Guard rails against over-de-essing [S].**
   - A small readout: **events per 10 s, average and maximum GR per event**.
   - A soft warning when the average cut goes past a limit you set (6 dB by default).
   - **Loudness-matched bypass**, so the de-essed vocal is not judged simply as "quieter".

   This answers the "transparent to a fault" review.

### P1: the "better than Pro-DS" layer

6. **An explainable voice detector [M].**
   - Build it from documented features: the ratio of HF to broadband energy, spectral flatness, zero-crossing rate, and onset shape.
   - Draw a **"sibilance confidence" trace (0–1)** in the detector lane, and expose one **Sensitivity** control.
   - Add an optional **Auto level tracking** threshold (SuprEsser, Pro-C 3 Auto Threshold), with the moving threshold drawn.
   - Record the feature set in an ADR, so the detector is testable. A golden-file test on a set of vocal clips should keep it stable across versions.
7. **Shape of the cut [M].** In Split mode, choose **High shelf** (what Pro-DS does), **Bell** (a dynamic EQ around the detected peak; follow-peak optional), or **Band-reject** (SuprEsser). The live cut is drawn on the spectrum. This fixes "split band gets lispy": a bell on the 6–8 kHz peak leaves the air band alone.
8. **Two targets [S].** An optional second detection band for **"sh / ch"** (3–6 kHz), with its own Range, so "s" and "sh" are handled separately.
9. **A smoothness control, in a drawer [S].** Attack, Release and a **minimum event length / hold** fix the "fragmented S" complaint. Defaults stay programme-dependent.
10. **DAW-native tools, which no plug-in can have [M]:**
    - **Learn from clip:** scan the whole vocal clip offline and set the threshold so that the loudest N % of sibilant events get the target cut.
    - **Print to clip gain:** write the de-essing envelope as editable clip-gain automation, then bypass the device. This gives the "hand-drawn automation" quality SOS wants, in one click, and it stays editable.
    - **Events on the timeline:** show sibilance events as thin marks on the arrangement's clip waveform.
    - **Key from any strip:** pick the source from a list of mixer strips (the "clean vocal keys the distorted copy" trick in the help, without host routing).

### P2: breadth

11. **Spectral mode [L]:** cut only the bins above threshold inside the detection band (soothe or Pro-Q 4 spectral). It shares its FFT engine with the multiband's spectral mode.
12. **Treat the sibilant separately [M]:** a small EQ plus gain applied **only to the separated sibilant component** (the Pro-DS 2 forum request), for example taming 11 kHz fizz while keeping 7 kHz clarity.
13. **Low-latency tracking mode** (soothe3) and **surround** once Cantus has multichannel buses.

### Interface and UX: look and feel better

- **Keep the good.** Pro-DS's two big knobs and the green waveform are the best part of its design. Keep the green-means-removed convention **in every Cantus dynamics face**: the gate uses the same colour for the attenuated floor, and the compressor for GR.
- **Theme-aware.** Light by default, editable palette, passes AA. The green-on-grey waveform needs a light-theme variant with a second cue (for example hatching on removed areas) for colour-blind users.
- **One size that adapts** instead of four fixed sizes. The compact face (knobs, detection band and GR meter) fits the Cantus inspector. Nothing important hides behind a small icon.
- Text entry understands `8k`, note names and `%`, like FabFilter's, and shows the unit while you type.

### One device or a separate de-esser?

A de-esser is a compressor whose detector hears only sibilance, applied wideband or to a split band. So in Cantus it should be **a face of the shared Dynamics device**, not a separate codebase. The browser still lists "De-esser" as its own entry, it opens on the two-knob face, and **Expand** reveals the full device. For example, you could add a second band or a side-chain EQ without losing settings. The **Voice detector is a pluggable detector stage** in that core. The general argument, for and against, is in [pro-mb.md §10](pro-mb.md).

### Suggested build order

1. The shared dynamics core (with the compressor), with PDC.
2. P0-1 and P0-2 (the face and the touchable waveform), then P0-3, 4 and 5.
3. P1-10 (learn, print to clip gain, key from any strip), because it is cheap for us and impossible for FabFilter.
4. P1-6 and 7 (the explainable detector and cut shapes), then the rest.

---

## Sources

Official FabFilter (all read 2026-09-23):

- [product] Product page: https://www.fabfilter.com/products/pro-ds-de-esser-plug-in
- [shop] Shop: https://www.fabfilter.com/shop/pro-ds-de-esser-plug-in
- [download] Download page with version numbers and dates: https://www.fabfilter.com/download
- [manual] PDF manual: https://www.fabfilter.com/downloads/pdf/help/ffprods-manual.pdf
- [help-about] https://www.fabfilter.com/help/pro-ds/intro/about
- [help-overview] https://www.fabfilter.com/help/pro-ds/using/overview
- [help-knobs] https://www.fabfilter.com/help/pro-ds/using/knobs
- [help-basic] https://www.fabfilter.com/help/pro-ds/using/basiccontrols
- [help-adv] https://www.fabfilter.com/help/pro-ds/using/advancedcontrols
- [help-metering] https://www.fabfilter.com/help/pro-ds/using/metering
- [help-os] https://www.fabfilter.com/help/pro-ds/using/oversampling
- [help-resize] https://www.fabfilter.com/help/pro-ds/using/fullscreenandresize
- [help-instlist] https://www.fabfilter.com/help/pro-ds/using/instancelist
- [help-output] https://www.fabfilter.com/help/pro-ds/using/outputoptions
- [help-midi] https://www.fabfilter.com/help/pro-ds/using/midilearn
- [help-undo] https://www.fabfilter.com/help/pro-ds/using/undoredo
- [help-presets] https://www.fabfilter.com/help/pro-ds/presets/loadingpresets, …/savingpresets, …/howpresetsarestored
- [help-extsc] https://www.fabfilter.com/help/pro-ds/support/externalsidechaining
- [press-2012] https://www.fabfilter.com/press/1349427600/fabfilter-releases-fabfilter-pro-ds-intelligent-de-esser-plug-in
- [press-2015] https://www.fabfilter.com/press/1450083600/fabfilter-updates-all-plug-ins-including-retina-support-announces-holiday-sale
- [news-jun24] https://www.fabfilter.com/news/1718618400/all-plug-ins-updated-with-bug-fixes
- [news-instlist] https://www.fabfilter.com/news/1768467600/fabfilter-introduces-multi-plugin-instance-list-in-pro-q-410
- [news-proc3] https://www.fabfilter.com/news/1768435200/fabfilter-releases-pro-c-3-compressor-plug-in
- [news-mar] https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
- [news-apr] https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
- [pq4-spectral] https://www.fabfilter.com/help/pro-q/using/spectral-dynamics

Reviews and user discussion:

- [sos] Sound On Sound, Sam Inglis (Jan 2013): https://www.soundonsound.com/reviews/fabfilter-pro-ds
- [musicradar] Computer Music / MusicRadar (Jan 2013): https://www.musicradar.com/reviews/tech/fabfilter-pro-ds-570358
- [modwheel] ModWheel review (Sep 2026): https://modwheel.net/reviews/fabfilter-pro-ds-review
- [forum-2298] FabFilter forum, "I feel like Pro-DS can be improved" (2014, search summary): https://www.fabfilter.com/forum/topic/2298/i-feel-like-pro-ds-can-be-improved
- [forum-2880] FabFilter forum, "FabFilter Pro-DS 2" (search summary): https://www.fabfilter.com/forum/topic/2880/fabfilter-pro-ds-2
- [gs-search] Gearspace threads on de-essers, e.g. https://gearspace.com/threads/best-3-de-essers-plugins.1223021/page-2 (search summary; Gearspace blocks direct fetch)

Competitors:

- [sonnox] Sonnox Oxford SuprEsser: https://sonnox.com/products/oxford-supresser and the SOS review https://www.soundonsound.com/reviews/sonnox-oxford-supresser
- [waves] Waves Sibilance: https://www.waves.com/plugins/sibilance
- [soothe2] oeksound soothe2 manual: https://oeksound.com/manuals/soothe2/
- [soothe3] soothe3 release coverage: https://www.soundonsound.com/news/oeksound-introduce-soothe3 and https://www.production-expert.com/production-expert-1/oeksound-soothe-3-released

**Access notes.** Reddit and Gearspace refused direct fetches from this environment. FabFilter forum threads sit behind a bot check. Their views appear here through search-result summaries, which are marked as such.

### Open items to verify in the trial

- The split frequency relative to the HP handle (is it equal, or offset?).
- The Split Band latency in samples.
- The effective attack and release times in Single Vocal versus Allround.
- Whether Range acts as a ratio or as a hard cap.
- What the Small layout hides.
