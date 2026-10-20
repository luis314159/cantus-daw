//! *Learn MIDI controller* on any parameter (round 29, ADR 0062): every native effect's
//! editor, every native instrument's editor in each of its views, the inspector's rows
//! (an insert's values, a send, the pinned strip's volume, pan and width), drawn headless
//! with the kit's probe on — every parameter control must offer MIDI learn in its menu (and
//! so in the learn mode). A control drawn through a path that forgot it is named here.

mod support;
use cantus_core::*;
use cantus_ui::kit::automate::probe;
use cantus_ui::views::inst::View;
use support::*;

fn unlearnable(drawn: Vec<(String, bool)>) -> Vec<String> {
    drawn
        .into_iter()
        .filter(|(_, l)| !l)
        .map(|(name, _)| name)
        .collect()
}

#[test]
fn every_native_effect_editor_offers_learn_on_every_control() {
    let mut bad = Vec::new();
    for kind in EffectKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let s0 = a.project().tracks[0].strip.unwrap();
        a.add_effect(s0, kind);
        let fx = a
            .project()
            .strip(s0)
            .unwrap()
            .effects
            .iter()
            .find(|e| e.spec.kind == kind)
            .unwrap()
            .id;
        if !kind.has_editor() {
            let track = a.project().tracks[0].id;
            a.select_track(track);
            a.inspector.show_insert_values(fx);
            probe::start();
            paint(&ctx(), egui::vec2(360.0, 1800.0), 3, |ui| {
                cantus_ui::views::inspector::show(&mut a, ui)
            });
            let drawn = probe::take_learn();
            assert!(
                !drawn.is_empty(),
                "{kind:?}: the inspector drew no parameter"
            );
            bad.extend(
                unlearnable(drawn)
                    .into_iter()
                    .map(|n| format!("{kind:?}: {n}")),
            );
            continue;
        }
        cantus_ui::views::fx::demo_setup(&mut a, s0, fx, kind);
        a.open_effect_editor(s0, fx);
        if kind == EffectKind::Eq && a.eq_editor.primary.is_none() {
            a.eq_editor.primary = Some(0);
        }
        probe::start();
        paint_editors(&mut a, egui::vec2(1600.0, 1000.0));
        let drawn = probe::take_learn();
        assert!(!drawn.is_empty(), "{kind:?}: its editor drew no parameter");
        bad.extend(
            unlearnable(drawn)
                .into_iter()
                .map(|n| format!("{kind:?}: {n}")),
        );
    }
    bad.dedup();
    assert!(
        bad.is_empty(),
        "controls without Learn MIDI controller: {bad:#?}"
    );
}

#[test]
fn every_native_instrument_editor_offers_learn_on_every_control() {
    let mut bad = Vec::new();
    for kind in InstrumentKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        a.open_inst_editor(track);
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            probe::start();
            paint(&ctx(), egui::vec2(1400.0, 900.0), 3, |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui)
            });
            let drawn = probe::take_learn();
            assert!(
                drawn.iter().any(|(_, l)| *l),
                "{kind:?} {view:?}: its editor drew no learnable parameter"
            );
            bad.extend(
                unlearnable(drawn)
                    .into_iter()
                    .map(|n| format!("{kind:?} {view:?}: {n}")),
            );
        }
    }
    bad.dedup();
    assert!(
        bad.is_empty(),
        "controls without Learn MIDI controller: {bad:#?}"
    );
}

#[test]
fn sends_and_the_pinned_strip_offer_learn() {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_strip();
    let ret = a.project().strips.last().unwrap().id;
    assert!(a.add_sends(&[s0], ret));
    let drawn_of = |a: &mut cantus_ui::CantusApp| {
        probe::start();
        paint(&ctx(), egui::vec2(360.0, 2400.0), 3, |ui| {
            cantus_ui::views::inspector::show(a, ui)
        });
        probe::take_learn()
    };
    a.select_track(track);
    let drawn = drawn_of(&mut a);
    for key in [
        "inspector.send_level",
        "kit.param.volume",
        "kit.param.pan",
        "kit.param.width",
    ] {
        assert!(
            drawn.iter().any(|(n, l)| n == key && *l),
            "the channel's {key} offers no Learn: {drawn:?}"
        );
    }
    assert!(unlearnable(drawn).is_empty());
    a.select_strip(StripId::MASTER);
    let drawn = drawn_of(&mut a);
    for key in ["kit.param.volume", "kit.param.pan", "kit.param.width"] {
        assert!(
            drawn.iter().any(|(n, l)| n == key && *l),
            "the master's {key}"
        );
    }
}

