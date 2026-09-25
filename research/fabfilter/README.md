# FabFilter research (September 2026)

Six agents studied every product in FabFilter's Pro bundle, and the family's shared
interface, from FabFilter's product pages, its online manuals and published reviews. The
goal was to understand the benchmark well enough to build Cantus's own plug-ins **better**:
matching or exceeding the features, and looking and working better. Nothing here is copied
into the product; the reports are paraphrase and analysis with their sources.

The screenshots the agents saved (about 85 official images) are in `screenshots/` and are
**git-ignored**: they are FabFilter's images, kept locally for reference and never
published with the source.

| Report | Product | Latest version found | The headline |
|---|---|---|---|
| [pro-q.md](pro-q.md) | Pro-Q 4 (EQ) | 4.13, 30 Jun 2026 | It wins on interaction, not DSP; spectral dynamics costs ≥70 ms of latency; legibility is open ground |
| [pro-c.md](pro-c.md) | Pro-C 3 (compressor) | 3.02, released 15 Jan 2026 | You can *see* the compression but not touch it; styles are black boxes |
| [pro-l.md](pro-l.md) | Pro-L 2 (limiter) | 2.26, 16 Apr 2026 | The history cannot be scrolled or tied to song time; latency is never shown |
| [pro-r.md](pro-r.md) | Pro-R 2 (reverb) | 2.06, 16 Apr 2026 | Great top-level controls, no depth behind them; decay shown as relative %, not seconds |
| [pro-mb.md](pro-mb.md), [pro-ds.md](pro-ds.md), [pro-g.md](pro-g.md) | Pro-MB, Pro-DS, Pro-G | 1.33 / 1.32 / 1.42 | None lets you drag a threshold; one dynamics engine with task-specific faces beats four products |
| [family.md](family.md) | The shared language, the bundles | Pro bundle: Q4, C3, L2, R2, MB, DS, G | Knob bodies measure 1.1–1.2:1 contrast against their background; seven rules for our family |

What was built from it is in ADR 0017 and STATUS, round 15. Two caveats the agents
reported themselves: several community sites blocked automated reading, so some user
opinion comes from search-result excerpts (marked as such in the reports); and a few
parameter defaults are not published by FabFilter and are marked "to verify in the trial".
