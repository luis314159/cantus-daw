# Round 39 — `des-controls`: every control family in Slate

**What was made.** Build sheets for every control family, in Slate Night and Slate Paper, at
100 % and 150 %. Each size and scale is drawn for itself, and every 100 % sheet is cut into
×4 nearest-neighbour crops. The full measured spec for the kit builder is in
`docs/round39/design/des-controls/README.md`.

| sheet | covers |
|---|---|
| knobs | three sizes; unipolar, bipolar, stepped; rest, hover, dragged, focused, disabled; automated, automated and held, modulated, MIDI learned |
| faders | the strip fader and its meter: silent, playing, peak hold, clip, hover, dragged, automated, VCA, frozen; the master; sliders; value bars |
| fields | numeric fields (scrubbing, typing, invalid); text, search and position fields; drop-downs |
| menus | menus with submenus, checks, radios and shortcuts; context menus; an overflowing menu |
| switches | tabs, segmented controls, switches, check boxes, radios, M/S/R keys, the enable lamp, scrollbars |
| tips | tooltips and the help line |
| dialogs | buttons (primary, secondary, destructive, quiet × six states) and the dialog frame |

The private comparisons with FL Studio 24.1.2 and Cantus beta.13 are in
`research/ui/round39/des-controls/`.

**What was decided.**

- **The driven signs.** Automation, a modulator, a MIDI controller and a VCA each have
  their own shape and colour, used the same way on knobs, faders, value bars, fields,
  tooltips and the help line.
- **The automated knob.** It turns violet in its arc, its pointer and its value, and
  carries the lane glyph in the groove's open quarter. Its hint names the clip and how to
  clear it.
- **New tokens.** `mod`, `fcap_*`, `danger`, `primary_ink` and `scrim`, each checked for
  contrast.
- **The fader cap** was made lighter and taller after the side-by-side with FL.
- **The meter** shows a single solid peak bar. Overlaying RMS read muddy at 100 %.

**How it was checked.** Every PNG was rendered with Edge headless and read. The defects
found were redone:

- overlaps in the menu and field sheets;
- the ragged spacing in tooltips (they are now laid out by the browser);
- the sign covering the knob's cap;
- the muddy peak overlay;
- the zoom bar's invisible grips in Paper;
- the destructive button's contrast at 4.0 : 1, now 4.8 : 1.

**Not verified / left open.**

- How the app behaves when a hand grabs an automated knob is `ux-a`'s finding; the
  *Automated, held* state may need a change once it is known.
- The glyphs inside keys are the `icons` agent's.
- The strip keys and the lamp are proposals for `des-mixer`.
- The owner's eyes decide. The README lists the five things to judge.