/// Chorda's tuning keys take no controller either (they would record automation).
#[test]
fn chordas_tuning_keys_are_not_learnt() {
    use cantus_core::controller::MappingTarget;
    use cantus_core::inst::chorda::DESIGN_KEYS;
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda)),
    );
    for key in DESIGN_KEYS {
        a.learn_target(MappingTarget::Param(AutoTarget::Instrument {
            track,
            key: key.to_string(),
        }));
        assert!(a.controllers.learning.is_none(), "{key} armed");
    }
}

/// Review 2 of ADR 0062, #44: the stepped parameters (switches, choices, steppers, a
/// band's lamp, a drawbar) that every native effect's and instrument's editor draws are
/// drawn with their parameter's offer — the menu's *Learn MIDI controller* and the learn
/// mode's outline — not with a plain control that only sets the value. Counted by key
/// (`probe::take_offers`: every offer asked) in each editor's demo state; the keys a state
/// shows (another band, a synced rate, a card scrolled below) are drawn by the same
/// helpers. A stepped control converted here that goes plain again is named.
#[test]
fn every_stepped_parameter_is_drawn_with_its_offer() {
    let mut offered_keys: Vec<String> = Vec::new();
    for kind in EffectKind::ALL.into_iter().filter(|k| k.has_editor()) {
        let mut a = app(cantus_ui::starter_project());
        let s0 = a.project().tracks[0].strip.unwrap();
        a.add_effect(s0, kind);
        let fx = a
            .project()
            .strip(s0)
            .unwrap()
            .effects
            .iter()
            .find(|e| e.spec.kind == kind)
            .unwrap()
            .id;
        cantus_ui::views::fx::demo_setup(&mut a, s0, fx, kind);
        a.open_effect_editor(s0, fx);
        if kind == EffectKind::Eq && a.eq_editor.primary.is_none() {
            a.eq_editor.primary = Some(0);
        }
        probe::start();
        paint_editors(&mut a, egui::vec2(1800.0, 3000.0));
        let offered = probe::take_offers();
        for info in kind.params().iter().filter(|i| i.stepped) {
            let target = AutoTarget::Effect {
                effect: fx,
                key: info.key.into(),
            };
            if offered.contains(&target) {
                offered_keys.push(format!("{kind:?}.{}", info.key));
            }
        }
    }
    for kind in InstrumentKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        a.open_inst_editor(track);
        let mut offered = Vec::new();
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            probe::start();
            paint(&ctx(), egui::vec2(1800.0, 4000.0), 3, |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui)
            });
            offered.extend(probe::take_offers());
        }
        for info in kind.params().iter().filter(|i| i.stepped) {
            let target = AutoTarget::Instrument {
                track,
                key: info.key.into(),
            };
            if offered.contains(&target) {
                offered_keys.push(format!("{kind:?}.{}", info.key));
            }
        }
    }
    let missing: Vec<&&str> = WIRED_IN_REVIEW_2
        .iter()
        .filter(|k| !offered_keys.iter().any(|o| o == **k))
        .collect();
    assert!(
        missing.is_empty(),
        "stepped parameters drawn without their offer: {missing:#?}"
    );
}

