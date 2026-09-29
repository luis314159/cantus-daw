//! A DX7 voice as a Modulus patch: DX operator *n* becomes operator letter *n* (1 → A), the
//! algorithm becomes matrix cells and outputs, and every value goes through the published
//! scalings. Where the DX has no documented number (LFO speed in hertz, the pitch-envelope
//! times, detune per step), the mapping is an approximation, named as such below.

use super::{DxVoice, DX_FULL_INDEX};
use cantus_core::inst::fm::{dx_template, OP_KEYS, OUT_OFF};
use cantus_core::{InstrumentKind, NativeSpec};
use cantus_dsp::modulus::env::dx_output_level_db;

/// Interpolate through `(x, y)` anchors.
fn through(points: &[(f32, f32)], x: f32) -> f32 {
    for w in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        }
    }
    points.last().map(|p| p.1).unwrap_or(0.0)
}

/// DX LFO speed 0…99 in hertz (approximate: about 0.06 Hz at 0, 5.4 at 35, 49 at 99).
pub fn lfo_hz(speed: u8) -> f32 {
    through(
        &[
            (0.0, 0.06),
            (20.0, 2.6),
            (35.0, 5.4),
            (50.0, 9.6),
            (70.0, 20.0),
            (99.0, 49.3),
        ],
        speed as f32,
    )
}

/// DX LFO delay 0…99 in milliseconds (approximate).
fn lfo_delay_ms(delay: u8) -> f32 {
    through(&[(0.0, 0.0), (50.0, 1000.0), (99.0, 4500.0)], delay as f32)
}

/// Semitones of vibrato at full pitch-modulation depth, per sensitivity 0…7 (approximate).
const PMS_SEMITONES: [f32; 8] = [0.0, 0.45, 0.75, 1.3, 2.2, 3.8, 6.5, 12.0];

/// A pitch-envelope rate as the time of a typical move (approximate): 1 ms at 99, doubling
/// every 8 steps down.
fn peg_ms(rate: u8) -> f32 {
    ((99.0 - rate as f32) / 8.0).exp2()
}

/// A pitch-envelope level: 50 is the note, 0 and 99 about four octaves away (linear here).
fn peg_st(level: u8) -> f32 {
    (level as f32 - 50.0) / 50.0 * 48.0
}

/// DX LFO waves (triangle, saw down, saw up, square, sine, S&H) in Modulus's order.
const LFO_WAVE: [f32; 6] = [1.0, 2.0, 3.0, 4.0, 0.0, 5.0];

/// The patch a DX voice describes. Keys at their defaults are left unstored.
pub fn to_spec(v: &DxVoice) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Fm);
    let mut set = |k: &str, x: f32| {
        s.set(k, x);
    };
    let alg = v.algorithm as usize + 1;
    let fb = if v.feedback == 0 {
        0.0
    } else {
        std::f32::consts::PI * ((v.feedback as f32) - 7.0).exp2()
    };
    for (k, x) in dx_template(alg, DX_FULL_INDEX, fb) {
        set(k, x);
    }
    for (i, o) in v.ops.iter().enumerate() {
        let k = &OP_KEYS[i];
        set(k.env_mode, 1.0);
        for j in 0..4 {
            set(k.dx_r[j], o.rates[j] as f32);
            set(k.dx_l[j], o.levels[j] as f32);
        }
        set(k.level, dx_output_level_db(o.output).max(-96.0));
        set(k.vel, o.velocity as f32 / 7.0);
        set(k.ams, o.ams as f32);
        set(k.detune, o.detune as f32 - 7.0);
        if o.fixed {
            set(k.freq_mode, 1.0);
            let base = [1.0, 10.0, 100.0, 1000.0][(o.coarse & 3) as usize];
            set(k.fixed, base * 10f32.powf(o.fine as f32 / 100.0));
        } else {
            let coarse = if o.coarse == 0 { 0.5 } else { o.coarse as f32 };
            set(k.ratio, coarse * (1.0 + o.fine as f32 / 100.0));
        }
        set(k.ks_break, o.break_point as f32 + 21.0);
        set(k.ks_ldepth, o.left_depth as f32 * 48.0 / 99.0);
        set(k.ks_rdepth, o.right_depth as f32 * 48.0 / 99.0);
        set(k.ks_lcurve, o.left_curve as f32);
        set(k.ks_rcurve, o.right_curve as f32);
        set(k.ks_rate, o.rate_scaling as f32);
        set(k.sync, if v.key_sync { 1.0 } else { 0.0 });
    }
    // Operators G and H stay silent and unconnected.
    for k in &OP_KEYS[6..] {
        set(k.out, OUT_OFF);
    }
    set("transpose", v.transpose as f32 - 24.0);
    set("lfo1.rate", lfo_hz(v.lfo_speed));
    set("lfo1.delay", lfo_delay_ms(v.lfo_delay));
    set("lfo1.wave", LFO_WAVE[v.lfo_wave.min(5) as usize]);
    set("lfo1.sync", if v.lfo_sync { 1.0 } else { 0.0 });
    set(
        "lfo1.pitch",
        v.lfo_pmd as f32 / 99.0 * PMS_SEMITONES[v.pms.min(7) as usize] * 100.0,
    );
    set("lfo1.amp", v.lfo_amd as f32 / 99.0 * 48.0);
    let [r1, r2, r3, r4] = v.pitch_rates;
    let [l1, l2, l3, l4] = v.pitch_levels;
    if [l1, l2, l3, l4] != [50; 4] {
        set("penv.l0", peg_st(l4));
        set("penv.t1", peg_ms(r1));
        set("penv.l1", peg_st(l1));
        set("penv.t2", peg_ms(r2) + peg_ms(r3));
        set("penv.l2", peg_st(l3));
        set("penv.t3", peg_ms(r4));
        set("penv.l3", peg_st(l4));
    }
    // Stored only what differs from the defaults.
    let defaults = NativeSpec::new(InstrumentKind::Fm);
    s.params.retain(|k, v| defaults.get(k) != *v);
    let name = v.name();
    if !name.is_empty() {
        let _ = s.set_text("name", Some(name));
    }
    s
}
