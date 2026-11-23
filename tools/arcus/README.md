# tools/arcus — Arcus, the bowed string (round 42, ADR 0157)

The scripts that render, measure and tune Arcus. Each wraps a cargo example or an ignored test
and goes through `tools/build/slot.sh` (three builds at once on a shared machine).

| script | what |
|---|---|
| `listen.sh [out-dir] [levels]` | the owner's renders: the cello and the double bass — a solo note at pp, mf and ff, a four-bar legato phrase with a CC1 swell, pizzicato, tremolo — 48 kHz 24-bit WAV (`crates/cantus-dsp/examples/arcus_listen.rs`); `levels` prints each one's peak and RMS |
| `pitch-table.sh` | measures the bowed pitch per key × dynamic × contact point, twice (the offset, then what is left with it applied), and writes `crates/cantus-dsp/src/arcus_pitch_data.rs` (`examples/arcus_pitch_table.rs`) |
| `abx.sh [out-dir]` | the blind ABX's phrase (`abx_phrase.mid`, the same file the recorded strings render; `python -I abx_phrase.py` writes it) rendered by the cello and the bass: `abx-{cello,bass}.wav` at −1 dBFS peak and `abx-*-lufs23.wav` at −23 LUFS (`examples/arcus_abx.rs`) |
| `compare.sh <near-Hz> <wav…>` | a render beside a recording: the fundamental, harmonics 1–10, the noise between harmonics by octave band, the centroid, the onset (`examples/arcus_compare.rs`) |
| `fides-pitch-table.sh` | Fides's model 2: its bowed-pitch table (`examples/fides_pitch_table.rs` → `fides_pitch_data.rs`) |
| `gate.sh` | the gate's probes printed: the Schelleng diagram of the cello's open D at 10 and 20 cm/s, the Guettler diagram, the friction curves and the torsion compared (`cargo test -p cantus-dsp --lib arcus::tests_gate::probe -- --ignored --nocapture`) |

The ABX references (the Karoryfer Bigcat cello and the D. Smolken double bass, both CC0) are
downloaded file by file to a folder outside the worktree (`$ARCUS_REFS`, default
`../downloads/arcus`), never into it; `docs/round42/arcus.md` names the files.