/// The stepped parameters each editor draws in its demo state, by kind and key — among
/// them every one review 2 found drawn by a plain control: each goes through its
/// parameter's offer.
const WIRED_IN_REVIEW_2: &[&str] = &[
    "Eq.b0.on",
    "Eq.b0.shape",
    "Eq.b0.place",
    "Eq.b0.dyn",
    "Eq.b0.gq",
    "Eq.mode",
    "Eq.auto",
    "Compressor.auto_rel",
    "Compressor.style",
    "Compressor.listen",
    "Compressor.auto_gain",
    "Compressor.os",
    "Compressor.key_eq",
    "Limiter.style",
    "Limiter.tp",
    "Limiter.dc",
    "Limiter.monitor",
    "Limiter.delta",
    "Limiter.dither",
    "Limiter.shape",
    "Space.style",
    "Space.sync",
    "Space.freeze",
    "Space.deq_on",
    "Space.peq_on",
    "Space.peq_auto",
    "Space.d1.on",
    "Space.d1.shape",
    "Multiband.b2.on",
    "Multiband.b2.solo",
    "Multiband.b2.mute",
    "Multiband.b2.place",
    "Multiband.b2.key",
    "Multiband.b2.key_free",
    "Multiband.mode",
    "Multiband.slope",
    "Multiband.listen",
    "DeEsser.mode",
    "DeEsser.proc",
    "DeEsser.look",
    "DeEsser.stereo",
    "DeEsser.listen",
    "Gate.style",
    "Gate.key_slope",
    "Gate.listen",
    "Gate.trig",
    "Saturator.model",
    "Saturator.boost",
    "Saturator.os",
    "Saturator.phase",
    "Saturator.chan",
    "Saturator.auto",
    "Stereo.swap",
    "Stereo.inv_l",
    "Stereo.inv_r",
    "Stereo.listen",
    "Stereo.mono_slope",
    "Stereo.tone_slope",
    "Stereo.haas_mode",
    "Maximizer.style",
    "Maximizer.tp",
    "Maximizer.dc",
    "Maximizer.bands",
    "Maximizer.xmode",
    "Maximizer.slope",
    "Maximizer.delta",
    "Maximizer.monitor",
    "Maximizer.dither",
    "Maximizer.shape",
    "Maximizer.all.place",
    "Chorus.mode",
    "Chorus.voices",
    "Chorus.sync",
    "Chorus.shape",
    "Flanger.mode",
    "Flanger.tz",
    "Flanger.invert",
    "Flanger.sync",
    "Flanger.shape",
    "Phaser.stages",
    "Phaser.sync",
    "Phaser.shape",
    "Tremolo.mode",
    "Tremolo.sync",
    "Tremolo.div",
    "Tremolo.shape",
    "Vibrato.sync",
    "Vibrato.shape",
    "RingMod.wave",
    "RingMod.sync",
    "RingMod.shape",
    "FreqShift.side",
    "FreqShift.sync",
    "FreqShift.shape",
    "VolumeShaper.div",
    "VolumeShaper.p0.s",
    "Clipper.curve",
    "Clipper.os",
    "Clipper.tp",
    "Clipper.match",
    "Transient.band",
    "TransientSplit.mode",
    "TransientSplit.listen",
    "Analyzer.view",
    "Analyzer.fft",
    "Analyzer.hold",
    "Analyzer.chan",
    "Analyzer.floor",
    "AutoTune.src",
    "AutoTune.key",
    "AutoTune.scale",
    "AutoTune.n0",
    "AutoTune.n1",
    "AutoTune.n2",
    "AutoTune.n3",
    "AutoTune.n4",
    "AutoTune.n5",
    "AutoTune.n6",
    "AutoTune.n7",
    "AutoTune.n8",
    "AutoTune.n9",
    "AutoTune.n10",
    "AutoTune.n11",
    "Wavetable.quality",
    "Wavetable.voice.poly",
    "Wavetable.voice.mode",
    "Wavetable.voice.prio",
    "Wavetable.voice.glide_mode",
    "Wavetable.voice.bend",
    "Wavetable.voice.seed",
    "Wavetable.osc1.on",
    "Wavetable.osc1.oct",
    "Wavetable.osc1.semi",
    "Wavetable.osc1.route",
    "Wavetable.osc1.uni",
    "Wavetable.osc1.ucurve",
    "Wavetable.osc1.stack",
    "Wavetable.osc1.warp1",
    "Wavetable.osc1.warp2",
    "Wavetable.osc1.engine",
    "Wavetable.osc2.on",
    "Wavetable.osc2.oct",
    "Wavetable.osc2.semi",
    "Wavetable.osc2.route",
    "Wavetable.osc2.uni",
    "Wavetable.osc3.on",
    "Wavetable.osc3.oct",
    "Wavetable.osc3.semi",
    "Wavetable.osc3.route",
    "Wavetable.osc3.uni",
    "Wavetable.sub.on",
    "Wavetable.sub.wave",
    "Wavetable.sub.oct",
    "Wavetable.sub.route",
    "Wavetable.noise.on",
    "Wavetable.noise.route",
    "Wavetable.flt1.on",
    "Wavetable.flt1.type",
    "Wavetable.flt2.on",
    "Wavetable.flt2.type",
    "Wavetable.lfo1.wave",
    "Wavetable.lfo1.mode",
    "Analog.voice.stack",
    "Analog.osc1.octave",
    "Analog.osc1.semi",
    "Analog.osc1.rev",
    "Analog.osc2.octave",
    "Analog.osc2.semi",
    "Analog.osc2.rev",
    "Analog.osc2.sync",
    "Analog.osc3.octave",
    "Analog.osc3.semi",
    "Analog.osc3.rev",
    "Analog.osc3.sync",
    "Analog.osc3.low",
    "Analog.osc3.kbd",
    "Analog.sub.shape",
    "Analog.noise.color",
    "Analog.hpf.pos",
    "Analog.vcf.model",
    "Analog.vcf.slope",
    "Analog.vcf.rev",
    "Analog.vca.source",
    "Analog.quality",
    "Analog.chorus.mode",
    "Fm.voices",
    "Fm.mode",
    "Fm.unison",
    "Fm.transpose",
    "Fm.pb_range",
    "Fm.quality",
    "Fm.guard",
    "Fm.fb_avg",
    "Fm.lfo1.wave",
    "Fm.lfo1.sync",
    "Fm.lfo2.wave",
    "Fm.lfo2.sync",
    "Fm.op.a.on",
    "Fm.op.a.wave",
    "Fm.op.a.freq_mode",
    "Fm.op.b.on",
    "Fm.op.b.wave",
    "Fm.op.b.freq_mode",
    "Fm.env.b.mode",
    "Fm.op.c.on",
    "Fm.op.c.wave",
    "Fm.op.c.freq_mode",
    "Fm.op.d.on",
    "Fm.op.d.wave",
    "Fm.op.d.freq_mode",
    "Fm.op.e.on",
    "Fm.op.e.wave",
    "Fm.op.e.freq_mode",
    "Fm.op.f.on",
    "Fm.op.f.wave",
    "Fm.op.f.freq_mode",
    "Fm.op.g.on",
    "Fm.op.g.wave",
    "Fm.op.g.freq_mode",
    "Fm.op.h.on",
    "Fm.op.h.wave",
    "Fm.op.h.freq_mode",
    "Sampler.transpose",
    "Sampler.voices",
    "Sampler.mode",
    "Sampler.bend",
    "Sampler.quality",
    "Sampler.rr.order",
    "Sampler.kit",
    "Sampler.filter.type",
    "Sampler.lfo1.shape",
    "Sampler.lfo2.shape",
    "Sampler.sw.lo",
    "Sampler.sw.hi",
    "Sampler.sw.default",
    "Bass.engine",
    "Bass.mode",
    "Bass.str.strings",
    "Bass.str.tuning",
    "Bass.str.frets",
    "Bass.str.fretless",
    "Bass.str.type",
    "Bass.str.technique",
    "Bass.str.pop_vel",
    "Bass.str.fingering",
    "Bass.str.pu_type",
    "Bass.drive.type",
    "ElectricPiano.engine",
    "ElectricPiano.range",
    "ElectricPiano.output",
    "ElectricPiano.trem.mode",
    "ElectricPiano.polyphony",
    "Organ.u1",
    "Organ.u2",
    "Organ.u3",
    "Organ.u4",
    "Organ.u5",
    "Organ.u6",
    "Organ.u7",
    "Organ.u8",
    "Organ.u9",
    "Organ.l1",
    "Organ.l2",
    "Organ.l3",
    "Organ.l4",
    "Organ.l5",
    "Organ.l6",
    "Organ.l7",
    "Organ.l8",
    "Organ.l9",
    "Organ.p1",
    "Organ.p2",
    "Organ.perc.harm",
    "Organ.perc.decay",
    "Organ.perc.vol",
    "Organ.perc.steal",
    "Organ.vib",
    "Organ.vib.upper",
    "Organ.vib.lower",
    "Organ.tuning",
    "Organ.rot.speed",
    "Organ.rot.cab",
];
