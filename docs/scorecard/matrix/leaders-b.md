# Scorecard research: leaders B and the free DAWs (checked 2026-11-04)

Versions: **Cubase Pro 16.0.0** · **Pro Tools 2026.11** (released 2026-11-03) · **Fender Studio Pro 8.2**
(Studio One Pro, renamed January 2026) · **REAPER 7.84** (2026-10-28) · **Ardour 9.8** (2026-08-23) · **LMMS 1.2.2**
stable (cells name 1.3.0-alpha.2 where it differs) · **Zrythm 1.0.0** stable plus the v2.0.0-DEV manual ·
**Waveform Free 14.1**.

`rows.md` defines **97 IDs** (T1..O4); every one is below in its order. Cell vocabulary: `have`, `partial: …`,
`missing`, a number or fact, or `unverified: why`. In the free columns "(TOC)" means absence inferred from
the manual's table of contents, and "paid exp." means a paid Waveform expansion (from $30).

| ID | Cubase | Pro Tools | Studio One / Fender Studio Pro | REAPER | Ardour | LMMS | Zrythm | Waveform Free |
|---|---|---|---|---|---|---|---|---|
| T1 | have: élastique, MPEX, Standard, Spectral (16.0) modes | have: Elastic Audio, élastique Pro V3 | have: zplane élastique Direct / Formant / Pro Mono | have: élastique 3.3, Rubber Band, SoundTouch, own | partial: manual region stretch (Rubber Band, StaffPad); cues auto-stretch | missing (no time-stretch in docs or releases) | partial: musical mode, offline time-stretch, stretch regions | partial: Warp Time (élastique Pro) in manual; Free engine unverified |
| T2 | have: event pitch shift, envelope-based pitch shift | have: Elastic Properties pitch shift on clips | unverified: event transpose page not read | have: élastique Pro/SOLOIST formant-preserving shift | have: region pitch-shift (features page) | missing for clips; 1.3 alpha adds Granular Pitch Shifter | unverified: not in manual/homepage | unverified: not on Free pages or Warp Time chapter |
| T3 | have: Tempo Track, Process Tempo; ramp detail unverified | have: Tempo Operations linear/parabolic/S-curve ramps | have: Tempo Track; ramps not checked | have: tempo map with linear (ramped) changes | have: tempo map with ramps | partial: tempo automatable; no tempo track | partial: tempo track, experimental (v2 manual) | unverified: no tempo chapter found |
| T4 | have: Signature Track | have: Meter ruler | have: Signature Track (8.2 shows detected key) | have | have: time-signature markers | missing (not in docs; inferred) | partial: tempo track automates signature (experimental) | unverified: not stated |
| T5 | have: Marker Track, cycle markers set locators | partial: Markers/Memory Locations; jump/loop actions unverified | partial: Marker Track; marker actions unverified | have: markers, regions; jump/loop via actions (detail unverified) | partial: location, range, cue markers; no actions stated | partial: loop points only (inferred) | partial: marker track, jump to markers; no actions | partial: Using Markers chapter; actions unverified |
| T6 | have: AudioWarp quantize, hitpoints | have: Elastic Audio event quantize | have: Transient Detection, Bend Markers, Quantize | have: transient guides, stretch-marker quantize | partial: Rhythm Ferret onset split; no grid quantize stated | missing | unverified: not documented | unverified: Warp Time is manual points only |
| T7 | have: Tempo Detection (reworked in 16.0) | have: Identify Beat, Beat Detective | have: Detect Tempo on audio events | partial: detection from time selection only | missing: onset detection only (TOC) | missing; 1.3 alpha adds Tap Tempo | unverified: not documented | unverified: not documented |
| A1 | have: Touch, Auto-Latch, Cross-Over, Trim | have: Write/Touch/Latch/Touch-Latch/Trim | have: Off/Read/Touch/Latch/Write | have: recording modes incl. touch | have: Manual/Play/Write/Touch/Latch | missing (not in docs; inferred) | partial: Touch, Latch; Write not stated | partial: Touch/Latch tagged Pro |
| A2 | have: automation tracks with curves | partial: per-parameter lanes; curve shapes unverified | have: envelopes per parameter; 8.2 trim layer | have: envelopes plus automation items with LFO shapes | have: lanes per parameter, linear/discrete, freehand | have: automation clips, curves (cubic in 1.3) | have: automation regions; lines, ramps, curves | have: lanes; clip automation Pro/paid exp. |
| A3 | have: controller, pitchbend, velocity, articulation lanes | partial: velocity/CC/bend/aftertouch lanes; release velocity unverified | partial: velocity, note controllers; release velocity undocumented | have: CC/velocity/pitch lanes; release unverified | have: velocity, CC, pitch bend, aftertouch | partial: per-note volume/pan; bend lane in 1.3 | partial: velocity editor; CC lanes undocumented | unverified: not documented |
| A4 | have (Pro): LFO, envelope follower, step, random, ModScripter | missing: none in 2024–2026 release notes | unverified: no modulator topic in manual TOC | partial: LFO, MIDI modulation, linking; no step/envelope | missing: modulation is a roadmap item | partial: LFO and peak controllers only | have: modulator track, LFO/envelope, macros | partial: 6 modifier types; more in paid exp. |
| A5 | have: automatable; modulator targeting detail unverified | partial: automatable; no modulation system | partial: automatable; no generic modulators documented | have: automatable and modulatable (CLAP, containers) | partial: automatable; no modulation | have: VST/LADSPA params, controller links | have: automatable and modulatable | have: modifiers target any plug-in parameter |
| P1 | have: VST3 (and VST2) | missing: AAX only | have: VST3 (and VST2) | have | have | missing: VST2 only (VeSTige) | have | have |
| P2 | missing: not in 16.0 manual | missing: AAX only | partial: Linux beta reads ~/.clap; Win/macOS none | have: CLAP 1.2.7 | missing: "no plans" (roadmap) | missing | have | unverified: Free page lists VST, VST3, AU |
| P3 | missing: VST only | missing: AAX only | have | have (macOS) | have (macOS) | missing | have | have (macOS) |
| P4 | missing: plug-in blocklist after crash only | unverified: no plug-in isolation documented | missing: Safety Options dialog after a crash only | have: bridging/"firewalling" (vendor claim) | unverified: manual silent | unverified: VST bridge not documented | have: Carla sandboxing/bridging | unverified for Free; Pro lists sandboxing |
| P5 | partial: Performance Monitor ranks tracks at dropouts | unverified: not found in docs read | partial: Performance Monitor; per-plug-in CPU not stated | have: per-FX CPU in chain and meter | have: Plugin DSP Load window | missing (not in docs) | unverified: not documented | unverified: not documented |
| P6 | have: side-chain inputs | have: key-input sidechain with delay compensation | have: sidechain in plug-in header | have: into any plug-in | have: Sidechaining chapter | partial: peak-controller sidechain only | have: plug-in sidechain ports | have: sidechain list in plug-in header |
| P7 | have: Plug-in Delay Compensation | have: Automatic Delay Compensation | have: automatic plug-in delay compensation | have | have: latency compensation chapter | unverified: not documented | unverified: not documented | have: PDC in all routings |
| P8 | have: ARA Extensions | have: ARA (Melodyne, Synchro Arts, Synth V) | have: ARA event editors (Melodyne) | have: ARA via VST3, pooled edits | missing: ARA on roadmap | missing | unverified: not mentioned | missing: ARA2 is a Pro feature |
| P9 | unverified: no vendor statement | unverified: no statement; stock plug-ins are AAX | unverified: no vendor statement | partial: ReaPlugs, 9 plug-ins, Windows, last 2016 | unverified: no vendor statement | missing | unverified: bundled plug-ins unnamed | unverified: not on Free pages |
| E1 | have: crossfade editor | have: fades/crossfades with curve dialogs | unverified: Events page silent on crossfades | have: crossfade editor | have: fades and crossfades | missing (not in docs) | partial: clip fades; crossfades unverified | unverified: not on Free pages |
| E2 | have: Event Volume Curves | have: Clip Gain line | have: Clip Gain Curves | have: take volume envelopes | have: region gain envelopes | missing | partial: single clip-gain value, no envelope | unverified |
| E3 | have: Sample Editor, Direct Offline Processing | partial: AudioSuite rendering; no Edison-class editor | missing: no destructive editor documented | partial: sample edits in arrange (7.70+); no editor window | missing (TOC) | missing: docs say no recording editing | missing (non-destructive only; inferred) | unverified: no destructive editor chapter |
| E4 | missing: no spectral editor in manual TOC | missing: none in docs read | missing: none in manual | have: spectral editing plus spectral repair (7.75) | partial: RTA/spectrogram; no spectral editing | partial: Spectrum Analyzer only | missing (TOC) | partial: spectrum analyser; no repair |
| E5 | missing: no denoiser in 16.0 Plug-in Reference | unverified: no stock denoiser seen | unverified: no denoiser in pages read | have: ReaFIR subtract mode | missing (none bundled) | missing | unverified: no bundled list | unverified |
| E6 | have: VariAudio | partial: Melodyne via ARA (third party) | partial: bundled Melodyne Essential 5 via ARA | partial: ReaTune manual mode; else ARA | missing: ARA/Melodyne on roadmap | missing | missing (TOC) | missing: Melodyne Essential is a Pro bundle item |
| E7 | have: Pitch Correct (real time) | unverified: none in docs read | have: Vocal Tune | have: ReaTune | missing (none bundled) | missing | unverified: no bundled list | missing: Auto-Tune is a Pro bundle item |
| E8 | have: lanes, takes, assembling a perfect take | have: playlists, loop takes, audio and MIDI comp lanes | have: takes to layers, range promotion | have: fixed-lane comping (7.0) | partial: playlist comping; take management on roadmap | missing | partial: takes into lanes; comping undocumented | have: Comping chapter (edition unmarked) |
| E9 | have: Render Tracks / Render Selection | have: Track Commit, Bounce, Consolidate | have: Bouncing | have: glue, apply FX, subprojects | have: bounce regions/ranges | missing | partial: bounce in v1; not yet v2 | unverified: Mixing Down chapter silent |
| E10 | have: freeze tracks and instruments | have: Track Freeze | have: Track Freeze | have | missing: freeze on roadmap | missing | unverified: not documented | unverified: not documented |
| E11 | have: Inserting Silence; delete-time title not seen | have: Insert Time, Shuffle mode | partial: add/delete time; ripple undocumented | have: ripple editing; insert-time unverified | have: Insert/Remove Time, ripple modes | partial: insert/remove whole bar | unverified: not documented | unverified: not documented |
| E12 | have: Shared Copies | unverified: clip groups not checked | have: shared Clips, Duplicate Shared | have: item groups, pooled MIDI/ARA | partial: dependent MIDI copies; clones on roadmap | have: Beat+Bassline pattern clips | have: linked/cloned objects | partial: edit groups via paid exp. |
| M1 | partial: Scale Assistant, scale snap; ghost notes unverified | unverified: not in docs read | partial: scale highlight, multi-part edit; ghost unnamed | have: key snap with scales; other items shown | have: ghost notes, scales, out-of-scale guard | partial: scale marking; ghost notes in 1.3 | partial: chord/scale highlight; snap undocumented | unverified: not documented |
| M2 | have: Arpache 5/SX, Chorder, Chord Pads | have: Note Stack, BLEASS Arpeggiator, SEQUND Lite | have: Arpeggiator, Chorder, Repeater, Scale Editor | partial: JSFX arpeggiator; chord tools unverified | have: 3 arpeggiators, chord editing | have: arpeggio and chord stacking | have: chord pad, chord track, MIDI FX | partial: 8 MIDI FX; chords in paid exp. |
| M3 | have: Groove Quantize presets | have: Groove Quantize, DigiGroove templates | partial: swing, quantize strength; templates unverified | partial: swing grid; no groove templates documented | unverified: not found in manual | missing (not in docs) | unverified | partial: Groove Doctor via paid exp. |
| M4 | have: Drum Machine + Pattern Editor | partial: GrooveCell 32-step drum sequencer | have: Patterns (drum/melodic steps) | partial: MIDI-editor step input; no channel rack | missing: step entry only (TOC) | have: Beat+Bassline editor | missing (TOC) | have: Step Clips |
| M5 | have: MPE via Note Expression | have: MPE record/edit (2026.11, all tiers) | have: MPE | partial: MPE editing aids only | unverified: not in manual/notes | missing | unverified | partial: MPE in Wavetable (Pro); DAW-wide unverified |
| M6 | partial: per-note controllers, orchestral articulations (16.0) | unverified: not in docs read | partial: macOS/Linux; Windows pending (8.2) | missing: not documented | unverified | missing | unverified | unverified |
| M7 | have: Learn mode, MIDI Remote | partial: MIDI Learn on Avid plug-ins | have: Control Link | have: MIDI/OSC learn | have: Generic MIDI learn | have: Connect to controller | partial: v1 bindings; not yet v2 | unverified: only Pro Drum Sampler |
| M8 | have: Score Editor; MusicXML unverified | have: Score Editor; MusicXML unverified | partial: Score Editor; MusicXML unverified | have: notation, MusicXML import/export | missing: MuseScore link on roadmap | missing | missing (TOC) | missing (TOC) |
| M9 | partial: Micro Tuner, Hermode; Scala/MTS-ESP unverified | unverified | unverified: Scale Editor only | partial: note-name maps; Scala/MTS-ESP undocumented | partial: Scala files; scales 12-TET only | partial: Microtuner in 1.3 alpha only | unverified | unverified |
| X1 | have | have | have | have: 128 channels per track, any routing | have: inserts, sends, any bus | have: mixer, effect chains, sends | have: inserts, sends, any routing | have: buses, submixes, racks |
| X2 | have: VCA Faders | have: VCA Master tracks | have: VCA Faders | have: VCA groups | have: VCA strips | missing | missing: no VCA track | unverified |
| X3 | have: MixConsole Snapshots | partial: snapshot automation; scenes unverified | have: Mixer Scenes | unverified: none native found | have: Mixer Scenes | missing | missing (TOC) | partial: Track Snapshots in paid exp. |
| X4 | have: Loudness Meter, SuperVision | partial: loudness metering; spectrum unverified | have: R128 Level Meter, Spectrum, K-System | have: master LUFS/LRA, spectrum | have: R128 meter, Loudness Analyzer, RTA | partial: spectrum; no loudness meter | unverified | partial: LUFS, spectrum; tier inconsistent |
| X5 | unverified: no reference-track topic; Control Room only | unverified | unverified: none found | partial: lane A/B action; no reference track | missing (TOC) | missing | missing (TOC) | unverified |
| X6 | have: Track Presets, channel strip | have: Track Presets | have: Track Presets, FX Chains | have: track templates, FX chains | have: track templates, strip presets | partial: presets, project templates | partial: project templates | have: racks, presets |
| X7 | have: Dolby Atmos, ADM, Ambisonics | have: Atmos; Studio 7.1.6, Ultimate 9.1.6 | have: Dolby Atmos, ADM, to 9.1.6 | partial: up to 64 ch; ADM metadata ignored | partial: VBAP panning; no Atmos | missing | unverified | partial: 5.1/7.1; no Atmos |
| R1 | have: punch, cycle recording, lanes | have: punch, loop record to alternate takes | have: loop takes to layers; punch unread | have: takes, punch, loop recording | have: punch, loop, layered takes | missing: docs say no audio recording | partial: punch (v1), takes to lanes | have: loop recording, comping, retrospective |
| R2 | have: ASIO Direct Monitoring, software monitoring | unverified: low-latency mode not in docs read | have: Native Low-Latency Monitoring | have: input monitoring with input FX | have: hardware/software monitoring | missing | partial: armed tracks monitor through chain | unverified |
| R3 | have: ASIO-Guard (enlarged in 16.0) | unverified | have: Audio Dropout Protection | have: anticipative FX | missing (TOC) | missing | missing (TOC) | missing (TOC) |
| R4 | partial: external instruments/effects; delay ping unverified | partial: hardware inserts; latency ping unverified | partial: External Effect; ping unverified | have: ReaInsert with ping test | partial: hardware inserts; ping unverified | missing | unverified | unverified |
| R5 | have: Remote Devices (Mackie Control, HUI) | partial: HUI and EUCON; MCU unverified | partial: Mackie Control; no HUI | have: MCU, HUI, OSC | partial: Mackie/Logic Control; no HUI | missing | unverified | unverified: grid controllers only |
| L1 | missing: no clip launcher in manual | partial: Sketch window clips, scenes; follow actions unverified | partial: Launcher; no follow actions | missing | have: Cue window, follow actions | missing | missing (TOC) | partial: Clip Launcher paid exp. |
| L2 | have: Ableton Link (new in 16.0) | unverified | have: with start/stop sync | missing: not documented | missing (inferred) | missing | unverified | partial: via paid exp. |
| L3 | partial: clock and MTC out; MTC in; no clock in | partial: Beat Clock out, MTC chase; clock-in unverified | partial: MTC in/out, clock out; no clock in | have: MTC, LTC, MIDI Clock | have: MTC, LTC, MIDI Clock in/out | missing | unverified | unverified |
| L4 | unverified: no grid-controller topic | unverified | partial: ATOM / ATOM SQ only | missing: not documented | have: Push 2, Launchpad Pro Mk3, Launchkey | missing | unverified | partial: Push 2, APC, Launchpad |
| W1 | have: project templates, track presets | have: session templates, track presets | have: templates, Smart Templates | have | have: session and track templates | have | partial: project templates | have |
| W2 | have: Edit History dialog | have: Undo History | have: Undo History | have: undo history window | missing: undo limit only (TOC) | missing | partial: undo history saved; panel unverified | unverified |
| W3 | partial: key-command search, no action palette | unverified | unverified: none found | have: Action List search | missing (TOC) | missing | unverified | unverified (Pro: Quick Action Bar) |
| W4 | partial: Arranger Track, Track Versions | partial: track playlists, Sketch arrangements | partial: Scratch Pads, Arranger Track | partial: project tabs, subprojects | partial: playlists, arrangement sections | missing | missing (TOC) | partial: Arranger Track |
| W5 | have: MediaBay previewers | have: Workspace Browser, Splice extension | partial: Browser; preset preview undocumented | have: Media Explorer, tempo-matched preview | partial: import-dialog audition, clip libraries | have: sidebar preview | unverified | have: tagged browser |
| W6 | have: Auto Save, backups; crash recovery unverified | unverified | have: autosave, post-crash Safety Options | have: auto-save, timestamped backups | have: backups; crash recovery (9.8) | have: autosave, recovery | have: backups offered on launch | unverified |
| W7 | partial: MIDI Remote API, Logical Editor, ModScripter | have: Pro Tools Scripting SDK; SoundFlow | unverified: no scripting API in manual | have: ReaScript Lua/EEL/Python, C++ SDK | have: Lua | missing | partial: Guile in v1; disabled in v2 | partial: JavaScript controller API (Pro page) |
| W8 | partial: Track Versions, backups | unverified | have: Save New Version / Restore Version | partial: timestamped backups only | have: Snapshots | partial: Save as New Version | unverified | unverified |
| W9 | have: video track, export; 16.0 multi-output | have: Studio 1 video track, Ultimate 64 | have: Video Track, 13 frame rates | have: video items, FFmpeg encode | have: video timeline and monitor | missing | missing (TOC) | partial: video chapter; Pro Video paid |
| X8 | partial: WAV/AIFF/MP3/FLAC/Ogg, stems; no AAC export | partial: WAV/AIFF/MP3, Track Bounce; FLAC/AAC not found | have: WAV/AIFF/FLAC/M4A/Ogg/Opus/MP3, stems | partial: WAV/FLAC/MP3/Ogg/Opus, stems; no AAC | have: WAV/FLAC/MP3/Ogg, FFmpeg, stems | partial: WAV/OGG/MP3, stems; FLAC in 1.3 | partial: mixdown, stems, MIDI; no MP3 | partial: WAV/MP3, per-track render; AAC unverified |
| X9 | partial: format presets, job queue; LUFS export unverified | unverified: loudness-normalised bounce not found | partial: Adjust Loudness presets; no export presets | have: LUFS/true-peak normalise, render queue | have: format profiles, Loudness Assistant | missing (not in docs) | missing | have: LUFS normalising, true-peak ceiling |
| X10 | have: import and export | missing: not in docs read | partial: import only | missing | missing: on roadmap | missing | missing (TOC) | partial: in manual; Pro-tagged |
| X11 | have: AAF and OMF import/export | have: AAF/OMF interchange | partial: AAF; OMF undocumented | missing (EDL import only) | partial: AAF import only | missing | missing (TOC) | missing |
| I1 | unverified count; Groove Agent SE 6, Drum Machine, Omnivocal | unverified count; Xpand!2, PlayCell, GrooveCell, SynthCell, players | 9 per vendor (manual lists 10) | 4: ReaSynth, RS5K, ReaSynDr, Spectral Hold | 3: ACE Fluid Synth, GM Synth, Reasonable Synth | 19 native (SF2, GIG, ZynAddSubFX, chips) | unverified: "a few bundled plugins" | 4: 4OSC, Micro Drum Sampler, Micro Sampler, Rompler |
| I2 | partial: Sampler Track; SFZ/SF2 import unverified | partial: PlayCell, Kontakt Player; SFZ/SF2 unverified | unverified: Presence/SampleOne not read | partial: RS5K velocity split; no SFZ/SF2 | partial: SF2 via ACE Fluid Synth; no SFZ | partial: SF2 and GIG; no SFZ | have: SFZ/SF2 as plug-ins | missing: SFZ/SF2 only in Pro |
| I3 | have: Sampler Track slicing | unverified | unverified: slicing pages not read | partial: REX import, dynamic split | missing (none bundled) | partial: SlicerT in 1.3 alpha | unverified | partial: slicing in paid exp. |
| I4 | have: Drum Machine, Groove Agent SE; outputs unverified | partial: GrooveCell 16 pads, 3 layers; outputs unverified | partial: Impact; per-pad outputs unverified | partial: ReaSynDr, RS5K, choke groups | missing (none bundled) | missing: Kicker synth only | unverified | partial: Micro Drum Sampler, 64 pads |
| I5 | unverified: instrument docs not read | unverified | unverified: Presence content not checked | missing | missing | missing | unverified | unverified |
| I6 | unverified: instrument docs not read | unverified | unverified | missing | missing | partial: Watsyn/BitInvader drawn waves | unverified | missing: Wavetable is Pro |
| I7 | unverified: instrument docs not read | unverified | unverified | missing | missing | partial: OpulenZ (OPL2 FM) | unverified | unverified |
| I8 | unverified: instrument docs not read | unverified | unverified | partial: Spectral Hold; no granular | missing | missing | unverified | unverified |
| I9 | unverified: instrument docs not read | unverified | unverified | missing | missing | have: Vibed, Mallets | unverified | unverified |
| I10 | have: Omnivocal (beta, Yamaha), all editions | missing stock; hosts Synthesizer V via ARA | unverified: none seen | missing | missing | missing | unverified | missing |
| F1 | 93 audio (incl. SuperVision) + 17 MIDI | unverified count; D-Verb, Maxim, EQ III, Dynamics III, Pitch II | "over 45" (vendor) + amps; Pro+ MixFX | 18 ReaPlugs + 4 utilities + hundreds JSFX | 7: comp, expander, delay, reverb, filters, EQ | 17 native + LADSPA; 1.3 adds ~8 | unverified: plug-ins unnamed | 14 audio + 8 MIDI + utilities |
| F2 | have: Frequency 2 per-band linear phase or dynamic | unverified: only EQ III seen stock | partial: Pro EQ3 dynamic; linear phase on low-cut | partial: ReaFIR FFT EQ; no dynamic EQ | missing | missing | unverified | partial: 8-band EQ, M/S; not linear/dynamic |
| F3 | have: MultibandCompressor, Expander, EnvelopeShaper | missing stock: Pro Multiband sold separately | have: Multiband Dynamics, Tricomp | have: ReaXComp | missing | partial: LOMM in 1.3; Crossover EQ | unverified | missing |
| F4 | have: REVerence convolution; REVelation, RoomWorks | partial: D-Verb; Space convolution sold separately | have: Open AIR2 convolution plus algorithmic | have: ReaVerb, ReaVerbate | partial: ACE Reverb; no convolution | partial: ReverbSC only | unverified | have: impulse response plus 3 algorithmic |
| F5 | have: Maximizer, Brickwall Limiter, Lin One Dither | partial: Maxim; Pro Limiter sold separately | have: Mastering page, Limiter², Multiband | partial: ReaLimit; no mastering suite | partial: Loudness Assistant; no maximizer | missing | unverified | partial: Limiter, LUFS export; no maximizer |
| C1 | unverified: steinberg.net is JS-only | unverified | "40+ GB instrument soundsets" (vendor) | none documented | minimal: GeneralUser GS soundfont | bundled; size unverified | none stated | minimal; packs sold |
| C2 | partial: Waves Illugen sample service (16.0) | have: Splice extension, 2,500+ free samples | have: Splice integration | missing | unverified | missing | missing | have: Download Manager; paid packs |
| AI1 | have (Pro): stems; 16.0 adds piano, guitar | have: Stem Separator, up to 6 stems, local | have: 6 stems, local | missing | missing | missing | missing | partial: via paid DJ Mix Tools |
| AI2 | partial: VariAudio Extract MIDI (monophonic), hitpoints to MIDI | have: Extract MIDI (Melodyne engine); polyphony unverified | have: audio-to-MIDI, local (vendor) | missing | missing | missing | missing | unverified |
| AI3 | missing: none in manual | missing: none documented | unverified: none found | missing | missing | missing | missing | partial: Mix Analysis, /mixcheck |
| AI4 | missing: Chord Pads players only, not AI | missing: none documented | missing: Chord Assistant only | missing | missing | missing | missing | partial: Pattern Generator; not chord-following |
| AI5 | missing | partial: Scripting SDK, Extensions; no MCP | missing: Studio Assistant is help chat | missing: scripting only, no agent API | have: experimental MCP HTTP server (9.5) | missing | missing | partial: LLM Assistant; no MCP stated |
| O1 | proprietary | proprietary | proprietary | proprietary | open source, GPL v2 | open source, GPL | open source, AGPL-3.0 | proprietary freeware |
| O2 | Pro €579 / $579.99 (15.0 list; 16.0 unverified) | unverified: subscription; Ultimate promo $99/month seen | $199.99 perpetual; $19.99/month; Pro+ $179.99/year | $60 discounted, $225 commercial | binaries $1+ once or $1–50/month | free | trial 25 tracks; $19/$31/$50 per year | free; Pro from $249 |
| O3 | Windows, macOS | Windows 11, macOS 14–26 | Windows, macOS; Linux public beta | Windows, macOS, Linux | Windows, macOS, Linux | Windows, macOS, Linux | Windows, macOS, Linux | Windows, macOS, Linux |
| O4 | unverified | unverified | unverified | unverified | unverified | unverified | unverified | unverified |

