# ADR 0154 — Fides's input is a sidechain

**Status:** Accepted (round 41; the integrator, with the owner's mandate, 2026-11-22) ·
**Date:** 2026-11-22 · **Builds on:** ADR 0150 (Fides), ADR 0014/0015/0017 (an insert's key from
another strip), ADR 0018 (delay compensation), ADR 0031 (the engine as a graph), ADR 0064
(freezing) · **Amends:** ADR 0150 (*input — a track's audio*) · **Schema:** 79

## Context

Fides's *Input* exciter (ADR 0150) plays a live signal through the resonators: the DSP
(`Fides::process_input`) takes a mono block the length of the call, and nothing else of it
depends on where the block comes from. ADR 0150 said only "*input* — a track's audio"; which
audio, with what latency, and what an offline render or a freeze hears were left open, and
until they were decided the exciter was offered and silent. `docs/round41/fides.md` (*Fides in
ALL*) set out three readings:

- **A — its own track's device input** (the interface channels `Track::input` names): the
  "live" in *live input*, but no device exists offline, so a render or a freeze differs from
  what is played unless a separate render rule is invented.
- **B — its own track's audio clips**, taken before they reach the strip: the cheapest, the
  least expected (the clips stop sounding on their own).
- **C — a sidechain from a strip**, chosen as an insert's key is (ADR 0014): any strip's
  post-fader output excites it — a drum bus ringing a marimba, a voice driving a string.

## Decision

**C.** A track's instrument may hear another strip as its input, exactly as an insert's
detector hears its key:

- **The model.** `Track::sidechain: Option<StripId>` (schema 79; absent — every older track —
  is none): the strip whose post-fader output the instrument hears. Only an instrument that
  takes one holds it (`InstrumentKind::takes_sidechain`: Fides); `Track::key()` is the stored
  key when the instrument takes it. It is a track's routing, as `Track::strip` is, not a
  parameter of the instrument: a preset or a Fides for a Fides keeps it.
- **The command.** `SetTrackSidechain { track, source }` (`cmd.set_track_sidechain`, its
  inverse the old source), refused as `SetEffectSidechain` refuses: an instrument that takes
  none, a VCA, the master, a strip that is not there, and a key that would close a loop — a
  strip the track plays into, or one that reaches it. The key is an edge of the processing
  order (`Project::downstream`: the key's strip before every strip the keyed track plays
  into), so every later route is checked against it by the walk that already checks outputs,
  sends and inserts' keys (`routing_would_cycle`); `SetTrackStrip` refuses to send a keyed
  track into its key or into a strip that reaches it; `RemoveStrip` refuses a strip a track
  is keyed from; `InsertTrack` checks a track it brings as the command would.
  `SetInstrument` to an instrument that takes none lets the key go in the same step (its
  inverse a `Batch` that puts both back), so no key ever sits where nothing hears it.
- **The engine.** The plan (`GraphPlan::compile_into`) runs a keyed track's node right after
  its key's strip — the nodes are the tracks, then each strip followed by the tracks keyed
  from it, then the master — with a dependency edge from that strip, so the serial engine and
  the pool both run the strip first. The key's strip keeps what leaves its fader (`is_key`,
  as for an insert's key), and the track's segment reads that pass's buffer into the slot's
  mono scratch and calls `InstrumentUnit::process_input` (default: `process_par`; Fides's
  unit hands it to `Fides::process_input` through its control-step loop). Nothing is
  allocated or locked; a key whose strip made nothing this pass is silence. A key the plan
  cannot honour — its strip not processed, resting, or not before the strip the track plays
  into (a loop a hand-edited file could hold) — is heard as silence, as a later key is.
- **Latency.** Unlike an insert's key (ADR 0018: *keys are not compensated*, a detector only
  shifts), what the instrument hears **is** its sound: a keyed track leaves at its key strip's
  output time plus its own latency (`compensate`: `leave = post[key] + latency`), so every
  other path waits for it and Fides meets the mix where the source does.
- **Offline = real time**, and a freeze holds its source: the key's strip and what it is
  made of join a frozen track's region (`freeze::region`, `region_through`), its key is
  woken when the track runs (`dormant`), and the strip a keyed track plays into counts the
  key as an input (`pre_only_senders`, `reaches_strip`).
- **The UI.** Fides's *Input* card shows the source as the house's keyed editors do — a
  caption (*Input from*), the kit's field and its list of the strips the command would take
  (`Project::can_key_track`), *No strip* first — and, with none chosen, says so plainly
  (*Choose a strip: silent until then*).

Why C: it is the one useful in a mix (any track drives it, not only a microphone); a render
and a freeze are what playing gives, with no rule of their own; and it is the house's one
mechanism for "hear another strip" — the same field, the same refusals, the same ordering
edge as an insert's key, so nothing new is learned and nothing is routed twice.

## Consequences

- An unkeyed project's plan is the one it had (no keyed track, the same nodes in the same
  order), and an unkeyed track's render is the one it had: no golden hash moves.
- A song with a keyed track does not open in beta.16 or older (schema 79; an older build would
  drop the field and silence the exciter).
- **A** (a microphone straight into Fides) is reached through C: the microphone's track
  monitors into its strip, and Fides is keyed from it — live, with the input's latency, and in
  a render with the take recorded on that track.
- The AI tools and the MCP server do not yet set an instrument's key (`docs/round41/fides.md`).

## What the tests hold

- `cantus-core` `tests/fides_sidechain.rs` (5): every refusal (an instrument that takes none,
  its own strip, the master, a strip that is not there, a strip its strip feeds by output and
  by send), the order with the key first, every later route that would reach it back refused
  (the track into its key or into a strip feeding it, its strip's output and send into the
  key, an insert on the key keyed from its strip), the key's strip in use, `SetInstrument`
  letting it go and undo bringing both back, `InsertTrack` checked, the freeze region, the
  snapshot, and the JSON round trip; `undo_invariants::fides_sidechain_undo_and_redo_among_
  the_others` (40 seeds of 80 commands, half the sidechain's own arms: no key on an
  instrument that takes none, the key's strip always ordered first, undo all and redo all).
- `cantus-project` `schema_79_keeps_fides_s_sidechain` (the fixture `v79.cantus`).
- `cantus-engine` `tests/fides_input.rs` (6): unkeyed, the Input exciter is silent to the bit
  and a keyed pluck is the unkeyed one's bits; a key whose strip is silent excites nothing; a
  strip's audio excites Fides (−28.8 dBFS RMS in the test); the same at blocks of 1, 37, 64,
  512, 1000 and 4096 and live in irregular blocks; on the pool at W = 1, 3, 7 the serial bits,
  no conflict; a key that would loop (its own strip, a strip its strip feeds) heard as silence
  offline and live on the pool; a 186-sample latent key strip moves Fides's onset by exactly
  186 samples, the mix's latency.
- `cantus-ui` `tests/fides_input_e2e.rs`: through `CantusApp::assemble`, the picker and the
  pointer in Fides's window, the source's audio excites Fides through the real engine
  wiring; one undo step; a save and a load keep it.
