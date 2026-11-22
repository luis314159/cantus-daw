//! Fragor's five circuits as pedals of their own on Tabula's board (round 41, ADR 0152): their
//! parameter models. Owned by the `pedals` builder.
//!
//! Fragor (`EffectKind::Pedal`, ADR 0128) enters a board as one pedal — the whole unit, three
//! slots. The owner (2026-11-18, BACKLOG §3b): the green overdrive, the op-amp distortion, the
//! silicon distortion, the germanium fuzz and the triangle muff each a stompbox of its own,
//! so several drives can sit anywhere in a chain, between other pedals, beyond Fragor's three
//! slots. Each is the same DK circuit and **equals Fragor with only that slot filled, to the
//! bit** ([`as_fragor`] is that Fragor). They are **board-only**
//! ([`crate::EffectKind::offered`] is false for them: never in an insert menu; Fragor is the
//! insert) and are not exported as plug-ins.
//!
//! ## The keys (forever once shipped)
//!
//! Each pedal reads **Fragor's own keys** for what it has, so a pedal and a one-slot Fragor
//! read the same values (a Fragor preset of one circuit is a pedal preset, key for key):
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `od.on` (`op.on`, `si.on`, `ge.on`, `mf.on`) | the circuit in; off, the pedal is a wire (true bypass, no latency) | 0 / 1 | **off** |
//! | the circuit's three knobs, Fragor's (`od.drive`, `od.tone`, `od.level` …) | as the pedal labels them | 0 … 100 % | Fragor's (noon; the fuzz's battery fresh) |
//! | `gate` | the noise gate before the circuit; −90 is off | −90 … −20 dB | off |
//! | `in` | the level into the circuit | −24 … 24 dB | 0 |
//! | `mix` | dry/wet, the dry path delayed to match | 0 … 100 % | 100 % |
//! | `mono` | one circuit for both channels (their sum), as a pedal is | 0 / 1 | stereo |
//! | `os` | oversampling: 2×, 4×, 8× | 0 … 2 | 4× |
//! | `out` | output gain | −24 … 24 dB | 0 |
//!
//! **The null.** A circuit colours by design — no knob of it is transparent — so, as Arca's
//! "no amp", the null is *the circuit out*: `on` defaults to off, and the pedal is then a wire
//! times its gains (0 dB): its defaults touch no sample, to the bit, with no latency. A
//! board's pedal always starts from its starting preset (ADR 0071, `starting_spec`), which
//! puts the circuit in. The board's own footswitch (`Pedal::on`) is the one a player stamps
//! on: it keeps the board's latency (ADR 0131 §3). `on` here is the circuit's presence, a
//! structural change as Fragor's slot is (its 10 ms dip, the latency following).

use crate::effect::{p, EffectKind, EffectParamInfo, EffectSpec};
use crate::fx::fragor;

/// The five pedal kinds, by circuit (`EffectKind::fragor_circuit`'s inverse).
pub const KINDS: [EffectKind; 5] = [
    EffectKind::FragorOverdrive,
    EffectKind::FragorOpAmp,
    EffectKind::FragorSilicon,
    EffectKind::FragorFuzz,
    EffectKind::FragorMuff,
];

/// The five circuits, in Fragor's model order (`fragor_circuits::Model`: Overdrive, OpAmp,
/// Silicon, Fuzz, Muff) and `EffectKind::ALL`'s.
pub const STEMS: [&str; 5] = [
    "fragor_od",
    "fragor_op",
    "fragor_si",
    "fragor_ge",
    "fragor_mf",
];

/// Each pedal's name key, by circuit.
pub const NAME_KEYS: [&str; 5] = [
    "effect.fragor_od",
    "effect.fragor_op",
    "effect.fragor_si",
    "effect.fragor_ge",
    "effect.fragor_mf",
];

pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
/// Whether it hears a track's notes ([`crate::EffectSlot::midi_from`], ADR 0022).
pub const TAKES_NOTES: bool = false;
/// The locale prefix of each stepped parameter's choices, by key (Fragor's words).
pub const CHOICES: &[(&str, &str)] = &[
    ("od.on", "fragor.onoff"),
    ("op.on", "fragor.onoff"),
    ("si.on", "fragor.onoff"),
    ("ge.on", "fragor.onoff"),
    ("mf.on", "fragor.onoff"),
    ("mono", "fragor.chan"),
    ("os", "fragor.os"),
];

