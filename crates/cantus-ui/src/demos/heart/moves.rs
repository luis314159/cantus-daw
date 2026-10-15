//! What moves: the automation lanes (Chorda's sustain pedal on every chord change, the
//! builds' filter sweeps, Translatio's riser, Fluctus coming in for the choruses, Anulus
//! growing in the breakdown, Vibrans's tape wow, the reverb throws) and the modulators
//! (an LFO on the pad's filter, a follower ducking the bass from the kick, a step sequence
//! and a random pan on the arpeggio).

use super::music::chart;
use super::{Band, Desk, FORM};
use crate::demos::song::{Part, Song};
use crate::i18n::t;
use cantus_core::modulator::{
    FollowSource, LfoShape, LfoSource, Listen, ModRate, RandomSource, Retrigger, StepSource,
    DIV_SIXTEENTH,
};
use cantus_core::{db_to_linear, AutoTarget, EffectId, ModHost, ModSource, Shape, StripId};

use Shape::{Curve, Hold, SCurve};

/// The insert of `kind` on `strip` (the first one).
fn fx(s: &Song, strip: StripId, kind: cantus_core::EffectKind) -> EffectId {
    s.p.strip(strip)
        .and_then(|st| st.effects.iter().find(|e| e.spec.kind == kind))
        .map(|e| e.id)
        .unwrap_or_else(|| panic!("no {kind:?} on the strip"))
}

fn param(effect: EffectId, key: &str) -> AutoTarget {
    AutoTarget::Effect {
        effect,
        key: key.to_string(),
    }
}

fn inst(s: &Song, part: Part, key: &str) -> AutoTarget {
    AutoTarget::Instrument {
        track: s.track_id(part),
        key: key.to_string(),
    }
}

pub fn automation(s: &mut Song, b: &Band, d: &Desk) {
    use cantus_core::EffectKind as K;
    pedal(s, b);

    // Vibrans's tape wow: the intro's piano is a memory that comes into focus for the verse;
    // the outro's drifts away again.
    let wow = param(fx(s, b.piano.strip, K::Vibrato), "depth");
    let name = t("demos.auto.wow");
    s.automate(
        b.piano,
        wow.clone(),
        &name,
        0.0,
        8.0,
        &[(0.0, 16.0, Curve), (20.0, 16.0, SCurve), (31.0, 0.0, Hold)],
    );
    s.automate(
        b.piano,
        wow,
        &name,
        72.0,
        8.0,
        &[(0.0, 0.0, Curve), (12.0, 0.0, SCurve), (30.0, 22.0, Hold)],
    );

    // The builds: the music bus's high-pass climbs through the last four bars before each
    // chorus and drops back on its downbeat.
    let sweep = param(fx(s, d.music, K::Eq), "b0.freq");
    let name = t("demos.auto.filter");
    for (at, top) in [(24.0, 650.0), (48.0, 900.0)] {
        s.automate(
            b.pad,
            sweep.clone(),
            &name,
            at,
            8.0,
            &[(0.0, 20.0, Hold), (16.0, 20.0, SCurve), (31.5, top, Hold), (31.9, 20.0, Hold)],
        );
    }

    // The riser: Translatio's barber pole fades in and spins faster into the drop.
    let rise = fx(s, b.rise.strip, K::FreqShift);
    for at in [28.0, 52.0] {
        s.automate(
            b.rise,
            param(rise, "mix"),
            &t("demos.auto.shift_mix"),
            at,
            4.0,
            &[(0.0, 0.0, Curve), (15.0, 0.7, Hold), (15.9, 0.0, Hold)],
        );
        s.automate(
            b.rise,
            param(rise, "shift"),
            &t("demos.auto.shift"),
            at,
            4.0,
            &[(0.0, 2.0, Curve), (15.9, 40.0, Hold)],
        );
    }

    // Fluctus pumps the pad and the keys in the choruses only.
    for part in [b.pad, b.keys] {
        let depth = param(fx(s, part.strip, K::VolumeShaper), "depth");
        for (key, at, _) in FORM.iter().filter(|(_, at, _)| *at >= 32.0) {
            let v = if *key == "chorus" { 0.75 } else { 0.0 };
            s.automate(part, depth.clone(), &t("demos.auto.pump"), *at, 1.0, &[(0.0, v, Hold)]);
        }
    }

    // Anulus grows over the breakdown's bells, then lets go before the last chorus.
    s.automate(
        b.bells,
        param(fx(s, b.bells.strip, K::RingMod), "mix"),
        &t("demos.auto.ring"),
        48.0,
        8.0,
        &[(0.0, 0.0, Curve), (14.0, 0.2, Curve), (28.0, 0.65, Hold), (31.0, 0.0, Hold)],
    );

    // A reverb throw on the last word of each chorus.
    let throw = AutoTarget::Send {
        strip: b.vox.strip,
        to: d.verb,
    };
    let (dry, wet) = (db_to_linear(-11.0), db_to_linear(-2.0));
    for at in [46.0, 70.0] {
        s.automate(
            b.vox,
            throw.clone(),
            &t("demos.auto.throw"),
            at,
            4.0,
            &[(0.0, dry, Hold), (4.0, dry, Curve), (5.0, wet, Hold), (13.0, wet, Curve), (15.0, dry, Hold)],
        );
    }
}