**Unverified cells** (starting with `unverified`, O4 included): Cubase 11 · Pro Tools 30 · Fender Studio Pro 19 ·
REAPER 2 · Ardour 7 · LMMS 3 · Zrythm 40 · Waveform Free 37, which is 149 of 776. More cells carry a narrower
"detail unverified" clause inside a `have` or `partial`.

## Sources

### Cubase Pro 16.0.0 (Steinberg)
- **Version:** the online manual's New Features chapter, "New Features in Version 16.0.0":
  https://www.steinberg.help/r/cubase-pro/16.0/en/cubase_nuendo/topics/new_features/new_features.html?contentId=lr1ENzAuRdsoSM~D_mz~yQ
  The 16.0 additions include Ableton Link, Spectral AudioWarp modes, better stem separation (piano and guitar
  added), instrument stacks, VocalTouch, DrumChain, Windows MIDI Services, MIDI 2.0 per-note controllers and
  orchestral articulations, and a bigger ASIO-Guard buffer.
- **Manual table of contents** (16.0 and 15.0 read in five chunks with keyword queries):
  https://www.steinberg.help/r/cubase-pro/16.0/en and https://www.steinberg.help/r/cubase-pro/15.0/en
- **Manual pages read:**
  - Synchronization Sources page: https://www.steinberg.help/r/cubase-pro/15.0/en/cubase_nuendo/topics/synchronization/synchronization_setup_sync_inputs_r.html?contentId=riJwOiQw0tY30_yoNjLAag (MTC, ASIO positioning and VST System Link; no MIDI clock).
  - Synchronization Destinations page: …/synchronization_setup_sync_outputs_r.html?contentId=Bed5lLN6IO2qdKdTITq2kA (MIDI clock and MTC out).
  - Audio Performance Monitor: …/optimizing/optimizing_vst_performance_window_r.html?contentId=YBgOq0ZeI3f5AR8pWF4Z6A
  - Range editing: …/range_editing/range_editing_c.html
