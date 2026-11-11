# Round 39 — `des-mixer`: the mixer and its inspector in Slate

Design only (no Rust, no cargo). Everything is in `docs/round39/design/des-mixer/`: the sheets,
the measured spec for the kit builder (`README.md`) and the generator (`src/`,
`render_all.sh` rebuilds every PNG with Edge headless). The comparisons with FL Studio and the
other DAWs are private, in `research/ui/round39/des-mixer/`.

- **Delivered:** the whole window with the mixer maximised in Compact, Wide and Large, Night and
  Paper, 100 % and 150 %, playing and stopped; master, track and return strips in the three
  densities and at three heights (what leaves when the desk is short); every state; the
  inspector with every section open and folded, an insert row's states and the empty sections;
  the overflow cases (24 strips scrolled, long names, Spanish with a solo on); redlines; ×4 cuts.
- **How it was checked:** every PNG read at 1× and the details at ×4; fixed on the way: boxes
  around every insert list and EQ (now quiet panels), the polarity row and the Compact arm as
  wells (now glyphs alone at rest), send names cut, add marks off the pixel grid, the inspector's
  channel switches cut in Spanish, a compressor's mini view shown under an EQ, gain reduction
  shown while stopped.
- **Verified only on paper:** nothing here runs; widths, rows and states are for the `kit`
  builder to turn into code and the owner to approve.
- **Found for others:** the top bar (dir-a) collides in Spanish.
- **Open for the owner:** the five questions at the end of the README.