/// The keys every pedal shares with Fragor beyond its circuit's own.
pub const SHARED: [&str; 6] = ["gate", "in", "mix", "mono", "os", "out"];

const fn knob(key: &'static str, label: &'static str, default: f32) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, default, "%", false, false)
}

/// The circuit's presence: off (the default, the null) or in.
const fn circuit_in(key: &'static str, label: &'static str) -> EffectParamInfo {
    p(key, label, 0.0, 1.0, 0.0, "", false, true)
}

/// A pedal's table: its circuit's switch and knobs, then Fragor's levels and quality (the
/// same ranges and defaults as Fragor's).
const fn table(head: [EffectParamInfo; 4]) -> [EffectParamInfo; 10] {
    [
        head[0],
        head[1],
        head[2],
        head[3],
        p(
            "gate",
            "fx.fragor.gate",
            fragor::GATE_OFF_DB,
            -20.0,
            fragor::GATE_OFF_DB,
            "dB",
            false,
            false,
        ),
        p("in", "fx.fragor.in", -24.0, 24.0, 0.0, "dB", false, false),
        p("mix", "fx.fragor.mix", 0.0, 1.0, 1.0, "%", false, false),
        p("mono", "fx.fragor.mono", 0.0, 1.0, 0.0, "", false, true),
        p("os", "fx.fragor.os", 0.0, 2.0, 1.0, "", false, true),
        p("out", "fx.fragor.out", -24.0, 24.0, 0.0, "dB", false, false),
    ]
}

const OD: [EffectParamInfo; 10] = table([
    circuit_in("od.on", "fx.fragor.od.on"),
    knob("od.drive", "fx.fragor.od.drive", 0.5),
    knob("od.tone", "fx.fragor.od.tone", 0.5),
    knob("od.level", "fx.fragor.od.level", 0.5),
]);
const OP: [EffectParamInfo; 10] = table([
    circuit_in("op.on", "fx.fragor.op.on"),
    knob("op.dist", "fx.fragor.op.dist", 0.5),
    knob("op.filter", "fx.fragor.op.filter", 0.5),
    knob("op.vol", "fx.fragor.op.vol", 0.5),
]);
const SI: [EffectParamInfo; 10] = table([
    circuit_in("si.on", "fx.fragor.si.on"),
    knob("si.dist", "fx.fragor.si.dist", 0.5),
    knob("si.tone", "fx.fragor.si.tone", 0.5),
    knob("si.level", "fx.fragor.si.level", 0.5),
]);
const GE: [EffectParamInfo; 10] = table([
    circuit_in("ge.on", "fx.fragor.ge.on"),
    knob("ge.fuzz", "fx.fragor.ge.fuzz", 0.5),
    knob("ge.bias", "fx.fragor.ge.bias", 1.0),
    knob("ge.vol", "fx.fragor.ge.vol", 0.5),
]);
const MF: [EffectParamInfo; 10] = table([
    circuit_in("mf.on", "fx.fragor.mf.on"),
    knob("mf.sustain", "fx.fragor.mf.sustain", 0.5),
    knob("mf.tone", "fx.fragor.mf.tone", 0.5),
    knob("mf.vol", "fx.fragor.mf.vol", 0.5),
]);

/// Each pedal's table, by circuit.
pub const PARAMS: [&[EffectParamInfo]; 5] = [&OD, &OP, &SI, &GE, &MF];

/// The parameters of circuit `i` (0 … 4, in [`STEMS`]' order; out of range, the muff's).
pub fn params(i: usize) -> &'static [EffectParamInfo] {
    PARAMS[i.min(4)]
}

/// Circuit `i`'s switch: its presence (`od.on` …), Fragor's footswitch key.
pub fn switch(i: usize) -> &'static str {
    fragor::SWITCHES[i.min(4)]
}

/// Circuit `i`'s three knobs, Fragor's keys in the DSP's order (gain, tone, level).
pub fn knobs(i: usize) -> [&'static str; 3] {
    fragor::KNOBS[i.min(4)]
}