- **Plug-in Reference 16.0** (counted 93 audio effects, the SuperVision analyser among them, and 17 MIDI effects;
  no denoiser): https://www.steinberg.help/r/cubase-pro/cubaseplugref/16.0/en
  Frequency 2 (per-band linear phase or dynamic): https://www.steinberg.help/r/cubase-pro/cubaseplugref/15.0/en/_shared/topics/plug_ref/frequency/frequency_r.html?contentId=0nysvyZ7CbaAYia6hIfg9g
- **Cubase 15 press release (2025-11-05):** edition tags (Pro / Artist / Elements), price, Omnivocal, DAWproject:
  https://ocl-steinberg-live.steinberg.net/_storage/asset/818311/storage/master/Press%20Release%20-%202025-11-05%20-%20Cubase%2015%20-%20EN.pdf
- **Mackie Control / HUI:** Steinberg's Remote Control Devices listing, from the archived Pro 9 help:
  https://archive.steinberg.help/cubase_pro_artist/v9/en/cubase_nuendo/topics/remote_control/remote_control_device_panel_parameters_assigning_c.html
  The 16.0 TOC still has "Connecting / Setting up Remote Devices".
- **Not reachable:** steinberg.net product, compare-editions and release-notes pages are JavaScript-only, so
  WebFetch got no content (instrument count, library size, 16.0 price and system requirements).

