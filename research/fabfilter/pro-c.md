# FabFilter Pro-C: product research for the Cantus compressor

*Researched 2026-09-23. Subject: **FabFilter Pro-C 3**, the current version. Pro-C 2 appears only where it explains Pro-C 3 or where users compare the two. All facts are paraphrased from the sources linked in each section; the full list is at the end. Anything marked **(unverified)** is not in an official document and should be checked in the 30-day trial before we copy a number.*

---

## 0. Summary

- **Pro-C 3 is out.** It was released on **15 January 2026** (around NAMM 2026), ten years after Pro-C 2 (2015). Bug-fix updates followed on 19 March and 16 April 2026. The newest FabFilter news item is Pro-Q 4.13 (30 June 2026), and there is no Pro-C 4. [news-release] [news-mar] [news-apr] [news-rss]
- It sells for **USD 199 / EUR 169 / GBP 149**. FabFilter customers get loyalty discounts. [news-release]
- What is new: **14 styles** (6 new), a **Character** saturation stage, a **6-band side-chain EQ** built on Pro-Q 4, **host-tempo and MIDI triggering**, **Auto Threshold**, **Dolby Atmos up to 9.1.6**, **up to 32x oversampling**, an adjustable **maximum lookahead** (sets the latency), a new preset browser, and control from **Pro-Q 4's Instance List**. [help-intro]
- What it is known for: the **real-time level and gain-reduction history with the knee curve drawn over it**. Tony Maserati calls the way it shows compression "ingenious". [product]
- **Weak spots we can exploit.** The detector is a set of black boxes: no RMS/peak choice and no detector controls. The auto features give little visual feedback. Auto Gain is an estimate, not a measurement. The side-chain EQ has no global bypass. You cannot drag anything on the level display. Loudness is Momentary only. The history has no speed or freeze options. The wavy sliders get complaints. Pro-C 2 automation cannot be read by Pro-C 3. The Vari-Mu style is easy to misread.

---

## 1. Screenshots saved

All images are official FabFilter images, downloaded from fabfilter.com. Each one was checked as a valid JPEG larger than 5 KB, and each was opened and looked at.

| File | What it shows | Source |
|---|---|---|
| `screenshots/pro-c-01-main-interface.jpg` | Full Medium-size window, side chain open. Top bar: undo/redo, A/B, Copy, preset arrows and name, Help, full-screen button. The level display has the red GR history on top and grey input/output waveforms, with the **knee display** (green transfer curve) over its left third behind a collapsible `« KNEE` tab. Knobs: Threshold (green side-chain level arc, headphone audition button, `AUTO` toggle), Ratio, Style button ("Pumping") with the Character icon, Attack, Release (`AUTO`), Gain (`AUTO GAIN`), Dry. Wavy sliders: Knee, Range, Lookahead, Hold. A meter-scale button (36 dB). GR and output meters sit on the right. The side-chain panel shows Level, trigger selector, Stereo Link and the EQ with 3 bands over a spectrum. Bottom bar: MIDI Learn, Oversampling 4x, Lookahead On (20 ms), bypass, Mix 100 %, In 0.0 dB, Out 0.0 dB, resize. | [img-product] |
| `screenshots/pro-c-02-annotated-overview.jpg` | FabFilter's labelled overview of every area, with the TTM style, a 72 dB scale and a 3-band side-chain EQ. | [help-overview] |
| `screenshots/pro-c-03-level-display-and-meters.jpg` | Close-up of the scrolling display: input (dark grey fill), output (lighter, outlined), red GR line, dashed threshold line, and the GR meter (−11 peak readout) next to the output meter (peak plus loudness). | [help-displays] |
| `screenshots/pro-c-04-knee-display.jpg` | The transfer curve. It is white, and turns green up to the current input level. The `« KNEE` tab hides it. | [help-displays] |
| `screenshots/pro-c-05-sidechain-panel.jpg` | Compact meters (GC, OUT) above the side-chain panel. A band is being dragged, and its value pop-up reads 79.93 Hz, Q 0.933, 24 dB/oct, with power and delete buttons. | [product] |
| `screenshots/pro-c-06-sidechain-eq-band-editor.jpg` | Side-chain EQ: the selected band is shaded in its own colour, the summed curve is yellow, and the parameter pop-up shows 280 Hz, +9 dB, Q 6. | [help-sidechain] |
| `screenshots/pro-c-07-style-menu.jpg` | The Style menu, grouped **Modern / Vintage / Utility**. The Vintage submenu holds Op-El, Vari-Mu, Classic and Opto. | [help-style] |
| `screenshots/pro-c-08-character-panel.jpg` | The Character pop-over: Off/Tube/Diode/Bright segments, a Drive slider and a Pre/Post toggle. | [help-style] |
| `screenshots/pro-c-09-compact-layout.jpg` | **Compact** size: no displays, horizontal GC/OUT meters, and the knobs only. It looks like a classic hardware compressor. It also shows the Auto Threshold lock (padlock). | [help-displays] |
| `screenshots/pro-c-10-oversampling-menu.jpg` | Oversampling menu: Off, 2x, 4x, 8x, 16x (CPU-intensive), 32x (very CPU-intensive). | [help-os] |
| `screenshots/pro-c-11-host-sync-trigger.jpg` | Side-chain trigger set to **Host sync**: 1/4 Note, Offset and Length sliders. The four trigger icons are Internal, External, Host sync and MIDI. | [help-sidechain] |
| `screenshots/pro-c-12-instance-list-in-pro-q4.jpg` | Pro-Q 4's Instance List with Pro-Q, Pro-C, Pro-DS and Pro-G instances laid out per track. One Pro-C is zoomed to a near-full editor. | [help-instancelist] |

We also downloaded the surround-linking, output-options, preset-browser and MIDI-learn images to the scratchpad but did not save them. The same help pages have them if we need them.

---

