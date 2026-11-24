# Round 42 — fix4-narrow: every instrument whole at every scale and the narrowest window

Branch `r42/fix4-narrow`, from `r42/integration` 21e7bce78. Not merged.

## The gap

`instruments_scale_e2e` held Chorda and Tympanum to "every word whole at 100/150/200 % and
at the narrowest window, in English and Spanish". Nothing held the other instruments, and
they were cut: Exemplar's Edit view at the narrowest window at 150 %, in Spanish, cut cells
and card titles ("Liberació…", "Transposic…", "Triá…", "R…", "Mapa tecla × veloc…").

The real hosted editor shows the same cuts (`inst::window` lets the window shrink to three
quarters of the shell at every scale — `min_size(INSTRUMENT * 0.75 * k)`); the first run of
the new test, through `CantusApp::assemble` and the editor's own host, found **1 375
problems** over ten instruments.

## What the test covers now

- `crates/cantus-ui/tests/inst_scale/mod.rs` — the host and the checks, shared by both
  binaries (moved out of `instruments_scale_e2e`, which now uses it).
- `crates/cantus-ui/tests/instruments_narrow_e2e.rs` — its own binary (its editors paint on
  its thread): a track for every native kind in `InstrumentKind::ALL` but Chorda and
  Tympanum (which `instruments_scale_e2e` keeps), plus Arcus (`BowedString`, not in `ALL`
  yet), each in its demo's sound (`inst::demo_setup`, the content the site's pictures show).
  Every view the header offers — Play, Edit, Matrix; the dancer's Play and Edit.
- `instruments_scale_e2e` now reads Chorda's Matrix view too, and the narrowest checks below.

For each view, in English and Spanish:

