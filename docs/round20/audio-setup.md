# Round 20 — audio-setup: configuring audio, made direct

Builder: **audio-setup**, branch `r20/audio-setup` (from 805a720). BACKLOG-excellence §6b.
Decision recorded as ADR 0025 §11 (an amendment, so no new ADR number is taken).

The bar: *a musician who has never opened Cantus records a guitar through a new interface
in under a minute without opening Settings.* What that is now: Cantus opens on the
first-run setup → click the interface's card (it opens with the suggested driver and
buffer) → strum: the meter moves and the input is found → **Record on Input 1**: an armed
audio track on that input, selected. Press record. Three clicks; Settings never opens
(`tests/audio_setup_first.rs` holds exactly that path).

## What is done

### Engine (`cantus-engine`)

- **`routing.rs`** — the output routing, run in the output callback *after* the processor
  (`OutputDriver::fill`): the master moved to another pair (`set_main_output`), a
  headphone **cue** that copies the master to a second pair at its own level
  (`set_cue_output`), and a **test tone** (440 Hz, −18 dBFS, 20 ms fades, on any pair:
  `set_test_tone`). The UI writes atomics (`OutputRouting`); the callback reads them once
  per buffer; no allocation, lock or I/O. With the defaults it returns before touching a
  sample: **null test**, bit-identical output.
- **`hotplug.rs`** — `DeviceWatch`: a thread lists the system driver's endpoints every
  1.5 s and sends the list **only when it changed**; `poll()` never blocks, dropping the
  watch never waits for a scan. It always lists WASAPI (an interface's endpoints come and
  go with its cable even while playing through its ASIO driver, and listing ASIO would
  load drivers). `DeviceList::added_since / removed_since / has`. `with_source` takes a
  scripted list for tests.
- **`devices.rs`** — the streams' error callbacks now count **xruns**
  (`DeviceTelemetry::xruns`) and mark the device **lost** on `DeviceNotAvailable` /
  `StreamInvalidated` / `HostUnavailable` (`stream_lost`). `describe_error` tags cpal's
  `DeviceBusy` (WASAPI `AUDCLNT_E_DEVICE_IN_USE`) with `BUSY_TAG`, so "another app holds the
  device" is recognised. `EngineHandle::device_lost()` (= stream lost or output stalled),
  `xruns()`, `output_channel_count()`, `output_route()`.

### UI (`cantus-ui`)