## 2. Version history

| Version | Date | Notes |
|---|---|---|
| Pro-C (1) | early 2010s (unverified) | Styles Clean, Classic, Opto. "Expert mode" held the side-chain controls. Level display with a 48 dB default scale. [manual-v1] |
| Pro-C 2 | 2015 (Audiofanzine review Sept 2015) | +5 styles (Vocal, Mastering, Bus, Punch, Pumping). Added lookahead (20 ms), hold (500 ms), variable knee (72 dB), Range, Mix 0–200 %, 4x oversampling, variable stereo link with M/S, a side-chain EQ with HP, LP and one "Mid" band (which had an **Auto** mode), full-screen mode, Audition Triggering, and a threshold that goes down to −60 dB. [manual-v2] |
| **Pro-C 3.00** | **15 Jan 2026** | The changes are listed in the next section. [news-release] [press] |
| 3.0x update | 19 Mar 2026 | The Show Input Level option is now remembered. VST3/CLAP windows can be dragged in and out of Compact. Eucon mapping fixed. More help on Vari-Mu and TTM. Copy/Paste on the preset button's right-click menu. Preset files can be dropped onto the window. [news-mar] |
| 3.0x update | 16 Apr 2026 | Paste now works from older plug-in versions. Preset-browser hover works better. [news-apr] |

### What changed in Pro-C 3 (vs Pro-C 2)

From the help page "About", the product page and the press release [help-intro] [product] [press]:

- **Six new styles:** Versatile, Smooth, Upward, TTM (To The Max), Op-El and Vari-Mu. That makes 14.
- **Character modes:** Tube, Diode and Bright saturation with "drift", a Drive control, and Pre/Post routing.
- **Side-chain EQ went from 3 fixed bands to up to 6 free bands.** They have all the Pro-Q 4 shapes, including all-pass, brickwall LP/HP and slopes up to 96 dB/oct, and each band can be set to stereo, mid or side. Pro-C 2's **Auto** mid band is gone, and a KVR user asked for it back. [kvr-p12]
- **Trigger input selector:** Internal, External, **Host Sync** (tempo pulse) and **MIDI**. MIDI triggering already existed but is now easier to find.
- **Auto Threshold** (the threshold follows the input level), with a **Lock** so presets do not change it.
- **Adjustable Maximum Lookahead** (Off, or 1–20 ms). It caps the latency.
- **Oversampling up to 32x** (was 4x).
- **Surround / Atmos up to 9.1.6**, with its own linking controls.
- **Instance List** control from Pro-Q 4.10, alongside Pro-G and Pro-DS.
- A new preset browser with search, tags, metadata and favourites.
- Showing or hiding the side-chain panel **no longer turns the side chain on or off**. In v2, the Expert button did both, which confused people.
- Bypass and MIDI triggering are easier to reach. Alt-drag reverse-links Dry and Wet better.
- **Compatibility:** Pro-C 3 installs next to Pro-C 2, not over it. It opens v1 and v2 presets. It **cannot read automation written for Pro-C 2 or Pro-C 1**, because the parameter set changed. [help-upgrading]

---

## 3. Complete feature list with parameters

Ranges come from the Pro-C 3 help unless marked. Pro-C 3 publishes **no parameter defaults**. The "Default Setting" preset is only described as "a general, allround setting". [help-presets-load]

### 3.1 Dynamics (detection path): [help-dyn]

| Control | Type | Range / values | Notes |
|---|---|---|---|
| **Threshold** | large knob | down to **−60 dB** (Pro-C 2 extended it to −60 [manual-v2]; the top of the range is not documented, presumably 0 dBFS) | A **circular side-chain level meter** around the knob shows the filtered, linked detector signal, which makes it easy to set the threshold. |
| **Auto Threshold** | the word `AUTO` beside the label | on/off | Makes the threshold independent of input level: the same material at different levels gets the same compression. Meant for dialogue. |
| **Lock Auto Threshold** | padlock, shown when Auto is on | on/off | Keeps the auto setting when you change presets. |
| **Audition Triggering** | headphone button at the top left of Threshold | on/off | You hear "which parts are triggering and how much compression". In practice it plays **the part of the signal being removed**, which is effectively a delta listen. [pe-5ways] |
| **Ratio** | large knob | **1:1 (off) … ∞:1 (limiter)** | In Vari-Mu, Ratio sets how hard the tubes are driven, not a true ratio. |
| **Knee** | wavy slider | **0 dB (hard) … 72 dB (soft)** | Above 60 dB, with a fast attack, it sounds almost like saturation. |
| **Range** | wavy slider | **0 … 60 dB** of maximum gain change (Pro-C 2 figure [mix-v2]) | Caps the GR rather than scaling it. |

### 3.2 Time and output: [help-time]

| Control | Range | Notes |
|---|---|---|
| **Attack** | **0.005 ms … 250 ms** | Program-dependent. "Super-fast attack times". |
| **Release** | **not documented (unverified)** | Program-dependent. Each style has its own release model: fast recovery after transients, slow after long GR. In Pro-C 1, Classic could stretch to "a few seconds" even at the fastest setting. [manual-v1] |
| **Auto Release** | `AUTO` beside the label | Release follows the current GR amount. The Release knob then scales how strong the auto effect is. |
| **Lookahead** | slider **0 … 20 ms**, limited by the global **Maximum Lookahead** (bottom bar: Off, 1–20 ms) | Called "smooth lookahead". Adds latency. Max = Off together with oversampling Off gives **zero latency**. [help-os] |
| **Hold** | **0 … 500 ms** | Holds GR peaks. Small values add transparency, large values add pumping. |
| **Wet Gain** (labelled GAIN) | knob with a **pan ring = mid/side balance** | Make-up gain. |
| **Auto Gain** | `AUTO` beside GAIN | Estimated from Threshold, Ratio, Knee and Attack, and aware of M/S. The help admits it **does not measure loudness**; it is "an educated guess". |
| **Dry Gain** | small knob | Adds the dry signal for parallel compression. **Alt-drag** on Wet reverse-links Dry. |

