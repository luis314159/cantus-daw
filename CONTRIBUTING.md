# Contributing to Cantus

Thank you for wanting to make Cantus better. Bug reports, ideas, translations, sounds and
code are all welcome. Cantus aims to be, measurably, the best DAW there is, so the bar is
high and the house rules below are what keep it there.

## Before you start

- **Read** [`docs/STATUS.md`](docs/STATUS.md) (what exists and what is next),
  [`docs/BACKLOG-excellence.md`](docs/BACKLOG-excellence.md) (what is wanted) and
  [`docs/DEVELOPING.md`](docs/DEVELOPING.md) (setting up a machine, building, testing).
- **For anything large, open an issue first**, so we can agree on the approach before you
  spend the time. Structural decisions are written down as ADRs in [`docs/adr/`](docs/adr/).

## The house rules (short version of [`CLAUDE.md`](CLAUDE.md))

- Every change to a project is a `Command` with an exact inverse, covered by the undo fuzz.
- Tests first. Audio code comes with null tests: a control at its default touches no
  sample, block-size independence, offline render equals real time, parallel equals serial.
- The audio thread never allocates, locks, blocks or does I/O.
- DSP uses `cantus_dsp::math`, never std's transcendentals.
- A change to what a project saves bumps the schema, with a migration and a fixture.
- Interface strings go in English and Spanish; the interface is the hand-drawn kit.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all` before every pull request.

## Licence and the contributor agreement

| Crates | Licence |
|---|---|
| `cantus-core`, `cantus-dsp`, `cantus-engine`, `cantus-project`, `cantus-rt` | MPL-2.0 |
| everything else (the application, the plug-ins, the tools, the website) | GPL-3.0-or-later |

Your contribution is licensed as the crates you change are. Before the first one is
merged we ask you to agree to the [Contributor Licence Agreement](CLA.md) (a checkbox in
the pull request): you keep your copyright, and the project promises that your work will
always stay available under the free licence you gave it under. Why it works this way:
[ADR 0047](docs/adr/0047-licensing-revised.md).

Code copied from elsewhere must say where from and under which licence; GPL code cannot go
into the MPL crates. Sounds and presets must be CC0 or CC BY with their credit
([ADR 0042](docs/adr/0042-attribution-licences-for-content.md)).

## The name

Forks are welcome under their own name and logo; see [`TRADEMARKS.md`](TRADEMARKS.md).

---

*"Whatever you do, work at it with all your heart." (Colossians 3:23)*
