# Round 30 — integration: Cantus on three systems

**The ask** (the owner, 2026-10-22, `docs/next-session.md` of round 29): releases of Windows,
Linux and macOS at the end of the round — Linux a complete product, macOS everything that
can be prepared without a Mac, a checklist for a session with a borrowed Mac, one tag
publishing three installers, and the owner's frictions. The owner asked to spend few tokens
and agents: **three builders, one reviewer per branch and one on the integrated tree**
(not six per review as in round 29).

## How

- Contract `8153666` on `r30/integration`: `docs/round30/BUILDING.md` (who owns which
  files, the packaging scripts' interface, ADR numbers). No integrator maps; each builder
  read what its brief needed.
- Builders in `F:\projects\daw-r30\{linux,macos,ship}`, all at once (jobs = 3). Each kept
  through its fix rounds; the round's frictions (given at the start through master's
  `docs/next-session.md` item 5) went to ship once its first delivery merged.
- Reviews (one agent each, a refuter's discipline inside): ship (2 findings: an inherited
  `$APPIMAGE` could make Cantus overwrite another app's AppImage; old downloads never
  deleted), macos (4: editors destroyed on deactivate/⌘H, entitlements for JIT plug-ins,
  `__LINKEDIT` spans overlapping in the shared cache, the Mac metronome key), linux (8:
  PipeWire default with no sink on stock 22.04, autosave/recordings/caches still in the old
  folder, VST3 run-loop handlers kept past close and unload, a non-resumable migration,
  stale `DISPLAY`, same-batch unregistration, the menu entry rewritten, JACK probed once),
  the integrated tree (6, all in the frictions: the play count-in's position on Stop,
  pattern mode, loops, a CLAP+VST3 pair counted twice, count-in state leaking into the next
  project, a scan result overwriting a *Try again*). **20 confirmed, all fixed with tests.**
  The integrated review found the seams (window dispatch, scan folders, data paths,
  workflows and scripts, stable names, menus) consistent.
- Master merged in four times: the kit windows' scroll fix, `v0.1.0-beta.2` and `beta.3`
  (the other session's Windows releases: the update check's TLS fix), CI margins, support
  links. This round's release is therefore **0.1.0-beta.4**.
- One flaky test fixed here: `session_faults5::an_instance_removed_during_a_dialog_is_ended_after_it`
  assumed the neighbour's dialog was open 300 ms after asking for it; under load the
  editor's timer came later (1 failure in 3 runs alone). The test synth now writes a marker
  as the dialog starts and the test waits for it (5 of 5).

## Results

| Where | Result |
|---|---|
| Windows (`56e7a00`) | fmt and clippy `-D warnings` clean; **4 469 passed**, 0 failed, 56 ignored; the engine's fuzz at `CANTUS_WORKERS=3`: 851 |
| Linux, WSL Ubuntu 24.04 with CI's flags | clippy clean (also `cantus-plugins` without the editor feature); **4 451 passed** — `bridge::a_plugin_that_hangs_never_holds_the_audio_thread_past_the_deadline` failed once while the Windows suite loaded the machine and passed 8 of 8 alone |
| Apple targets from Windows | `tools/macos/xcheck.sh`: clippy `-D warnings` clean for the workspace on `aarch64-apple-darwin` and `x86_64-apple-darwin` |
| macOS runner | runs after publishing (`macos.yml` on every push; the null hashes' macOS leg) — the tag waits for it |
| Measured (linux) | PipeWire 1.0.5 duplex at 48 kHz: 0 xruns at 128, 64 and 32 frames; jackd2 at 128: 0 xruns, in 133 / out 252 frames; the AppImage 60.5 MB, starting without PipeWire (ALSA offered) |

## Decisions

ADR 0065 (the Linux platform layer), 0066 (macOS), 0067 (releases for three systems and the
updater per system). No schema bump (schema 41); keymap 10 (⌘/Ctrl+, *Settings*, the macOS
preset).

## What only the owner can do

- **The Mac session** (`docs/mac-session.md`, in Spanish): Gatekeeper, the microphone
  prompt, 10 minutes of duplex, a real plug-in's editor in and out of process, menus and
  keys, trackpad, Retina, CPU on Apple silicon, an update from an older build, opening
  from Finder.
- **The Apple Developer account** and the five `APPLE_*` secrets in `cantus-daw` (steps in
  `docs/mac-session.md`); until then the disk image is signed ad hoc, not notarised.
- **An hour on a real Linux desktop**: an interface in duplex on PipeWire, a real plug-in
  editor (Surge XT, Vital, a JUCE VST3) on X11 and on Wayland, rtkit granting real time.
- **Flathub**: submitting the manifest in `packaging/linux/flatpak/`.
- The machine: C: fell to 2.8 GB during the round (the WSL disk image grows there); compact
  it as an administrator (`docs/STATUS.md`, round 26's note).
