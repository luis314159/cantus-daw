# Round 21 — requests between builders

## From *math* to *graph*: route the last std transcendentals in your files

ADR 0031 R9: every transcendental on a rendered path goes through `cantus_dsp::math`
(the pinned pure-Rust `libm`), the same bits on every machine. `crates/cantus-engine/clippy.toml`
now bans std's (`f32::powf`, `f64::log2`…) in the whole engine crate; these call sites are in
files *graph* owns this round, so *math* left them, and `cargo clippy` warns on them until
they move:

| File | Line (at 49f02f3) | Today | Becomes |
|---|---|---|---|
| `src/processor_rec.rs` | 319 | `10f32.powf(i.trim_db / 20.0)` | `math::powf(10f32, i.trim_db / 20.0)` |
| `src/processor_rec.rs` | 332 | `10f32.powf(db / 20.0)` | `math::powf(10f32, db / 20.0)` |
| `src/profile.rs` | 51 | `us.log2()` | `math::log2(us)` |
| `src/profile.rs` | 59, 65 | `2f64.powf(x)` | `math::powf(2f64, x)` |
| `src/profile.rs` | 371, 374 (tests) | `2f64.powf(0.25)` | `math::powf(2f64, 0.25)` |

with `use cantus_dsp::math;` at the top. `processor_rec.rs`'s two are the input trim of a
recording (rendered bits); `profile.rs`'s are the bench's histogram buckets (no audio, done
only so the lint is clean). After the math merge the backend is libm, so the trim's gain can
differ from std's by an ulp — no golden covers a recording's trim; nothing to re-bless.

Any new DSP code in `graph/`: `cantus_dsp::math::*`, never std's `sin`/`exp`/`powf`/….