### Pro Tools 2026.11 (Avid)
- **Release notes 2026.11** (released 2026-11-03; MPE, Extensions, Stem Separator, all tiers):
  https://kb.avid.com/pkb/articles/Knowledge/Pro-Tools-2026-11-Release-Notes
- **What's New in Pro Tools 2026.11** (PDF, read in full: Demucs-based stem separation run locally with 4/6-stem
  models, MPE, the élastique Pro V3 credit): https://resources.avid.com/SupportFiles/PT/Whats_New_in_Pro_Tools_2026.11.pdf
- **Release notes 2026.4** (system requirements: Windows 11 24H2/25H2, macOS 14.8–26.3; tier restrictions):
  https://kb.avid.com/pkb/articles/Knowledge/Pro-Tools-2026-4-Release-Notes
- **Release notes 2025.6** (Splice, ARA for Synthesizer V / Sync Vx, Speech to Text Studio/Ultimate):
  https://kb.avid.com/pkb/articles/Knowledge/Pro-Tools-2025-6-Release-Notes
- **Documentation index** (Reference Guide 2026.11 and earlier):
  https://kb.avid.com/pkb/articles/en_US/user_guide/Pro-Tools-Documentation
- **Read only through search excerpts of resources.avid.com PDFs:**
  - What's New 2023.12 (Sketch, Atmos renderer Studio/Ultimate), 2024.3 (Note Stack), 2024.10 (BLEASS
    Arpeggiator, SEQUND Lite, MIDI comp lanes, Kontakt Player).
  - Audio Plug-Ins Guide 2023.3 (included vs sold-separately list): https://resources.avid.com/SupportFiles/PT/Audio_Plug-Ins_Guide_2023.3.pdf
  - EUCON Commands 2025.6 / 2025.12 (automation modes, VCA, snapshots, MIDI Beat Clock).
  - GrooveCell / PlayCell / SynthCell guides.
  - KB "Pro Tools Plugin Compatibility" (AAX only): https://kb.avid.com/pkb/articles/en_US/Knowledge/en343311
  - KB "Session Compatibility and Interchange" (AAF/OMF): https://kb.avid.com/pkb/articles/en_US/Knowledge/en353093
  - Scripting SDK: https://developer.avid.com/scripting/
