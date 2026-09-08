# ADR 0008 — MVP scope (Phase 0/1)

**Status:** Accepted · **Date:** 2026-09-07

A working musician must be able to: open Cantus, draw notes in a piano roll on a track that
plays a built-in polyphonic synth, hear it glitch-free, set the tempo, save/load the project,
undo anything, and export a WAV that is bit-identical to what they heard. Nothing else.

| In | Out (later phases) |
|---|---|
| Audio callback (cpal: WASAPI / CoreAudio / ALSA) | ASIO, JACK, PipeWire native |
| Transport with loop, tempo, 4/4, metronome | tempo track, time-signature changes |
| MIDI clips + piano roll (draw, move, resize, velocity, delete, quantize) | step sequencer, tracker |
| Subtractive poly synth "Firmus" (2 osc, SVF, ADSR×2) | FM, wavetable, sampler |
| Per-track gain/pan/mute/solo, master meter | sends, buses, automation |
| Computer keyboard as MIDI input | hardware MIDI in (midir), recording |
| Save/load versioned JSON, autosave, undo/redo | consolidation, collaboration |
| Offline WAV render (bit-identical to realtime) | MP3/FLAC, stems |
| `AiProvider` trait + command bridge, no live provider | cloud/local providers, MCP |
| EN/ES strings, keyboard-reachable transport | full screen-reader pass |
