# How the market's engines do it, and what Cantus should take

*Round 20, engine-research, 2026-10-06. Extends the table in ADR 0031 "How the others do
it" with primary sources where the architect had only excerpts. **[fetched]** = page read;
**[excerpt]** = search excerpt only. Blocked this time: steinberg.help (one article read
through helpcenter.steinberg.de instead), dlz.reaper.fm (the user guide PDF exceeded the
fetch limit), wiki.cockos.com (bot wall), the Cockos forum (login). Nothing was bypassed.
The fetch tool summarises pages, so quotations from fetched pages are near-verbatim; check
the wording against the page before quoting it in user-facing text. Open-source engines
(Ardour, tracktion_graph, JACK2, PipeWire, Supernova, Faust) are in `scheduling-papers.md`.*

## 1. REAPER — anticipative FX

- Renders FX "at irregular intervals, often out of order, and slightly ahead of time", so
  the cores rarely synchronise — "over 95 percent utilisation of all cores" on 8+ cores; the
  exceptions are low-latency input monitoring and DSP cards, which want "synchronous FX
  multi-processing" (M. Walker, "Running multiple plug-ins", *Sound On Sound*, July 2011,
  https://www.soundonsound.com/techniques/running-multiple-plug-ins **[fetched]**).
- Render-ahead "default being 200ms", "about 50ms or greater recommended"; a track is
  rendered ahead only "if it is not record monitoring **or dependent on a track that is**";
  an option renders even FX-less tracks ahead (Cockos wiki, *Preferences: Audio: Buffering*,
  https://www.cockos.com/wiki/index.php/Preferences_Audio_Buffering **[excerpt]**).
- A competitor's description: "render all plugins offline ahead of time by default 200 ms,
  in a background thread. This evens out any CPU spikes that normally would occur with very
  short buffers"; parameter changes are heard late; armed tracks are exempt (F. Vanhoef,
  Zynewave forum, 2009-02-22, https://zynewave.com/topic/cpu-cpu-usage-meter-pretty-weird-solved/page/3/
  **[fetched]**). Users fix parameter and MIDI lag with the per-track *Prevent anticipative
  FX* **[excerpt]** — no evidence of an automatic switch when an FX window opens.
- A thread "behaviour" setting from relaxed to "very aggressive" — effectively a spin-vs-sleep
  knob; Scan's DAWbench moved it "from the old default of 4 to 3 which halved the reserved
  CPU resource load", and REAPER adopted 3 as its default (Scan Pro Audio, *DAWBench CPU
  performance results*, 2021-10-07, https://www.scan.co.uk/info/proaudio/presszone/dawbench
  **[fetched]**).
- Auto-bypass on silence (6.71, 2022-11-30): per project for plug-ins that report a tail,
  per instance with automatic tail detection, a **user-defined silence threshold**, and "a
  rendering option to disable auto-bypass when using offline render" (KVR news,
  https://www.kvraudio.com/news/56611 **[fetched]**).
- Isolation: native, separate process (shared), dedicated process (per instance)
  **[excerpt]**.
- Best primary source not yet heard: J. Frankel interviewed by V. Curigliano and P. Brown
  (Microsoft), *DAWbench podcast* ep. 32 "DAW Evolution IX: Reaper — revisited", 2024-08-24,
  "a deep technical dive into … the Anticipative FX processing engine"
  (https://dawbench.libsyn.com/episode-32-daw-evolution-ix-reaper-revisited **[fetched,
  description]**).

## 2. Cubase / Nuendo — ASIO-Guard and the two-path meter

- ASIO-Guard shifts "as much processing as possible from the ASIO realtime path to the
  ASIO-Guard processing path"; "high ASIO-Guard levels lead to an increased ASIO-Guard
  latency" (e.g. on fader moves); monitored input and instrument channels switch to real
  time automatically; external devices excluded; plug-ins excludable in the plug-in manager
  (Nuendo 8 manual, https://archive.steinberg.help/nuendo/v8/en/cubase_nuendo/topics/optimizing/asio_guard_c.html
  **[fetched]**). Levels Low/Normal/High; no published millisecond figures
  (helpcenter.steinberg.de 206103564 **[fetched]**). ASIO-Guard 2 (Cubase 8) made the switch
  dynamic **[excerpt]**.
