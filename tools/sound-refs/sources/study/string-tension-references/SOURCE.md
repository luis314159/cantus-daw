# Cello string tension references (for range checks)

- **URLs:**
  - Aitchison & Mnatzaganian, "Selected cello string tensions chart 1922-2022":
    https://www.aitchisoncellos.com/string-tensions-charts/ (page saved in `raw/`; the page says
    it was last modified 2024-08-01).
  - D'Addario, "Orchestral strings comparison chart" (PDF, © 2016):
    https://integration.daddario.com/globalassets/pdfs/orchestral/orchestral-strings-comparison-chart.pdf
- **Access date:** 2026-11-23

## Terms

No licence is stated on either source; both are © their publishers.

The chart page says, verbatim:

> Please note that most manufacturers publish string tensions for A-440Hz tuning and a vibrating string length of 700mm. However, Pirastro's published. data is based on a 690mm string length. Professor Jim Woodhouse of Cambridge University's Engineering Department confirmed that 3% is the correction to apply to make Pirastro's data comparable to other manufacturers, so we offer the adjusted figure first followed by Pirastro's published data in brackets

## Files

- **`aitchison_cello_tensions_extracted.csv`:** made here from the page's HTML table, 109
  string rows. Columns:
  - `set_and_year`
  - `string` (A/D/G/C)
  - `medium_tension_lb_at_700mm`, with Pirastro's published 690 mm value in brackets where given
  - `windings`
  - `core`
  - `tension_N_approx`, which was added here as lb × 4.44822
- **`raw/aitchison_cello_string_tensions.html`:** the source page.
- **`raw/daddario_orchestral-strings-comparison-chart.pdf`:** core types (solid steel, stranded
  steel, synthetic, gut) and the D'Addario cello and bass lines. **It gives no tension figures.**

## Not obtained

D'Addario's Helicore cello and bass product pages have a "Tension Chart" tab, but it is filled
in by script, so no figures were in the HTML. No double-bass tension table was found in a form
that could be downloaded.

## Verdict

**Reference facts only, for range checks.** Do not redistribute the pages. Using the figures as
range checks is fine.