1. at its whole size × 100, 150 and 200 % with the clipped probe: no word elided, dropped or
   painted past its card; no two overlapping; none under a button; every word read at 100 %
   read at 150 and 200 % (a designed `<key>.short` counts as its word — the kit's own
   abbreviation, not a cut; a word's box clipped by under a point is whole);
2. at the shell's own size;
3. **new** — at the narrowest window the editor allows **at each scale** (it was 100 % only);
4. **new** — at that narrowest width and the view's whole height, the clipped probe on where
   the view does not scroll sideways: what the narrowest window shows as it scrolls down. The
   kit paints no word out of sight, so the narrowest window alone never saw Exemplar's lower
   cards — that is why the brief's cuts were missed even by a narrowest check.

An engraved plate's name (one word in three layers a point apart) is one word, not three
overlapping. `CANTUS_INST_ONLY=vetus,bass` reads only those kinds; `CANTUS_PATHS_SHOTS=<dir>`
writes every hosting as a PNG.

Run time: `instruments_narrow_e2e` 6.5 s, `instruments_scale_e2e` 26 s (its three tests).

## What it found, and the fixes

| Where | Cut | Fix |
|---|---|---|
| Every Play view but Exemplar's, Chorda's and Tympanum's (Calor, Bassus, Fides, Organum, Unda, Modulus, the e-piano, Arcus, the dancer) | At the narrowest window the cards were crushed: Calor's cards crossed each other (109 overlaps), Organum's and Fides's lower cards squeezed to their titles, Fides's cells cut ("CONTA…", "VEL TO…") | **A view keeps the room it was laid out for and scrolls below it** (`frame::least_view`, `keeps_designed`): the room the view has at the shell's own size with the same bands under it (`designed_view`). Below it the Play view sits in a scroll area that scrolls the way it lacks room, never squeezing. At the shell's size and larger, nothing changes. |
| Every Edit view but Exemplar's (and Chorda's, Tympanum's) | At the narrowest width cards squeezed below their cells: Unda's routing card ("ROUT…"), Calor's wheel cells, Modulus's operator table ("Hz" dropped 16 times), Organum's preamp card, Fides's cells | The Edit view, which already kept `BODY_MIN` of height and scrolled down, now also keeps its designed width and scrolls sideways below it (`at_least` takes the width). |
| Exemplar's Edit view | The brief's cuts: rows of cards squeezed (Amp envelope, Output's third cell under the edge, Voice's "Transposic…"), LFO 2's selector "Triá…", the map's title under its Fit button, the zone names cut in a map a fifth of the row, "Bucle mientras se mantiene" → "Mante…" | **Exemplar reflows** instead of scrolling: each row of cards flows over as many lines as it needs at each card's need (`sampler_cards::lines`, `section_row`), the need counting its title and its selector's longest name (new `kit::band::title_need`); the map goes above the zone card when beside it it would be under 400 points, the sample card above the seam card under 600. |
| Exemplar's Play view | Its four cards squeezed at the narrowest width ("GANANCIA" past the edge) | The same flowing row (two lines there); the display above gives way, and the view keeps its designed height (scrolls down) so the cards are never crushed. |
| The macro band, every instrument | "BRIGHTNESS" → "BRIGHTN…" at the narrowest window while half the band stood empty | Columns sized by each name's need (`frame::macro_columns`): a quarter each when they fit, more for a long name, shared out when the band is short. |
| Organum's rotary speaker (Play and Edit) | The left mic's "L" sat on "Horn 420 rpm → 420" at every scale | The mic's letter goes above it when under it would meet the readouts. |
| The dancer's character card | Its line cut to two rows with a hand-made "…" ("…y una estrella en el…") at every scale | The line takes the rows it needs; the tiles above give way (they already scroll). |
| The dancer's step pads (Spanish, 150/200 %) | "Paso a la / izquierda" pressed into each other and into the note above | The note's line as tall as its letters; the pads' card as tall as three lines at the scale. |
| Arcus (Spanish, every scale) | "RESONANCIA" → "RESONAN…", "CORTE BAJO/ALTO" → "CORTE BA…/AL…" | Clear Spanish words that fit whole: `arcus.ring` = "Cola" (the string's tail, in seconds — beside the body's "Resonar"), `arcus.lowcut`/`highcut` = "Paso alto"/"Paso bajo" (the standard Spanish filter names), and their `.short`s the same. |

No text was made smaller than the kit's sizes; no knob size changed.

What the test now leaves alone by design: a view that keeps its designed width scrolls
sideways at the narrowest window, so the narrowest-tall pass reads it with the plain probe
(elided or dropped words, overlaps) rather than the clipped one — exactly as Chorda's and
Tympanum's mixers already were.

### A choice to note for the owner

Exemplar reflows; the other nine scroll sideways below the width they were laid out for. A
reflow per instrument (each has its own hand-laid rows) is the better end and is open as a
follow-up; the scroll is what guarantees no card is ever crushed or cut today. At the shell's
own size and any larger size nothing scrolls and every layout is as before.

## Pictures (before → after)

In `docs/round42/fix4-narrow/` (`before-*.png`, `after-*.png`):

- `exemplar-Edit-narrowest-150-es` — the narrowest window at 150 % (map above the zone card,
  title and Fit whole, zone names whole);
- `exemplar-Edit-narrowest-tall-150-es` — the same width scrolled whole (cards over more rows,
  sample card above the seam card, LFO 2's "Triángulo" whole);
- `exemplar-Play-narrowest-tall-150-es`;
- `vetus-Play-narrowest-100-en`, `bass-Play-narrowest-100-en`, `fides-Play-narrowest-100-en`,
  `organum-Play-narrowest-100-en` — crushed before, scrolling after;
- `unda-Edit-narrowest-tall-100-en` — its row of five squeezed before;
- `organum-Edit-100-en` — the mic's "L" off the readout;
- `dancer-Play-200-es` — the character's line whole, the pads' two lines apart;
- `arcus-Edit-100-es` — "COLA", "PASO ALTO", "PASO BAJO".

Every hosting of every kind (580 PNGs) can be rendered with
`CANTUS_PATHS_SHOTS=<dir> cargo test -p cantus-ui --test instruments_narrow_e2e`.

## Gates

Run on this branch (`CARGO_TARGET_DIR=C:/cantus-r42-target/fix2-model`, debug info off,
through `tools/build/slot.sh`), all green:

- `instruments_narrow_e2e` (1 test, 6.6 s) and `instruments_scale_e2e` (3 tests, 26.7 s);
- `ux_whole_words` (1, 1 ignored as before), `font_coverage` (4), `learn_everywhere` (12);
- every `inst_*` binary: analog 9, bass 9, chorda 11, chorda_boot 1, epiano 10, faces 2,
  fm 11, frame 4, mixer_delay_e2e 2, organ 11, pulsus 9, sampler 16, sampler_footer 1,
  shell 3, views_scroll 1, wavetable 13;
- the `cantus-ui` lib tests: 620 passed, 2 ignored (the i18n key check among them);
- `cargo clippy -p cantus-ui --all-targets -- -D warnings` — clean;
- `rustfmt --edition 2021` on each changed file.

Not run: the Linux check in WSL (the integrator's), the whole workspace suite.

Verified only in the editor's own host (the test's and the site's): not looked at in a live
window on a monitor at 150 %/200 % — worth a glance on the owner's machine (`docs/beta-checklist.md`
is not changed: nothing here needs hardware).
