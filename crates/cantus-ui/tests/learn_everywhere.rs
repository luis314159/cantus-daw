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
        // The unit at its defaults: a neutral insert (ADR 0071).
        a.settings.insert_neutral = true;
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

/// Review 2 of ADR 0062, #44, and review 3, #35: every stepped parameter (a switch, a
/// choice, a stepper, a band's lamp, a drawbar, a zone's field) that an editor draws is
/// drawn with its parameter's offer — the menu's *Learn MIDI controller* and the learn
/// mode's outline — never by a plain control that only sets the value. Exhaustive: each
/// kind's every stepped key (and the selected sampler zone's) is offered, or [`not_drawn`]
/// says why the demo state draws no control for it — a slot not in use, another band or
/// operator than the one shown, a field shown in another state. A plain control is named.
#[test]
fn every_stepped_parameter_is_drawn_with_its_offer() {
    let mut offered_keys: Vec<String> = Vec::new();
    let mut unexplained: Vec<String> = Vec::new();
    for kind in EffectKind::ALL.into_iter().filter(|k| k.has_editor()) {
        let mut a = app(cantus_ui::starter_project());
        let s0 = a.project().tracks[0].strip.unwrap();
        // The unit at its defaults: a neutral insert (ADR 0071).
        a.settings.insert_neutral = true;
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
        let spec = a.project().effect(s0, fx).unwrap().spec.clone();
        for info in kind.params().iter().filter(|i| i.stepped) {
            let target = AutoTarget::Effect {
                effect: fx,
                key: info.key.into(),
            };
            let name = format!("{kind:?}.{}", info.key);
            if offered.contains(&target) {
                offered_keys.push(name);
            } else if not_drawn(&name, &|k| spec.get(k), "").is_none() {
                unexplained.push(name);
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
        // The Edit view's cards whole (the window scrolls them) — FM's on each of its
        // detail pages.
        let pages: &[Option<cantus_ui::views::inst::fm::Detail>] = if kind == InstrumentKind::Fm {
            &[
                Some(cantus_ui::views::inst::fm::Detail::Envelope),
                Some(cantus_ui::views::inst::fm::Detail::Scaling),
                Some(cantus_ui::views::inst::fm::Detail::Oscillator),
            ]
        } else {
            &[None]
        };
        for page in pages {
            if let Some(d) = page {
                a.inst.fm.detail = *d;
            }
            probe::start();
            paint(&ctx(), egui::vec2(1400.0, 6000.0), 3, |ui| {
                cantus_ui::views::inst::edit_body_for_tests(&mut a, ui, track)
            });
            offered.extend(probe::take_offers());
            if page.is_some() {
                a.inst.view = View::Play;
                probe::start();
                paint(&ctx(), egui::vec2(1800.0, 4000.0), 3, |ui| {
                    cantus_ui::views::inst::window_in(&mut a, ui)
                });
                offered.extend(probe::take_offers());
            }
        }
        let spec = a
            .project()
            .track(track)
            .unwrap()
            .instrument
            .native()
            .unwrap()
            .clone();
        let mut keys: Vec<String> = kind
            .params()
            .iter()
            .filter(|i| i.stepped && kind.automatable(i.key))
            .map(|i| i.key.to_string())
            .collect();
        // The sampler's selected zone: its stepped fields (loop mode and direction,
        // reverse, crossfade curve, trigger, release follow, note-off mode).
        if kind == InstrumentKind::Sampler {
            if let Some(n) = a.inst.sampler.selected {
                let (all, _) = cantus_core::inst::sampler::zone_keys(&spec, n);
                keys.extend(all.into_iter().filter(|k| {
                    cantus_core::automation::range::range_of(
                        a.project(),
                        &AutoTarget::Instrument {
                            track,
                            key: k.clone(),
                        },
                    )
                    .is_some_and(|r| r.stepped)
                }));
            }
        }
        // FM's detail card shows the selected operator's pages.
        let shown = ["a", "b", "c", "d", "e", "f", "g", "h"][a.inst.fm.selected.min(7)];
        for key in keys {
            let target = AutoTarget::Instrument {
                track,
                key: key.clone(),
            };
            let name = format!("{kind:?}.{key}");
            let steps = cantus_core::automation::range::range_of(a.project(), &target)
                .map_or(0.0, |r| r.max - r.min);
            if offered.contains(&target) {
                offered_keys.push(name);
            } else if steps < 1.0 {
                // A constant (one choice): nothing to choose.
            } else if steps > 64.0 {
                // A number with its own field (a key, a frame), not a choice.
            } else if not_drawn(&name, &|k| spec.get(k), shown).is_none() {
                unexplained.push(name);
            }
        }
    }
    assert!(
        unexplained.is_empty(),
        "stepped parameters drawn without their offer: {unexplained:#?}"
    );
    let missing: Vec<&&str> = WIRED_IN_REVIEW_2
        .iter()
        .filter(|k| !offered_keys.iter().any(|o| o == **k))
        .collect();
    assert!(
        missing.is_empty(),
        "stepped parameters drawn without their offer: {missing:#?}"
    );
}

/// Why an editor's demo state draws no control for the stepped key `name` (`Kind.key`),
/// `get` reading the kind's values, `shown` the operator FM's detail card shows: `None` when
/// it does draw one (then it must be offered).
fn not_drawn(name: &str, get: &dyn Fn(&str) -> f32, shown: &str) -> Option<&'static str> {
    let (kind, key) = name.split_once('.')?;
    let (head, field) = key.split_once('.').unwrap_or((key, ""));
    // A band's, a point's, a key band's slot: `used` is its being, not a control; a slot
    // not in use draws nothing; of the slots in use, the editor shows one at a time.
    let digits = head.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let prefix = &head[..head.len() - digits.len()];
    let slotted = matches!(
        (kind, prefix),
        ("Eq", "b")
            | ("Multiband", "b")
            | ("Compressor", "sc")
            | ("Space", "d" | "e")
            | ("Saturator", "p")
            | ("VolumeShaper", "p")
    );
    if slotted && !digits.is_empty() {
        if field == "used" {
            return Some("a slot's being in use, never a control");
        }
        if get(&format!("{head}.used")) < 0.5 {
            return Some("a slot not in use draws nothing");
        }
        return match (kind, digits, field) {
            // The band shown: its slope only for a cut, its key and direction with dynamics.
            ("Eq", "0", "slope") => Some("a cut's only (the band shown is no cut)"),
            ("Eq", "0", "dir" | "key") => Some("with dynamics on only"),
            ("Eq", "0", _) => None,
            _ => Some("another band or point than the one the editor shows"),
        };
    }
    match (kind, key) {
        // Culmen's named bands: shown when the bands are split.
        ("Maximizer", k)
            if k.starts_with("lo.") || k.starts_with("mid.") || k.starts_with("hi.") =>
        {
            Some("a band of a split Culmen (one band in the demo)")
        }
        ("Maximizer", "all.on") => Some("the whole band's switch, shown when split"),
        // A synced rate's division: shown while synced.
        (_, "div") => Some("the rate shown in beats only while synced"),
        ("Eq", "lp_res") => Some("linear phase only"),
        ("Saturator", "c.sym") => Some("the curve editor's page"),
        // Drawn by `segmented_param` / `param_with` on a page the demo does not show.
        ("Analyzer", "sync" | "beats") => Some("the analyser's timing page"),
        ("AutoTune", "formant" | "trans") => Some("the scale source's page"),
        // Set by editing the engine, never a control of its own.
        ("Compressor", "custom") => Some("set by editing the detector's engine"),
        // A wavetable's frame is chosen with its table; a warp's source only while its
        // warp takes one; the additive and granular engines' fields with those engines.
        ("Wavetable", k) if k.ends_with(".frame") => Some("chosen with the table"),
        ("Wavetable", k) if k.ends_with(".src") => Some("only while its warp takes a source"),
        ("Wavetable", k) if k.ends_with(".partials") || k.ends_with(".gwin") => {
            Some("the additive or granular engine's (the demo's is classic)")
        }
        ("Analog", "vcf.mid") => Some("the SVF model's only"),
        ("Bass", k) if k.starts_with("syn.") => Some("the synth engine's (the demo plays another)"),
        ("Fm", "priority") => Some("no control in the editor (presets set it)"),
        // An envelope's stage count, sustain and loop points: set on its display (a point's
        // menu, the add-stage button), not by a control of their own.
        ("Fm", k)
            if k.starts_with("env.")
                && (k.ends_with(".n") || k.ends_with(".sus") || k.ends_with(".loop")) =>
        {
            Some("set on the envelope's display")
        }
        ("Sampler", k) if k.ends_with(".gen") => Some("the zone's source, chosen with its sound"),
        _ => None,
    }
    .or_else(|| {
        // An instrument's slot (an oscillator, an LFO, an envelope, an operator, a key
        // scaling) other than the one its editor shows.
        let first = matches!(digits, "" | "1");
        match (kind, prefix) {
            ("Wavetable", "osc" | "lfo") | ("Analog", "lfo" | "env") if !first => {
                Some("another oscillator or LFO than the one shown")
            }
            _ => None,
        }
    })
    .or_else(|| {
        let op = key
            .strip_prefix("op.")
            .or_else(|| key.strip_prefix("env."))
            .or_else(|| key.strip_prefix("ks."))
            .and_then(|r| r.split('.').next());
        (kind == "Fm" && op.is_some_and(|o| o != shown))
            .then_some("another operator than the one shown")
    })
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

/// Review 4 of ADR 0062, #7, #22, #33, #45: the instruments' choice table the controllers
/// pane reads (`cantus_ui::inst_choices`) names each choice as its editor draws it: every
/// editor painted whole (every view, FM's every detail page), and every choice it drew
/// found in the table with the same words (a short variant of a word where a narrow cell
/// draws one; Gravis's play mode draws *Strings* with its string engine only).
#[test]
fn the_choice_table_names_choices_as_the_editors_draw_them() {
    let mut compared = 0;
    for kind in InstrumentKind::ALL {
        let mut a = app(cantus_ui::starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        cantus_ui::views::inst::demo_setup(&mut a, track, kind);
        a.open_inst_editor(track);
        for view in [View::Play, View::Edit, View::Matrix] {
            a.inst.view = view;
            paint(&ctx(), egui::vec2(1800.0, 4000.0), 3, |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui)
            });
        }
        if kind == InstrumentKind::Fm {
            for d in [
                cantus_ui::views::inst::fm::Detail::Envelope,
                cantus_ui::views::inst::fm::Detail::Scaling,
                cantus_ui::views::inst::fm::Detail::Oscillator,
            ] {
                a.inst.fm.detail = d;
                paint(&ctx(), egui::vec2(1400.0, 6000.0), 3, |ui| {
                    cantus_ui::views::inst::edit_body_for_tests(&mut a, ui, track)
                });
            }
        }
        let spec = a
            .project()
            .track(track)
            .unwrap()
            .instrument
            .native()
            .unwrap()
            .clone();
        let mut keys: Vec<String> = kind.params().iter().map(|i| i.key.to_string()).collect();
        keys.extend((1..=4).map(|n| format!("mod{n}.curve")));
        if let Some(n) = a.inst.sampler.selected {
            keys.extend(cantus_core::inst::sampler::zone_keys(&spec, n).0);
        }
        for key in keys {
            let Some(drawn) = a.controllers.inst_words.get(kind, &key).cloned() else {
                continue;
            };
            let table = cantus_ui::inst_choices::words(kind, &key)
                .unwrap_or_else(|| panic!("{kind:?} {key}: drawn with names the table lacks"));
            assert!(
                drawn.len() <= table.len(),
                "{kind:?} {key}: {} drawn, {} in the table",
                drawn.len(),
                table.len()
            );
            for (d, w) in drawn.iter().zip(&table) {
                let (d, w) = (d.show(), w.show());
                assert!(
                    d == w || short_of(&w, &d),
                    "{kind:?} {key}: the editor draws {d:?}, the table says {w:?}"
                );
            }
            compared += 1;
        }
    }
    assert!(compared > 40, "{compared} choices compared");
}

/// Whether `drawn` is the short form of the table's `word`: one of the narrow cells' own
/// words (Modulus's operator table: its waves' and its frequency modes' short names).
fn short_of(word: &str, drawn: &str) -> bool {
    use cantus_ui::i18n::t;
    let pairs = [
        ("fm.wave.sine", "fm.wave.sine_short"),
        ("fm.wave.tri", "fm.wave.tri_short"),
        ("fm.wave.saw", "fm.wave.saw_short"),
        ("fm.wave.square", "fm.wave.square_short"),
        ("fm.op.ratio", "fm.freq.ratio_short"),
        ("fm.op.fixed", "fm.freq.fixed_short"),
    ];
    pairs
        .iter()
        .any(|(long, short)| t(long) == word && t(short) == drawn)
}
