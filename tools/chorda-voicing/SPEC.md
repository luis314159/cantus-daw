# A Chorda candidate: its three files

A candidate voicing of Chorda's model 2 is three small text files — the same three the voicing
lab (`chorda_compare lab`), the listening kit (`chorda_abx --candidate/--partials/--levels`) and
the shipped module (`ship_rust.py` → `crates/cantus-dsp/src/chorda_voicing_c16.rs`) read.
Candidate 16, what ships since `DSP_VERSION` 7, is the worked example: `candidates/c16/`.

## 1. The spec — `spec.txt`

One line, `key=value,key=value,…` (a later line is ignored: the search writes its score there).

- Applied **in order over beta.14's voicing** (`Voicing::beta14`, what model 2 shipped until
  `DSP_VERSION` 6). An empty spec plays beta.14. A key given twice keeps the later value —
  candidate 16 is candidate 15's spec with its changes appended (`draw.35=3` early, `draw.35=1`
  at the end), which keeps a candidate's history readable in its own line.
- A value is a number (`f64`; per-key tables take `f32`, draws a whole number 0–255).
- An unknown key **stops the render** (the lab asserts `no voicing key …`; the renders of a
  probe then come out missing, which is how a typo shows). `Voicing::apply` (the shipped path)
  reports false instead.
- Every key and what it does, with beta.14's value and candidate 16's: **`KNOBS.md`**, generated
  from the Rust source by `python -I knobs.py > KNOBS.md` — regenerate it when a knob is added.
- Besides the voicing's keys the lab takes the instrument's own settings, as a user sets them
  (they are not part of a voicing and the shipped module ignores them): `room`, `noise`,
  `width`, `body`, `tilt`, `decay`, `hardness`, `model`, `persp`, `lid`, `roomsize`, `gain`,
  `sym`, `detune`.

### The knobs round 40 added (all neutral at beta.14's value: the default renders bit-identical)

| key | candidate | what it does (KNOBS.md has the source's full comment) | c16 |
|---|---|---|---|
| `detune_top`, `.from` | 7 | the unison's detune at C8 against the key's own, log from `.from` (84) — the top unisons tight, so their fundamentals sing Weinreich's slow second stage | 0.15 |
| `vel_top.from/.range/.curve` | 7 | the treble's velocity curve: speed range and exponent at C8 — C7–C8 mf→ff 18 dB → 14.6 (reference 13.3) | 90 / – / 0.7 |
| `itd_ends` | 7 | extra arrival-time difference at the keyboard's ends (`itd_ends · x³` ms on `itd_ms`'s curve) | – |
| `uni_weight_bass` | 7 | extra hammer-weight scatter at A0, none by C4 — the bass unisons' beat depth | 0.4 |
| `detune_bass` | 7 | the unison's detune at A0 against C4's | 0.5 |
| `hf_top`, `.from` | 7 | the loss's rise above the first partial at C8 — the top's 4–8 kHz second partials die faster while their fundamentals live | 1.5 |
| `damper_key`, `damper_key_bass` | 10 | the damper's time scale when a key comes up with the pedal up (single notes end slower than pedalled phrases) and its scale at A0 | 3 / 0.4 |
| `strike_true` | 11 | the strike point against the loop's whole period, not the delay lines alone (0 = beta.14's misplaced comb: middle C notched at partials 12–13, not 8 — the "nasal" middle) | 1 |
| `vmin_bass` | 11 | the slowest hammer's scale at A0 against C4's (soft strikes faster in the middle only) | 0.593 |
| `detune_low`, `.to` | 12 | the unison's detune below the treble scaled, faded to none at `.to` (84) — C4/A3 beat depth with the strike placed true | 0.81 |
| `draw.<key>` | 13 | a key's unison hashed as another, fixed draw (0 = its own): re-voices one key whose unison landed badly (D#2's flanger) without touching another | 12 keys |
| `felt_soft`, `.from`, `.to` | 13 | the felt's stiffness for strikes ≤ velocity 75 in a key range, none by 115 — brighter soft middle, ff untouched | – |
| `hf_key.<key>` | 14 | a key's frequency-dependent loss divisor scaled (< 1: its upper partials die faster) — middle C's partial 9 | C4, C5–B5 |
| `itd_shape` | 6 | the arrival-time curve's shape (0 straight; `k` > 0 a spaced pair's path difference) | 0.5 |
| `uni_rank` | 6 | the unison's shares by rank instead of hashed | 1 |
| `rise.c3…c7`, `top_damper`, `landing.rise`, `pp_trim.*`, `bass_hp(_order)`, `bridge.*`, `uni_b*`, `uni_loss`, `uni_weight_low`, `uni_fade.*`, `felt_upper`, `even*`, `levels_amount`, `keyboard.*`, `coupling_bass/_treble` | 3–6 | the earlier round-40 parts: the strings' rise, the top keys' release, the damper's breath, the soft strikes' level, the bass high-pass, the bridge's resonances, the unison's scatters, the keyboard's evening and inharmonicity | KNOBS.md |

`reg.<bass|mid|treble>.<0..6>` (dB per band at `registers::CENTRES`) and `reg.<…>.level` are
the register EQ every candidate since round 37 carries.

## 2. The partial table — `partials.csv` (`CHORDA_PARTIALS`, `--partials`)

88 lines `key,g2,g3,g4,g5,g6,g7`: MIDI key (21 = A0 … 108 = C8), then the dB added to partials
2…7 against the fundamental, applied at `partials_amount` (candidate 16: 0.6). A missing key is
0 dB; a line that is not seven numbers is skipped. Made by `ana2/fit_partials2.py` (a fit
against the reference's single notes, `-gain × difference`, smoothed 1-2-1, clamped) and edited
with `probes/c16/mkpart.py` (rows lo…hi of a source table, scaled, into chosen columns).
Candidate 16's is candidate 6b's fit with C4–B5 zeroed except partial 2 of C5–B5 (the fast run's
2–2.5 kHz dip against the single notes' upper colour: `docs/round40/chorda.md`).

## 3. The level table — `levels.csv` (`CHORDA_LEVELS`, `--levels`)

88 lines `key,dB`: each key's own level, applied at `levels_amount` (candidate 16: 1). Refit
after any change that moves the keyboard's evenness:
`python -I ana2/keylev.py <cand keys_mf.wav> fit <old levels.csv> <new levels.csv> [gain 0.6] [lo 21] [hi 108]`
(each key's deviation from a quadratic trend through its neighbours, against the reference's).

## 4. The environment

The lab reads the tables from the environment, so **an exported `CHORDA_PARTIALS` /
`CHORDA_LEVELS` applies to every render that follows** — unset them for a candidate without
tables (beta.14, candidate B: `references.sh` does). The probes set them from `PARTIALS` /
`LEVELS` (default candidate 16's), the full run from the caller's.