- **Not reachable:** every www.avid.com page (comparison, what's new, store, press room) answers 403 to WebFetch.
  The tier and price cells rest on search excerpts of those pages, and price is left unverified. The Reference
  Guide PDF could not be read through the tools allowed.
- **Edition differences:** Studio is capped at 512 audio tracks with 1 video track and immersive up to 7.1.6
  (one Avid page says 7.1.4). Ultimate has 2,048 tracks, 64 video tracks and 9.1.6. Speech to Text, Batch Rename,
  the MPEG-H/Audio Vivid panner and full Clip FX are Studio/Ultimate only. Artist plays Clip FX back only.

### Fender Studio Pro 8.2 (ex-Studio One Pro)
- **Rename:** Fender newsroom, January 2026:
  https://www.fender.com/newsroom/270270-fender-unveils-connected-music-ecosystem-with-fender-studio-pro-audio-interfaces-and-controllers/
  Sound On Sound dates it 13 January 2026: https://www.soundonsound.com/news/studio-one-becomes-fender-studio-pro
- **8.1 / 8.2 features:**
  - https://www.fender.com/newsroom/270193-introducing-fender-studio-pro-8-1-keeping-artists-at-the-core-of-the-creative-process/
  - https://www.fender.com/newsroom/273556-fender-studio-pro-8-2-expands-mixing-and-creative-production-tools/ (dated 2026-10-20)
- **Price and counts:** https://www.fender.com/products/fender-studio-pro and https://www.fender.com/pages/fender-studio-pro.
  fender.com answers 403 to WebFetch, so these facts come from search excerpts of these exact URLs.
- **Manual (8.2):**
  - TOC: https://fenderstudiopromanual.fender.com/en/Data/Tocs/Fender_Studio_Pro_User_Manual_Online_Chunk0.js
  - About 25 pages under https://fenderstudiopromanual.fender.com/en/Content/ : Timestretching, Set_Up_Your_MIDI_Devices
    (MPE, MIDI 2.0), Advanced_Options (sync, Link), Patterns, Stem Separation, Vocal Tune, Comping,
    Automation_Modes, The Launcher, Scenes, Metering, Mackie_Control, Mixing_Down, Importing_Other_Application,
    Saving Options, Recovery_options, Sync_to_Video, EQ/Dynamics/Mastering/Reverb, System_Requirements.
- **CLAP on Linux:** https://support.presonus.com/hc/en-us/articles/19214558269581-Linux-Getting-Started (search excerpt)

### REAPER 7.84 (Cockos)
- **Version and platforms:** https://www.reaper.fm/download.php (7.84, 2026-10-28; Windows, macOS, and Linux on
  x86_64/i686/aarch64/armv7l)
