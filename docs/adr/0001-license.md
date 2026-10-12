# ADR 0001 — Licensing model

**Status:** Accepted, **amended by ADR 0047** (2026-10-11: the SDK crates are MPL-2.0,
not MIT OR Apache-2.0; a contributor agreement is required; the name and logos are the
owner's marks) · **Date:** 2026-09-07

## Context
The master spec (idea.md §8) says the license must be decided in Phase 0 because it is
nearly irreversible and interacts with the VST3 SDK (GPLv3 or proprietary agreement).

## Decision
A **split license**:

| Crates | License | Why |
|---|---|---|
| `cantus-core`, `cantus-dsp`, `cantus-engine`, `cantus-project` | **MIT OR Apache-2.0** | These are the "SDK". Plugin developers, researchers and other DAWs can embed the project format, the DSP and the engine without copyleft obligations. Maximizes ecosystem adoption. |
| `cantus-ui`, `cantus-ai`, `cantus` (the application) | **GPL-3.0-or-later** | Protects user freedom for the product itself, and is the only open license compatible with hosting VST3 via Steinberg's GPLv3 grant. |

Contributions to the permissive crates are accepted under MIT OR Apache-2.0; to the
application under GPL-3.0-or-later. A CLA is **not** required (Ardour lesson: friction kills contributors).

## Consequences
- CLAP (MIT) is the primary plugin format; VST3 hosting lives in a future `cantus-plugin-host`
  crate under GPL-3.0-or-later.
- The engine crate must never depend on the UI or AI crates (dependency direction = license direction).
- Bundled samples/presets must be original or CC0 (idea.md §8). *Amended by ADR 0042
  (2026-10-10): CC BY 3.0/4.0 content is accepted too, with its credit always shown.*
- TODO before public release: trademark search on the name "Cantus" in software class 9.
