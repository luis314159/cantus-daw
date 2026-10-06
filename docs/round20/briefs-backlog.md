# Round 20, second wave — four items from `docs/BACKLOG-excellence.md`

Chosen because they do not collide with the five builders already running (automation,
device-ux, browser, hosted-inst, samples). Read `docs/round20/BUILDING.md` (rules) and your
section of `docs/BACKLOG-excellence.md` first. Each registers its own locale area in
`i18n.rs::sources()` (one line per locale) — the integrator resolves the adjacent lines.

## verse — a verse at every start (BACKLOG §6c)

- On the splash (round 18) and on the start/welcome view, a Bible verse with its reference
  in the **interface language**, random per start from a curated list of a few hundred
  verses (encouragement, work, creativity, praise, music — Psalms 33:3, 96:1, 150;
  Colossians 3:23; Ephesians 5:19; Exodus 35:31–35; 1 Chronicles 15:16 …).
- **Translations**: Spanish **Reina-Valera 1909** (public domain) — the owner prefers the
  RVR1960 if its terms allow; they are not confirmed (see §6c), so ship RV1909 and leave a
  clean way to add RVR1960 later. English: **World English Bible** (public domain
  everywhere) or KJV (public domain outside the UK) — pick WEB unless there is a reason not
  to, and say why. Get the **exact text** of each verse from a public-domain source
  (e.g. ebible.org's public-domain texts, Wikisource) — never from memory; record the
  source URL and date in `locales/verses/README.md`. Book names in each language.
- Files: `crates/cantus-ui/locales/verses/{es,en}.txt` (reference + text per line),
  loaded like the locales; the translation's name and licence in the About box.
- **On by default with an On/Off switch** (the owner: so someone who does not share our
  beliefs can turn it off): Settings ▸ General ▸ *Show a verse at start*, and a small
  *don't show again* on the verse itself; persisted in `settings.json`.
- The kit's look (no stock widgets), beautiful typography, never blocking start-up.
- Tests: every verse file parses; every locale has a file; the switch hides it; a verse
  fits the splash at 75 % and 200 % UI scale in both languages. Render and look.

## packaging — installer, signing, updates (BACKLOG §6)

- A Windows installer (WiX MSI or NSIS — choose and justify; per-user install without
  admin rights preferred), Start-menu shortcut, `.cantus` file association with the
  app's icon (round 18 made the icons), uninstaller, optional desktop shortcut; the
  plug-ins (`cargo run -p xtask -- bundle`) optionally installed to the standard
  CLAP/VST3 folders; a portable zip as well. Driven by `cargo run -p xtask -- package`.
- An ASIO variant of the package (the `asio` feature; `docs/round19/devices.md` has the
  build steps — the ASIO SDK is in `C:\projects\daw-r19\asio\ASIOSDK`; never commit it).
- **Code signing**: the pipeline (signtool / Azure Trusted Signing / a certificate in the
  build environment), working end to end with a self-signed test certificate, and a doc
  of what the owner must buy or set up for real signing (options and costs) — no secrets
  in the repo.
- **Auto-update**: check GitHub Releases of the project's repository (the remote is
  `luis314159/cantus`) at most once a day, on start, without telemetry; show the release
  notes; download, verify (SHA-256, and the signature when signed), install on quit or
  on request; a setting to turn checks off; works behind a proxy that may quarantine
  downloads (verify what arrives; never bypass TLS). A GitHub Actions workflow that
  builds, tests and publishes a release on a tag (the owner decides when to tag).
- Locale area `locales/areas/update.*`. Tests: the update check parses a recorded GitHub
  API response, compares versions, refuses a bad hash; the package task produces the
  expected file list. Do not publish anything; do not push tags.

## audio-setup — audio configuration that is direct (BACKLOG §6b)

- **An audio chip always visible** (transport or status bar): interface, driver, rate,
  buffer, round-trip latency; amber on dropouts, red when the device is lost; one click
  opens a compact popover to switch interface, driver, rate and buffer in place, with the
  latency of each buffer size shown before choosing. Builds on round 19's
  `views/settings_audio.rs` and `cantus_engine::devices` (read `docs/round19/devices.md`).
- **First-run setup**: choose the interface, a test tone on the chosen outputs, live input
  meters ("speak or play — the meter should move"), the suggested driver (ASIO when the
  interface has one) and buffer, an offer to run the loopback calibration.
- **Plug-and-play**: notice an interface being connected or removed (poll the device
  list off the UI thread) and offer to switch; settings remembered **per interface**; a
  lost device keeps the session editable and reconnects when it returns.
- **Channel names and simple routing**: name the inputs/outputs, a small routing page,
  a second output pair for a headphone cue.
- **Problems in words with a fix button** (rate mismatch, device in exclusive use, a
  stalled ASIO driver — round 19's `output_stalled()`).
- Strings in `locales/areas/devices.*` (round 19's area). Tests through published
  geometry; the popover switches the config; per-interface memory round-trips; the
  device-lost path keeps the app working with a dummy engine. Render and look.

## bench — measure before optimising (BACKLOG §4b)

- A benchmark suite (`crates/cantus-bench` or `benches/`, criterion or a small harness of
  our own): DSP kernels (each effect at default and heavy settings, each instrument with
  N voices), the processor with 10/50/100-track sessions (the demo songs scaled up),
  offline render speed (× real time), UI frame times for big sessions (the existing
  `frame_budget` tests are a start), start-up and project-open time.
- A DAWbench-style **DSP test** and **VI test** defined so the same session can be built in
  REAPER/Ableton/FL for comparison later (write the method in `docs/perf/benchmark.md`).
- **Callback histograms**: the engine records per-block times into a lock-free histogram
  (no allocation) exposed in `EngineStats`; a **dropout detector with blame** — which node
  (track instrument, insert, master chain) ran longest in an overrun block — surfaced in
  the status line. Real-time safe; null tests unchanged.
- **CI gates**: a GitHub Actions job (or an `xtask perf` command for now) that runs the
  quick benchmarks and fails on a regression beyond a threshold against a stored
  baseline; baselines per machine class, documented.
- Report the numbers you measure today in `docs/perf/baseline.md` — they are the
  starting line for the "better than the market" work.