- **`audio_setup/`** (the model):
  - `interfaces.rs` — endpoints grouped into **interfaces** by the adapter name WASAPI puts
    in parentheses ("Speakers (UMC204HD 192k)" → "UMC204HD 192k"; "2- " prefixes dropped);
    ASIO drivers matched by a distinctive word ("UMC ASIO Driver" → UMC204HD; "Focusrite
    USB ASIO" → Focusrite USB Audio); a **kind** for the picture (interface, this computer,
    headphones, screen, virtual); `suggest`: ASIO at 48 kHz / 128 when the build has it and
    the interface is a real interface (never Realtek's ASIO, which stalls with both
    directions — round 19), else WASAPI at 256 with the interface's own input.
  - `memory.rs` — **per-interface memory** in `settings.json` (`audio_setup.profiles`):
    driver/device/input/rate/buffer/offset, channel names, main pair, cue pair and level;
    `setup_done`; `last`. Old files read as "nothing remembered".
  - `apply.rs` — one path for every change: `apply` stores the choice, remembers it for its
    interface, reopens the device (or, `offline`, records it), re-applies the routing.
    `choose_interface` (remembered settings, else the suggestion), `reconnect`, `fix`,
    `tick` (once a frame, allocation-free: scan results, the watcher, dropouts, a device
    the engine lost, routing and names when the device changes), `estimate_ms` (the round
    trip a buffer will give: `2 × buffer + overhead`, the overhead measured on the running
    device), first-run open/close/finish.
  - `plug.rs` — plug-and-play: the interface in use leaving → *lost* (card, red chip,
    status line; the session keeps working on the handle, which accepts and drops
    messages); it returning → reopened by itself with its settings; another arriving → a
    card offers to switch.
  - `problems.rs` — `diagnose`: lost, stalled (ASIO says so), would not open, **busy**
    (exclusive use), **rate mismatch** (the input at 44.1 kHz, the output at 48 —
    fixes: *Run at 44.1 kHz*, *Use the interface's input*), input failed, **dropouts**
    (3+ in 30 s → *Use 256 samples*). Each with its sentence (EN/ES) and fix buttons.
  - `names.rs` — the channel names where every input menu reads them: `input_label`
    (track header, inspector, mixer menus) shows "Guitar", "Guitar / Vocal", "Keys" (for
    "Keys L"/"Keys R") instead of "In 1".
  - `demo.rs` — a plausible studio (UMC204HD + ASIO, Realtek, Bluetooth headphones, a
    screen) for tests and screenshots, offline.
- **Views**:
  - `audio_chip.rs` — the chip at the right of the status bar: lamp + `UMC204HD · ASIO ·
    48 k · 128 · 6.9 ms` + ▾. Green / amber (5 s after a dropout, or an input problem) /
    red (lost, or no device: "No audio — click to set up"). Text rebuilt only on change.
    The status bar's engine readout no longer repeats the device and rate (`CPU · dropouts
    · voices`).
  - `audio_popover.rs` (+ `_lists.rs`) — over the chip: the interface's picture, name,
    what it runs at and its state; **problems with fix buttons first**; Interface (a list
    with each kind, ASIO marked), Driver (segmented), Input (the interface's own first),
    Rate (Auto 44.1 48 88.2 96), **Buffer** — a key per size with "≈ 4.2 ms" under it and a
    green/amber/red dot; what the drivers measure now; Test tone, Channels & routing…,
    Setup assistant…, Settings. Every choice applies at once. Escape / click outside close.
  - `audio_firstrun.rs` — the first-run setup (opens once when audio was never set:
    `arm_first_run` in `assemble`, i.e. only the real app): four numbered steps — cards of
    the interfaces with drawn pictures; the test tone; live input meters ("Input 1 is
    moving: that is the one to record from"); latency in words ("about 10.7 ms from string
    to speaker — fine for most playing") and the loopback measurement offered; *Skip* /
    *Record on Input 1*.
  - `audio_routing.rs` — channels & routing: main output pair, headphone cue pair and
    level (a knob), test tone on any pair, a name field for every output and input, input
    meters. Stored with the interface.
  - `audio_notice.rs` — device cards at the bottom right (connected / disconnected /
    back), with their buttons.
  - `audio_art.rs` — the pictures, hand-drawn vectors in theme tokens: an interface's front
    panel (combo jacks, gain knobs, signal lamps, monitor knob), a laptop, headphones, a
    screen, a virtual cable.
  - `audio_parts.rs` — a wrapped paragraph (the kit's text boxes are single-line), a
    primary button, a choice key with a caption, a horizontal level bar.
  - `audio_drivers.rs` + `audio_setup/drivers.rs` — **Get a low-latency driver** (the
    owner's request of 2026-10-06): opened from *Get ASIO…* beside the Driver row of the
    popover (always there) and from the first run's latency step. It says which ASIO
    drivers are installed (read from the registry even in a build without ASIO) and, in a
    build without the `asio` feature, says plainly that this build cannot use them — which
    is very likely why the owner could not find ASIO4ALL after installing it. Then:
    FlexASIO's latest release page; ASIO4ALL's site (a link only: its licence forbids
    redistribution); the makers' driver pages (13 makers, the detected interface's maker
    first and marked); an obvious **Rescan drivers** button, and a rescan whenever the
    window gets the focus back while the popover, the setup or this window is open, so a
    driver installed meanwhile appears. In an ASIO build, Driver ▸ ASIO now appears as
    soon as *any* ASIO driver is installed (FlexASIO, ASIO4ALL, an interface's), and an
    **ASIO driver** row lists them all by name.
  - The owner also asked for FlexASIO to be **downloaded and installed from inside
    Cantus** (GitHub API → download to temp → verify → run with UAC). The pure parts are
    here and tested — `parse_release` (GitHub's answer: tag, `.exe`, size, `.sigstore`)
    and `verify_installer` (refuses an HTML page — the proxy's quarantine — a non-PE file,
    a wrong size). **The download-and-run step itself was not wired**: the session's
    permission policy refused it as an execution surface (fetch an installer and run it
    elevated). Today the button opens the release page and the text says how to install.
    Wiring it needs the owner's explicit approval of that capability.
- Settings › Audio: *Apply* goes through `audio_setup::apply` (remembered per interface);
  two buttons open the setup assistant and the routing page.
- A track armed with no input open now opens the **audio popover** (not Settings).
- Strings: `locales/areas/devices.{en,es}.txt` (round 19's area), ~100 keys each.
- `examples/shoot_audio_setup.rs` renders every view in Slate and Paper (chip at rest,
  amber, lost; popover; popover with a problem; interface list; first run; routing;
  cards; Spanish; first run in Spanish at 150 %).

## Tests

- `cantus-engine/tests/routing.rs` (4): **null test** — the default routing leaves 1, 2,
  4 and 8-channel output bit-identical; the master moves to another pair (and falls back
  to 1–2 on a device without it); the cue is a copy at its level, the gain clamped; the
  tone plays only on its pair at −18 dBFS, fades in from silence and out, then the
  routing is null again.
- `cantus-engine/tests/hotplug.rs` (2): the watcher sends the first list, then only
  changes (added / removed); polling and dropping never wait for a slow scan.
- `cantus-ui/tests/audio_setup.rs` (4): endpoints → interfaces, ASIO matching, kinds,
  suggestions; per-interface memory round-trips through `settings.json` (and a pre-20
  file reads as nothing remembered); **the chip opens the popover, and buffer, rate and
  interface (from its list) switch in place through the published rects**, each
  interface getting its own settings back; the round trip per buffer estimate.
- `cantus-ui/tests/audio_setup_flows.rs` (3): **a lost interface** — red chip, the card,
  the session still edits and undoes, frames still draw, the problem said by name — and
  **it comes back by itself** with its settings; **a new interface is offered** through
  the watcher thread and one click on the card switches; **problems with fixes**: rate
  mismatch (both fixes clicked in the popover), busy, stalled ASIO, dropouts → bigger
  buffer, every sentence in EN and ES.
- `cantus-ui/tests/audio_setup_first.rs` (4): **the first run — three clicks to an armed
  track on the input that moved**, Settings never opened, the tone stopped, one undo;
  it opens only for a fresh install and only in the real app; channel names replace
  "In 1" in `input_label`; the routing page sets cue and main pair through its rects and
  stores them; every view painted in EN and ES without a missing key; the chip's lamp
  (green, amber after a dropout, red lost / no device).
- `cantus-ui/tests/audio_drivers.rs` (3): a recorded GitHub API response is parsed (tag,
  installer, size, signature bundle; a proxy page and junk refused); installer
  verification (PE + size; HTML quarantine page, zip, wrong size refused; every refusal
  worded in both languages); the makers' list with the detected maker first.
- `rec_flows.rs`: the no-input case now opens the popover.

Runs: `cargo test -p cantus-ui` — 527 passed (frame budget, kit lint, gallery included);
`cargo test -p cantus-engine` — 276 passed; `cargo clippy -p cantus-engine -p cantus-ui
--tests --examples` clean; `cargo fmt` applied.

Looked at: `cargo run -p cantus-ui --example shoot_audio_setup` (26 PNGs in
`target/shoot-audio`), Slate and Paper, English and Spanish, 100 % and 150 %. Fixed after
looking: captions shouted "MS" (caps off), the cue value elided, the first run overflowed
at 150 % (it scrolls now, the footer stays), the popover's rows moved for a frame after
opening (it is placed from its own height), the first run's veil could come in front.

## Files touched outside this area

`cantus-engine`: `backend.rs` (the routing handed to the output driver; error text),
`devices.rs` (telemetry fields, error callbacks, the routing call in `fill`), `lib.rs`
(2 modules). `cantus-ui`: `app.rs` (field, init, `arm_first_run` in `assemble`, `tick` and
`views::audio_setup::frame` in `frame`, `apply_routing` after a restart — 6 one-line
hunks), `lib.rs` (1), `settings.rs` (a field), `views/mod.rs` (module list),
`views/workspace_status.rs` (room for the chip, the engine readout without device/rate),
`views/settings_audio.rs` (Apply through `apply`, two buttons), `record_session.rs`
(`input_label` reads names; no-input opens the popover), `tests/rec_flows.rs` (that
assertion), `docs/adr/0025-recording-audio.md` (§11).

## Left

- **In-app FlexASIO install** (see above): parsing and verification are done and tested;
  the download → run-elevated step awaits the owner's approval of that capability.
- Real hardware: the hot-plug path and the routing were tested with scripted device
  lists and a hand-driven engine; a run with a USB interface plugged and unplugged, and
  a 4-output interface for the cue, is still to do (the machine here has only Realtek).
- A WASAPI endpoint that exposes only 2 channels cannot carry a cue on 3–4; the routing
  page offers the pairs the open stream has. Opening WASAPI with all of a device's
  channels (not the mix format's) would widen that.
- The cue is a copy of the master; a separate cue mix (per-channel sends to the cue) is
  the next step.
- Exclusive-mode WASAPI (lower latency without ASIO) is not offered.
