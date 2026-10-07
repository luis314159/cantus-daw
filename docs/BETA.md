# The road to the first public beta (0.1.0-beta.1)

*Written 2026-10-06 at the owner's request ("¿qué nos falta para hacer nuestro primer release
de beta oficial?"). The release machinery exists (round 20: installer, auto-update, a
tag-driven release workflow); what is missing is that Cantus is **dependable on someone
else's machine**. Each item says how we will know it is done.*

## Resumen (español)

Bloqueantes: (1) audio en hardware real — un solo build con todos los drivers y ASIO dúplex
probado en ≥ 3 interfaces, empezando por la Behringer UMC; (2) rendimiento mínimo — la
canción de inicio y las demos sin cortes al buffer por defecto en una laptop normal (ronda
21 + la ola del motor multinúcleo); (3) estabilidad — sesiones largas sin cuelgues,
autoguardado y recuperación probados con fallos reales; (4) el uso real del dueño — 2–3
canciones completas y sus fricciones resueltas; (5) firma de código; (6) legal — búsqueda de
marca "Cantus", avisos de terceros, RVR1960, privacidad. Muy recomendables: exportar MP3,
nivelar Lamina y Exemplar, un manual de inicio rápido EN/ES, plantillas de issues, versión y
notas. No hace falta para la beta: warp, MIDI learn, IA, macOS/Linux, aislar plug-ins.

## Blockers

1. **Audio on real hardware** — one distributed build with WASAPI and ASIO (round 21,
   `asio`); ASIO duplex (input + output) running for 10 minutes without a dropout on at least
   three interfaces (the owner's Behringer UMC, ASIO4ALL, FlexASIO; ideally a Focusrite);
   hot-plug and a lost device recover; the first-run setup records a guitar in under a minute.
   *Done when:* a compatibility table with ≥ 3 interfaces passes in `docs/hardware.md`.
2. **Performance floor** — the startup song and every demo play with zero dropouts at the
   default buffer (and at 256 frames) on a mid-range laptop; no silent 64-track cap (round
   21 graph); the multicore live pool (round 22) lands. *Done when:* `xtask perf` shows the
   demos under 70 % of the budget at 256 frames and a 10-minute live run has 0 overruns.
3. **Stability** — 2-hour sessions without a crash or a hang; autosave and recovery proved
   by killing the process mid-edit and mid-recording (a recording's WAV survives); a corrupt
   or newer project refuses cleanly; a crashing hosted plug-in is documented as a known
   limitation until sandboxing (ADR 0031 Phase 6). *Done when:* a soak-test checklist passes
   and the recovery tests exist.
4. **Real use by the owner** — 2–3 complete songs (record, program, arrange, mix, master,
   export) with every friction logged and the worst fixed. *Done when:* the friction log has
   no open blocker.
5. **Code signing** — choose SignPath Foundation (free for open source), Azure Trusted
   Signing or Certum (`docs/round20/signing.md`); set the secrets and `CANTUS_PUBLISHER`
   once. *Done when:* the installer and `cantus.exe` show a trusted publisher.
6. **Legal** — a trademark search for "Cantus" (ADR 0001 has it pending); a
   `THIRD-PARTY-NOTICES` file (libm, the patched cpal/asio-sys, the Steinberg ASIO SDK and
   its trademark notice, egui and other crates, CC0 packs); the RVR1960 written
   confirmation or RV1909 as the default; a privacy statement (no telemetry; what the
   update check sends). *Done when:* all four are in the repo and linked from About.

## Strongly recommended

- **MP3 export** (and FLAC/Ogg) — the first thing users ask for.
- **Instrument gain staging** — Lamina and Exemplar ~8 dB quiet (with a migration).
- **A quick-start manual** (EN/ES): install, audio setup, record, edit, mix, master, export;
  the shortcut sheet; known issues.
- **Feedback** — GitHub issue templates (bug, hardware report, feature), a link from Help.
- **Versioning** — `0.1.0-beta.1` in `Cargo.toml`, release notes in
  `docs/releases/v0.1.0-beta.1.md`, then the tag (the workflow refuses a mismatch).

## Not needed for the beta (known limitations)

Warping/time-stretch, MIDI learn and control surfaces, AI features, macOS and Linux builds,
hosted plug-in crash isolation, multiple arrangements.

## Path

1. Finish round 21 (performance wave 1 + ASIO) and round 22 (the multicore live pool).
2. A **beta-hardening round**: hardware matrix and audio robustness, soak and recovery
   tests, MP3/FLAC export, gain staging, third-party notices and privacy, the quick-start
   manual, issue templates, version and notes. In parallel the owner tests, chooses the
   signing route and orders the trademark search.
3. Tag `v0.1.0-beta.1`.