- **VST3's side** (steinbergmedia/vst3_pluginterfaces, `ivstaudioprocessor.h`,
  `ivstprefetchablesupport.h` **[fetched]**): `kRealtime`, `kPrefetch` ("could be called at a
  variable frequency (jitter, slower / faster than realtime)"), `kOffline`;
  **`IPrefetchableSupport`** returns `kIsNeverPrefetchable` / `kIsYetPrefetchable` /
  `kIsNotYetPrefetchable`, changed through `restartComponent(kPrefetchableSupportChanged)`;
  `silenceFlags` "intended to be used as help for optimizations"; `getTailSamples`
  (`kNoTail`, `kInfiniteTail`). CLAP has no prefetch mode, only `CLAP_RENDER_REALTIME` /
  `OFFLINE` and `has_hard_realtime_requirement` (`ext/render.h` **[fetched]**).
- Audio Performance Monitor (Cubase 12→14): "a new algorithm to separately measure the load
  for real-time and prefetch (ASIO-Guard) threads"; **Real-Time** average, **Peak** ("if this
  meter peaks/clips, it will cause an audio dropout"), **ASIO-Guard** average; Cubase 14 adds
  a per-track Dropout list (helpcenter.steinberg.de 4454943549330 **[fetched]**).
- *Constrain Delay Compensation*: on instrument, record-enabled, group and output channels,
  switches VST3 plug-ins to their low-latency mode where they have one, otherwise turns them
  off above a threshold (Nuendo 12 manual **[fetched]**).
- Hybrid CPUs: Cubase/Nuendo 12 on P/E CPUs "can lead to audio dropouts"; Thread Director
  placed "rendering jobs, minimized windows, or real-time audio tasks" on E-cores; "13 and
  later are optimized" (helpcenter.steinberg.de 8412891538066 **[excerpt]**). And with the
  window in the background, E-cores running **prefetch** threads cause dropouts; the fix is a
  `powercfg /powerthrottling disable` per executable (steinberg.help **[fetched]**, see
  `realtime-systems.md` §4).

## 3. Pro Tools — dual buffer, dynamic processing, hybrid DSP

- Pro Tools 11 (AAE, 64-bit): "Playback tracks use a dynamically set buffer … and a variable
  buffer is featured for recording"; offline bounce "up to 150 times faster than real-time"
  (B. Rudolph, 2013, https://barryrudolph.com/newtoys/toys13/avidprotools11.html
  **[fetched]**); the small hardware buffer applies only when an input is monitored,
  otherwise it "continues to play using the best possible playback buffer size"; bounces at
  "60x" for some sessions and "4x" for others (R. Hughes, *Production Expert*, 2013-06-27
  **[fetched]**).
- *Dynamic Plug-in Processing*: plug-ins "only use the CPU when there is audio passing thru"
  (PC Audio Labs **[fetched]**); AAX plug-ins can opt out (`JucePlugin_AAXDisableDynamicProcessing`,
  JUCE forum **[fetched]**); the criterion is not public.
- HDX Hybrid Engine (2021): "intelligently splitting processing between native and HDX DSP
  mix engines"; a per-track **DSP Mode** gives zero-latency recording; 2 048 voices
  (Sonicstate, 2021-07-01 **[fetched]**).

## 4. Studio One — Dropout Protection and a second, low-latency mix

"The Dropout Protection setting determines the size of the Process Block Size buffer,
which is dedicated to track playback and plug-in processing"; the device block sets
monitoring latency (*SOS*, March 2018, https://www.soundonsound.com/node/4918234
**[fetched]**). Native Low-Latency Monitoring admits only plug-ins under 3 ms of latency;
excluded: Pipeline, analysers, splitter chains; and inserts "need to be processed in two
separate audio processes to be heard in both the Z-mix output and other … outputs" —
**processing duplicated** to give monitoring its own path (PreSonus KB 360045431251
**[fetched]**).

## 5. Logic Pro — the live track and the process buffer

*Multithreading*: "Playback Tracks" runs live input DSP on one thread; "Playback & Live
Tracks" spreads live strips (Apple 101975 **[fetched]**). *Process Buffer Range*
Small/Medium/Large; selecting a software-instrument track "can put more strain on the CPU"
(Apple HT203930 **[fetched]**); threads "Automatic", "the recommended setting" (Apple 101921
**[fetched]**). Community analysis: the live chain is the instrument, its inserts, its
sends' destinations, the subgroups it feeds **and the master**, on one reserved core
(vi-control **[excerpt]**). *Low Latency Mode* bypasses the highest-latency plug-ins first to
stay under a limit **[excerpt]**. Workgroups: WWDC20 10224 (D. Wyatt) asks parallel
real-time threads to join the device workgroup and threads **with a different deadline** to
create their own interval (`AudioWorkIntervalCreate`) **[fetched]** — which is what ADR 0031
§2.6 already does for ahead workers.

## 6. Bitwig — sandboxing

Five hosting modes, *Within Bitwig* to *Individually* ("most memory-intensive … but the
safest"); *Reload plug-in* / *Reload all* after a crash; changing modes reloads the engine
(Bitwig user guide, https://bitwig.com/userguide/latest/vst_plug-in_handling_and_options
**[fetched]**). Bitwig 2.5 stopped "allocating a new shared memory area for each plug-in"
**[excerpt]**. *Modern Foundations*: application, engine and plug-ins apart; the engine
reloads alone after a crash; "since Bitwig Studio 5.3, Bitwig's device/DSP code compiles in
parallel" (https://www.bitwig.com/modern-foundations/ **[fetched]**) — native devices are
compiled at load (users call the language *Nitro*; unofficial, KVR **[fetched]**). No public
scheduler design; "multithreading per track" **[excerpt]**.

## 7. Ableton Live — the honest FAQ

"A thread typically processes a single audio signal path segment"; rack chains may get
their own thread; sidechained tracks are one path; "summing points … add 'wait points'" and
the longest path bounds capacity; up to 64 threads; Live "processes audio exclusively on
Performance cores, if available" (Ableton, *Multi-core performance FAQ*,
https://help.ableton.com/hc/en-us/articles/209067649 **[fetched]**). Silent plug-ins are
bypassed after "a while" **when their UI is closed** — generator developers inject a 1/32767
sample to keep them alive (JUCE forum, 2015 **[fetched]**). *Reduced latency when monitoring*
bypasses device latency on monitored tracks **[excerpt]**.

## 8. FL Studio

Multithreaded generator and mixer processing; *Smart disable* "globally disables both
instruments and effects, when inactive" and "can interfere with time-based or long-decaying
effects"; *Triple buffer*; bridging "approx 1-2% per plugin"; plug-ins receive "variable sized
chunks" unless *Use fixed size buffers* (Image-Line manual, `envsettings_audio.htm`,
`plugins/wrapper.htm` **[fetched]**). "Each Mixer Track represents an 'opportunity' to create
these independent tasks" (Image-Line KB 476 **[fetched]**); its threading map shows
depth-layered scheduling with the master last (unofficial analysis **[fetched]**) — a
barrier per layer, coarser than a dependency graph.

## 9. JUCE AudioProcessorGraph and Tracktion Graph

JUCE's graph renders on **one thread**; since JUCE 7 (2023) `rebuild()` runs only on the
main thread, synchronously, asynchronously or by hand (https://docs.juce.com/master/classjuce_1_1AudioProcessorGraph.html
**[fetched]**; a 2024 request for multicore has no answer, forum **[fetched]**). Tracktion
Graph is the multicore one; D. Rowland: waking workers "will vary a lot by OS, buffer size,
power requirements … We have six options … most people say the one using a condition
variable or semaphore combined with a bit of spinning backoff works the best" (JUCE forum,
2021-02-05 **[fetched]**); audio workgroups in Tracktion Engine v3 (forum **[fetched]**).