- **Full feature list:** https://www.reaper.fm/about.php · price: https://www.reaper.fm/purchase.php ·
  ReaPlugs for other hosts: https://www.reaper.fm/reaplugs/
- **Changelog:** https://www.reaper.fm/whatsnew.txt, read from 7.84 back to 4.57. The cells cite entries such as
  7.75 (spectral repair), 7.70 (sample editing), 7.68 (CLAP 1.2.7), 7.0 (fixed-lane comping, ARA pooling),
  6.71 (BW64 "ADM metadata is ignored"), 6.47/6.67 (per-FX CPU), 6.30 (LUFS, normalise, VST3 firewalling),
  5.50 (spectral editing) and 4.611 (ReaInsert ping).
- **User Guide:** the 7.84 edition (29 MB PDF) is over the fetch limit and was not read.

### Ardour 9.8
- What's new: https://ardour.org/whatsnew.html
- 9.0 notes: https://ardour.org/news/9.0.html
- 9.5 notes (MCP HTTP server, ghost notes, chord sidebar): https://ardour.org/news/9.5.html
- Manual TOC: https://manual.ardour.org/toc/ , with pages on plug-ins, DSP load, automation, interchange, export,
  stretching and sync
- Features: https://ardour.org/features.html · roadmap: https://ardour.org/future.html ·
  price: https://community.ardour.org/download (search excerpt)

