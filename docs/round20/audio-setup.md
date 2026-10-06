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
- Settings › Audio: *Apply* goes through `audio_setup::apply` (remembered per interface);
  two buttons open the setup assistant and the routing page.
- A track armed with no input open now opens the **audio popover** (not Settings).
- Strings: `locales/areas/devices.{en,es}.txt` (round 19's area), ~100 keys each.
- `examples/shoot_audio_setup.rs` renders every view in Slate and Paper (chip at rest,
  amber, lost; popover; popover with a problem; interface list; first run; routing;
  cards; Spanish; first run in Spanish at 150 %).

## Tests

(see the report at the end of this file)

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