## 10. What the benchmarks say

DAWbench runs in REAPER, with anticipative FX "across all tracks" (Scan 2021 **[fetched]**).
2025 edition (P. Kaine, Scan, 2025-08-05, https://www.scanproaudio.info/2025/08/05/dawbench-testing-2025-edition/
**[fetched]**): **large L3 caches (X3D) help most at the lowest buffer**, and a single-CCD
9800X3D beat dual-CCD parts at 64 samples — attributed to **inter-CCD latency**; Intel gains
at larger buffers. On Intel 12th gen, REAPER could "fully load up both the performance and
efficiency cores" (Scan 2021-12-01 **[fetched]**). An informal M1 Pro comparison at 128
samples (Pigments instances): REAPER > Logic > Cubase > Live > Studio One > Bitwig, with the
caveat that pre-processing buffers were not matched (KVR, 2023-02-09 **[fetched]**). No
rigorous matched-latency multi-DAW comparison was found — the gap Cantus's published
benchmark (`benchmark.md` §5) can fill.

## 11. What this changes in ADR 0031

1. **Anticipation is the industry's main lever and every leader has it** — REAPER,
   Cubase, Studio One, Pro Tools (since 11), Logic. The ADR is right to build it; the
   evidence argues for building it **sooner** (it is Phase 4, behind parallel export).
   Parallel export and the cache are where Cantus is first; anticipation is where it is
   behind everyone it measures itself against.