### LMMS 1.2.2 (stable, 2020-06-25) and 1.3.0-alpha.2
- Releases: https://github.com/LMMS/lmms/releases (v1.2.0 / v1.3.0-alpha.1 / alpha.2 notes)
- Home: https://lmms.io/
- Manual: https://docs.lmms.io/user-manual/llms.txt , with pages on the piano roll, automation, instrument
  window, mixer, song editor, MIDI, exporting and VST

### Zrythm 1.0.0 (2024-11-21) and v2.0.0-alpha.7
- Home: https://www.zrythm.org/en/index.html · download and prices: https://www.zrythm.org/en/download.html ·
  v2 alpha: https://www.zrythm.org/en/download-v2-alpha.html
- README (AGPL-3.0; the list of v1 features not yet ported to v2): https://github.com/zrythm/zrythm
- Manual (v2.0.0-DEV): https://manual.zrythm.org/en/index.html

### Waveform Free 14.1 (Tracktion)
- Free: https://www.tracktion.com/products/waveform-free and https://www.tracktion.com/products/waveform-free-features
- Pro and what's new: https://www.tracktion.com/products/waveform-pro-features and
  https://www.tracktion.com/products/waveform-pro-whats-new
- Expansions: https://www.tracktion.com/products/waveform-compare-expansions
- Manual v14: https://tracktion.github.io/waveform_manual/latest/

