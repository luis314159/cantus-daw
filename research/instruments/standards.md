# Plug-in standards: hosting third-party plug-ins and exporting our own

*Researched 2026-09-24 for Cantus. Sources: the official specifications and SDK repositories (Steinberg's VST 3
developer portal and GitHub, free-audio/clap, Apple's developer forums, bitwig/dawproject), the LICENSE files of
every crate named (read through GitHub's licence API or the raw file), crates.io, vendor manuals and press. The
codebase was read at the same date: ADR 0001, 0002, 0005, 0006, 0017 and 0018, `cantus_engine::{fx, processor,
effects, render}` and `cantus_core::{effect, instrument, snapshot}`. Everything is paraphrased. Where something
could not be verified, the text says so. No screenshots were taken for this report.*

---

## 0. The short version

1. **Host CLAP and VST 3; add AU on macOS; skip VST 2 and AAX.** CLAP (spec 1.2.10, MIT) is loaded by Bitwig,
   Reaper, FL Studio and Studio One, and has what instruments need (note ids, note expressions, polyphonic
   modulation, explicit thread roles). VST 3 (SDK 3.8.1) is what nearly every commercial plug-in ships, and it is
   the only one of the two that Live and Cubase load. VST 2 was withdrawn in 2018 and was not relicensed. AAX needs an Avid agreement and
   PACE signing. Logic loads only AU.
2. **VST 3 is MIT now, so ADR 0001's main reason for a GPL host is gone.** Since SDK 3.8.0 (2025-10-20) the SDK is
   MIT ("Licensing under GPLv3 and the Steinberg proprietary license is no longer available"); VSTGUI is BSD-3;
   the logo is optional. ASIO went the other way, to **GPLv3 or proprietary**, and Ableton Link is GPLv2+ or
   proprietary. **Recommendation:** keep the application GPL-3 (it can then use ASIO and Link freely), put plug-in
   hosting in **MIT/Apache** crates beside the engine, and decide the licence of exported editors when export
   starts. The DSP of an exported plug-in can be MIT/Apache; an editor taken from `cantus-ui` makes it GPL-3.
3. **The Rust crates are ready enough, and all permissive:** **clack** 0.2.0 (CLAP host and plug-in, MIT/Apache,
   on crates.io since May 2026, CLAP 1.2.2 through clap-sys) and the **`vst3`** crate 0.3.0 (MIT/Apache, bindings
   pregenerated from the MIT 3.8.0 headers). Avoid `vst3-sys` (GPL-3, unmaintained). Both are 0.x and low-level:
   the threading, event lists, sandbox and editor windows are Cantus's to write. There is prior art to read
   (clack's cpal host example, clap-validator, vst3-host, and Maolan, a Rust DAW with out-of-process plug-ins).
4. **For export, go CLAP first on clack-plugin, with clap-wrapper for VST 3 and AU.** Cantus's parameters are a
   runtime `key → f32` table with stable string keys, which is CLAP's model. A key becomes a fixed 31-bit id,
   valid for both CLAP and VST 3. nih-plug is in maintenance mode, and its VST 3 builds are still GPL. Its
   community fork **nice-plug** (ISC, egui 0.36) is the fallback. **egui-baseview 0.7.2 is on egui 0.36.1**,
   Cantus's version, so the existing editors can run inside a plug-in once they depend on a small `FxHost` trait
   instead of the app.
5. **The engine already has the right shape.** Units are built on the UI thread and handed over with
   `AddEffect`/`GarbageEffect`, and latency feeds ADR 0018's per-edge compensation. Four changes are needed:
   - a main-thread callback pump in the eframe loop;
   - latency cached for the audio thread;
   - a `ProcessContext` (transport) in `FxUnit::process`;
   - hosted units processed **per whole block with timestamped events**. Today the strip loop runs inside the
     note-event segment loop, so an insert can be called for a single sample.

   The last change comes first, as a separate step held by every existing null test.
6. **Scan in a separate process with a cache from day one; isolate plug-ins by choice, and automatically after a
   crash.** CLAP descriptors and VST 3 `moduleinfo.json` make rescans cheap.
   - Isolation should be lock-step shared memory with no added latency, as every surveyed host does it. Apple
     quoted about 40 µs per render cycle for this; FL quotes 1–2 % CPU per bridged plug-in.
   - Plug-in editors should be **top-level windows**, one per plug-in. That is the only design that works the
     same in and out of process on every OS. It also allows a per-plug-in DPI mode on Windows.
7. **Where Cantus can beat every host surveyed:**
   - per-*plug-in* CPU and latency in the mixer (Reaper shows CPU per track; Bitwig shows only a global meter);
   - crashes that name the plug-in and reopen the project safely;
   - isolation that turns itself on after a crash;
   - one undo history that includes plug-in changes;
   - a generic editor that the AI bridge and automation also use;
   - host-side A/B, presets and oversampling for any plug-in;
   - preset search across plug-ins through CLAP preset discovery.
8. **First three steps:**
   1. whole-block unit processing plus `ProcessContext`, held bit-identical by the null tests;
   2. an in-repo set of CLAP test plug-ins (pass-through, delay, instrument, crash, hang), checked by
      clap-validator in CI;
   3. an out-of-process scanner with a cache and a read-only plug-in list.

   Hosting a CLAP effect with a generic editor comes next. Its gate is a bit-exact null through the pass-through
   plug-in, plus the ADR 0018 tests through the delay plug-in. The strongest test comes later: Cantus hosting its
   own exported plug-in must null against the native unit.

---

## 1. Formats and host support in 2026

### 1.1 The formats

| Format | Owner, current version | Licence of the API | Platforms | What defines it | State in 2026 |
|---|---|---|---|---|---|
| **VST 3** | Steinberg; SDK **3.8.0** (2025-10-20), **3.8.1** (2026-08-11) | **MIT** since 3.8.0 (§2) | Windows, macOS, Linux | processor/controller split (`IComponent` + `IAudioProcessor` / `IEditController`, talking through `IConnectionPoint`); **normalised** parameters 0–1 with sample-accurate change queues; buses (main, aux for sidechain, event buses); note expression; `IMidiMapping` instead of MIDI CC; latency and tail reporting; `.vstpreset`; `moduleinfo.json` for scanning without loading | the market's default format; every major host except Logic and Pro Tools loads it |
| **CLAP** | free-audio (started by u-he and Bitwig); spec **1.2.10** (2026-07-13) | **MIT** | Windows, macOS, Linux | a C ABI with a small core and extensions; **plain** parameter values with sample-accurate events; per-voice (polyphonic) modulation and note expressions keyed by note id; explicit thread roles; `thread-pool` for host threads; preset-discovery factory for scanning presets without instances | 19 host entries on clapdb.tech (Sep 2026); Wikipedia's count, of uncertain date, says 15 DAWs and 394 plug-ins from 93 makers |
| **VST 2** | Steinberg | proprietary; SDK withdrawn in 2018, **not** relicensed with 3.8 | — | — | still loaded by FL, Reaper, Bitwig, Live, Studio One; **off by default in Cubase 14/15**; new developers cannot license it |
| **Audio Unit v2 / v3** | Apple | Apple SDK terms; AUv3 is an app extension | macOS (v2, v3), iOS (v3) | v2 loaded in process via AudioToolbox; v3 runs **out of process by default** over XPC and loads in process only if both host and plug-in opt in | required for Logic; Live, Reaper, FL (mac), Studio One also load AU |
| **LV2** | lv2plug.in community | ISC | mostly Linux | RDF (Turtle) metadata, URID-mapped events, atom ports | Ardour, Reaper, Zrythm, Qtractor, Carla; Ardour declines CLAP because "CLAP offers nothing in an Ardour context that is not offered by LV2" (Paul Davis, quoted by Libre Arts) |
| **AAX** | Avid | **GPLv3 or commercial** SDK; release builds must be **PACE/iLok-signed** for retail Pro Tools | Windows, macOS | Native and DSP variants | Pro Tools only; Pro Tools loads nothing else |

CLAP's recent stable additions (from its ChangeLog): 1.2.0 made `param-indication`, `remote-controls`,
`preset-discovery` and others stable; later releases added the drafts `undo`, `scratch-memory`,
`project-location`, `gain-adjustment-metering`, `mini-curve-display`, `webview`, `octave-number`, `param-origin`,
background activation/state/progress (1.2.9) and `param-hovered` (1.2.10). The stable extensions in
`include/clap/clap.h` are: ambisonic, audio-ports(-activation, -config), configurable-audio-ports, context-menu,
event-registry, gui, latency, log, note-name, note-ports, param-indication, params, posix-fd-support,
preset-load, remote-controls, render, state(-context), surround, tail, thread-check, thread-pool,
timer-support, track-info and voice-info. `tuning` and `undo` are still drafts.

VST 3.8 added `IMidiMapping2`/`IMidiLearn2` (MIDI 2.0), Wayland interfaces as a preview, and VSTGUI 4.15;
3.8.1 added `ITransportControl` (a plug-in may ask the host for transport actions), MIDI 2.0 per-note-controller
note-expression ids, an Orchestral Articulation interface, a "Clipboard VST-XML" exchange format, and a validator
check that parameter value↔string conversions round-trip.

### 1.2 What the hosts load (latest versions, September 2026)

| Host (version seen) | VST 2 | VST 3 | CLAP | AU | AAX | Other | Sandbox |
|---|---|---|---|---|---|---|---|
| **FL Studio 2026.1** | yes | yes | **yes** (since FL 2024; first in the 21.3 betas) | macOS | — | FL native | per-plug-in "Make bridged" (≈1–2 % CPU each, per the manual) |
| **Bitwig Studio 6.x** | yes | yes | **yes**, the reference host | **no** | — | — | five modes, default "Together" (§6.1) |
| **REAPER 7.80** | yes | yes | **yes** | macOS | — | LV2, JSFX, DX | per plug-in: separate or dedicated process, "no additional latency" |
| **Ableton Live 12.4** | yes (64-bit) | yes | **no**, nothing announced | AUv2, AUv3 | — | — | none |
| **Logic Pro 12** | — | — | — | **AU only** | — | — | AUs hosted out of process on Apple silicon (AUHostingService; forum-sourced) |
| **Cubase 15** | off by default | yes | **no** | — | — | — | scan sentinel + blocklist; no run-time sandbox |
| **Studio One Pro 7 / Fender Studio Pro 8** | yes | yes | **yes** since 7.0 (Oct 2024) | macOS | — | Linux build | scanner blocklist; no run-time sandbox |
| **Pro Tools 2026.4** | — | — | — | — | **only** | — | — |
| **Ardour 9.8** | yes | yes | no, by choice | macOS | — | LV2, LADSPA | — |
| **Renoise 3.5** | yes | yes | no | yes | — | — | sandboxed plug-ins |
| **Waveform 14, Cakewalk Sonar, Digital Performer 12, LUNA** | mostly yes | yes | **not confirmed** for any | macOS | — | — | — |
| **Zrythm (v2 alpha), MultitrackStudio, MuLab, n-Track, Qtractor, Carla** | varies | yes | **yes** | varies | — | LV2 (Linux hosts) | Carla bridges |

**Host features that matter to an instrument host:**

| Host | CLAP poly-mod | MPE | MIDI 2.0 | DAWproject |
|---|---|---|---|---|
| Bitwig | yes (reference) | yes ("Force MPE mode" per plug-in) | internal only (forum) | yes, since 5.0.9 |
| FL Studio | not verified | not verified | none found | — |
| Ableton Live | n/a | yes since 11 | none found | — |
| Logic Pro | n/a | not verified | reported working (Microsoft's Pete Brown, forum) | — |
| Cubase 15 | n/a | not verified | partial (MIDI 2.0 controllers in 15) | yes, since 14 |
| Studio One / Studio Pro | not indicated | yes | unclear | yes, since 6.5 |
| Waveform 14 | — | — | — | yes (new) |

### 1.3 Related interop, as far as it touches plug-ins

- **MIDI 2.0.** Windows MIDI Services became generally available in February 2026; Cubase 15 handles MIDI 2.0
  controllers in part, Logic and MultitrackStudio are reported working (Microsoft's Pete Brown, on a forum), and
  no support was found in Reaper or Live. In the formats: CLAP has a `MIDI2` note dialect (`CLAP_EVENT_MIDI2`,
  UMP words); VST 3.8 added `IMidiMapping2`/`IMidiLearn2` and 3.8.1 per-note-controller note-expression ids.
  **Consequence:** Cantus's internal note model should be per-note expression keyed by note id (§5.2); MIDI 2.0,
  MPE and note expression are then encodings chosen per plug-in, and a MIDI 2.0 *input* path (the `midi2` crate)
  can come later without touching the plug-in layer.
- **MPE** is supported by Live (since 11), Bitwig (with a per-plug-in "Force MPE mode") and Studio One. It is the
  lowest common denominator for expressive third-party instruments and must be one of the encodings.
- **Ableton Link** is in Bitwig, Studio One 7, Pro Tools and Logic, not in Cubase or FL; Live 12.4 (May 2026) added
  *Link Audio*. It is a transport feature, not a plug-in one; its licence (GPLv2+) is the only thing that touches
  this report (§2.1).
- **DAWproject** (MIT, format 1.0) is read and written by Bitwig, Studio One, Cubase 14+, Cubasis, VST Live,
  n-Track and, since 2026, Waveform 14; not by Live, FL or Reaper (only through third-party converters). It stores
  each plug-in's state as a file referenced from `<ClapPlugin>`, `<Vst3Plugin>` or `<AuPlugin>`, and defines
  built-in `Equalizer`, `Compressor`, `NoiseGate` and `Limiter` devices with named parameters (bands with
  frequency, gain, Q). **Consequences:** store hosted plug-in state as the plug-in's own bytes (§3.7), and keep in
  mind that Aequalis, Comprimo, Porta and Terminus have a DAWproject counterpart for a later export (the user
  deferred interop to the end of the plug-in round).

**What this says for Cantus.** CLAP and VST 3 together cover every host that matters for *hosting* third-party
plug-ins on Windows and Linux; AU is required on macOS (many Mac-only or Logic-first plug-ins ship AU first). For
*exporting* Cantus's plug-ins, CLAP alone reaches Bitwig, Reaper, FL and Studio One; VST 3 adds Live and Cubase;
AU adds Logic; AAX (Pro Tools) is a business agreement, not code. JUCE 9 (July 2026) announced native CLAP export
but its change lists do not show it yet, so most commercial plug-ins still reach CLAP through
clap-juce-extensions or not at all — VST 3 hosting is not optional for a host that wants the market.

---

## 2. Licences, precisely

| Component | Licence | Source |
|---|---|---|
| VST 3 SDK 3.8.x (`base`, `pluginterfaces`, `public.sdk` with its samples and hosting helpers, `cmake`, `doc`, `tutorials`) | **MIT** ("Copyright (c) 2026, Steinberg Media Technologies GmbH"). The README: "Licensing under GPLv3 and the Steinberg proprietary license is no longer available." | `steinbergmedia/vst3sdk` README and `LICENSE.txt` |
| VSTGUI 4 (the SDK's GUI toolkit, a submodule) | **BSD-3-Clause** | `steinbergmedia/vstgui` `LICENSE` |
| "VST" name and logo | trademark; use is now **optional**, and the usage guidelines apply only if a developer chooses to use them ("Use of branding is not required under MIT license") | vst3sdk README; Steinberg's engineering director quoted by CDM |
| VST 2 SDK | proprietary; withdrawn from distribution and from new licensing in October 2018; **not** part of the MIT change | Steinberg forum / press (gearnews, CDM) |
| ASIO SDK 2.3.4 | **GPLv3 or proprietary** (dual) since October 2025; before, proprietary only | KVR, Libre Arts, `audiosdk/asio` |
| CLAP (headers) | **MIT** | `free-audio/clap` licence API |
| clap-wrapper | **MIT** | `free-audio/clap-wrapper` licence API |
| clap-validator | **MIT** | `free-audio/clap-validator` licence API |
| pluginval (Tracktion's validator) | **GPL-3.0** — fine as an external CI tool, never linked | `Tracktion/pluginval` licence API |
| AAX SDK | **GPLv3 or commercial**; retail Pro Tools also requires PACE signing | JUCE forum "AAX open source release", Dplug's AAX guide |
| DAWproject (spec and reference code) | **MIT** | `bitwig/dawproject` README |
| Ableton Link | **GPLv2-or-later or proprietary** ("If you would like to incorporate Link into a proprietary software application, please contact link-devs@ableton.com") | `Ableton/link` `LICENSE.md` |
| clack (host, plug-in, extensions) | MIT OR Apache-2.0 | `prokopyl/clack` Cargo.toml, README |
| clap-sys | MIT/Apache-2.0 | `micahrj/clap-sys` Cargo.toml |
| `vst3` crate (bindings pregenerated from the MIT 3.8.0 headers) | MIT OR Apache-2.0 | `coupler-rs/vst3-rs` Cargo.toml, CHANGELOG 0.3.0 |
| `vst3-sys` (old bindings) | **GPL-3.0** | `RustAudio/vst3-sys` Cargo.toml, `license.md` |
| nih-plug | ISC; its VST 3 output GPL-3 through `vst3-sys` | `robbert-vdh/nih-plug` README, Cargo.toml |
| nice-plug, nice-plug-egui | ISC (VST 3 through the MIT/Apache `vst3` crate) | `codeberg.org/RustAudio/nice-plug` README |
| baseview, egui-baseview | MIT OR Apache-2.0 | crates.io metadata, Cargo.toml |
| `clap-wrapper` (Rust crate) | MIT OR Apache-2.0 | `blepfx/clap-wrapper-rs` |
| truce | "Truce License 1.0" (MIT/Apache grant plus a rider on reselling the framework; not OSI) | `truce-audio/truce` LICENSE |
| objc2 AudioToolbox bindings | Zlib OR Apache-2.0 OR MIT | crates.io metadata |
| livi / lilv | MIT / ISC | `wmedrano/livi-rs`; lilv upstream |
| rusty_link | GPL-2.0-or-later | `anzbert/rusty_link` Cargo.toml |
| egui, eframe | MIT OR Apache-2.0 | `emilk/egui` |

### 2.1 The consequence for ADR 0001

ADR 0001 made the application GPL-3.0-or-later partly because it was "the only open license compatible with
hosting VST3 via Steinberg's GPLv3 grant", and put VST 3 hosting in a future GPL crate. **That reason is gone:**
VST 3 is MIT, and every piece a host or an exporter needs (VST 3 SDK, CLAP, clap-wrapper, the Rust bindings
listed in §3.1) is MIT or MIT/Apache.

What follows:

1. **Plug-in hosting can live in the permissive "SDK" half.** `cantus-plugin-host` should be MIT OR Apache-2.0,
   like the engine it plugs into. A headless renderer or another project could host plug-ins through Cantus's
   engine without copyleft.
2. **Exported plug-ins can be permissive — except for what they link.** A CLAP/VST 3 build of a Cantus effect
   links `cantus-core`, `cantus-dsp`, `cantus-engine` (all MIT/Apache) and the MIT format code, so the DSP-only
   plug-in can be MIT/Apache. Its *editor* comes from `cantus-ui`, which is GPL-3; a plug-in that includes it is
   GPL-3 as a whole. That is common practice (Surge XT, Vital and Dexed are GPL-3 plug-ins loaded by
   proprietary DAWs every day; the usual reading is that the user combines them and the DAW vendor distributes no
   combination — a reading, not legal advice), but
   it is a choice to make on purpose: either keep the editors GPL (exported plug-ins are GPL-3 open source), or
   move the editors into a permissive crate (§7.1) so anyone may embed them.
3. **The application can stay GPL-3, and still has reasons to.** Two components Cantus is likely to want are
   GPL-compatible but not permissive: **ASIO** (GPLv3 or a proprietary agreement; cpal's `asio` feature builds
   against it) and **Ableton Link** (GPLv2+ or proprietary). A GPL-3 application can include both; a permissive
   one would need Steinberg's and Ableton's proprietary licences. ADR 0001's other reason (protect the product's
   freedom) is unchanged. So the recommendation is: **keep the app GPL-3, move plug-in hosting to MIT/Apache, and
   decide the editors' licence when export starts.** ADR 0001's "Consequences" line about a GPL
   `cantus-plugin-host` should be superseded by the new ADR.
4. **Rust bindings that predate the relicence carry the old terms.** The `vst3-sys` crate (RustAudio) was GPL-3
   because it was derived from the GPL SDK; nih-plug's VST 3 exports inherited it. Check each crate's LICENSE
   rather than assuming the SDK's change propagated (§3.1 does this).
5. **AAX stays out** until there is a business reason; it is the one format whose licence (and signing) is a
   contract with a vendor.

---

## 3. Hosting from Rust

### 3.1 The crates, September 2026

Licences are quoted from each crate's `Cargo.toml` or LICENSE file (GitHub's licence label was wrong for several
dual-licensed repositories). Dates are the latest crates.io release and the last commit on the default branch.

| Crate | Purpose | Licence | Latest release · last commit | Maturity |
|---|---|---|---|---|
| **clack** (`clack-host`, `clack-plugin`, `clack-extensions`, `clack-common`) — prokopyl/clack | safe CLAP host **and** plug-in layer | `MIT OR Apache-2.0` | **0.2.0** (2026-09-12; on crates.io since 2026-05-03) · 2026-09-12 | README: "feature-complete… still under active development… APIs can still have breaking changes"; for hosts "no higher-level alternative available". Every stable extension is wrapped (params, note-ports, audio-ports, latency, state, gui, timer, posix-fd, thread-check, thread-pool, voice-info, remote-controls, preset-discovery, tail, render, track-info, surround…); **no draft extensions** (undo, transport-control, webview). A `host/examples/cpal` (cpal + winit 0.30 + midir + rtrb) is the template for a host. `clack-finder` (discovery) is unpublished |
| **clap-sys** — micahrj/clap-sys | raw CLAP FFI | `MIT/Apache-2.0` | **0.5.0** (2025-01-03) = **CLAP 1.2.2** · 2026-07-20 (1.2.3 update unreleased) | stable, thin; clack depends on it, so clack speaks CLAP 1.2.2 until it is released again. The 1.2.3–1.2.10 additions are all drafts |
| **vst3** — coupler-rs/vst3-rs | raw VST 3 COM bindings (host and plug-in) | `MIT OR Apache-2.0` | **0.3.0** (2025-12-07) · 2026-03-28 | bindings **pregenerated** from `vst3_pluginterfaces` v3.8.0 (the MIT headers), identical on all three OSes, no SDK needed to build. Low-level: COM pointers, `unsafe`. Used by nice-plug, vst3-host, truce-rack, beamer, Maolan's host |
| vst3-sys / vst3-com — RustAudio | old VST 3 bindings | **`GPL-3.0`** ("a derivative work of the original SDK") | never published · 2023-06-19 | unmaintained; the reason nih-plug's VST 3 builds are GPL. **Avoid** |
| **vst3-host** — HelgeSverre/rust-vst3-host | a VST 3 *hosting* library on `vst3` 0.3 | `MIT` | 0.9.0 (2026-07-28) · 2026-08-09 | discovery, parameters, sample-accurate MIDI/MPE, state and `.vstpreset`, **out-of-process crash isolation**, editor embedding (egui 0.34); README: runtime-verified on macOS only. A useful reference, not yet a dependency |
| **truce-rack** — truce-audio/truce-rack | multi-format host library (CLAP, VST 3; AU, AUv3, LV2 behind features) | `MIT OR Apache-2.0` | 1.1.5 (2026-06-19) · 2026-06-19 | on `clap-sys` 0.5 and `vst3` 0.3; a reference standalone host (cpal + baseview window). Young, little adoption |
| rack — sinkingsugar/rack | host library via a C++ layer | `MIT OR Apache-2.0` | 0.4.8 (2025-11-15) | AU "production-ready" on macOS; VST 3 tested on macOS only; CLAP planned |
| objc2-audio-toolbox, objc2-avf-audio | macOS Audio Unit APIs (AUv2 and AUv3 hosting) | `Zlib OR Apache-2.0 OR MIT` | 0.3.2 (2025-10-04) | part of the objc2 project; raw but complete |
| livi — wmedrano/livi-rs | LV2 host over the C `lilv` | `MIT` | 0.7.5 (2024-11-12) · 2025-08-26 | "work in progress… not yet fully tested". `lv2` (plug-in side) is dormant since 2020–21; `yeli` is a pure-Rust host under **LGPL-2.1+** |
| **clap-validator** — free-audio | tests CLAP plug-ins (written in Rust, runs each test in its own process) | `MIT` | · 2026-07-24 | the CI gate for anything Cantus exports, and a model for a Rust CLAP host |
| midi2 — midi2-dev/bl-midi2-rs | MIDI 2.0 UMP messages | `MIT OR Apache-2.0` | 0.11.1 (2026-04-09) | "early development" |
| dawproject — bkbkb-net/dawproject | read/write `.dawproject` from the official XSD | `MIT OR Apache-2.0` | 0.11.0 (2026-08-08) | small, generated models |
| rusty_link — anzbert/rusty_link | Ableton Link | **`GPL-2.0-or-later`** (Link itself: GPLv2+ or proprietary) | 0.4.9 (2026-04-02) | wraps the official C extension; CMake |

**Rust hosts that exist.** **Maolan** (BSD-2-Clause, iced 0.14, last commit 2026-09-23) is a Rust DAW that runs
plug-ins **out of process** (`maolan-plugin-host` on `vst3` 0.3 plus its own LV2; CLAP and VST 3 on Windows). BillyDM's
Meadowlark/Dropseed (AGPL-3.0) are dormant. No egui-based CLAP host was found. So Cantus would be among the first
egui hosts, and clack's cpal example, clap-validator, vst3-host and Maolan are the prior art to read.

**Recommendation for hosting:** **clack-host for CLAP, the `vst3` crate for VST 3, objc2's AudioToolbox for AU**
— all permissive, all maintained this year. Write the adapter layer (threading, event lists, the bridge) in
Cantus; it is the part that must fit ADR 0005 and 0018 and no library does it for us. Pin versions (both binding
crates are 0.x). For CLAP drafts Cantus wants (`undo`, `transport-control`): `clack-extensions` is a separate crate
built on clack's public extension traits, so the same can be done in `cantus-plugin-host` with a small amount of
FFI (inferred from the crate layout; not tried).

### 3.2 What the formats demand of a host: threads

Both formats split a plug-in's calls into **roles**, not OS threads, and a host that gets this wrong crashes
plug-ins that are otherwise fine.

| Role | CLAP (from the headers) | VST 3 (from the call-sequence page) | In Cantus today |
|---|---|---|---|
| **main thread** | `create`, `init`, `activate(sample_rate, min_frames, max_frames)`, `deactivate`, every `params` query, `state` save/load, `latency.get`, all `gui` calls, `on_main_thread`, `timer` callbacks | `initialize`, `setBusArrangements`, `setupProcessing`, `activateBus`, `setActive`, `getState`/`setState`, the whole edit controller and its `IPlugView` | the eframe UI thread, which already builds every `EffectUnit` and sends it with `EngineMessage::AddEffect` |
| **audio thread** | `start_processing`, `stop_processing`, `reset`, `process`, `params.flush` while active | `process`, and `setProcessing` *may* be called here ("must be lock-free and without memory allocation") | the cpal callback, or the offline renderer's thread |
| **any thread** | `get_extension`, and the host callbacks `request_callback`, `request_restart`, `request_process`, `gui.request_resize` | `IComponentHandler` calls (`restartComponent`, `performEdit`) belong on the UI thread; a defensive host marshals any that arrive elsewhere | — |

The audio-thread role may move between OS threads but is **never concurrent** for one instance. That is what
lets a host schedule plug-ins on a pool of worker threads later (multi-core), and it is also the rule a
sandboxed host must keep across a process boundary.

**How it maps onto Cantus.** The lifecycle already has the right shape. clack-host encodes it in types: the
main-thread `PluginInstance` is activated into a *stopped* audio processor, which is sent to the audio thread and
started there; to deactivate, the stopped processor must come back to the main thread. That is exactly
`AddEffect` in and `GarbageEffect` out (ADR 0005 rule 1). What Cantus does not have yet:

1. **A main-thread pump.** A plug-in calls `host.request_callback()` from any thread and expects
   `on_main_thread()` soon. eframe sleeps when nothing repaints, so the host must hold a lock-free queue of
   main-thread tasks, wake egui with `Context::request_repaint()` (egui's context is `Send + Sync`) and drain the
   queue at the top of `update()`. CLAP's `timer-support` needs `request_repaint_after(period)`. On Linux,
   `posix-fd-support` (CLAP) and `IRunLoop` (VST 3) need file descriptors watched; a helper thread can `poll()`
   them and wake the UI, which then calls the plug-in on the main thread.
2. **Latency read on the audio thread.** `FxUnit::latency()` is read by `pdc::compensate` on the audio thread;
   CLAP's `latency.get` is `[main-thread & (being-activated | active)]` and a change is only allowed during
   `activate`, announced with `request_restart`. The adapter must cache the value (an atomic written on the main
   thread after each activation), and a restart is: stop on the audio thread, hand back, deactivate, activate,
   read latency, hand over. VST 3 is the same with `restartComponent(kLatencyChanged)`.
3. **Transport in `process`.** `FxUnit::process(l, r, key)` has no tempo, position or play state. CLAP sends
   `clap_event_transport`; VST 3 fills `ProcessContext`. The contract needs a `ProcessContext` argument (Cantus's
   own tempo-synced delay would use it too).
4. **Whole blocks.** `Processor::render` splits the block at every note event, beat and loop boundary, and the
   strip loop runs *inside* that segment loop, so an insert is called once per segment — sometimes for a single
   sample. Both formats take variable block sizes up to `max_frames`, so this is legal, but it multiplies the
   per-call overhead of third-party code, and some plug-ins misbehave with tiny blocks (FL Studio has a
   per-plug-in "fixed size buffers" option for exactly those). Hosted units should get the whole block with
   sample-offset events; the segment split should stay inside Cantus's own instruments. This is the one
   structural change to the processor that hosting forces, and it must keep every null test bit-identical.

### 3.3 Scanning, in a separate process, with a cache

A scan loads foreign code; a plug-in that crashes, hangs or pops a licence dialog during the scan must not take
Cantus with it. Every serious host scans out of process now (the host survey in §6 gives the details). The
design:

- A **scanner executable** (`cantus-plugin-scan`) takes a list of files and writes one JSON record per plug-in:
  format, id, name, vendor, version, features/categories, audio and note ports, whether it has a GUI, and the
  file's size, modification time and a content hash. The parent runs it with a timeout per file.
- **CLAP**: open the bundle, call `clap_entry.init`, read the plug-in factory's descriptors (no instance needed),
  and optionally walk the **preset-discovery factory** so presets are indexed without instantiating anything.
  Ports and GUI support need an instance; creating one and calling `init` is cheap by the spec's rules.
- **VST 3**: read `Contents/Resources/moduleinfo.json` when present (since SDK 3.7.5, moved to `Resources` in
  3.7.8; it mirrors the factory and has a `Compatibility` list mapping old class ids to new). Only when it is
  absent is the binary loaded to read `IPluginFactory2/3` class infos.
- **The cache** is keyed by path + size + mtime (+ hash when cheap). A crash or timeout writes a `failed` record
  with the reason, so the plug-in is not retried at every start and the browser can show *why* it is missing.
- **Blocklist and quarantine** are user-visible lists, not files to edit.

### 3.4 Crash isolation (sandboxing): what it costs

In-process hosting is the fastest and the most fragile: one bad pointer in a plug-in ends the session. The
options, cheapest first:

| Mode | How | Cost | Protection |
|---|---|---|---|
| In process | the plug-in's `process` called directly from the audio callback | none | none |
| In process, attributed | before each call the audio thread writes the plug-in's id into a "breadcrumb" atomic; a crash handler (Windows unhandled-exception filter, POSIX signal handler, async-signal-safe `write`) records it | ~nothing | the crash is *named* on the next start, and the project reopens with that plug-in disabled or sandboxed |
| Out of process, lock-step | audio in and out through shared memory; the audio thread signals the plug-in process (Windows event / futex / Mach semaphore) and waits for it inside the same block | a context switch each way per block per process — Apple quoted about 40 µs per render cycle for AUv3 (5.5 % of a 32-frame buffer), FL's manual 1–2 % CPU per bridged plug-in — and the plug-in process's thread must run at real-time priority (MMCSS "Pro Audio" on Windows, the device's `os_workgroup` on Apple Silicon or it lands on efficiency cores) | a crash stops only that plug-in; the host replaces its output with silence and offers a reload |
| Out of process, pipelined | the plug-in process computes block *n* while the host plays block *n−1* | **one block of latency**, reported to delay compensation (ADR 0018 handles it like any other latency) | same, and the audio thread never waits |

Structural consequences, whichever mode ships first:

- **Grouping is a setting.** One process per plug-in is safest and costs the most memory and switching; one
  process for all third-party plug-ins is cheapest and still protects the host. Bitwig offers the whole range;
  the survey in §6 gives its modes.
- **Editors live where the plug-in lives.** A sandboxed plug-in's GUI is created in its own process. On Windows a
  child `HWND` can be parented across processes (with focus and keyboard quirks); on macOS an `NSView` cannot be
  embedded across processes at all. The only design that is the same on every OS and in every mode is **each
  plug-in editor in its own top-level window** (owned by the main window so it floats above it). Bitwig and
  Reaper float plug-in editors in their own windows, and FL Studio can detach any plug-in window from its
  workspace; Cantus's own editors already open one window per editor (ADR 0018), so the two would look alike.
- **Offline rendering must be lock-step.** A pipelined bridge would make a bounce depend on timing; ADR 0005 rule 5
  (deterministic output) requires the lock-step path, or the one-block delay compensated exactly, for renders.
- **"Restricted syscall access"** (idea.md §7) is not realistic for commercial plug-ins: they read sample libraries
  anywhere on disk, talk to licence services (iLok, their own activation) and write caches. Isolation of a
  *process* is achievable; a permissions sandbox would break most of the market.

### 3.5 Editor windows: HWND, NSView, X11, and DPI

- **Getting a parent.** eframe exposes the root window's native handle through `raw_window_handle::
  HasWindowHandle` (on `CreationContext`, per its docs), which is all an owned top-level window needs. For plug-in windows Cantus creates native top-level windows itself (Win32 `CreateWindowExW`
  owned by the main `HWND`; an `NSWindow` whose content view receives the plug-in's `NSView`; an X11 window
  the plug-in reparents into, XEmbed as CLAP's gui header says). On Windows and macOS these are serviced by the
  same message loop winit already runs, because the loop dispatches every window on the thread.
- **CLAP's GUI sequence** (from `ext/gui.h`): `is_api_supported` → `create(api, is_floating)` → for an embedded
  window `set_scale`, `can_resize`, `get_size`/`set_size`, `set_parent` → `show`. Sizes are *physical pixels* on
  Win32 and X11 and *logical points* on Cocoa, where `set_scale` must not be called. The host callbacks
  (`request_resize`, `resize_hints_changed`, `request_show/hide`, `closed`) are thread-safe; everything else is
  main-thread. Wayland supports floating windows only.
- **VST 3**: `IEditController::createView("editor")` → `IPlugView::isPlatformTypeSupported` → `attached(parent,
  type)`; the host implements `IPlugFrame::resizeView`; `IPlugViewContentScaleSupport::setContentScaleFactor`
  passes the DPI scale on Windows and Linux. 3.8 added `IWaylandHost`/`IWaylandFrame` (a preview).
- **DPI on Windows** is where hosts differ most in quality. Since the Windows 10 Anniversary Update a thread can
  switch its DPI awareness with `SetThreadDpiAwarenessContext`, create a *top-level* window, and switch back;
  that window keeps the awareness it was created with, and **a child always has its parent's awareness**
  (Microsoft's "Mixed-Mode DPI Scaling"). So a per-plug-in choice — per-monitor aware, system aware, or
  unaware and bitmap-stretched by Windows — is only possible with a top-level window per editor. That is one
  more reason for floating editor windows, and it lets Cantus offer a per-plug-in "scaling" menu for the old
  plug-ins that draw tiny or blurry.
- **Keyboard.** A plug-in window takes keyboard focus; the host's shortcuts (space to play) stop working unless
  the host forwards unhandled keys. Reaper, for example, makes it a per-plug-in option. Cantus's keymap (ADR
  0010) should get the keys a plug-in window does not consume.

### 3.6 Parameters, automation and undo for a hosted plug-in

| | CLAP | VST 3 | Cantus's model |
|---|---|---|---|
| Identity | `clap_id` (u32), stable across versions by rule, plus name and a module path (`"Band 3/Gain"`) | `ParamID` (u32), stable, plus title, units and a `unitId` tree | `key: String`, stable forever (ADR 0017), plus `EffectParamInfo` |
| Value | **plain** `f64` in `[min, max]`; the header says automation and state should store plain values "robust to a range increase" | **normalised** `f64` in `[0, 1]`; `normalizedParamToPlain` and `getParamStringByValue` on the controller | plain `f32`, clamped, `stepped` rounded |
| Flags | stepped, enum, periodic, hidden, read-only, bypass, automatable (per note id/key/channel/port), modulatable (same granularity), requires-process | `kCanAutomate`, `kIsReadOnly`, `kIsWrapAround`, `kIsList`, `kIsHidden`, `kIsProgramChange`, `kIsBypass` | `logarithmic`, `stepped` |
| From host to plug-in | `CLAP_EVENT_PARAM_VALUE` in the input event list, sample-accurate; `params.flush` when not processing | `IParameterChanges` in `ProcessData`: a queue per parameter of (sample offset, value) points; controller told separately with `setParamNormalized` | `configure(spec)` per snapshot |
| From plug-in GUI to host | `PARAM_GESTURE_BEGIN` / `PARAM_VALUE` / `GESTURE_END` in the *output* event list | `IComponentHandler::beginEdit` / `performEdit` / `endEdit` on the UI thread | a `Command` per gesture (the house knob) |
| MIDI CC | delivered as MIDI or mapped by the plug-in | **not delivered**: the host asks `IMidiMapping::getMidiControllerAssignment` (bus, channel, CC) and turns CCs, pitch bend and aftertouch into parameter changes; 3.8 adds `IMidiMapping2`/`IMidiLearn2` for MIDI 2.0 | live MIDI: notes and CC 120/123 only |

The mapping that keeps Cantus's command bus, undo, AI bridge and (future) automation working unchanged:

- A hosted parameter is addressed with a key derived from its id, `p{id}` (e.g. `p1024`). The command layer,
  `SetEffectParam`/`ClearEffectParam`, the AI tool schema and automation lanes then treat a third-party
  parameter exactly like `b3.gain`. The parameter *list* comes from the plug-in at run time instead of from a
  `const` table, so `EffectKind::params()` becomes a lookup that can answer from the instance's cached info.
- Values stored and automated are **plain for CLAP, normalised for VST 3**, as each format says; `f32` loses
  nothing that matters (a 24-bit mantissa; stepped values are exact).
- **The plug-in's state blob is the truth; the key map is a view.** The project stores the blob; the sparse key
  map holds only what the host itself is driving (automation base values, the AI's edits). A save asks the
  plug-in for its state; a load restores the blob and then applies the map.
- **Undo.** A gesture from the plug-in's own window becomes one `SetEffectParam` command with the old value read
  when the gesture began, exactly as the house knob does, and applying it re-sends a value the plug-in already
  has (idempotent). Changes that are not parameters — a preset loaded inside the plug-in, a sample dropped on
  it — arrive as CLAP `state.mark_dirty` or a VST 3 `restartComponent`/dirty flag; the host then captures the
  state blob and records a `SetPluginState { before, after }` step, debounced. CLAP's draft `undo` extension
  lets a plug-in hand the host small deltas and join a single history "shared by the host and many plugins";
  Cantus should implement the host side for plug-ins that offer it, and fall back to whole-state steps.
- **Values from the plug-in to the UI** travel on a lock-free ring from the audio thread (output events) like the
  meters, never through a lock.

### 3.7 State inside the project file

- **What is stored.** CLAP: one opaque stream from `state.save` (with `state-context` the host can say whether it
  is saving for a project, a preset or a duplicate). VST 3: the component state *and* the controller state; the
  natural container for the pair is the `.vstpreset` layout (a 48-byte header: `VST3`, a version, the class id
  in 32 ASCII characters and the offset of a chunk list; chunks `Comp`, `Cont` and optionally `Info`
  metadata). AU: the `ClassInfo` property list.
- **Keep the bytes exactly as the plug-in gave them.** DAWproject (MIT) stores each plug-in's state as a file
  inside its zip, referenced from `<ClapPlugin>`, `<Vst3Plugin>` or `<AuPlugin>`; storing the native blob
  unchanged makes a later DAWproject export a copy, not a conversion.
- **Where.** The project is pretty-printed JSON with an integrity hash (ADR 0006). A sampler's state can be
  megabytes, which would drown a diff in base64. Proposal: blobs up to a small size (say 16 KiB) inline as
  base64; larger ones as sidecar files `plugins/<sha256>.state` in the project folder, named by content hash so
  identical states are stored once, written **before** the JSON in the atomic save, listed with their hashes in
  the JSON so the integrity check covers them, and orphans removed only after the new file verifies.
- **A missing plug-in keeps its state.** A project opened where a plug-in is not installed must show the slot
  as missing and write the blob back untouched on save. Hosts that drop it lose users' work.

### 3.8 Offline rendering

`render::render` builds a fresh `Processor` and fresh `EffectUnit`s on the render thread. For a hosted plug-in
that breaks two rules at once: instances must be created and activated on the main thread, and a second instance
means a second load (memory, time, sometimes a licence seat). Two workable designs:

- **Fresh instances from state** (fits today's code): the main thread creates and activates a second instance of
  each plug-in from the current state blob, switches it to offline mode (CLAP `render.set(CLAP_RENDER_OFFLINE)`;
  VST 3 `setupProcessing` with `processMode = kOffline`, which needs a deactivate/activate), and hands it to the
  render thread, which then *is* the audio-thread role. The main thread keeps pumping callbacks meanwhile. A
  sampler that streams from disk may not be ready for the first blocks of a fresh instance; the render must wait
  for it (a short warm-up of silent blocks, or waiting until its state load has finished) and say which plug-in
  it waited for. How each commercial sampler behaves here was not checked.
- **Take the live instances offline**: stop the audio stream, switch the live processor's plug-ins to offline mode,
  render, switch back. Cheaper in memory; blocks playback while rendering.

Determinism (ADR 0005 rule 5) can only be *promised* for Cantus's own units. A third-party plug-in may seed noise
from the clock, run its own threads, or smooth parameters per block. The null tests therefore run through test
plug-ins built in the repository (§7), never through the user's.

### 3.9 Buses: sidechain, multi-out, surround

- **Sidechain.** CLAP describes it as a second *input* audio port (non-main, usually stereo, named "Sidechain");
  VST 3 as an auxiliary input bus (`kAux`), inactive until the host calls `activateBus`. Cantus's `EffectSlot::
  sidechain` route (ADR 0017) maps directly: the key buffer becomes that port's data; when no key is routed the
  port gets silence (CLAP's `constant_mask` can say so cheaply). Note ADR 0018's rule that keys are not delay
  compensated — acceptable for a detector, now documented.
- **Multi-out instruments** (a drum sampler with 16 stereo outputs). CLAP: several output audio ports, and
  `audio-ports-config` for presets like "16 × stereo"; VST 3: several output buses, activated on demand. The
  FL-shaped answer is the one FL uses: each extra output is offered as its own mixer strip, created when the
  user enables it. In the processor that is a track slot writing into more than one strip buffer.
- **Surround/ambisonic** exist in both formats (`surround`, `ambisonic` extensions; VST 3 speaker arrangements).
  Cantus is stereo end to end; a hosted plug-in is configured stereo (`setBusArrangements` with stereo on every
  bus; CLAP port type `stereo`), and a plug-in that refuses stereo is not offered.

### 3.10 Latency and tail into what Cantus already has

- **Latency** → `FxUnit::latency()` from the cached value (§3.2); ADR 0018's per-edge compensation then works for
  third-party plug-ins with no change, including the grown pool for long latencies. A sandboxed pipelined
  plug-in adds its one block to what it reports.
- **Tail and sleep.** Cantus skips a strip that made no sound. CLAP's `process` returns `CONTINUE`,
  `CONTINUE_IF_NOT_QUIET`, `TAIL` or `SLEEP`, and the `tail` extension gives a length (infinite for a freeze
  reverb); VST 3 has `getTailSamples` and `kNoTail`/`kInfiniteTail`. The adapter should report "still ringing"
  from these, and the skip must respect it, or a reverb's tail is cut when its source stops.
- **Bypass.** A parameter flagged bypass (`CLAP_PARAM_IS_BYPASS`, `kIsBypass`) should drive Cantus's slot bypass
  so the plug-in can keep its latency through a "soft" bypass — the behaviour ADR 0018 leaves for later.

---

## 4. Exporting Cantus's plug-ins

### 4.1 The options

| Option | Formats | Licence | State in Sept 2026 | Fit with Cantus |
|---|---|---|---|---|
| **nih-plug** (robbert-vdh) | CLAP, VST 3, standalone | framework ISC; **VST 3 builds are GPL-3** (still on `vst3-sys`) | README: "currently in maintenance mode", points to the community fork; not on crates.io; nih_plug_egui pinned to **egui 0.31** | no: GPL VST 3, old egui, maintenance mode |
| **nice-plug** (RustAudio on Codeberg; BillyDM's fork renamed) | CLAP, VST 3, standalone (JACK, cpal, ASIO); no AU | **ISC**, and VST 3 now via the MIT/Apache `vst3` crate, so no GPL caveat | 0.4.2 (2026-09-14), very active; `nice-plug-egui` 0.5.1 on **egui 0.36.1**; README: "experimental… expect some bugs" and "the recommended toolkit for Rust audio plugin developers" | good for effects quickly; its parameter model is a derive on a struct (string ids hashed to numbers), so Cantus's runtime parameter tables would need a hand-written `Params` implementation; no AU |
| **truce** | CLAP, VST 3, AUv2, AUv3 (macOS, iOS), AAX, VST 2, LV2, standalone | "Truce License 1.0": Apache-2.0/MIT **plus a rider** — redistributing truce itself as a commercial framework or service needs written permission; plug-in authors are unaffected; **not OSI-approved** | 6.3.0 (2026-07-18), 210 stars, created March 2026; VST 3 through a C++ shim; egui, iced, Slint or Vizia editors | the widest format list, but a single-vendor, six-month-old framework under a custom licence; a GPL project can probably ship plug-ins built with it (the rider restricts reselling the framework, not use), but that is a question for the ADR, not a certainty |
| **clack-plugin + clap-wrapper** ("CLAP first") | CLAP natively; VST 3, AUv2, AUv3, AAX and standalone from the C++ **clap-wrapper** (MIT; v0.16.0, 2026-08-08); or VST 3 and AUv2 only from the Rust crate **`clap-wrapper`** (blepfx; `MIT OR Apache-2.0`; 0.3.1, builds with `cc`, no CMake) | all permissive | clack 0.2.0 as above; clap-wrapper is the free-audio project for exactly this | **best fit** (below) |
| Own VST 3 export on the `vst3` crate | VST 3 | MIT/Apache | the crate is raw COM | only if clap-wrapper's VST 3 proves limiting |
| beamer (helpermedia) | AU (hybrid v2/v3), VST 3, WebView editors | MIT | 0.2.4, small | not egui |

### 4.2 Recommendation: CLAP first, on clack-plugin, with clap-wrapper for VST 3 and AU

1. **The parameter model is already CLAP-shaped.** CLAP asks the plug-in for its parameter list at run time
   (`count`, `get_info(i)`), with plain values, names and a module path. Cantus's `EffectKind::params()` is
   exactly that list, and Aequalis's 337 parameters with stable `b{n}.{field}` keys map to module paths
   (`Band 3/Gain`). A struct-derive framework wants the list at compile time.
2. **The instruments want CLAP's features** — note ids, note expressions, polyphonic modulation, `NOTE_END`,
   voice info (§5). A framework built around VST 3's model hides them.
3. **One Rust library on both sides.** The test plug-ins of step 2 in §7.3 are clack-plugin plug-ins; the export
   wrapper is the same code grown up; clap-validator (Rust, MIT) checks both.
4. **Permissive all the way down**, so the only licence question left is the editors' (§2.1).
5. **The fallback is cheap.** If the egui glue or clap-wrapper costs too much, nice-plug (ISC, egui 0.36 today) can
   host the same `FxUnit` behind a hand-written `Params` implementation. Keep that door open by keeping the
   export adapter thin.

### 4.3 How Cantus's parameters map onto CLAP and VST 3

| Cantus | CLAP | VST 3 |
|---|---|---|
| key `"b3.gain"` (stable forever, ADR 0017) | `clap_id` = a fixed 31-bit hash of the key (e.g. FNV-1a masked to 31 bits); a unit test per kind proves there are no collisions, and because keys never change, ids never change | the same number as `ParamID` — VST 3 allows `[0, 0x7FFFFFFF]` and reserves the upper half for the host, and forbids reassigning ids |
| `label_key` → locale text | `name` in the host's language at load time (Cantus has both locales) and `module` = group path (`"Band 3"`) | `title`, `shortTitle`, `units`; groups as `UnitInfo` |
| `min`, `max`, `default`, `unit` | `min_value`, `max_value`, `default_value` in plain values; `value_to_text` prints the unit | normalised 0–1 always; the controller converts with the same taper |
| `logarithmic: true` | expose the **normalised position** (0–1 through Cantus's log taper), not Hz: a host can only map a CLAP parameter's plain range linearly onto its knobs and lanes (the spec has no taper), so a 20 Hz–20 kHz lane would put most of the audible range in its bottom tenth. nih-plug's CLAP wrapper does the same for every continuous parameter (it reports `min_value = 0`, `max_value = 1`, checked in its source) | normalised through the taper |
| `stepped: true` | `CLAP_PARAM_IS_STEPPED` (plus `IS_ENUM` for shapes and styles) with integer plain values | `stepCount` and a `kIsList` string list |
| everything automatable | `IS_AUTOMATABLE` and `IS_MODULATABLE`; per-note-id flags only on instrument parameters that support it | `kCanAutomate` |
| sparse storage (`EffectKind::sparse`) | irrelevant to the host: `state.save` writes Cantus's own JSON (`EffectSpec` with only what moved, plus a version); `state.load` reads it and old versions through Cantus's migrations | the same bytes as the component state; the controller state can be empty |
| slot bypass | a parameter with `CLAP_PARAM_IS_BYPASS`, so hosts merge it with their own bypass button | `kIsBypass` |
| `FxUnit::latency()` | `latency` extension; a change (the EQ switching to linear phase) calls `host.request_restart()` | `getLatencySamples`; `restartComponent(kLatencyChanged)` |
| `TAKES_SIDECHAIN` | a second input port named "Sidechain" | an aux input bus |
| factory presets | the plug-in's own preset-discovery provider (so Bitwig-class browsers list them) | `.vstpreset` files installed in the standard location |
| host A/B and host undo (ADR 0018) | lost inside another DAW, unless the plug-in keeps its own; CLAP's draft `undo` lets a supporting host fold the plug-in's steps into its history | lost; plug-in-side undo is the only option |

That last row is the trade FabFilter lives with (research/fabfilter/family.md, finding 5): a plug-in inside
someone else's host must re-implement A/B, undo and an instance list itself. Cantus's editors should keep that
frame header able to run in "plug-in mode", where A/B and undo are local to the plug-in.

### 4.4 Editors inside a plug-in

- **The window.** baseview (`MIT OR Apache-2.0`, 0.3.4, commits this week) creates the child window under the
  host's `HWND`/`NSView`/X11 window; **egui-baseview 0.7.2 (2026-09-18) is on egui 0.36.1**, the version Cantus
  uses, with `egui_glow` or `egui-wgpu` painters. No Wayland (floating only would be needed, as CLAP says).
- **Known problems to plan for** (open baseview/egui-baseview issues): keyboard input in a reparented window
  that is not focused (X11, and egui-baseview #20, with a `keyboard_focus_workaround` feature), Reaper-on-macOS
  key events, a Win32 mouse-capture leak, an FL Studio minimum-size issue. Cantus's house knob uses right-click
  text entry, so keyboard focus matters. nice-plug's Windows DPI fix warns against enabling it in processes that
  host other plug-ins — i.e. inside a DAW.
- **The seam.** Editors take an `FxHost` trait instead of the app (§7.2); the plug-in implements it with
  gestures and parameter values, and telemetry and the analyser tap come from the plug-in's own audio thread
  through the same lock-free rings the engine uses.
- **Resizing and scale.** CLAP's `gui.set_scale` (Windows, Linux) and VST 3's content-scale factor feed egui's
  `pixels_per_point`; `can_resize` and resize hints come from the editor's minimum size.

### 4.5 An instrument contract, exported

The same `InstrumentUnit` sketched in §5.4 exports as a CLAP instrument: a note port accepting the CLAP, MIDI and
MPE dialects (preferring CLAP), note expressions mapped to per-voice parameters, per-note-id modulation flags on
the parameters the voice reads per sample, `NOTE_END` when a voice finishes, and `voice-info`. Through
clap-wrapper the VST 3 build gets note expression where the wrapper maps it and plain MIDI otherwise.

---

## 5. Instruments specifically

### 5.1 Notes in, voices out

| Need | CLAP | VST 3 | Cantus today |
|---|---|---|---|
| Declaring note input | `note-ports`: each port lists the dialects it accepts (`CLAP`, `MIDI`, `MIDI_MPE`, `MIDI2`) and prefers one | an event input bus (`kEvent`) | — |
| Note identity | `clap_event_note { note_id, port_index, channel, key, velocity }`; `note_id` is the host's (≥ 0) or −1; −1 in port, channel or key is a wildcard | `NoteOnEvent { channel, pitch, tuning, velocity, length, noteId }` | `NoteEvent { tick, pitch, velocity, note: NoteId }` and a `u64` tag per voice |
| Note end | `NOTE_OFF` from the host; `NOTE_CHOKE`; **`NOTE_END` from the plug-in** when a voice has actually finished | `NoteOffEvent`; no "voice ended" signal | — |
| Velocity | `f64` 0–1 on and off (release velocity) | `f32` 0–1 on and off | `u8`, note-on only |
| Controllers | MIDI events (dialect MIDI) or parameters | never MIDI: CC, pitch bend and aftertouch go through `IMidiMapping` into parameters (§3.6) | — |

What this asks of Cantus:

- **Note ids.** `NoteId` already exists per note in the project. The host assigns CLAP/VST 3 note ids (i32) per
  voice from a counter per track and keeps the map note id → `NoteId` for the block; the same id is used for the
  matching note-off, expressions and per-note modulation.
- **Voice lifetime.** With `NOTE_END`, a host knows when a voice is really silent — a release tail of four seconds
  included. Per-voice modulators in Cantus's future modulation system (idea.md §5.7) need that signal to stop.
- **Release velocity** is worth adding to `Note` (FL's piano roll has it as a note property); both formats carry it.
- **Live MIDI** today handles notes and CC 120/123 only. A hosted instrument needs the rest: pitch bend, channel
  pressure, CCs, program change, sustain — translated per format (CLAP: MIDI events or `CLAP_EVENT_MIDI2`; VST 3:
  `IMidiMapping` into parameter changes, or `LegacyMIDICCOutEvent` for output).

### 5.2 Expression: MPE, note expression, MIDI 2.0

- **CLAP note expressions** are per-note (by note id or key) and have fixed meanings: volume (0 < x ≤ 4, linear
  gain), pan (0–1), tuning (±120 semitones), vibrato, expression, brightness and pressure (0–1). They are events
  in the same list as the notes, so they are sample-accurate.
- **VST 3 note expression**: `NoteExpressionValueEvent { typeId, noteId, value }` with the standard types (volume,
  pan, tuning, vibrato, expression, brightness) plus plug-in-defined ones, discovered through
  `INoteExpressionController`; `INoteExpressionPhysicalUIMapping` tells the host which expression an MPE
  controller's X, Y and pressure should drive. 3.8.1 added note-expression type ids for MIDI 2.0 per-note
  controllers and attributes and an Orchestral Articulation profile interface.
- **MPE** is "one channel per note" MIDI 1.0. A host that has per-note expression internally can feed an
  MPE-dialect port by allocating channels, a note-expression port directly, and a MIDI 2.0 port with per-note
  controllers. So Cantus's internal model should be **per-note expression with note ids**, and MPE and MIDI 2.0
  are encodings chosen per plug-in at the port.
- **FL's note properties map onto this cleanly.** FL's piano roll gives every note pan, fine pitch, release
  velocity, and "mod X/Y". Pan and fine pitch are CLAP's `PAN` and `TUNING` expressions; mod X and Y can be
  offered as `BRIGHTNESS`/`EXPRESSION` or bound to a parameter as per-note modulation. That gives Cantus's
  piano roll a reason to exist for third-party instruments too.

### 5.3 Polyphonic modulation

CLAP is the only mainstream format with per-voice modulation of *any* parameter (VST 3's nearest thing is a
plug-in-defined note-expression type, which the plug-in must declare one by one): a parameter flagged `IS_MODULATABLE_PER_NOTE_ID` (or per key,
channel, port) accepts `CLAP_EVENT_PARAM_MOD { param_id, note_id, port_index, channel, key, amount }` — an
*offset* applied on top of the parameter's value for one voice, not a change of the value. Automation and
modulation are therefore separate streams, and a host can show both. The voice ends with `NOTE_END`, which
is when the host's per-voice modulator for it can stop.

For Cantus this is the bridge between idea.md §5.7 ("per-voice modulation, each note carries its own") and
third-party instruments: the modulation system produces per-voice offsets; the host sends them as `PARAM_MOD`
events with the note id; for VST 3 and non-modulatable parameters it degrades to monophonic modulation of the
value.

### 5.4 Voice info, multi-out, and the rest

- **`voice-info`** reports `voice_count`, `voice_capacity` and a flag "supports overlapping notes" (the same key
  sounding twice). The host uses it to size per-voice modulation and to decide whether a repeated key is a
  new voice. Cantus's `EngineStats::active_voices` could sum it for hosted instruments.
- **Multi-out** is covered in §3.9. The FL answer (outputs become strips on demand) is right for a
  mixer-centred host.
- **Note names** (`note-name` extension; VST 3 `INoteNameController`/unit pitch names) label drum-kit keys in the
  piano roll; Cantus's channel rack could show "Kick", "Snare" for a hosted drum sampler as it does for Pulsus.
- **Tuning** (a CLAP draft) lets the host send a tuning table; microtuning is a niche Cantus can serve later.
- **An instrument contract** for Cantus's own engines (Firmus, Pulsus and the ones the other reports propose)
  should be the same shape as the hosted adapter, so one processor path serves both:

```rust
/// Engine-side instrument contract (sketch). Everything but `activate` runs on the audio thread.
pub trait InstrumentUnit: Send {
    fn activate(&mut self, sample_rate: f32, max_block: usize); // main thread, before handing over; may allocate
    fn process(&mut self, ctx: &ProcessContext, events: &[InstrumentEvent], out: &mut [&mut [f32]]) -> Status;
    fn reset(&mut self);
    fn latency(&self) -> usize { 0 }
    fn telemetry(&self) -> Telemetry { [0.0; TELEMETRY_LEN] }
}
/// Note on/off with a note id and a sample offset, expression per note id, parameter value and
/// per-voice modulation, raw MIDI — one list per block, sorted by offset.
pub enum InstrumentEvent { NoteOn { .. }, NoteOff { .. }, Expression { .. }, Param { .. }, ParamMod { .. }, Midi { .. } }
```

  Today `TrackSlot` holds a `Firmus` *and* a `Pulsus` by value so that switching never allocates. A hosted
  instrument must be boxed; it arrives on the existing message path (built and activated on the main thread,
  sent like `AddEffect`, returned like `GarbageEffect`), which keeps the rule without keeping every engine
  resident.

---

## 6. How Cantus beats other hosts at hosting

### 6.1 What the others do

| Host | Scanning | Isolation | Crash UX | Cost and latency shown | Browser |
|---|---|---|---|---|---|
| **Bitwig** | fast (a forum thread timed 5.1.8 at "twenty seconds" where Live took many minutes); shows CLAP preset-discovery presets in the browser | five modes: *Within Bitwig*, *Together* (default: one sandbox for all plug-ins), *By manufacturer*, *By plug-in*, *Individually*; 2.5 stopped allocating a shared-memory area per plug-in | the crashed device's panel is replaced by a notice with **Reload**; "Reload All Plug-ins"; other instances survive | global DSP meter only; no per-device CPU | favourites, collections, smart collections |
| **REAPER** | scans at launch, cache in `reaper-vstplugins64.ini`; "Re-scan" retries failures | per plug-in "Run as": separate process (shared, cheaper), dedicated process (safest, more CPU), native; "no additional latency" for a dedicated process | "Open with FX offline (recovery mode)" | Performance Meter shows FX CPU **per track** (not per plug-in) and a "longest block" view; each FX chain window shows the chain's latency; PDC per chain or per FX | custom folders (one plug-in in several), smart folders with OR/NOT filters |
| **FL Studio** | Plugin Manager: find, verify (sorts generators from effects), rescan errors, highlights failures | per-plug-in *Make bridged* (≈1–2 % CPU each); *Use fixed size buffers* (2 ms) and *Process maximum size buffers* for plug-ins that dislike variable blocks | — | — | favourites database with pictures (`.fst`/`.nfo`/`.png`), user folders |
| **Ableton Live** | a separate scanner process; since 12.1 its own database; forum users report scans of 5 to 30 minutes with several hundred to ~1,000 plug-ins | **none** (requested since 2022) | hold Alt/Option at launch to skip scanning | — | collections |
| **Cubase** | Plug-in Sentinel scans at start in isolation; failures go to a blocklist with **Reactivate** | none at run time | blocklist | — | plug-in manager collections |
| **Studio One / Studio Pro** | scanner with a blocklist ("Reset Blocklist") | none at run time | blocklist | — | — |
| **Logic** | `auval` validation; "Reset & Rescan Selection" | AUv3 always out of process; on Apple silicon AUs run in AUHostingService (forum-sourced) | failed-validation list | — | — |

**Published costs of out-of-process hosting.** Apple (WWDC 2015, AUv3): "on the order of 40 microseconds per
render cycle", 5.5 % at 32-frame buffers. FL's manual: 1–2 % CPU per bridged plug-in. REAPER: a dedicated process
raises CPU and adds no latency. Carla's bridge design (falkTX): shared memory with one semaphore-synchronised
real-time channel (audio, MIDI, transport) and two ring buffers for non-real-time traffic (state, GUI, latency
changes). yabridge: grouping plug-ins in one process "can greatly reduce overall CPU usage". Forum measurements
of Bitwig (hearsay): *Together* 16–23 % versus *Individually* 24–27 % CPU on a ten-track project. No rigorous
benchmark comparing lock-step with pipelined designs was found; every host surveyed uses lock-step with no added
latency.

**The gaps no major host fills:** per-*plug-in* CPU (REAPER is per track, Bitwig is global), per-plug-in latency
in the mixer, a crash that names the plug-in *and* reopens the project safely, isolation that turns itself on
after a crash, and preset search across plug-ins outside Bitwig.

### 6.2 What Cantus should do better

Each item names what it rests on, so none is a promise without a mechanism.

1. **A scan that never blocks and always explains.** Scanning runs in background processes, several in parallel,
   and results stream into the browser as they arrive; a startup never waits for it. CLAP descriptors and VST 3
   `moduleinfo.json` make a rescan of an unchanged system a cache read. A plug-in that failed shows *why*
   (crashed, timed out, wrong architecture, not a valid bundle, blocked by the user) and when, with one button to
   retry and one to retry *isolated*.
2. **One row per plug-in, formats as badges.** A product installed as CLAP, VST 3 and AU is one entry with the
   formats it has; Cantus prefers CLAP, then VST 3, then AU, and the user can pin a format per plug-in. Moving an
   existing slot from VST 3 to CLAP keeps its settings when the plug-in offers CLAP's draft
   **plugin-state-converter** factory ("convert a plugin state and its automation points to a new plugin").
3. **A browser that knows what is inside.** Favourites, user tags, automatic tags from CLAP `features` and VST 3
   sub-categories, "recently used", "used in this project", and **preset search across plug-ins** through CLAP's
   preset-discovery factory, indexed by the scanner without instantiating anything.
4. **Per-slot cost and latency, always visible.** The processor times every unit's `process` (native and hosted)
   and reports it with the meters; each mixer slot shows its latency in samples and milliseconds and its share
   of the block budget, and the status bar lists the three most expensive. ADR 0018 already reports the total
   latency and unserved edges; this adds *who* causes them.
5. **A generic editor that is good, and always there.** Every hosted plug-in has a parameter view even when it has
   its own GUI: searchable, grouped by CLAP module path or VST 3 unit, drawn with the house knob (ADR 0017), text
   entry through the plug-in's own `value_to_text`/`text_to_value`, and CLAP's **remote-controls** pages (the
   plug-in's own choice of eight controls per page) as the default view. It is also what the AI bridge and
   automation use, because third-party parameters are `p{id}` keys like any other.
6. **Host services for every plug-in.** The slot's dry/wet (FL's per-slot mix, already built), host A/B and
   presets for third-party plug-ins by state blob (ADR 0018's A/B already lives in the host), input/output trim,
   and an **oversampling wrapper** (run a plug-in at 2× or 4× through Cantus's own resampler — Comprimo already
   has one — with the added latency reported to compensation). Most hosts leave oversampling to each plug-in.
7. **Undo that includes the plug-ins.** Parameter gestures become commands; other changes become state steps;
   plug-ins that implement CLAP's draft undo extension give deltas. FabFilter's users complain that each plug-in
   keeps its own undo (research/fabfilter/family.md); in Cantus, one Ctrl+Z would undo the last thing, wherever it
   was done.
8. **Crashes that name names.** Attribution by breadcrumb in process, safe mode on the next start (open the
   project with the named plug-in disabled, state kept), and **automatic isolation**: a plug-in that crashed once
   runs out of process from then on, until the user says otherwise. Users choose grouping for the rest.
9. **Missing plug-ins keep their state.** A project opened on a machine without a plug-in shows the slot as
   missing, keeps the blob and the automation, and writes both back on save; VST 3's `moduleinfo.json`
   compatibility list and CLAP's state converter can offer a replacement.
10. **Per-plug-in compatibility switches** in one place (fixed block size, DPI mode, run isolated, forward keys to
    Cantus), with a notes field — the switches FL and Reaper have, discoverable instead of buried.
11. **Multi-core, later.** Strips whose inputs are complete can run in parallel on a small pool of real-time
    threads (the audio-thread role may move between threads, never concurrently); Reaper's "anticipative"
    processing — rendering unarmed tracks slightly ahead on worker threads — is the next step, with the same
    exception Reaper makes for record-armed tracks.

---

## 7. A proposed architecture

### 7.1 Crates, and the licence of each

The dependency direction of ADR 0002 stays: nothing in the engine learns a plug-in format; format crates depend
on the engine's contracts, and the UI wires them together.

```
cantus (bin, GPL-3) ─→ cantus-ui (GPL-3) ─→ cantus-fx-ui (egui editors + widgets + theme; licence: §2.1)
        │                    │
        │                    ├→ cantus-plugin-host ──→ cantus-plugin (format-neutral types)
        │                    │     ├ clap/  (clack-host)          ──→ cantus-engine (FxUnit, InstrumentUnit)
        │                    │     ├ vst3/  (vst3 crate)
        │                    │     └ au/    (objc2 AudioToolbox, macOS only)
        │                    └→ cantus-plugin-bridge (bin: sandbox process, same host code behind shared memory)
        ├→ cantus-plugin-scan (bin: scanner, writes the cache)
        │
exports:  cantus-clap (lib, cdylib per plug-in: FxUnit / InstrumentUnit → CLAP, on clack-plugin)
          + VST 3 / AUv2 from the same binary (clap-wrapper, or a Rust VST 3 adapter on the vst3 crate)
tests:    cantus-test-plugins (cdylib CLAP + VST 3: pass-through, gain, pure delay, sidechain echo,
          impulse-per-note instrument, crash-on-process, hang-on-init, huge-state, latency-changer)
```

| Crate | Licence | Why |
|---|---|---|
| `cantus-plugin` (types: ids, descriptors, cache records, state refs, param info) | MIT OR Apache-2.0 | used by `cantus-project` (state in the file) and by any tool that reads projects |
| `cantus-plugin-host` (CLAP + VST 3 + AU adapters) | **MIT OR Apache-2.0** — no longer needs GPL (§2) | the SDK side; embeddable by other hosts and headless renderers |
| `cantus-plugin-scan`, `cantus-plugin-bridge` (binaries) | MIT OR Apache-2.0 | same code as the host crate |
| `cantus-clap` (export wrapper) | MIT OR Apache-2.0 | links only `cantus-engine`/`cantus-dsp`/`cantus-core` and the MIT/Apache format crates |
| exported plug-in binaries *with editors* | GPL-3 if `cantus-fx-ui` stays GPL, else MIT/Apache | a binary that links a GPL crate is distributed under the GPL as a whole |
| `cantus-test-plugins` | MIT OR Apache-2.0 | test fixtures |

### 7.2 The adapter: a hosted plug-in behind Cantus's contracts

```rust
// cantus-engine (sketch): the contract grows a context and a status, nothing format-specific.
pub struct ProcessContext<'a> {
    pub frames: usize,
    pub tempo_bpm: f64, pub time_sig: (u16, u16),
    pub position_ticks: f64, pub playing: bool, pub looping: bool,
    pub steady_time: u64,            // samples since start, never jumps (CLAP steady_time)
    pub offline: bool,
    pub params: &'a [ParamEvent],    // automation from Cantus's lanes, sample-accurate: (offset, key, value);
                                     // edits from the UI reach a hosted unit on its own queue instead
}
pub trait FxUnit: Send {
    fn configure(&mut self, spec: &EffectSpec);          // native units, unchanged
    fn process(&mut self, ctx: &ProcessContext, l: &mut [f32], r: &mut [f32],
               key: Option<(&[f32], &[f32])>) -> Status; // Status: Continue | Tail(n) | Sleep
    fn reset(&mut self);
    fn latency(&self) -> usize { 0 }
    fn telemetry(&self) -> Telemetry { [0.0; TELEMETRY_LEN] }
}
```

The CLAP adapter (`cantus_plugin_host::clap::HostedFx`) then holds:

- the **audio side** (clack's started processor), preallocated input/output event lists and scratch buffers sized
  to `MAX_BLOCK_FRAMES`, a consumer of parameter events from the UI and a producer of output events to it;
- an `Arc` of **shared atomics**: cached latency, "restart wanted", "processing", the plug-in's id for the crash
  breadcrumb, and its measured time per block;
- **no** main-thread object: the `PluginInstance` stays in the UI crate's plug-in registry, which pumps
  `on_main_thread`, owns the GUI window and performs restarts.

`process` translates: Cantus's `l`/`r` into the main port (in place only if the plug-in declares an in-place
pair, else through scratch), `key` into the sidechain port (silence and `constant_mask` when `None`), `ctx` into
a transport event and parameter events, calls the plug-in, returns output parameter events on the ring, and maps
CLAP's process status to `Status`. VST 3's adapter has the same outer shape (`ProcessData`, `IParameterChanges`,
`IEventList`, `ProcessContext`).

**The project model** (cantus-core): `EffectSlot` gains `plugin: Option<PluginRef>` (`PluginRef { format, id,
name, vendor, version, state: StateRef }`) with `EffectKind::Plugin`; `InstrumentSpec::Plugin(PluginRef)` is
the variant the code already anticipates ("plugin hosting will add a `Plugin { format, id, state }` variant").
Parameter keys are `p{id}` (§3.6). Schema bump with a fixture, as for every model change.

**The editors' seam, for export.** Cantus's editors (`views::fx::*`, `eq_editor`, `comp_editor`) take the app
(`crate::app::CantusApp`) and emit `Command`s. To reuse them inside an exported plug-in they need a small trait
instead — `FxHost { spec(), set(key, value), begin/end gesture, telemetry(), analyser() }` — implemented by the
app (commands, undo) and by the plug-in wrapper (CLAP gestures and values). Moving them with `widgets`, `theme`
and `i18n` into `cantus-fx-ui`, which depends on `egui` only (no `eframe`, no `cantus-ai`), is the refactor that
makes export possible, and is where the licence decision for exported editors is made.

### 7.3 The order of work, in small shippable steps

Each step ends with something the user can see or a test that holds, and none depends on a later one.

| # | Step | Ships | Held by |
|---|---|---|---|
| 0 | **ADR**: formats (CLAP first, VST 3 second, AU on macOS, no VST 2), licence re-evaluation, crates, the contract changes | a decision | — |
| 1 | **Whole-block processing for units** and `ProcessContext`/`Status` on `FxUnit`; segment splitting kept inside native instruments | nothing visible; groundwork | every existing null test and the golden hash **bit-identical**; `fx_contract` block-size test unchanged |
| 2 | **`cantus-test-plugins`** (CLAP on clack-plugin) and **`clap-validator` in CI** against them | the fixtures every later step uses | clap-validator passes on the good ones; fails as expected on the bad ones |
| 3 | **Scanner + cache**, CLAP only; a read-only "Plug-ins" list in the browser (name, vendor, format, status, *why* it failed) | the user sees their CLAP plug-ins | scan of the fixture folder; cache hit on second run (no process spawned); mtime change → rescan; crash plug-in → `failed: crashed`, parent unharmed; hang plug-in → `failed: timeout` |
| 4 | **Host a CLAP effect** on an insert, in process, **generic parameter editor** (egui, from the params extension); state in the project | third-party effects on the mixer | null test: a track through the pass-through plug-in equals the track without it, bit-exact; a 100-sample delay plug-in passes the ADR 0018 PDC tests unchanged; state round-trip (save → load → identical bytes, identical parameter values); undo fuzz with `p{id}` parameters; offline render equals live |
| 5 | **Plug-in editor windows** (Windows first), per-plug-in DPI mode, gestures → one undo step each | the plug-in's own GUI | gesture coalescing unit test (begin/values/end → one command, old value from the begin); manual checklist on 10 popular free CLAP plug-ins |
| 6 | **Crash attribution and safe mode** (breadcrumb, next-start dialog, open with plug-in disabled) | a crash names the plug-in | the crash-on-process fixture run in a child process: the breadcrumb file names it; project reopens with the slot disabled and the state intact |
| 7 | **CLAP instruments**: `InstrumentUnit`, note ids and sample offsets, note end, voice info, all-notes-off on stop; multi-out as strips on demand | third-party synths on the channel rack | the impulse-per-note fixture puts its impulse at the exact sample of each note at block sizes 1, 64, 1000; stop sends note-offs; multi-out fixture fills two strips |
| 8 | **Out-of-process bridge**, lock-step, per plug-in and "automatic after a crash" | a crashing plug-in no longer ends the session | the pass-through fixture bridged nulls bit-exact against in-process; crash fixture bridged → slot shows "crashed", audio continues, reload works; measured round-trip overhead recorded in STATUS |
| 9 | **VST 3 hosting** on the same adapters: `moduleinfo.json` fast scan, `IMidiMapping`, `.vstpreset`-shaped state, `IPlugView` | the rest of the market | the same suite, parameterised by format, using Steinberg's MIT sample plug-ins (`again`, `adelay`, `note_expression_synth`) built in CI; the SDK's validator on VST 3 builds of the test plug-ins (through clap-wrapper) |
| 10 | **Export one plug-in** (Terminus, the limiter) as CLAP from `cantus-clap`, editor via egui in baseview; then VST 3 (clap-wrapper or the vst3 crate) | a Cantus plug-in loads in Reaper/Bitwig/FL | clap-validator and the VST 3 validator in CI; **the dogfood null**: Cantus hosting its own exported Terminus nulls bit-exact against the native Terminus |
| 11 | **Instruments exported**, polyphonic modulation both ways, MPE/note expression, preset discovery; AU hosting and AUv2 export on macOS; LV2 on Linux; DAWproject export | the long tail | per-feature fixtures |

The **dogfood null** in step 10 is the strongest single test available: the host and the export check each other,
and any rounding, block-splitting or parameter-mapping error in either shows up as a non-zero residual. It can be
bit-exact at default settings; a parameter that travels through the normalised taper (plain → 0–1 → plain) may
come back one rounding step away, so moved parameters need either values that survive the round trip exactly
(a test of the taper itself) or a stated tolerance.

---

## 8. Risks and unknowns

| Risk | Why it matters | Mitigation |
|---|---|---|
| **The processor's segment loop** (§3.2 item 4) | the first hosting step touches the heart of the engine; a mistake moves every render | step 1 is its own round, held by the golden hash and every null test before any plug-in exists |
| **Third-party code on the main thread** | `state.save`, `activate`, `setState` of a big sampler can take seconds; egui freezes | save and autosave through a progress-aware path; CLAP's new draft background-state and background-activation extensions (1.2.9) allow it off the main thread for plug-ins that support them; show "waiting for <plug-in>" instead of freezing silently |
| **Rust bindings' maturity** | clack and the vst3 crate are young and pre-1.0 (§3.1); their APIs will move | pin versions; keep all `unsafe` FFI in `cantus-plugin-host` (idea.md §12 already asks for that); `clap-validator` and the test plug-ins catch regressions |
| **egui version skew in exported editors** | an exported plug-in's editor must use the same egui as `cantus-fx-ui`; the glue has lagged before (nih_plug_egui is still on egui 0.31) although egui-baseview is on 0.36.1 today (§4.4) | upgrade egui in the app and the glue together, as one step; if the glue falls behind, the glue is small enough to own (a baseview window and an egui painter), and Cantus's software rasteriser (`raster.rs`) is a last-resort painter where GL is unavailable |
| **Several egui/GL instances in another host's process** | two plug-ins with different egui/GL versions in one DAW process; macOS has deprecated OpenGL | each plug-in statically links its own egui (Rust does by default); prefer a wgpu or software painter on macOS; test with two different Cantus plug-ins in one Reaper project |
| **Main-thread identity on macOS** | AppKit requires the process's first thread for UI; eframe already runs there | keep all plug-in GUI and main-thread calls on the eframe thread; never create plug-in windows from worker threads |
| **Real-time scheduling of plug-in threads** | on Apple Silicon a real-time thread that has not joined the audio device's `os_workgroup` can be scheduled on efficiency cores; plug-ins with internal threads suffer too | join the workgroup for Cantus's own worker threads and for the bridge process's thread; CLAP's `thread-pool` lets plug-ins use host threads. Whether cpal 0.18 exposes the workgroup was **not verified** |
| **Determinism** | ADR 0005 rule 5 cannot hold for arbitrary third-party code | promise it for native units only; null tests use fixtures; renders of projects with hosted plug-ins are "as the plug-ins allow" |
| **Offline render instances** | a second instance per plug-in doubles memory and may consume a licence seat | allow both designs (§3.8); measure on a large project before choosing the default |
| **Sandbox latency and CPU** | lock-step IPC costs context switches per plug-in per block; pipelined costs a block of latency | per-plug-in choice, grouping by vendor, default in-process with *automatic* isolation after a crash |
| **VST 2** | many old free plug-ins are VST 2 only; the SDK was withdrawn in 2018 and was **not** relicensed | out of scope; say so in the plug-in manager ("VST 2 is not supported; look for a VST 3 or CLAP version") |
| **AAX** | Pro Tools loads only PACE-signed AAX in release builds; the AAX SDK is GPLv3 or commercial | exporting to Pro Tools is a business decision (Avid agreement, iLok/PACE account), not an engineering step; not in the plan |
| **ASIO** | the ASIO SDK is GPLv3 or proprietary; cpal's `asio` feature builds against it | the application stays GPL-3 (§2), so enabling ASIO in the app binary is licence-compatible; the MIT/Apache crates must not enable it |
| **Ableton Link** | Link is GPLv2-or-later or a proprietary licence | same: fine inside the GPL-3 app, never inside the MIT/Apache crates |
| **Legal wording** | "VST" is Steinberg's trademark; the logo is now optional and its guidelines apply only if used | say "VST 3" in text as the format name; do not use the logo unless the guidelines are followed |
| **Plug-in behaviour we cannot fix** | plug-ins that ignore the thread rules, allocate in `process`, or mis-report latency | CLAP's `thread-check` lets the host tell plug-ins which thread they are on; a debug build of Cantus can assert roles; per-plug-in compatibility switches in the manager (fixed block size, DPI mode, run isolated), as FL and Reaper have |

**Could not be verified for this report** (web searches ran out part-way, and some vendor pages refused the
fetch; nothing was worked around):
- exact CLAP host and plug-in counts for September 2026 beyond clapdb's 19 host entries and Wikipedia's undated
  figures; whether FL Studio or Studio One implement CLAP polyphonic modulation; which hosts implement CLAP's draft
  `undo`; whether JUCE 9 ships CLAP export yet (announced, absent from its change lists); any Ableton CLAP plans;
- that Logic runs AUs in AUHostingService on Apple silicon (forum-sourced); the name and structure of Bitwig's
  sandbox processes; the Bitwig CPU figures (forum measurements);
- current, rigorous measurements of lock-step IPC cost per block (Apple's 2015 figure and FL's manual are the only
  published numbers found);
- whether cpal 0.18 exposes Apple's audio workgroup; whether clack's extension traits admit a draft extension
  defined outside `clack-extensions` (inferred, not tried); who uses clack through git dependencies; which CLAP
  binding Maolan uses;
- how commercial samplers behave when a fresh instance is rendered offline immediately after loading state.

---

## 9. Sources

**Specifications and SDKs**
- VST 3 SDK repository and README (licence statement, branding): https://github.com/steinbergmedia/vst3sdk
- VST 3 SDK `LICENSE.txt` (MIT): https://raw.githubusercontent.com/steinbergmedia/vst3sdk/master/LICENSE.txt
- VSTGUI licence (BSD-3-Clause): https://github.com/steinbergmedia/vstgui/blob/develop/LICENSE
- VST 3 version history: https://steinbergmedia.github.io/vst3_dev_portal/pages/Versions/Index.html
- VST 3.8.0 changes: https://steinbergmedia.github.io/vst3_dev_portal/pages/Versions/Version+3.8.0.html
- VST 3.8.1 changes: https://steinbergmedia.github.io/vst3_dev_portal/pages/Versions/Version+3.8.1.html
- VST 3 audio processor call sequence: https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Workflow+Diagrams/Audio+Processor+Call+Sequence.html
- VST 3 `moduleinfo.json`: https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/VST+Module+Architecture/ModuleInfo-JSON.html
- VST 3 preset format: https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Preset+Format.html
- VST 3 MIDI mapping: https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Change+History/3.0.1/IMidiMapping.html and https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Communication.html
- CLAP repository, headers and ChangeLog: https://github.com/free-audio/clap (`include/clap/clap.h`, `all.h`, `ext/params.h`, `ext/gui.h`, `ext/latency.h`, `events.h`, `ext/draft/undo.h`, `factory/draft/plugin-state-converter.h`, `factory/preset-discovery.h`), https://raw.githubusercontent.com/free-audio/clap/main/ChangeLog.md
- CLAP tags and the 1.2.10 commit (2026-07-13): https://api.github.com/repos/free-audio/clap/tags
- clap-wrapper: https://github.com/free-audio/clap-wrapper · clap-validator: https://github.com/free-audio/clap-validator · pluginval: https://github.com/Tracktion/pluginval
- DAWproject (README and `Project.xsd`): https://github.com/bitwig/dawproject
- Microsoft, mixed-mode DPI scaling: https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-improvements-for-desktop-applications
- Apple developer forums on AUv3 in-process loading: https://developer.apple.com/forums/thread/66557
- WWDC 2015 session 508 (AUv3 out-of-process cost): https://asciiwwdc.com/2015/sessions/508

**Licensing news**
- KVR, "Steinberg moves VST 3 SDK to MIT, ASIO now GPLv3" (2025-10-29): https://www.kvraudio.com/news/steinberg-moves-vst-3-sdk-to-mit-open-source-license-asio-now-gplv3-65179
- CDM, "Open Steinberg": https://cdm.link/open-steinberg-vst3-and-asio/
- Libre Arts, "VST3 becomes open-source, ASIO goes GPL-compatible": https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/
- ASIO SDK copy: https://github.com/audiosdk/asio
- VST 2 end of licensing: https://www.gearnews.com/steinberg-discontinues-support-for-vst2-plug-ins/ and https://forums.steinberg.net/t/vst-2-sdk-discontinued/201774
- AAX SDK open-source release: https://forum.juce.com/t/aax-open-source-release/44017 · Dplug's AAX guide: https://github.com/AuburnSounds/Dplug/wiki/Dplug-AAX-Guide

**Hosts**
- FL Studio supported formats: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins_supported.htm · wrapper: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/wrapper.htm · plug-in manager: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_externalplugins.htm
- Bitwig plug-in hosting options: https://www.bitwig.com/userguide/latest/vst_plug-in_handling_and_options/ · crash protection: https://www.bitwig.com/learnings/plug-in-hosting-crash-protection-in-bitwig-studio-20/ · 2.5 notes: https://downloads.bitwig.com/stable/2.5/Release-Notes-2.5.html · browsers: https://www.bitwig.com/userguide/latest/browsers/
- REAPER user guide: https://www.reaper.fm/userguide.php · dedicated process: https://reaper.blog/2012/02/run-plugin-as-dedicated-process/ · anticipative FX: https://forums.cockos.com/showthread.php?p=1840896
- Ableton formats: https://help.ableton.com/hc/en-us/articles/5937501570460-Supported-Plug-in-Formats · rescanning in 12.1: https://help.ableton.com/hc/en-us/articles/16261934134940-Rescanning-plug-ins-in-Live-12-1 · MPE: https://help.ableton.com/hc/en-us/articles/360019144999-MPE-in-Live-11-and-later-FAQ · release notes: https://www.ableton.com/en/release-notes/live-12/
- Cubase VST 2 in 14/15: https://helpcenter.steinberg.de/hc/en-us/articles/31858223176210-Using-VST-2-Plug-ins-in-Cubase-Nuendo-14-and-Cubase-15 · Plug-in Sentinel: https://helpcenter.steinberg.de/hc/en-us/articles/207348390-Plug-in-Sentinel-for-Cubase-9
- Studio One 7 release notes (PDF): https://assets.ctfassets.net/b7gl3dxk4jyc/2RnD4exKaDajTrR1zGj9kh/2e44026185d2625d34fde7d10c702d57/Studio_One_7_-_Release_Notes.pdf
- Logic and Audio Units: https://support.apple.com/guide/logicpro/work-with-audio-units-in-logic-pro-for-mac-lgcp22a0dab0/mac · https://support.apple.com/en-us/122179
- Ardour on CLAP (Libre Arts, "CLAP two years later"): https://librearts.org/2024/11/clap-api-two-years-later/
- clapdb (CLAP hosts and plug-ins): https://clapdb.tech/ · cleveraudio: https://cleveraudio.org/hosts-and-plug-ins/ · Wikipedia: https://en.wikipedia.org/wiki/CLever_Audio_Plug-in
- JUCE roadmap (CLAP in JUCE 9): https://juce.com/blog/juce-roadmap-update-q3-2025/ · JUCE change list: https://raw.githubusercontent.com/juce-framework/JUCE/master/CHANGE_LIST.md
- Carla's bridge design (falkTX): https://github.com/obsproject/obs-studio/discussions/5074 · yabridge: https://github.com/robbert-vdh/yabridge
- Forum measurements (hearsay, cited as such): https://www.kvraudio.com/forum/viewtopic.php?t=591669 · https://www.kvraudio.com/forum/viewtopic.php?t=610071 · https://www.kvraudio.com/forum/viewtopic.php?t=629382

**Rust crates** (licences read from Cargo.toml/LICENSE; versions from https://crates.io/api/v1/crates/NAME)
- clack: https://github.com/prokopyl/clack · https://crates.io/crates/clack-host
- clap-sys: https://github.com/micahrj/clap-sys
- vst3 (coupler): https://github.com/coupler-rs/vst3-rs · vst3-sys: https://github.com/RustAudio/vst3-sys
- vst3-host: https://github.com/HelgeSverre/rust-vst3-host · truce-rack: https://github.com/truce-audio/truce-rack · rack: https://github.com/sinkingsugar/rack
- nih-plug: https://github.com/robbert-vdh/nih-plug (and its CLAP wrapper source, `src/wrapper/clap/wrapper.rs`, read for the parameter ranges)
- nice-plug: https://codeberg.org/RustAudio/nice-plug · egui-baseview: https://codeberg.org/RustAudio/egui-baseview · baseview: https://github.com/RustAudio/baseview
- truce: https://github.com/truce-audio/truce (and its `LICENSE`) · beamer: https://github.com/helpermedia/beamer · aura (GPL-3, 4 stars, an example of a CLAP-first Slint framework): https://github.com/LX-Audiolabs/aura
- clap-wrapper (Rust): https://github.com/blepfx/clap-wrapper-rs
- objc2 AudioToolbox: https://crates.io/crates/objc2-audio-toolbox · livi: https://github.com/wmedrano/livi-rs · rust-lv2: https://github.com/RustAudio/rust-lv2
- rusty_link: https://github.com/anzbert/rusty_link · Ableton Link licence: https://github.com/Ableton/link/blob/master/LICENSE.md
- midi2: https://github.com/midi2-dev/bl-midi2-rs · dawproject crate: https://github.com/bkbkb-net/dawproject
- Maolan: https://github.com/maolan/maolan · Meadowlark: https://codeberg.org/Meadowlark/Meadowlark
- VST 3 parameter id range and stability: https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Parameters+Automation/Index.html

**Cantus** (read at the date of the report): `docs/adr/0001-license.md`, `0002`, `0005`, `0006`, `0017`, `0018`;
`crates/cantus-engine/src/{fx/mod.rs, processor.rs, effects.rs, render.rs, messages.rs}`;
`crates/cantus-core/src/{effect.rs, instrument.rs, snapshot.rs}`; `crates/cantus-engine/tests/fx_contract.rs`;
`idea.md` §5.7–5.8.