2. **Liveness follows the graph down to the master** (REAPER's "dependent on a track that
   is", Logic's live chain to the master). The ADR has this. Its consequence deserves a line
   in the targets: with one monitored track, **the master chain is live** — and Cantus's
   master chain holds the mastering effects (ADR 0026), lookahead limiters and linear-phase
   EQ. The live path's cost floor is the master chain, serial.
3. **A low-latency monitoring rule** — Cubase's *Constrain Delay Compensation*, Logic's *Low
   Latency Mode*, Live's *Reduced latency when monitoring*, Studio One's < 3 ms rule: all
   four leaders bypass or switch latent plug-ins on the live path while monitoring. Cantus
   has none; with a lookahead limiter on its master, a monitored instrument is heard tens of
   milliseconds late. It belongs beside ADR 0022, but ADR 0031 §3.2 should name it as the
   companion of liveness.
4. **Read VST3's `IPrefetchableSupport`** in §3.2's eligibility table: `kIsNeverPrefetchable`
   is live, and a change arrives by `restartComponent`. Today the table names only CLAP's
   `has_hard_realtime_requirement` and the user's switch.
5. **Exact suspension is stricter than the market** — REAPER needs a threshold and offers to
   turn auto-bypass off for offline render; FL and Live warn about long tails; Live keeps a
   plug-in alive while its UI is open. ADR §5.1's `at_rest` is the better method; keep it,
   and keep hosted plug-ins on their own declared tails (unknown or infinite ⇒ never
   suspended).
6. **The meter is two meters** — real-time peak, ahead average (Cubase). The ADR's readout
   says "ahead lead beside the load"; make the load the *live path's peak* and the ahead
   path's average, separately, and keep the per-node blame (Cubase 14 names tracks; Cantus
   names the plug-in).
7. **Topology beyond P/E** — the 2025 DAWbench finding (L3 size, inter-CCD latency at 64
   samples) says the live pool should stay inside **one last-level-cache domain** where the
   machine has several (AMD dual-CCD parts): `cantus-rt::topology` reads cache relationships
   (`GetLogicalProcessorInformationEx`, `RelationCache`) as well as `EfficiencyClass`.
8. **Spin-then-park is what Tracktion's users found best** among six strategies; REAPER
   exposes it as a user knob. Keep §2.4's policy, expose *Eco* as the ADR plans.
