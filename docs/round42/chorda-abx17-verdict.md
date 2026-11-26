# Chorda candidate 17 against the Salamander: the owner's blind verdict (2026-11-26)

The owner took kit `abx5-50f5d1b224` (built with `chorda_abx --shipped` from candidate 17 as the app ships it, DSP_VERSION 16) alone, on headphones, on 2026-11-26 (15 minutes). The key is in `F:\projects\cantus-work\listen\chorda-abx17-key`; his results are in the main tree at `docs/prueba_chorda_last/listening-luis-d1b224.json` (untracked; his). Read with `tools/listening/abx_read.py <results> <key.json>`.

Voices: **chorda3** = candidate 17 (shipped), **chorda2** = candidate 16 (rendered by the voicing lab), **salamander** = the Salamander Grand.

## Results

- **ABX, Chorda 17 against the Salamander: 16/16** (p < 0.0001). A constant cue still gives Chorda away.
- **Both catch questions passed** (top, ballad: "suenan igual", "casi idénticos"), so his preferences are reliable this time.
- **Preferences against the Salamander: the Salamander 5, Chorda 2** — the same count as candidate 16, but the items moved:

| phrase | c16 verdict | c17 verdict | his words (c17) |
|---|---|---|---|
| bass | Salamander | **Chorda** | Chorda "tiene algo en los muy graves (las 2 primeras notas) que aún no me encanta"; otherwise "sonando mejor me iría con" Chorda |
| soft_chords | Chorda | Chorda | Chorda "más medios", the Salamander "más graves"; "ambos son buenos la verdad" |
| middle_c | Salamander | Salamander | Chorda "tiene un medio que delata… pero ambos suenan reales" |
| fast_run | Salamander | Salamander | "son casi iguales, es muy difícil" |
| ff_chord | Chorda | Salamander | — |
| ballad | Salamander | Salamander | — |
| top | Salamander | Salamander | the Salamander's "sonido del martillo y mecánica… no solo es un grave, hay detalles ahí" |

- **Candidate 17 against candidate 16:** the same on 6 of 7 phrases (soft_chords, ballad, bass, middle_c, fast_run, ff_chord). On **top he preferred c16**: c17 has "sonido de los martillos o algún golpe con reverb, no me convence".

## What this says

1. c17's added action noise is heard on the top phrase, and it reads as **a knock with reverb**, not as a mechanism. The Salamander's top has detailed mechanics: several small transients (key bottoming, hammer, damper, felt), dry and short, not one low thump.
2. **Middle C** still carries a mid that "gives it away", though both now "sound real".
3. **The bass's first two notes** (the lowest) still have something he dislikes: likely the open item of A0–G#1 inharmonicity above both grands.
4. Fast runs are now "almost equal, very difficult".
5. ff_chord moved from Chorda to the Salamander; no comment.

## The next Chorda pass aims at

1. The top's action noise: replace the reverberant knock with dry, detailed mechanics measured from the grands' top notes (onset structure, spectra and decay of each transient); check that the knock does not pass through the body or room ring.
2. Middle C's give-away mid: find the remaining band with the difference analysis on the middle_c phrase.
3. The lowest bass notes: inharmonicity and the first partials' attack against both grands.
4. ff_chord: what moved.
5. Still the constant cue (16/16): the sound lab on all seven phrases.

Then a new blind kit, the owner alone.