### 3.3 Styles (14), grouped as in the UI [help-style]

The help text calls the middle group "Classic", but the UI menu calls it "Vintage" (screenshot 07). The product page spells one style "El-Op", while the help and UI say "Op-El".

| Group | Style | What it does |
|---|---|---|
| Modern | **Clean** | All-round, low distortion, feed-forward, program-dependent. |
| Modern | **Versatile** *(new)* | General purpose: punchy with long attacks, tight and smooth with short ones. |
| Modern | **Smooth** *(new)* | Always smooth. Aimed at low-ratio, slow "glue". FabFilter's developer says it behaves much like RMS compression. [kvr-p8] |
| Modern | **Punch** | Traditional, analog-like behaviour. |
| Modern | **Upward** *(new)* | Upward compression: raises the level when it drops below the threshold. Like Saturn's Dynamics knob, with more control. |
| Modern | **TTM** *(new)* | "To The Max": upward plus downward compression in **3 bands** (crossovers fixed at about **90 Hz and 2.5 kHz**). The threshold becomes a **target level**. The knee blends the up and down stages. The side-chain EQ changes the loudness of each band. It resembles the OTT effect. [attack] |
| Vintage | **Op-El** *(new)* | Opto-plus-tube feel, smooth and warm (LA-2A-like). |
| Vintage | **Vari-Mu** *(new)* | **Feedback** topology modelling remote-cutoff tubes. The ratio varies with level (a built-in soft knee). The sweet spot is **−12 … +3 dBFS** input. Hard driving clips the tubes, which caps the maximum GR. Changing the side-chain level sounds different from changing the threshold. |
| Vintage | **Classic** | Vintage feedback style, very program-dependent (from v1). |
| Vintage | **Opto** | Slow, very soft-knee, more linear opto (from v1). |
| Utility | **Vocal** | Knee and ratio are **automatic**, so only the threshold needs setting. |
| Utility | **Mastering** | As transparent as possible, with minimal harmonic distortion, but still catches transients. |
| Utility | **Bus** | Glue for buses and drums. |
| Utility | **Pumping** | Deep, over-the-top pumping (EDM, drums). |

### 3.4 Character (saturation) [help-style]

- **Mode:** Off, Tube, Diode, Bright. The official copy calls these "analog saturation, coloring and drift".
- **Drive:** how hard the saturation stage is driven. **Reverse-linked with Threshold** (Alt-drag), so you can trade compression for saturation in one move.
- **Routing:** Pre or **Post** (the default). Post saturates the transients that get through a slow attack.
- Opened from a small tube icon under the Style button, which turns red when active. The mouse wheel over the icon cycles the modes.

### 3.5 Side-chain section [help-sidechain]

- The **Side Chain** toggle at the bottom centre shows or hides the panel. It **glows yellow** when any non-default side-chain setting is active, and it shows the trigger icon, so presets with side-chain tricks are visible even when the panel is closed.
- **Level** (vertical slider): side-chain input gain. The range is not documented. Use it with the circular meter on Threshold.
- **Audition** (headphones): hear the filtered, linked trigger signal. Click to latch, click-and-hold to listen momentarily. Available for Internal and External only.
- **Trigger input:** **Internal**, **External** (host side-chain bus), **Host Sync**, **MIDI**.
  - Host Sync generates a pulse. **Sync** is a note value (for example 1/4), **Offset** runs 50–200 % (for dotted and triplet feels), **Length** is a percentage of the sync period.
  - MIDI: any held note acts as a 0 dBFS side-chain signal.
- **Stereo Link** knob: 0–100 % from unlinked to fully linked. Past 100 % it moves into the **mode** chosen by a small button: **Mid**, **Side**, **M>S** (detect on mid, compress side) or **S>M**. Disabled in mono.
- **Surround** version: the first half links each stereo pair (L/R, Ls/Rs, Lc/Rc, Lts/Rts). The second half blends to **all-channel linking**, where every channel follows the maximum of all channels. A **C** button adds centre channels to their pair. An **All** menu includes Surrounds, Tops and LFE (LFE is off by default). The All button lights red when customised. [help-surround]
- **Side-chain EQ controller:** up to **6 bands**, all the Pro-Q shapes including **All Pass**, LP/HP up to **96 dB/oct** plus **Brickwall**, and a **stereo/mid/side** setting per band (a speaker set in surround). The spectrum behind it uses a **4.5 dB/oct tilt** so it looks like perceived loudness. There is **no global EQ bypass**, only per-band bypass, which both Sound On Sound and Mix call out. [sos] [mix-v3]

### 3.6 Global and bottom-bar options [help-output] [help-os] [help-resize]

- **Oversampling:** Off, 2x, 4x, 8x, 16x, 32x. Adds latency on top of the lookahead. FabFilter's advice is to start at 2x or 4x, because high factors cost a lot of CPU.
- **Maximum Lookahead:** Off, or 1–20 ms (screenshots show "On (20 ms)" and "On (5 ms)").
- **Global Bypass:** latency-compensated and soft (no clicks). The display dims and the bypass button shows a red line.
- **Output options pop-up** opens on hover and can be pinned with a click. It holds **Input level/pan**, **Output level/pan** and **Mix 0–200 %**. Mix scales both the dynamic and the static gain change, so 200 % doubles the processing. The readouts can be dragged, scrolled or double-clicked for text entry. Alt-drag moves In and Out in opposite directions.
- **MIDI Learn**, with a menu containing Enable MIDI, Clear, Revert and Save.
- **Resize:** Compact, Small, Medium (default), Large. A **Scaling** submenu offers, for example, 150–300 % on Retina. VST3/CLAP windows can be dragged to any size. **Full Screen** (Esc to leave) picks a larger scaling automatically. Scalings are remembered per monitor type.
- **Formats:** VST, VST3, AU, CLAP, AAX Native, AudioSuite, 32/64-bit, Windows Vista–11, macOS 10.13+, **iPad AUv3**. Also: GPU-accelerated drawing, sample-accurate automation, "Smart Parameter Interpolation", NEON/SSE code paths, Pro Tools control surfaces and Eucon. [product] [help-ios]

