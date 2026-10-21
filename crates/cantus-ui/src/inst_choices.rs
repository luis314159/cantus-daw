//! The native instruments' choices by name (round 29, review 4 of ADR 0062, #7, #22, #33,
//! #45): what each stepped parameter's values are called, keyed by instrument and field —
//! the locale keys the editors draw — so the controllers pane names a mapping's *From* and
//! *To* (and reads a typed name back) whatever editor was opened this session, in any
//! language. `learn_everywhere.rs` paints every editor and holds what each draws to this
//! table.

use crate::controllers_words::Word;
use cantus_core::inst::{fm, wavetable as unda};
use cantus_core::InstrumentKind;

/// A parameter's field: its key with each slot's number or letter left out — `osc2.wt`
/// → `osc.wt`, `op.c.wave` → `op.x.wave`, `z12.mode` → `z.mode`, `mod7.curve` →
/// `mod.curve`, `osc1.warp2.src` → `osc.warp.src`.
pub fn field(key: &str) -> String {
    key.split('.')
        .map(|seg| {
            let head = seg.trim_end_matches(|c: char| c.is_ascii_digit());
            match head {
                "" => seg,
                // An FM operator's letter, a segment of its own.
                h if seg.len() == 1 && ('a'..='h').contains(&h.chars().next().unwrap_or(' ')) => {
                    "x"
                }
                h => h,
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn keys(list: &[&str]) -> Vec<Word> {
    list.iter().map(|k| Word::Key((*k).to_string())).collect()
}

/// `prefix.name` for each name (Unda's lists, as its editor builds them).
fn named(prefix: &str, list: &[&str]) -> Vec<Word> {
    list.iter()
        .map(|n| Word::Key(format!("{prefix}.{n}")))
        .collect()
}

/// The words of `key`'s values from its lowest up; `None` when its values are numbers (or
/// it is no stepped parameter of `kind`).
pub fn words(kind: InstrumentKind, key: &str) -> Option<Vec<Word>> {
    use InstrumentKind::*;
    let f = field(key);
    // Every kind's modulation slots (the instrument frame's curve field).
    if f == "mod.curve" {
        return Some(keys(&[
            "mod.curve.linear",
            "mod.curve.exp",
            "mod.curve.log",
            "mod.curve.s",
            "mod.curve.steps",
        ]));
    }
    Some(match (kind, f.as_str()) {
        // Calor (analog_edit.rs, analog.rs).
        (Analog, "voice.mode") => {
            keys(&["calor.mode.poly", "calor.mode.mono", "calor.mode.legato"])
        }
        (Analog, "voice.assign") => keys(&[
            "calor.assign.rotate",
            "calor.assign.reuse",
            "calor.assign.lowest",
        ]),
        (Analog, "voice.priority") => keys(&[
            "calor.priority.last",
            "calor.priority.low",
            "calor.priority.high",
        ]),
        (Analog, "glide.mode") => keys(&["calor.glide.by_time", "calor.glide.by_rate"]),
        (Analog, "glide.curve") => keys(&["calor.glide.rc", "calor.glide.linear"]),
        (Analog, "osc.rev") => keys(&[
            "calor.rev.ideal",
            "calor.rev.vintage_a",
            "calor.rev.vintage_b",
        ]),
        (Analog, "sub.shape") => keys(&["calor.sub.sq1", "calor.sub.sq2", "calor.sub.pulse1"]),
        (Analog, "noise.color") => keys(&["calor.noise.white", "calor.noise.pink"]),
        (Analog, "hpf.pos") => keys(&["calor.hpf.pre", "calor.hpf.post", "calor.hpf.boost"]),
        (Analog, "vca.source") => keys(&["calor.vca.env2", "calor.vca.gate"]),
        (Analog, "env.model") => keys(&[
            "calor.env.model.rc",
            "calor.env.model.ads",
            "calor.env.model.digital",
            "calor.env.model.oneshot",
        ]),
        (Analog, "env.retrig") => keys(&["calor.retrig.current", "calor.retrig.reset"]),
        (Analog, "lfo.shape") => keys(&[
            "calor.lfo.tri",
            "calor.lfo.sine",
            "calor.lfo.saw_up",
            "calor.lfo.saw_down",
            "calor.lfo.square",
            "calor.lfo.sh",
            "calor.lfo.smooth",
        ]),
        (Analog, "lfo.restart") => keys(&[
            "calor.restart.free",
            "calor.restart.gate",
            "calor.restart.random",
        ]),
        (Analog, "analog.phase") => keys(&["calor.phase.free", "calor.phase.reset"]),
        (Analog, "vcf.model") => keys(&[
            "calor.model.ladder",
            "calor.model.cascade",
            "calor.model.diode",
            "calor.model.svf",
            "calor.model.sk",
        ]),
        // From 1 (6 dB) to 4 (24 dB).
        (Analog, "vcf.slope") => keys(&[
            "calor.slope.6",
            "calor.slope.12",
            "calor.slope.18",
            "calor.slope.24",
        ]),
        (Analog, "vcf.mid") => keys(&["calor.mid.notch", "calor.mid.band"]),
        (Analog, "vcf.rev") => keys(&[
            "calor.vcf.rev.clean",
            "calor.vcf.rev.rough",
            "calor.vcf.rev.pushed",
        ]),
        (Analog, "quality") => keys(&[
            "calor.quality.draft",
            "calor.quality.good",
            "calor.quality.best",
        ]),
        (Analog, "chorus.mode") => keys(&[
            "calor.chorus.off",
            "calor.chorus.one",
            "calor.chorus.two",
            "calor.chorus.both",
        ]),
        // Gravis (bass.rs, bass_edit.rs).
        (Bass, "engine") => keys(&["bass.engine.808", "bass.engine.synth", "bass.engine.string"]),
        // All three, whichever engine the editor shows (it draws *Strings* with the
        // string engine only).
        (Bass, "mode") => keys(&["bass.mode.legato", "bass.mode.mono", "bass.mode.strings"]),
        (Bass, "priority") => keys(&[
            "bass.priority.last",
            "bass.priority.low",
            "bass.priority.high",
        ]),
        (Bass, "glide.trigger") => {
            keys(&["bass.glide.legato", "bass.glide.always", "bass.glide.off"])
        }
        (Bass, "glide.rate") => keys(&["bass.glide.time", "bass.glide.rate"]),
        (Bass, "glide.shape") => keys(&["bass.shape.exp", "bass.shape.linear", "bass.shape.s"]),
        (Bass, "phase") => keys(&["bass.phase.reset", "bass.phase.random", "bass.phase.free"]),
        (Bass, "lfo.shape") => keys(&[
            "bass.lfo.sine",
            "bass.lfo.triangle",
            "bass.lfo.square",
            "bass.lfo.saw",
        ]),
        (Bass, "syn.wave") => keys(&[
            "bass.wave.saw",
            "bass.wave.square",
            "bass.wave.pulse",
            "bass.wave.triangle",
        ]),
        (Bass, "syn.filter") => keys(&[
            "bass.filter.ladder",
            "bass.filter.diode",
            "bass.filter.svf_lp",
            "bass.filter.svf_bp",
        ]),
        // From 4 strings to 6.
        (Bass, "str.strings") => keys(&["bass.strings.4", "bass.strings.5", "bass.strings.6"]),
        (Bass, "str.tuning") => keys(&[
            "bass.tuning.standard",
            "bass.tuning.drop",
            "bass.tuning.half",
        ]),
        (Bass, "str.type") => keys(&[
            "bass.type.round",
            "bass.type.half",
            "bass.type.flat",
            "bass.type.tape",
        ]),
        (Bass, "str.technique") => keys(&[
            "bass.tech.finger",
            "bass.tech.pick",
            "bass.tech.slap",
            "bass.tech.mute",
        ]),
        (Bass, "str.fingering") => keys(&[
            "bass.fingering.lowest",
            "bass.fingering.position",
            "bass.fingering.low",
        ]),
        (Bass, "str.pu_type") => keys(&["bass.pu.single", "bass.pu.humbucker"]),
        (Bass, "sub.wave") => keys(&["bass.lfo.sine", "bass.lfo.triangle", "bass.lfo.square"]),
        (Bass, "drive.type") => keys(&[
            "bass.drive.tube",
            "bass.drive.tape",
            "bass.drive.fold",
            "bass.drive.hard",
        ]),
        (Bass, "duck.source") => keys(&["bass.duck.none", "bass.duck.kick", "bass.duck.sidechain"]),
        (Bass, "duck.shape") => keys(&["bass.shape.linear", "bass.shape.exp", "bass.shape.s"]),
        (Bass, "key.snap") => keys(&["bass.snap.off", "bass.snap.scale", "bass.snap.roots"]),
        // Lamina (epiano.rs).
        (ElectricPiano, "engine") => keys(&["epiano.engine_tine", "epiano.engine_reed"]),
        (ElectricPiano, "trem.mode") => {
            keys(&["epiano.trem_auto", "epiano.trem_pan", "epiano.trem_amp"])
        }
        (ElectricPiano, "output") => keys(&["epiano.out_direct", "epiano.out_speaker"]),
        (ElectricPiano, "range") => {
            keys(&["epiano.range_73", "epiano.range_64", "epiano.range_88"])
        }
        // Organum (organ.rs): the scanner's modes are their own names in every language.
        (Organ, "perc.harm") => keys(&["organ.perc_2nd", "organ.perc_3rd"]),
        (Organ, "perc.decay") => keys(&["organ.perc_fast_sw", "organ.perc_slow_sw"]),
        (Organ, "perc.vol") => keys(&["organ.perc_normal", "organ.perc_soft_sw"]),
        (Organ, "vib") => cantus_core::inst::organ::VIB_MODES
            .iter()
            .map(|m| Word::Text((*m).to_string()))
            .collect(),
        (Organ, "rot.speed") => keys(&["organ.speed_stop", "organ.speed_slow", "organ.speed_fast"]),
        (Organ, "tuning") => keys(&["organ.tuning_gear", "organ.tuning_equal"]),
        (Organ, "rot.cab") => keys(&["organ.cab_closed", "organ.cab_open"]),
        // Modulus (cantus_core::inst::fm's lists).
        (Fm, "mode") => keys(&fm::MODES),
        (Fm, "quality") => keys(&fm::QUALITIES),
        (Fm, "guard") => keys(&fm::GUARDS),
        (Fm, "lfo.wave") => keys(&fm::LFO_WAVES),
        (Fm, "op.x.wave") => keys(&fm::WAVES),
        (Fm, "op.x.freq_mode") => keys(&["fm.op.ratio", "fm.op.fixed"]),
        (Fm, "env.x.mode") => keys(&["fm.env.time_mode", "fm.env.dx_mode"]),
        (Fm, "ks.x.lcurve" | "ks.x.rcurve") => keys(&fm::KS_CURVES),
        // Exemplar (sampler_cards.rs, sampler_map.rs, sampler_wave.rs).
        (Sampler, "filter.type") => keys(&[
            "smp.filter.off",
            "smp.filter.lp",
            "smp.filter.bp",
            "smp.filter.hp",
        ]),
        (Sampler, "lfo.shape") => keys(&[
            "smp.shape.sine",
            "smp.shape.triangle",
            "smp.shape.square",
            "smp.shape.saw",
        ]),
        (Sampler, "mode") => keys(&["smp.play.poly", "smp.play.mono", "smp.play.legato"]),
        (Sampler, "quality") => {
            keys(&["smp.q.draft", "smp.q.standard", "smp.q.high", "smp.q.best"])
        }
        (Sampler, "rr.order") => keys(&["smp.rr.by_note", "smp.rr.in_order"]),
        (Sampler, "z.mode") => keys(&[
            "smp.mode.no_loop",
            "smp.mode.one_shot",
            "smp.mode.continuous",
            "smp.mode.sustain",
        ]),
        (Sampler, "z.dir") => keys(&["smp.dir.forward", "smp.dir.pingpong", "smp.dir.backward"]),
        (Sampler, "z.lxc") => keys(&["smp.xf_linear", "smp.xf_power"]),
        (Sampler, "z.trig") => keys(&[
            "smp.trig.attack",
            "smp.trig.release",
            "smp.trig.first",
            "smp.trig.legato",
            "smp.trig.release_key",
            "smp.trig.controller",
        ]),
        // Unda (cantus_core::inst::wavetable's lists, `prefix.name`).
        (Wavetable, "quality") => named("unda.q", unda::QUALITY),
        (Wavetable, "voice.mode") => named("unda.mode", unda::VOICE_MODES),
        (Wavetable, "voice.prio") => named("unda.prio", &["last", "low", "high"]),
        (Wavetable, "voice.glide_mode") => named("unda.glide_mode", &["legato", "always"]),
        (Wavetable, "osc.wt") => named("unda.wt", unda::TABLES),
        (Wavetable, "osc.route" | "sub.route" | "noise.route") => named("unda.route", unda::ROUTES),
        (Wavetable, "osc.ucurve") => named("unda.ucurve", unda::UNISON_CURVES),
        (Wavetable, "osc.stack") => named("unda.stack", unda::STACKS),
        (Wavetable, "osc.warp") => named("unda.warp", unda::WARPS),
        (Wavetable, "osc.warp.src") => named("unda.src", unda::WARP_SOURCES),
        (Wavetable, "osc.engine") => named("unda.engine", unda::ENGINES),
        (Wavetable, "osc.gwin") => named("unda.gwin", unda::GRAIN_WINDOWS),
        (Wavetable, "sub.wave") => named("unda.sub", unda::SUB_WAVES),
        (Wavetable, "flt.type") => named("unda.flt", unda::FILTERS),
        (Wavetable, "lfo.wave") => named("unda.lfo", unda::LFO_WAVES),
        (Wavetable, "lfo.mode") => named("unda.lfo_mode", unda::LFO_MODES),
        _ => return None,
    })
}

/// Firmus's waves.
pub fn firmus_waves() -> Vec<Word> {
    cantus_core::Waveform::ALL
        .iter()
        .map(|w| Word::Key(w.label_key().to_string()))
        .collect()
}

/// Every key the table names, for the i18n test (both languages must have each).
pub fn all_keys() -> Vec<String> {
    let mut out = Vec::new();
    for kind in InstrumentKind::ALL {
        for info in kind.params() {
            for w in words(kind, info.key).into_iter().flatten() {
                if let Word::Key(k) = w {
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
        }
    }
    // Keys that are slots' (a zone's fields, a modulation slot's curve).
    for (kind, key) in [
        (InstrumentKind::Sampler, "z0.mode"),
        (InstrumentKind::Sampler, "z0.dir"),
        (InstrumentKind::Sampler, "z0.lxc"),
        (InstrumentKind::Sampler, "z0.trig"),
        (InstrumentKind::Analog, "mod1.curve"),
    ] {
        for w in words(kind, key).into_iter().flatten() {
            if let Word::Key(k) = w {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
    }
    for w in firmus_waves() {
        if let Word::Key(k) = w {
            out.push(k);
        }
    }
    out
}