## Notes

- **Newer versions than the brief named.** Cubase **16.0.0** is out; its manual exists, but its release date and
  price could not be read because steinberg.net is JS-only. Pro Tools **2026.11** shipped the day before this check.
  Studio One is now **Fender Studio Pro 8.2**, renamed January 2026, with 8.2 dated 2026-10-20. Cubase 16 changed
  three cells against 15: Ableton Link (now `have`), MIDI 2.0 (now partial) and stem separation with piano/guitar.
  Pro Tools 2026.11 changed three: stem separation, MPE, and Extensions. Its Stem Separator is documented as
  available in all tiers, Intro included.
- **Nuendo was not checked** (time budget). The Cubase column is Cubase Pro only. Rows Cubase lacks (CLAP, AU,
  clip launcher, spectral editor, denoiser, mastering assistant) are worth one Nuendo 14/15 manual pass before
  the matrix is published.
- **Marketing vs documented:**
  - **Pro Tools:** "Stem Separator in all versions" is stated in Avid's own What's New PDF. Studio vs Ultimate
    immersive formats differ between two Avid pages (7.1.4 vs 7.1.6). Space, Pro Limiter and Pro Multiband
    Dynamics are sold separately and are not stock, which weakens Pro Tools on F2–F5.
  - **Fender Studio Pro:** marketing says 9 instruments; the manual TOC has 10. The bass-amp count is 18 on one
    Fender page and 39 on another. Audio-to-MIDI and Chord Assistant are on the product page but have no manual
    topic. DAWproject is import-only in the manual although Studio One 6.5 marketing claimed export. Linux is a
    public beta.
  - **REAPER:** "hundreds" of JSFX gives no count. P4 rests on the vendor's "firewalling" claim. ReaPlugs, the
    only effects usable in other hosts, are Windows-only and last updated in 2016.
  - **Waveform:** Tracktion's pages contradict each other on effect counts (14 vs 15) and on edition tags. Many
    features are paid expansions on top of Free. **On Linux, Waveform Free unlocks Pro**, per Tracktion; the
    column describes the Windows/macOS Free edition.
- **Surprises:**
  - **Ardour 9.5 ships an experimental MCP HTTP server,** the only documented agent interface in these eight
    columns (AI5).
  - Ardour 9.x added ghost notes, a chord sidebar, scales and arpeggiators. CLAP, freeze, comping, ARA and
    DAWproject are still roadmap items.
  - REAPER added sample-level editing and spectral repair in 2026 (7.70–7.75). Its "MPE" is only editing aids,
    and it ignores ADM metadata, so it has no Atmos.
  - Cubase still has no clip launcher, no CLAP, no AU and no MIDI-clock input (the manual's Sources page lists
    MTC, ASIO positioning and VST System Link only).
  - Fender Studio Pro has no MIDI-clock input either; its manual says it syncs to MTC instead. It supports Mackie
    Control only, without HUI.
  - The LMMS manual says LMMS cannot record audio.
  - Zrythm v2 (a rewrite) has not yet ported stem export, bounce, MIDI binding, aux sends and punch recording.
- **How absences were judged:** `missing` means the feature appears in none of the vendor documents read: the
  manual TOC, the full feature list, or the changelog. No vendor states an absence outright except where a cell
  quotes it (Avid "AAX only", REAPER "ADM metadata is ignored", LMMS "can't record").
- **Fetch limits, stated plainly:** www.avid.com, fender.com, support.presonus.com and steinberg.net product pages
  blocked or rendered nothing to WebFetch. For Avid and Fender, cells rest on search-engine excerpts of those
  vendor URLs (named above). Avid's and REAPER's full reference PDFs were too large to read. Nothing was downloaded
  on purpose and nothing was run.
- **O4 (bit-exact offline = real time)** is not stated by any of the eight vendors, so it is `unverified` everywhere.
- **Helpers:** the Fender Studio Pro, REAPER and free-DAW columns were researched by three helper agents under
  the same rules. I merged their results without re-checking each cell; their raw notes are in the session
  scratchpad.