---

## 4. The display, in detail [help-displays] [help-overview]

### What is drawn

1. **Level display (the background of the whole top half).** It scrolls right to left and holds three traces:
   - **Input level:** dark grey filled waveform.
   - **Output level:** lighter grey with a bright outline, so the gap between the two traces *is* the compression.
   - **Gain reduction:** a **red line** hanging from the top edge (0 dB) and falling as GR grows. The help does not say how gain *added* by Upward and TTM is drawn.
   - A **dashed horizontal line at the threshold** (observed in screenshots 01, 03 and 04).
   - The main knobs **float over the display**; the display is not a separate panel.
2. **Knee display:** overlaid on the left of the level display, using **the same dB scale** as the level display and meters, so the threshold line matches across all three.
   - Horizontal axis is input dB, vertical axis is output dB.
   - It draws the transfer curve including Threshold, Ratio, Knee and **Range** (the curve bends back to 1:1 once Range is used up, as screenshots 02 and 04 show).
   - The curve is white and **turns green up to the current input level**, which works as a live "you are here" marker.
   - The `« KNEE` tab hides or shows it, and the choice becomes the default for new instances.
3. **Circular side-chain meter** around the Threshold knob: the detector level after the side-chain EQ and linking.
4. **Side-chain EQ display:** Pro-Q-style curve, band dots, the selected band's shading, and a spectrum (pre and post EQ, judging from the screenshots).

### Scale and speed

- A **meter-scale** button (bottom right) sets the range for the display, knee and meters together, from **9 dB** (mastering) to **90 dB** (mixing). The UI shows values such as 36 and 72 dB. The choice sticks for new instances.
- **Scroll speed:** Pro-C 3 documents **no speed modes, no freeze and no history scrubbing**. Its sibling Pro-L 2 has Slow Down, Fast, Slow and Infinite display modes, so FabFilter knows the idea and has not applied it to Pro-C. [prol-metering]

### Interaction (what can be dragged, and how)

- **The level and knee displays are read-only.** The help documents no dragging of the threshold, ratio or knee on the graph. All dynamics are set with knobs and sliders. The only graph that can be edited is the **side-chain EQ**. [help-displays] [help-sidechain]
- **Knobs** [help-knobs]:
  - Drag vertically, speed-sensitive. **Shift** gives fine control (Ctrl/Cmd in Pro Tools).
  - **Mouse wheel**, also with Shift for fine steps.
  - **Double-click** for text entry, which accepts `1k`, `A4`, `C#3+13`, `2x` (= +6 dB) and `50%` (= knob centre).
  - **Ctrl/Cmd-click** resets (Alt-click in Pro Tools).
  - **Alt-drag** moves linked pairs in opposite directions: Threshold/Drive, Wet/Dry, In/Out.
  - Hovering lights the knob up and shows a value pop-up.
- **Side-chain EQ** [help-sidechain]:
  - Drag the yellow curve to **create** a band; a preview shows the shape. Double-clicking the background also creates one, with a shape that depends on where you click.
  - Select by clicking a dot, rubber-banding, Ctrl-click (add to selection) or Shift-click (range). Click the background to deselect.
  - Drag a dot to change frequency and gain; with several bands selected, gains scale relative to each other.
  - The **wheel changes Q**, or the slope on cut filters. Ctrl+wheel changes gain. **Ctrl-drag** changes Q. **Alt-drag** constrains to one axis.
  - **Alt-click** bypasses a band. **Ctrl+Alt-click** cycles the shape. **Alt+Shift-click** cycles the slope.
  - Double-click a dot for text entry (Tab moves between fields). **Right-click** opens a band menu (shape, stereo placement, copy). Right-click the background to paste or copy all.
  - A floating **parameter pop-up** next to the band has bypass, delete, shape and a ▾ menu.
- **Meters:** click a peak readout to reset it, or click the clip area to clear clipping.
- **Keyboard:** Esc leaves full screen. The preset browser uses arrow keys, Enter, `[` `]` and type-to-search. There are no other documented shortcuts.
- **Touch (iPad):** drag, second finger for fine control, double-tap to reset, long-press to type a value. [help-ios]

### Visual state cues

These patterns are worth copying:
- The **"AUTO" word toggles**, lit when on and dimmed when off, sit right in the parameter labels.
- A **dimmed preset name** means the preset has been edited.
- The **side-chain button glows yellow** when non-default.
- The **Character icon turns red** when on.
- The **surround All button turns red** when customised.
- The **whole UI dims** in bypass.
- A **disabled Copy** button means A and B are equal.

---

## 5. Metering [help-displays]

- **Gain-reduction meter** per channel, red, with a numeric peak readout (for example −11) and a peak-hold tick.
- **Output meter** per channel: **peak**, plus **loudness per channel drawn on top** (lighter band) in **Momentary** mode of EBU R128 / ITU-R BS.1770. A floating marker shows the **combined** momentary loudness of all channels, with a numeric readout of the highest peak.
- **Input meter:** optional, and hidden behind **Help ▸ Show Input Level**. It is never shown in surround.
- **No short-term, integrated or LRA loudness, no true-peak, no RMS/VU ballistics, and no GR statistics** (such as average GR). Pro-L 2 has Short-Term, Integrated, LRA and true-peak; Pro-C does not.
- In **Compact**, the meters become horizontal **GC** (gain change) and **OUT** bars across the window.
- The surround output meter shows all channels with labels.
- Clip indicators refer to the rest of the chain, since Pro-C itself has floating-point headroom.

