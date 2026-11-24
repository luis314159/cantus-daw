# tools/arcus — Arcus, the bowed string (round 42, ADR 0157)

The scripts that render, measure and tune Arcus. Each wraps a cargo example or an ignored test
and goes through `tools/build/slot.sh` (three builds at once on a shared machine).

| script | what |
|---|---|
| `listen.sh [out-dir] [levels]` | the owner's renders: the cello and the double bass — a solo note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, pizzicato, tremolo — 48 kHz 24-bit WAV (`crates/cantus-dsp/examples/arcus_listen.rs`); `levels` prints each one's peak and RMS |
| `presets.sh [out-dir] [fit]` | every factory preset on its role's audition phrase and as five held notes in its range, through the real engine, with its LUFS against its role's target; `fit` moves each `gain` there and writes `arcus_presets.rs` (`crates/cantus-ui/examples/fides_listen.rs … arcus`) |
| `match.sh [refs-dir] [out-dir] [fit]` | Arcus against the recorded references (Iowa MIS, TinySOL, VSCO 2 CE, Karoryfer, Smolken): each note measured the same way in both, the distances to the references' spread on the held-out third before and after, a plot per held-out mf note; `fit` fits model 2's match and writes it into `arcus_tables.rs` (`examples/arcus_match/`) |
| `pitch-table.sh` | measures the bowed pitch per key × dynamic × contact point, twice (the offset, then what is left with it applied), and writes `crates/cantus-dsp/src/arcus_pitch_data.rs` (`examples/arcus_pitch_table.rs`) |
| `abx.sh [out-dir]` | the blind ABX's phrase (`abx_phrase.mid`, the same file the recorded strings render; `python -I abx_phrase.py` writes it) rendered by the cello and the bass: `abx-{cello,bass}.wav` at −1 dBFS peak and `abx-*-lufs23.wav` at −23 LUFS (`examples/arcus_abx.rs`) |
| `compare.sh <near-Hz> <wav…>` | a render beside a recording: the fundamental, harmonics 1–10, the noise between harmonics by octave band, the centroid, the onset (`examples/arcus_compare.rs`) |
| `upper-body.sh [admittances-dir] [violin]` | the violin's body: the CNSM dataset's bridge admittances (Zenodo 10.5281/zenodo.18696786, CC BY 4.0) averaged, the acquisition's delay taken out, fitted by positive band-pass modes, the fit's distance beside the three violins' — into `arcus_upper_body_data.rs` (`examples/arcus_upper_body.rs`) |
| `upper-pitch.sh` | the violin's and the viola's bowed-pitch table, their voices at 4× the rate, into `arcus_upper_pitch_data.rs` (`examples/arcus_upper_pitch.rs`) |
| `upper-compare.sh [refs-dir] [out-dir]` | the violin and the viola beside the Iowa MIS and VSCO 2 CE recordings, feature by feature, Arcus–recording beside recording–recording (`examples/arcus_upper_compare.rs`) |
| `upper-listen.sh [out-dir] [refs-dir]` | the owner's violin and viola renders, the ABX pairs at −23 LUFS in `<out>/ref/`, `abx.html` beside them (`examples/arcus_upper_listen.rs`) |
| `upper-measure.sh [out-dir] [refs-dir] [study-dir]` | the violin and the viola measured in one pass: the study's notes rendered and read by the study's own estimator (`upper_study.py`: arco scalars and the vibrato's AM per partial against the real instruments' band), then `upper-compare.sh` |
| `gate.sh` | the gate's probes printed: the Schelleng diagram of the cello's open D at 10 and 20 cm/s, the Guettler diagram, the friction curves and the torsion compared (`cargo test -p cantus-dsp --lib arcus::tests_gate::probe -- --ignored --nocapture`) |

The ABX references (the Karoryfer Bigcat cello and the D. Smolken double bass, both CC0) are
downloaded file by file to a folder outside the worktree (`$ARCUS_REFS`, default
`../downloads/arcus`), never into it; `docs/round42/arcus.md` names the files.