/// Chorda's sustain pedal, played as a pianist does: up exactly on each chord change and
/// down again a sixteenth later (legato pedalling), so every chord rings with its own
/// strings and their sympathy, never with the last one's.
fn pedal(s: &mut Song, b: &Band) {
    let target = inst(s, b.piano, "pedal");
    let name = t("demos.auto.pedal");
    for (key, at, len) in FORM {
        let chart = chart(key);
        let cycle: f64 = chart.iter().map(|(_, l)| l).sum();
        let mut points = Vec::new();
        let mut t0 = 0.0;
        while t0 < len * 4.0 - 1e-9 {
            let mut c = t0;
            for (_, l) in chart {
                if c >= len * 4.0 - 1e-9 {
                    break;
                }
                points.push((c, 0.0, Hold));
                points.push((c + 0.25, 1.0, Hold));
                c += l;
            }
            t0 += cycle;
        }
        s.automate(b.piano, target.clone(), &name, at, len, &points);
    }
}

pub fn modulators(s: &mut Song, b: &Band) {
    // An LFO opens and closes the pad's filter over four bars.
    let pad = inst(s, b.pad, "vcf.cutoff");
    s.modulate(
        ModHost::Track(s.track_id(b.pad)),
        &t("demos.mod.breath"),
        ModSource::Lfo(LfoSource {
            shape: LfoShape::Sine,
            rate: ModRate::Sync { division: 1 },
            phase: 0.75,
            retrigger: Retrigger::Song,
            unipolar: false,
            notes: None,
        }),
        &[(pad, 0.12)],
    );
    // The kick's level ducks the bass's fader: a sidechain made of a follower.
    s.modulate(
        ModHost::Strip(b.bass.strip),
        &t("demos.mod.duck"),
        ModSource::Follower(FollowSource {
            listen: Listen::Strip(b.kick.strip),
            attack_ms: 2.0,
            release_ms: 140.0,
            gain_db: 6.0,
        }),
        &[(AutoTarget::StripGain(b.bass.strip), -0.12)],
    );
    // A sixteen-step sequence on the arpeggio's filter, and a slow random walk of its pan.
    let cutoff = inst(s, b.arp, "flt1.cutoff");
    s.modulate(
        ModHost::Track(s.track_id(b.arp)),
        &t("demos.mod.steps"),
        ModSource::Step(StepSource {
            steps: vec![
                1.0, -0.6, 0.3, -0.8, 0.7, -0.6, 0.1, -0.9, 1.0, -0.5, 0.5, -0.8, 0.8, -0.4, -0.1,
                0.4,
            ],
            division: DIV_SIXTEENTH,
            glide: 0.15,
            unipolar: false,
        }),
        &[(cutoff, 0.16)],
    );
    s.modulate(
        ModHost::Track(s.track_id(b.arp)),
        &t("demos.mod.drift"),
        ModSource::Random(RandomSource {
            rate: ModRate::Sync {
                division: cantus_dsp::modfx_clock::DIV_EIGHTH as u8,
            },
            smooth: true,
            seed: 116,
            unipolar: false,
        }),
        &[(AutoTarget::StripPan(b.arp.strip), 0.22)],
    );
}