---

## 6. Workflow

- **Presets** [help-presets-load] [help-presets-save] [help-presets-stored]:
  - A browser with folders (Bass, Drums, Guitar, Mastering, Mixbus, Piano, Special, Vocals, Clean, Default Setting), search, **tags**, **author**, **description**, **favourites**, and type-to-search.
  - Previous/next arrows, `[` `]`, and loading with the right arrow without closing the browser.
  - Copy/Paste of whole settings, drag-and-drop of `.ffp` files (added in the March update), and **MIDI program change** loading.
  - Save As Default. Presets are `.ffp` files under `Documents/FabFilter/Presets/Pro-C 3`, and v1/v2 preset folders are readable.
  - Reviewers praise how well curated the factory presets are, including **suggested GR ranges** written into the descriptions. [kvr-p11] [sos]
- **A/B** with **Copy**. [help-undo]
- **Undo/Redo** of UI changes. **Automation and MIDI changes are not recorded.** The history is local to the plug-in.
- **MIDI Learn:** a modal mode with balloon labels; touch a control, then move a controller. A menu offers Enable MIDI, Clear per mapping or all, Revert and Save. [help-midi]
- **Resizing and full screen:** see §3.6.
- **Instance List:** all Pro-Q 4, Pro-C 3, Pro-DS and Pro-G instances in one window per track. Zoom shows more parameters. Each instance can be bypassed, have its preset changed and be copied or pasted from there. Not on iOS yet. [help-instancelist]
- **Help:** a full online and PDF manual with interactive hints. [manual-v3]

---

## 7. What reviewers and users say

### Praise

- **The visual feedback is what users value most.** It works as a teaching tool: "nothing comes close" for teaching compression, per Audio Plugin Guy. [apg] Tony Maserati calls it "ingenious". [product] Sound On Sound calls it "supremely elegant". [sos]
- **Range:** users describe it as able to cover "delicate mastering to destructive mangling" (Sound On Sound) and as "multiple compressors in one". [sos] [product]
- **Side-chain EQ:** described as "a mini Pro-Q inside", with a workflow that makes experimenting "quick and intuitive". [attack] [pe]
- **Auto Gain:** Production Expert's panel called it "seriously liberating", since level-matched tweaking stops loudness from fooling the ear. [pe]
- **Host Sync:** praised for quick ducking. [apg]
- **Atmos:** linking of each stereo pair is praised in immersive work. [pe]
- **Presets:** "usable and helpful for understanding the controls". [sos]
- **v3 UI:** cleaner and more readable than v2 (KVR, Audio Plugin Guy). [kvr-p8] [apg]
- **Scores:** MusicTech 8/10 ("could easily become your preferred compression plugin"), Production Expert Gold Award, Mix "another hit", Attack Magazine "most fun you can have with a compressor". [mt] [pe] [mix-v3] [attack]

### Complaints

These are the openings for us.