/// The one-slot Fragor a pedal is, to the bit: its circuit in the first slot when it is in
/// (the board empty when it is out), that circuit's footswitch on, and every value the pedal
/// has under the same key. For a kind that is not one of the five, an empty Fragor.
pub fn as_fragor(spec: &EffectSpec) -> EffectSpec {
    let mut f = EffectSpec::new(EffectKind::Pedal);
    let Some(i) = spec.kind.fragor_circuit() else {
        return f;
    };
    let on = spec.get(switch(i)) >= 0.5;
    f.set(fragor::SLOTS[0], if on { i as f32 + 1.0 } else { 0.0 });
    f.set(switch(i), 1.0);
    for k in knobs(i).iter().chain(SHARED.iter()) {
        f.set(k, spec.get(k));
    }
    f
}

/// The pedal a one-slot Fragor is: its first slot's circuit, in, with its knobs, its levels
/// and its quality (an empty first slot gives the overdrive, out). How a pedal preset is made
/// from one of Fragor's single-circuit presets.
pub fn from_fragor(spec: &EffectSpec) -> EffectSpec {
    let v = spec.get(fragor::SLOTS[0]).round().clamp(0.0, 5.0) as usize;
    let i = v.saturating_sub(1);
    let mut s = EffectSpec::new(KINDS[i]);
    let on = v > 0 && spec.get(switch(i)) >= 0.5;
    s.set(switch(i), if on { 1.0 } else { 0.0 });
    for k in knobs(i).iter().chain(SHARED.iter()) {
        s.set(k, spec.get(k));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each table is whole (every key once), its keys and ranges Fragor's, and its defaults a
    /// null: the circuit out, the gate off, 0 dB.
    #[test]
    fn each_table_is_fragors_keys_and_its_defaults_are_a_null() {
        for (i, kind) in KINDS.iter().enumerate() {
            assert_eq!(kind.fragor_circuit(), Some(i));
            assert_eq!(kind.params(), params(i));
            let t = params(i);
            let mut keys: Vec<&str> = t.iter().map(|p| p.key).collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), t.len(), "{kind:?}");
            for p in t {
                let f = fragor::PARAMS
                    .iter()
                    .find(|f| f.key == p.key)
                    .unwrap_or_else(|| panic!("{kind:?}: {} is not Fragor's", p.key));
                assert_eq!((p.min, p.max, p.unit), (f.min, f.max, f.unit), "{}", p.key);
                assert_eq!((p.stepped, p.logarithmic), (f.stepped, f.logarithmic));
                assert_eq!(p.label_key, f.label_key);
                if p.key != switch(i) {
                    assert_eq!(p.default, f.default, "{}", p.key);
                }
            }
            let get = |k: &str| t.iter().find(|p| p.key == k).unwrap().default;
            assert_eq!(get(switch(i)), 0.0, "{kind:?}: the null is the circuit out");
            assert_eq!(get("out"), 0.0);
            assert_eq!(get("in"), 0.0);
            assert_eq!(get("gate"), fragor::GATE_OFF_DB);
            for k in knobs(i) {
                assert!(t.iter().any(|p| p.key == k));
            }
            for p in t.iter().filter(|p| p.stepped) {
                assert!(CHOICES.iter().any(|(k, _)| *k == p.key), "{}", p.key);
            }
        }
    }

    /// A pedal at its defaults is an empty Fragor (a wire); in, it is Fragor with only its
    /// slot filled; and back again.
    #[test]
    fn a_pedal_is_a_one_slot_fragor_and_back() {
        for (i, kind) in KINDS.iter().enumerate() {
            let d = EffectSpec::new(*kind);
            assert_eq!(as_fragor(&d).get("s1"), 0.0);
            let mut s = EffectSpec::new(*kind);
            s.set(switch(i), 1.0);
            s.set(knobs(i)[0], 0.8);
            s.set("mix", 0.4);
            s.set("os", 2.0);
            let f = as_fragor(&s);
            assert_eq!(f.get("s1"), i as f32 + 1.0);
            assert_eq!((f.get("s2"), f.get("s3")), (0.0, 0.0));
            assert_eq!(f.get(knobs(i)[0]), 0.8);
            assert_eq!(f.get("mix"), 0.4);
            assert_eq!(f.get("os"), 2.0);
            let back = from_fragor(&f);
            for k in back.kind.params() {
                assert_eq!(back.get(k.key), s.get(k.key), "{kind:?} {}", k.key);
            }
        }
    }
}