1. **The auto features are opaque.** Sound On Sound says there is little visual feedback for what **Auto Threshold** and **TTM** are actually doing, and that TTM's sound is hard to relate to the display. [sos] Audio Plugin Guy would not trust auto-threshold on vocals. [apg]
2. **Detector black boxes.** KVR users asked for **RMS control** and "more detection circuit parameters instead of those 'smooth'-like black-box modes". The developer (bmanic) answered that FabFilter left them out to keep the controls few, and that styles cover the need. [kvr-p8]
3. **Behaviour changes between styles.** Attack and release mean very different things per style, which makes a style change unpredictable (user MacGyver). The help itself warns that you may need to retweak after switching, especially to Vari-Mu. [kvr-p11] [help-style]
4. **Vari-Mu confusion.** Users see the GR meter move with no audible GR at low levels. A bug with GR display after switching presets and styles was also reported. The developer says Vari-Mu needs about 0 dBFS input and is "finicky" at low levels. [kvr-p17] [gs-search]
5. **No global side-chain EQ or side-chain bypass.** [sos] [mix-v3]
6. **Wavy sliders** (Knee, Range, Lookahead, Hold) are "visually annoying" and awkward to use. **Character is buried in a pop-over** rather than having its own controls (users Funkybot's Evil Twin, jtsterays). [kvr-p12] Some users think FabFilter UIs in general are "too fiddly". [kvr-p4] One older KVR review complains about the graph's colours. [kvr-reviews]
7. **The styles sound alike and Character is subtle.** One reviewer needed three A/B passes to tell Versatile from Smooth, and heard the saturation only at around 75 % drive. [lordreverb]
8. **Price and upgrade.** USD 199 for "just a compressor" is questioned (MusicTech con), and free or cheap alternatives are named: TDR Kotelnikov, ZL Compressor, Melda MCompressor, Klanghelm DC8C at about €23. The upgrade price (about $69–82) is seen as high. [mt] [kvr-p8] [kvr-p12] [kvr-p17] [mpw]
9. **32x oversampling is impractical** across many instances. One reviewer suggests 2–8x as the realistic range. [lordreverb]
10. **Missing tools:** tempo-synced release as note values (debated), an expander or gate, a way to see frequency clashes, and the removed Pro-C 2 **auto band**. [kvr-p11] [kvr-p12]
11. **Automation break:** Pro-C 2 automation does not carry over to Pro-C 3. [help-upgrading]
12. **The docs show seams:** "Classic" vs "Vintage", "Op-El" vs "El-Op", leftover "Expert mode" wording in the side-chain routing steps, and a side-chain EQ shortcut ("Alt+wheel = dynamic range") copied from Pro-Q. [help-style] [help-extsc] [help-sidechain]

---

## 8. Competitors: what they have that Pro-C 3 lacks

| Product | What it does that Pro-C 3 does not |
|---|---|
| **TDR Kotelnikov / Kotelnikov GE** (Tokyo Dawn Labs, standard edition free) [tdr] [tdr-ge] | **Crest-factor detection:** a Peak Crest control sets the balance between peak and RMS compression, with **separate Peak and RMS release** times. **Stereo Sensitivity** points the detector at parts of the stereo image. A **Delta** listen mode, and a signal path that is **bit-transparent at 0 dB GR**. GE adds **equal-loudness** workflows and **frequency-dependent ratio**. In short, it offers the detector control Pro-C users ask for. |
| **Weiss DS1-MK3** (Softube / Weiss) [weiss-manual] [weiss] | Ratio from **1000:1 down to 1:5**, so it also **expands**. **Two release times** (fast and slow, chosen by comparing peak and RMS) plus a **release delay**. **Variable RMS averaging** or peak mode. **Band-selective compression** with **linear-phase crossovers** (LP/BP/HP), which gives de-essing and split-band compression in one device. **Parallel** full-band or band modes. **Look-ahead/delay set in frames** for video post. True-peak metering. |
| **DMG Audio Compassion** [dmg] [dmg-sos] | **A fully configurable topology.** Detectors: **feed-forward/feedback**, RMS/peak, and **noise bleed** into the side chain. Attack and release get a **curve law**, can be **decoupled or reordered**, and have a hold. **Compression, expansion, upward compression and upward expansion** in one unit. A transient shaper. A hybrid clip limiter (pre or post). Linear-phase split EQ. A **Response graph** that plots the attack/release curves. VU/K-meters with variable ballistics. **8 A/B banks**. |
| *Also worth knowing* | **Sonible smart:comp 2**: spectral compression across about 2,000 bands, AI "learn" profiles, and an **impact prediction** display with an RMS histogram. [sonible] **Klanghelm DC8C**: a very flexible detector for about €23. [kvr-p12] |

---

## 9. How we beat it: proposals for the Cantus compressor

**Principle.** Pro-C's strength is that you **see** the compression. Its weakness is that you **cannot touch or understand** what you see: the displays are read-only, the styles are black boxes, the auto features give no feedback, the loudness readout is Momentary only, and undo is local to the plug-in. We keep the clarity and add **direct manipulation, explanation and native DAW integration**, three things a third-party plug-in cannot fully do.

Priorities: **P0** = the first release must have it. **P1** = what makes people say "better than Pro-C". **P2** = later. Rough size is in brackets: S, M or L.

### P0: parity on core sound, plus the three cheapest wins

1. **Core parameter set at or above Pro-C [M]:**
   - Threshold −80…0 dBFS.
   - Ratio 1:1…∞:1, with click-stops like Pro-C 1's ratio dots.
   - Knee 0–72 dB, Range 0–60 dB.
   - Attack 0.005–250 ms, Release 5 ms–5 s with program dependence, Hold 0–500 ms.
   - Lookahead 0–20 ms under a global **max-lookahead latency cap**.
   - Auto Release, Mix 0–200 %, Wet and Dry gains, In/Out with pan, soft latency-compensated bypass.
   - **Null test at defaults, bit-transparent at 0 dB GR** (Kotelnikov's promise), and block-size independence, following the Cantus workflow habits: null test first, ADR per decision.
2. **Side chain that unblocks sidechain sends [M]:**
   - Internal, External, **Tempo pulse** and **MIDI** trigger.
   - External is chosen **inside the compressor from a list of mixer strips**, with no host routing dance. Six steps per DAW take a whole help page for Pro-C [help-extsc].
   - The side-chain EQ reuses the Aequalis engine and view: all shapes, per-band stereo/mid/side, **plus a global side-chain-EQ bypass and a side-chain bypass**, which fixes the Sound On Sound and Mix complaint.
3. **The display is the control [M]:** the one thing Pro-C lacks most visibly.
   - **Drag the dashed threshold line** on the level display.
   - On the knee graph, **drag the threshold point**, **drag the curve above it to set the ratio** (it shows the ratio live), **drag the knee handles** apart and together, and drag the **Range floor**.
   - Wheel over the knee changes knee width; Shift gives fine steps; double-click resets.
   - Hover anywhere on the history to read input, output and GR at that moment.
   - Use the same gestures as the house knob in STATUS §4.1.
4. **Loudness-true Auto Gain and level-matched A/B [S]:**
   - Pro-C's auto gain is an "educated guess" by its own help. Ours **measures** input and output loudness (short-term LUFS, gated) and makes up the difference, with the estimate only as the starting value.
   - Add an "**equal-loudness compare**" button that bypasses the processing *at matched loudness*, like Kotelnikov GE.
5. **Honest listening modes [S]:** three clearly named buttons instead of one headphone icon with vague help text: **Side-chain** (hear the trigger), **Delta** (hear what is removed, or added in upward modes), and **Wet only**.
6. **Metering Pro-C does not have [S]:**
   - GR, In and Out with peak, plus **LUFS Momentary, Short-term and Integrated**, and true-peak on Out. The input meter is **visible by default**, not hidden in a Help menu.
   - A small **GR statistics** readout: average GR and peak GR over the last 10 s, and **crest factor in → out**.

### P1: the "better than Pro-C" layer

7. **Styles that are not black boxes [M]:**
   - Each style is a **named preset of a visible detector**: peak↔RMS (crest), RMS window, feed-forward↔feedback blend, **dual release** (peak and average, as in Kotelnikov and Weiss), release curve, and knee law.
   - The default view stays as simple as Pro-C's. An **"Engine" drawer** reveals the detector, and editing it turns the style into "Custom (from Opto)".
   - This answers the KVR requests for RMS and detector controls without cluttering the main view.
8. **Response graph [S]:** a small static plot of **how GR evolves for a test burst** (step up, hold, step down) for the current style, attack, release and hold. It updates as you turn knobs.
   - It explains why attack means different things per style (the MacGyver complaint) and makes Vari-Mu or feedback styles readable. DMG has one; Pro-C does not.
9. **Explainable auto features [S]:**
   - With **Auto Threshold** on, draw the **moving effective threshold** as a line in the history, plus the offset it applies.
   - With multiband upward/downward (our TTM equivalent), show **one GR lane per band**, the **crossovers as draggable markers** on the side-chain spectrum (Pro-C's are fixed at about 90 Hz and 2.5 kHz), and the up and down targets on the knee graph.
   - This fixes Sound On Sound's main criticism.
10. **History you can inspect [S]:**
    - **Speed modes** (Fast, Slow, Slow-down, Infinite, as in Pro-L 2).
    - **Freeze on transport stop**, **scroll back** with the wheel, and **click-to-freeze**.
    - Optional **time grid in bars and beats** from the Cantus transport, so pumping can be read against the beat, which a plug-in cannot do reliably.
11. **Native DAW integration that no plug-in can match [M]:**
    - A **GR meter on every mixer strip** (and in FL-style meter-wave) whenever a compressor sits in its rack.
    - A **"dynamics overview"** that lists every compressor in the project with threshold, ratio and GR, editable inline. It is our Instance List, but for every native dynamics effect and not tied to one EQ.
    - **Parameter changes go through the Cantus command bus**, so they share **project undo/redo** with everything else and can be undone after automation edits. Pro-C's undo is local and ignores automation.
    - **Stable parameter IDs with schema-versioned migration**, so a future "Compressor 2" **opens old projects with their automation** (Pro-C 3 cannot read Pro-C 2 automation). Bump the schema and add a fixture per change.
12. **Ducking as a first-class gesture [S]:** "Duck this strip from…" in the strip menu creates the send, the compressor and a sensible preset (fast attack, tempo-aware release) in one undoable command. FL users do this constantly.
13. **Tempo-aware release (optional) [S]:** Release can be typed or snapped as a note value (1/8, 1/16.) with the ms equivalent shown. This settles the KVR debate by offering it as a snap, not a mode.

### P2: breadth

14. **Full transfer curve [M]:** downward compression, **upward compression**, **expansion down to 1:5** and a gate floor on one multi-segment knee graph, each segment draggable. It covers Weiss's expansion and Compassion's four modes without a separate gate plug-in.
15. **Split-band / frequency-dependent ratio [M]:** compress only a band with linear-phase crossovers (Weiss DS1), or vary the ratio with frequency (Kotelnikov GE). It shares code with the Multiband plan.
16. **Character with evidence [S]:** saturation stages (tube, diode, bright equivalents), Pre or Post, Drive linked to Threshold. Show **THD %** and a small **harmonic bar chart**, so a subtle mode is visibly doing something. This answers the "nearly inaudible" complaint.
17. **Four A/B/C/D snapshots [S]:** morph between them, and show a readable undo history list (Compassion has 8 banks; Pro-C has 2).
18. **Presets with intent [S]:** tags, author, a **target GR range** field (reviewers praise this in Pro-C), and "suggest from audio": a short analysis pass through `cantus-ai` that proposes a threshold for a target GR, in the spirit of smart:comp's learn.
19. **Surround and Atmos [L]:** a deliberate non-goal until Cantus has multichannel buses. Plan the linking UI on Pro-C's model (pairs, then all-channel, plus centre, surrounds, tops and LFE).

### Interface and UX: how it should look and feel better

- **One control language.** No wavy sliders. Knee, Range, Lookahead and Hold use the house **Bar** or small knobs. **Character gets inline controls** instead of a pop-over. Nothing important sits in a Help menu.
- **Theme-aware and accessible.** Pro-C is dark-only. Ours follows the Cantus theme (light by default, editable palette) and passes the theme editor's **AA contrast check**. Input, output and GR colours must stay distinguishable for colour-blind users (one KVR review complains about the graph colours). Everything is reachable by keyboard and announced through AccessKit. Strings exist in EN and ES.
- **Keep Pro-C's good cues:** the AUTO words inside labels, a dimmed preset name when edited, a glowing side-chain button when non-default, and a dimmed UI in bypass. Add a **latency badge** (ms) next to Lookahead and Oversampling, so the latency cost is visible.
- **Sizes:** continuous resize that remembers its size, a **Compact** mode that fits the inspector or a strip, and full screen from a docked panel.
- **Motion:** 60 fps meters with real ballistics. The history uses the analyser tap (lock-free triple buffer from the audio thread; drawing on the UI thread), as planned in STATUS §2(b).
- **Performance:** polyphase oversampling up to 16x as a shared DSP utility, reporting latency to PDC. Measure CPU per instance and publish it, since 32x in Pro-C is impractical according to reviews.

### Suggested build order

1. P0-1, P0-2 and the null tests.
2. P0-3 (the draggable display) and P0-6 (metering).
3. P0-4 and P0-5 (auto gain and listening modes).
4. P1-11 (strip GR and project undo, which is cheap in Cantus and impossible for Pro-C).
5. P1-7 and P1-8 (visible detector and response graph).
6. Then the rest by user demand.

---

## Sources

Official FabFilter:
- [product]: https://www.fabfilter.com/products/pro-c-3-compressor-plug-in
- [img-product]: https://cdn-b.fabfilter.com/img/products/pro-c-3-screenshot.jpg
- [help-intro]: https://www.fabfilter.com/help/pro-c/intro/fabfilterproc3
- [help-overview]: https://www.fabfilter.com/help/pro-c/using/overview
- [help-knobs]: https://www.fabfilter.com/help/pro-c/using/knobs
- [help-dyn]: https://www.fabfilter.com/help/pro-c/using/dynamicscontrols
- [help-time]: https://www.fabfilter.com/help/pro-c/using/timecontrols
- [help-style]: https://www.fabfilter.com/help/pro-c/using/styleandcharacter
- [help-sidechain]: https://www.fabfilter.com/help/pro-c/using/sidechainsection
- [help-displays]: https://www.fabfilter.com/help/pro-c/using/displays
- [help-os]: https://www.fabfilter.com/help/pro-c/using/oversampling
- [help-resize]: https://www.fabfilter.com/help/pro-c/using/fullscreenandresize
- [help-surround]: https://www.fabfilter.com/help/pro-c/using/surround
- [help-instancelist]: https://www.fabfilter.com/help/pro-c/using/instancelist
- [help-output]: https://www.fabfilter.com/help/pro-c/using/outputoptions
- [help-midi]: https://www.fabfilter.com/help/pro-c/using/midilearn
- [help-undo]: https://www.fabfilter.com/help/pro-c/using/undoredo
- [help-ios]: https://www.fabfilter.com/help/pro-c/using/ios
- [help-presets-load]: https://www.fabfilter.com/help/pro-c/presets/loadingpresets
- [help-presets-save]: https://www.fabfilter.com/help/pro-c/presets/savingpresets
- [help-presets-stored]: https://www.fabfilter.com/help/pro-c/presets/howpresetsarestored
- [help-upgrading]: https://www.fabfilter.com/help/pro-c/support/upgrading
- [help-extsc]: https://www.fabfilter.com/help/pro-c/support/externalsidechaining
- [manual-v3]: https://www.fabfilter.com/downloads/pdf/help/ffproc3-manual.pdf
- [manual-v2]: https://www.fabfilter.com/downloads/pdf/help/ffproc2-manual.pdf
- [manual-v1]: https://www.fabfilter.com/downloads/pdf/help/ffproc-manual.pdf
- [news-release]: https://www.fabfilter.com/news/1768435200/fabfilter-releases-pro-c-3-compressor-plug-in
- [press]: https://www.fabfilter.com/press/1768467600/fabfilter-releases-fabfilter-pro-c-3-compressor-plugin-and-updates-fabfilter-pro-q-4
- [news-mar]: https://www.fabfilter.com/news/1773907200/all-plug-ins-updated-with-bug-fixes
- [news-apr]: https://www.fabfilter.com/news/1776330000/small-bug-fixes-for-all-plug-ins
- [news-rss]: https://www.fabfilter.com/rss/news
- [prol-metering]: https://www.fabfilter.com/help/pro-l/using/metering
- Tutorial video: https://www.youtube.com/watch?v=mSzvpCz-M2k

Reviews:
- [sos] Sound On Sound: https://www.soundonsound.com/reviews/fabfilter-pro-c-3
- [mt] MusicTech (8/10): https://musictech.com/reviews/plug-ins/fabfilter-pro-c-3-review/
- [pe] Production Expert (Gold): https://www.production-expert.com/production-expert-1/fabfilter-pro-c-3-expert-review
- [pe-5ways] Production Expert, Pro-C 2 audition: https://www.production-expert.com/production-expert-1/5-ways-fabfilter-pro-c2-can-help-you-learn-compression
- [attack] Attack Magazine: https://www.attackmagazine.com/reviews/gear-software/fabfilter-pro-c-3-the-most-fun-you-can-have-with-a-compressor/
- [mix-v3] Mix: https://www.mixonline.com/technology/reviews/plug-ins/mix-review-fabfilter-pro-c3-compressor
- [mix-v2] Mix, Pro-C 2: https://www.mixonline.com/technology/fabfilter-pro-c-2-426417
- [apg] Audio Plugin Guy: https://www.audiopluginguy.com/review-pro-c-3-by-fabfilter/
- [lordreverb] LordReverb: https://lordreverb.com/plugins/fabfilter-pro-c-3-review/
- [mpw] Music Production Wiki, Pro-C 2 vs 3: https://musicproductionwiki.com/articles/fabfilter-pro-c-2-vs-pro-c-3
- Magnetic Magazine: https://magneticmag.com/2026/01/fabfilter-pro-c-3/

Forums:
- [kvr-p4] https://www.kvraudio.com/forum/viewtopic.php?t=627007&start=45
- [kvr-p8] https://www.kvraudio.com/forum/viewtopic.php?t=627007&start=105
- [kvr-p11] https://www.kvraudio.com/forum/viewtopic.php?t=627007&start=150
- [kvr-p12] https://www.kvraudio.com/forum/viewtopic.php?p=9186526
- [kvr-p17] https://www.kvraudio.com/forum/viewtopic.php?t=627007&start=240
- [kvr-reviews] https://www.kvraudio.com/product/pro-c-3-by-fabfilter/reviews
- [gs-search] Gearspace NAMM 2026 thread. It is blocked to direct fetch, so it was read through search snippets only: https://gearspace.com/threads/namm-2026-fabfilter-releases-fabfilter-pro-c-3.1459954/
- Reddit could not be reached from this environment.

Competitors:
- [tdr] https://www.tokyodawn.net/tdr-kotelnikov/
- [tdr-ge] https://www.tokyodawn.net/tdr-kotelnikov-ge/ and manual https://docs.tokyodawn.net/kotelnikov-ge-manual/
- [weiss] https://weiss.ch/products/pro-audio/ds1-mk3/
- [weiss-manual] https://www.softube.com/us/user-manuals/weiss-ds1-mk3
- [dmg] https://dmgaudio.com/products_compassion.php
- [dmg-sos] https://www.soundonsound.com/reviews/dmg-audio-equality-compassion
- [sonible] https://www.soundonsound.com/reviews/sonible-smartcomp-2

### Open items to verify in the trial

- Release range, the top of the threshold range, and the ranges of side-chain Level, Wet/Dry gain and In/Out gain.
- Default values of the Default Setting preset.
- Whether any hidden drag gestures exist on the level and knee displays.
- The exact list of Host Sync note values.
- Latency per oversampling factor.
