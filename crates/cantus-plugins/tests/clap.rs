//! Each exported plug-in, driven through its CLAP entry point in-process, against the
//! same DSP called directly (ADR 0020).

mod common;

use cantus_core::{EffectKind, EffectSpec};
use cantus_plugins::ftz::DenormalGuard;
use cantus_plugins::params::{self, BYPASS_KEY, SIDECHAIN_KEY};
use common::*;

const N: usize = 48_000;

/// Set a plug-in's values the way a host does before it activates one.
fn configure(plug: &Plug, values: &[(&str, f32)]) {
    let events: Vec<Ev> = values
        .iter()
        .map(|(k, v)| {
            let (id, value) = clap_value(plug.kind, k, *v);
            (0, id, value)
        })
        .collect();
    plug.flush(&events);
}

/// The plug-in's output for the test signal in blocks of `blocks`.
fn through_plugin(
    kind: EffectKind,
    values: &[(&str, f32)],
    blocks: &[usize],
    n: usize,
) -> Vec<f32> {
    let plug = Plug::new(kind);
    configure(&plug, values);
    plug.activate(4096);
    let (mut l, mut r) = signal(n, 7);
    plug.run(&mut l, &mut r, None, &[], blocks);
    l.extend_from_slice(&r);
    l
}

/// The DSP's output for the test signal in one call, under the same denormal mode.
fn through_dsp(spec: &EffectSpec, n: usize, ftz: bool) -> Vec<f32> {
    let _guard = ftz.then(DenormalGuard::new);
    let mut d = Direct::new(spec);
    let (mut l, mut r) = signal(n, 7);
    d.process(&mut l, &mut r, None);
    l.extend_from_slice(&r);
    l
}

#[test]
fn the_factory_lists_the_seven() {
    let f = factory();
    unsafe {
        assert_eq!(
            f.get_plugin_count.unwrap()(f),
            cantus_plugins::desc::ALL.len() as u32
        );
        for (i, d) in cantus_plugins::desc::ALL.iter().enumerate() {
            let cd = &*f.get_plugin_descriptor.unwrap()(f, i as u32);
            assert_eq!(std::ffi::CStr::from_ptr(cd.id).to_str().unwrap(), d.clap_id);
            assert_eq!(
                std::ffi::CStr::from_ptr(cd.name).to_str().unwrap(),
                d.full_name
            );
        }
    }
}

#[test]
fn through_clap_is_bit_identical_to_the_dsp() {
    for kind in kinds() {
        let values = settings(kind);
        let spec = spec_with(kind, &values);
        let plugin = through_plugin(kind, &values, &[512], N);
        let dsp = through_dsp(&spec, N, true);
        assert_identical(&format!("{kind:?} against cantus-dsp"), &plugin, &dsp);
        assert!(plugin.iter().all(|x| x.is_finite()), "{kind:?} is finite");
        // And it did something: the comparison is not of two wires — but for Speculum,
        // whose whole point is to be one (ADR 0056).
        let (mut dry, dry_r) = signal(N, 7);
        dry.extend_from_slice(&dry_r);
        let moved = plugin.iter().zip(&dry).filter(|(a, b)| a != b).count();
        if kind == EffectKind::Analyzer {
            assert_eq!(moved, 0, "Speculum is a pass-through");
        } else {
            assert!(moved > N / 10, "{kind:?} changed only {moved} samples");
        }
    }
}

#[test]
fn through_clap_is_bit_identical_to_the_engine_without_ftz_too() {
    // The DSPs flush their own state in software, so the hardware flags change nothing
    // on this material: the plug-in nulls against Cantus's engine, which does not set
    // them.
    for kind in kinds() {
        let values = settings(kind);
        let spec = spec_with(kind, &values);
        let plugin = through_plugin(kind, &values, &[512], N);
        let mut unit = engine_unit(&spec);
        let (mut l, mut r) = signal(N, 7);
        unit.process_keyed(&mut l, &mut r, None);
        l.extend_from_slice(&r);
        assert_identical(&format!("{kind:?} against the engine"), &plugin, &l);
    }
}

/// Round 31 (ADR 0071): a fresh plug-in is the DSP on its unit's starting preset — what
/// an insert in Cantus starts on — and every parameter's default, what a host resets to,
/// is still the unit's null (ADR 0018).
#[test]
fn a_fresh_plugin_is_the_dsp_on_its_starting_preset_and_its_defaults_stay_the_null() {
    for kind in kinds() {
        let plugin = through_plugin(kind, &[], &[256], 24_000);
        let dsp = through_dsp(&cantus_core::starting::starting_spec(kind), 24_000, true);
        assert_identical(&format!("{kind:?} as it starts"), &plugin, &dsp);
        let table = cantus_plugins::params::table(kind);
        for p in table
            .params
            .iter()
            .filter(|p| p.role == cantus_plugins::params::Role::Spec)
        {
            let null = kind
                .params()
                .iter()
                .find(|i| i.key == p.key)
                .map(|i| i.default);
            assert_eq!(
                Some(p.default),
                null,
                "{kind:?}: {} defaults to its null",
                p.key
            );
        }
    }
}

#[test]
fn any_block_size_is_identical() {
    for kind in kinds() {
        let values = settings(kind);
        let n = 24_000;
        let reference = through_plugin(kind, &values, &[4096], n);
        for blocks in [
            &[1][..],
            &[7],
            &[64],
            &[1000],
            &[4096],
            &[1, 7, 64, 1000, 4096, 3, 512, 2],
        ] {
            let out = through_plugin(kind, &values, blocks, n);
            assert_identical(
                &format!("{kind:?} in blocks of {blocks:?}"),
                &out,
                &reference,
            );
        }
    }
}

/// The reference for automation: the DSP set anew at the first 16-sample grid point at
/// or after each change, as the plug-in promises.
fn automated_dsp(
    kind: EffectKind,
    base: &[(&str, f32)],
    changes: &[(u32, &str, f32)],
    n: usize,
) -> Vec<f32> {
    let _guard = DenormalGuard::new();
    let mut spec = spec_with(kind, base);
    let mut d = Direct::new(&spec);
    let (mut l, mut r) = signal(n, 7);
    let mut at = 0usize;
    let mut points: Vec<usize> = changes
        .iter()
        .map(|(t, _, _)| (*t as usize).div_ceil(16) * 16)
        .collect();
    points.sort_unstable();
    points.dedup();
    for p in points {
        let p = p.min(n);
        d.process(&mut l[at..p], &mut r[at..p], None);
        at = p;
        for (t, k, v) in changes {
            if (*t as usize).div_ceil(16) * 16 == p {
                spec.set(k, *v);
            }
        }
        d.set(&spec);
    }
    d.process(&mut l[at..], &mut r[at..], None);
    l.extend_from_slice(&r);
    l
}

#[test]
fn automation_lands_on_the_grid_at_any_block_size() {
    for kind in kinds() {
        let base = settings(kind);
        // Three changes of a parameter that matters to each plug-in: off the grid, on
        // it, and near the end of a block.
        let key = match kind {
            EffectKind::Eq => "b0.gain",
            EffectKind::Compressor => "thr",
            EffectKind::Limiter => "gain",
            EffectKind::Space => "mix",
            EffectKind::Multiband => "b1.thr",
            EffectKind::DeEsser => "thr",
            EffectKind::Stereo => "width",
            EffectKind::Maximizer => "gain",
            EffectKind::Saturator => "drive",
            EffectKind::Chorus => "mix",
            EffectKind::Flanger => "mix",
            EffectKind::Phaser => "mix",
            EffectKind::Tremolo => "depth",
            EffectKind::Vibrato => "depth",
            EffectKind::RingMod => "freq",
            EffectKind::FreqShift => "shift",
            EffectKind::VolumeShaper => "depth",
            EffectKind::Clipper => "drive",
            EffectKind::Transient => "attack",
            EffectKind::TransientSplit => "t.gain",
            EffectKind::Analyzer => "avg",
            // The test signal is not a voice: the throat works on it all the same.
            EffectKind::AutoTune => "throat",
            EffectKind::Aula => "mix",       // r39:conv
            EffectKind::Delay => "feedback", // r40:delay
            EffectKind::AutoFilter => "out", // fx:contract
            EffectKind::Pedal => "out",      // fx:contract
            EffectKind::TimeShaper => "out", // fx:contract
            EffectKind::LoFi => "out",       // fx:contract
            EffectKind::Utility => "out",    // fx:contract
            EffectKind::Board => "out",      // fx3:contract
            EffectKind::PitchShift => "out", // fx3:contract
            EffectKind::Rotary => "out",     // fx3:contract
            EffectKind::Amp => "out",        // fx3:contract
            EffectKind::Vocoder => "out",    // fxa:loquax
            EffectKind::Glitch => "out",     // fxa:fractus
            EffectKind::Shimmer => "out",    // fxa:caelum
            EffectKind::ResonanceSuppressor => "depth", // fxb:lenis
            EffectKind::Ducker => "out",                // fxb:cedo
            _ => "thr",
        };
        let table = params::table(kind);
        let p = table.by_key(key).unwrap();
        let a = p.min + (p.max - p.min) * 0.25;
        let b = p.min + (p.max - p.min) * 0.75;
        let changes = [(1_003u32, p.key, a), (9_600, p.key, b), (20_479, p.key, a)];
        let n = 24_000;
        let expected = automated_dsp(kind, &base, &changes, n);
        let still = automated_dsp(kind, &base, &[], n);
        // Speculum's parameters are how it is looked at: automating one changes no sample.
        assert!(
            diff(&expected, &still).is_some() != (kind == EffectKind::Analyzer),
            "{kind:?}: automating {key} changes the sound"
        );
        let events: Vec<Ev> = changes
            .iter()
            .map(|(t, k, v)| {
                let (id, value) = clap_value(kind, k, *v);
                (*t, id, value)
            })
            .collect();
        for blocks in [&[512][..], &[1], &[7], &[1000], &[4096], &[5, 333, 64]] {
            let plug = Plug::new(kind);
            configure(&plug, &base);
            plug.activate(4096);
            let (mut l, mut r) = signal(n, 7);
            plug.run(&mut l, &mut r, None, &events, blocks);
            l.extend_from_slice(&r);
            assert_identical(
                &format!("{kind:?} automating {} in blocks of {blocks:?}", p.key),
                &l,
                &expected,
            );
        }
    }
}

#[test]
fn the_sidechain_keys_the_detector_only_when_asked() {
    let kind = EffectKind::Gate;
    let values = [("thr", -30.0), ("range", 80.0)];
    let spec = spec_with(kind, &values);
    let n = 24_000;
    let (key_l, key_r) = signal(n, 99);
    // A quiet input the key opens.
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.02).collect(),
            r.iter().map(|x| x * 0.02).collect(),
        )
    };
    for keyed in [false, true] {
        let plug = Plug::new(kind);
        configure(&plug, &values);
        if keyed {
            configure(&plug, &[(SIDECHAIN_KEY, 1.0)]);
        }
        plug.activate(4096);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        plug.run(&mut l, &mut r, Some((&key_l, &key_r)), &[], &[480]);

        let _guard = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("keyed {keyed}, right"), &r, &er);
    }
}

/// fxa:caelum — Caelum's ducking keyed by the sidechain input: a quiet input under a loud
/// key, the ducking's threshold between the two. With the plug-in's switch on the external
/// key the wet steps aside while the key plays, exactly as the DSP keyed by it; on its own
/// input the quiet input never reaches the threshold and the wet is not ducked.
#[test]
fn caelum_s_ducking_is_keyed_by_the_sidechain_input_when_asked() {
    let kind = EffectKind::Shimmer;
    let values = [
        ("mix", 0.5),
        ("decay", 2.0),
        ("shim", 0.4),
        ("duck", 24.0),
        ("dthr", -30.0),
        ("datt", 5.0),
        ("drel", 200.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let plug = Plug::new(kind);
        configure(&plug, &values);
        if keyed {
            configure(&plug, &[(SIDECHAIN_KEY, 1.0)]);
        }
        plug.activate(4096);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        plug.run(&mut l, &mut r, Some((&key_l, &key_r)), &[], &[480]);

        let _guard = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("keyed {keyed}, right"), &r, &er);
        // The wet alone: the output less the dry (at unity to half way).
        let wet: Vec<f32> = l.iter().zip(&quiet_l).map(|(o, i)| o - i).collect();
        outs.push(wet);
    }
    let energy = |x: &[f32]| x[24_000..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) < 0.1 * energy(&outs[0]),
        "the key ducks the wet: {} keyed against {} on its own input",
        energy(&outs[1]),
        energy(&outs[0])
    );
}

/// fxa:loquax — Loquax's carrier through the sidechain input: with the plug-in's switch on the
/// external key the voice speaks through the key's sound, exactly as the DSP keyed by it; on
/// its own input there is no carrier (the synthesiser plays no notes; the consonants' noise
/// and sibilance are off) — the key heard only when asked for.
#[test]
fn loquax_s_carrier_is_the_sidechain_input_when_asked() {
    let kind = EffectKind::Vocoder;
    // Off the starting preset's robot, the consonants' noise and sibilance off: only the
    // key carries.
    let values = [
        ("mix", 1.0),
        ("robot", 0.0),
        ("src", 0.0),
        ("bands", 16.0),
        ("sib", 0.0),
        ("unv", 0.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 24_000;
    let (key_l, key_r) = signal(n, 99);
    let (voice_l, voice_r) = signal(n, 3);
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let plug = Plug::new(kind);
        configure(&plug, &values);
        if keyed {
            configure(&plug, &[(SIDECHAIN_KEY, 1.0)]);
        }
        plug.activate(4096);
        let (mut l, mut r) = (voice_l.clone(), voice_r.clone());
        plug.run(&mut l, &mut r, Some((&key_l, &key_r)), &[], &[480]);

        let _guard = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (voice_l.clone(), voice_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("keyed {keyed}, right"), &r, &er);
        outs.push(l);
    }
    let energy = |x: &[f32]| x.iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) > 4.0 * energy(&outs[0]) && energy(&outs[1]) > 1.0,
        "the key carries the voice: {} against {} unkeyed",
        energy(&outs[1]),
        energy(&outs[0])
    );
}

/// fxb:vinculum — Vinculum's detector keyed by the sidechain input: a quiet input under a
/// loud key, the threshold between the two. With the plug-in's switch on the external key
/// the input is pressed down while the key plays, exactly as the DSP keyed by it; on its own
/// input the quiet input never reaches the threshold and passes untouched.
#[test]
fn vinculum_s_external_key_presses_the_input_only_when_asked() {
    let kind = EffectKind::BusCompressor;
    let values = [
        ("thr", -30.0),
        ("ratio", 2.0),
        ("att", 0.0),
        ("rel", 1.0),
        ("makeup", 0.0),
        ("out", 0.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let plug = Plug::new(kind);
        configure(&plug, &values);
        if keyed {
            configure(&plug, &[(SIDECHAIN_KEY, 1.0)]);
        }
        plug.activate(4096);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        plug.run(&mut l, &mut r, Some((&key_l, &key_r)), &[], &[480]);

        let _guard = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("Vinculum keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("Vinculum keyed {keyed}, right"), &r, &er);
        outs.push(l);
    }
    assert_identical("unkeyed: the quiet input untouched", &outs[0], &quiet_l);
    let energy = |x: &[f32]| x[4_800..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) < 0.25 * energy(&outs[0]),
        "the key presses the input: {} keyed against {} on its own input",
        energy(&outs[1]),
        energy(&outs[0])
    );
}

/// fxb:vinculum — each model through CLAP is its DSP to the bit: the VCA on Auto, the
/// opto's Limit, the vari-mu's programme position with its colour (the family's settings
/// cover one; these the other two and the VCA's Auto).
#[test]
fn vinculum_s_models_match_the_dsp() {
    let kind = EffectKind::BusCompressor;
    let models: [&[(&str, f32)]; 3] = [
        &[
            ("model", 0.0),
            ("thr", -24.0),
            ("ratio", 1.0),
            ("rel", 4.0),
            ("makeup", 4.0),
        ],
        &[
            ("model", 1.0),
            ("char", 1.0),
            ("thr", -28.0),
            ("hpf", 90.0),
            ("mix", 0.8),
        ],
        &[
            ("model", 2.0),
            ("char", 1.0),
            ("time", 5.0),
            ("colour", 1.0),
            ("thr", -20.0),
        ],
    ];
    for values in models {
        let spec = spec_with(kind, values);
        let plugin = through_plugin(kind, values, &[512], N);
        let dsp = through_dsp(&spec, N, true);
        assert_identical(&format!("Vinculum {values:?}"), &plugin, &dsp);
        let (mut dry, dry_r) = signal(N, 7);
        dry.extend_from_slice(&dry_r);
        let moved = plugin.iter().zip(&dry).filter(|(a, b)| a != b).count();
        assert!(moved > N / 10, "{values:?} changed only {moved} samples");
    }
}

#[test]
fn a_bypass_is_the_input_delayed_by_the_latency() {
    // Terminus has a lookahead: the bypass keeps it, so the host's compensation holds.
    let kind = EffectKind::Limiter;
    let values = [("gain", 12.0)];
    let plug = Plug::new(kind);
    configure(&plug, &values);
    configure(&plug, &[(BYPASS_KEY, 1.0)]);
    plug.activate(4096);
    let latency = plug.latency() as usize;
    assert!(latency > 0);
    let (in_l, in_r) = signal(N, 7);
    let (mut l, mut r) = (in_l.clone(), in_r.clone());
    plug.run(&mut l, &mut r, None, &[], &[333]);
    for i in 0..N {
        let want = if i >= latency { in_l[i - latency] } else { 0.0 };
        assert_eq!(l[i].to_bits(), want.to_bits(), "left at {i}");
        let want = if i >= latency { in_r[i - latency] } else { 0.0 };
        assert_eq!(r[i].to_bits(), want.to_bits(), "right at {i}");
    }
}

#[test]
fn state_round_trips_and_moves_between_cantus_and_the_plugin() {
    for kind in kinds() {
        let values = settings(kind);
        let a = Plug::new(kind);
        configure(&a, &values);
        let bytes = a.save();

        // Cantus reads the plug-in's state as its own EffectSpec…
        let spec: EffectSpec = serde_json::from_slice(&bytes).expect("an EffectSpec");
        let want = spec_with(kind, &values);
        for info in kind.params() {
            assert_eq!(
                spec.get(info.key),
                want.get(info.key),
                "{kind:?} {}",
                info.key
            );
        }
        // …and the plug-in reads Cantus's.
        let b = Plug::new(kind);
        assert!(b.load(&serde_json::to_vec(&want).unwrap()));
        let c = Plug::new(kind);
        assert!(c.load(&bytes));
        for i in 0..a.param_count() {
            let id = a.param_info(i).id;
            assert_eq!(a.value(id), b.value(id), "{kind:?} param {i} from Cantus");
            assert_eq!(a.value(id), c.value(id), "{kind:?} param {i} round trip");
        }
        // A user preset from the DAW (a bare map) loads too.
        let preset: std::collections::BTreeMap<String, f32> =
            values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        // A bare map's missing keys read as their defaults (the null), not as a fresh
        // instance's starting preset (ADR 0071): the same state as those values over the
        // defaults.
        let d = Plug::new(kind);
        assert!(d.load(&serde_json::to_vec(&preset).unwrap()));
        let mut over_defaults = EffectSpec::new(kind);
        for (k, v) in &values {
            over_defaults.set(k, *v);
        }
        let e = Plug::new(kind);
        assert!(e.load(&serde_json::to_vec(&over_defaults).unwrap()));
        assert_eq!(d.save(), e.save(), "{kind:?}: the preset is the same state");
        // Another kind's state is refused.
        let other = if kind == EffectKind::Gate {
            EffectKind::Eq
        } else {
            EffectKind::Gate
        };
        assert!(!d.load(&serde_json::to_vec(&EffectSpec::new(other)).unwrap()));
    }
}

#[test]
fn loading_a_state_asks_the_host_to_read_the_values_again() {
    let kind = EffectKind::Limiter;
    let a = Plug::new(kind);
    configure(&a, &settings(kind));
    let b = Plug::new(kind);
    assert_eq!(b.host.rescans.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(b.load(&a.save()));
    assert_eq!(b.host.rescans.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn the_same_state_plays_the_same() {
    let kind = EffectKind::Compressor;
    let values = settings(kind);
    let a = Plug::new(kind);
    configure(&a, &values);
    let b = Plug::new(kind);
    assert!(b.load(&a.save()));
    let mut outs = Vec::new();
    for p in [&a, &b] {
        p.activate(1024);
        let (mut l, mut r) = signal(N, 7);
        p.run(&mut l, &mut r, None, &[], &[1024]);
        outs.push(l);
    }
    assert_identical("loaded state", &outs[0], &outs[1]);
}

#[test]
fn latency_is_reported_and_a_change_restarts_the_plugin() {
    for kind in kinds() {
        let values = settings(kind);
        let plug = Plug::new(kind);
        configure(&plug, &values);
        plug.activate(512);
        let spec = spec_with(kind, &values);
        assert_eq!(
            plug.latency() as usize,
            engine_unit(&spec).latency(),
            "{kind:?}"
        );
    }
    // Aequalis to linear phase: the latency grows while it runs.
    let kind = EffectKind::Eq;
    let plug = Plug::new(kind);
    plug.activate(512);
    assert_eq!(plug.latency(), 0);
    assert_eq!(
        plug.host
            .latency_changed
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    let (id, v) = clap_value(kind, "mode", 2.0);
    let (mut l, mut r) = signal(512, 1);
    plug.process(&mut l, &mut r, None, &[(0, id, v)], false);
    assert_eq!(
        plug.host.restarts.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a restart is asked for"
    );
    plug.process(&mut l, &mut r, None, &[], false);
    assert_eq!(
        plug.host.restarts.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "once"
    );
    plug.deactivate();
    plug.activate(512);
    let want = engine_unit(&spec_with(kind, &[("mode", 2.0)])).latency();
    assert!(want > 0);
    assert_eq!(plug.latency() as usize, want);
    assert_eq!(
        plug.host
            .latency_changed
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the host is told on activation"
    );
}

#[test]
fn the_parameters_are_the_tables() {
    for kind in kinds() {
        let plug = Plug::new(kind);
        let table = params::table(kind);
        assert_eq!(plug.param_count() as usize, kind.params().len() + 2);
        for (i, info) in kind.params().iter().enumerate() {
            let p = plug.param_info(i as u32);
            assert_eq!(p.id, params::param_id(info.key));
            let name = unsafe { std::ffi::CStr::from_ptr(p.name.as_ptr()) };
            assert!(
                !name.to_bytes().is_empty(),
                "{kind:?} {} has a name",
                info.key
            );
            let tp = &table.params[i];
            let (min, max, default) = tp.clap_range();
            assert_eq!(
                (p.min_value, p.max_value, p.default_value),
                (min, max, default)
            );
            // A fresh instance holds its starting preset's value (ADR 0071); the default
            // reads back as text and as a value.
            let start = tp.clamp(f64::from(
                cantus_core::starting::starting_spec(kind).get(info.key),
            ));
            let start = if tp.clap_is_position() {
                tp.to_normalized(start)
            } else {
                f64::from(start)
            };
            assert_eq!(plug.value(p.id), start, "{kind:?} {}", info.key);
            let text = plug.text(p.id, default);
            let back = plug
                .parse(p.id, &text)
                .unwrap_or_else(|| panic!("{kind:?} {}: its own text {text:?} parses", info.key));
            let tol = if info.stepped {
                0.0
            } else {
                (max - min).abs() * 0.01 + 1e-6
            };
            assert!(
                (back - default).abs() <= tol,
                "{kind:?} {}: {text} read {back}, not {default}",
                info.key
            );
        }
    }
    // What the DAW prints, the plug-in prints.
    let plug = Plug::new(EffectKind::Gate);
    let (id, v) = clap_value(EffectKind::Gate, "thr", -40.0);
    assert_eq!(plug.text(id, v), "-40.0 dB");
    let (id, v) = clap_value(EffectKind::Gate, "hpf", 2_000.0);
    assert_eq!(plug.text(id, v), "2.0 kHz");
    let (id, v) = clap_value(EffectKind::Gate, "style", 1.0);
    assert_eq!(plug.text(id, v), cantus_plugins::text::t("gate.style.1"));
}

#[test]
fn a_full_spec_is_the_same_effect_as_a_sparse_one() {
    // The plug-in keeps every key (so a change never inserts one); the DAW keeps only
    // what moved. They must be the same effect.
    for kind in kinds() {
        let sparse = EffectSpec::new(kind);
        let mut full = EffectSpec::new(kind);
        for info in kind.params() {
            full.params.insert(info.key.to_string(), info.default);
        }
        let mut a = engine_unit(&sparse);
        let mut b = engine_unit(&full);
        assert_eq!(a.latency(), b.latency(), "{kind:?}");
        let (mut l1, mut r1) = signal(24_000, 5);
        let (mut l2, mut r2) = (l1.clone(), r1.clone());
        a.process_keyed(&mut l1, &mut r1, None);
        b.process_keyed(&mut l2, &mut r2, None);
        assert_identical(&format!("{kind:?} full against sparse"), &l1, &l2);
    }
}

#[test]
fn a_bypass_fades_to_the_input_and_back_to_the_running_effect() {
    // Porta has no latency: bypassed, it is the input itself; un-bypassed, it is the
    // effect exactly as if it had never stopped, because it never did.
    let kind = EffectKind::Gate;
    let values = settings(kind);
    let (id, _) = clap_value(kind, BYPASS_KEY, 0.0);
    let n = 48_000;
    let fade = 480; // 10 ms at 48 kHz
    let (on, off) = (9_600u32, 28_800u32);
    let plug = Plug::new(kind);
    configure(&plug, &values);
    plug.activate(4096);
    let (in_l, in_r) = signal(n, 7);
    let (mut l, mut r) = (in_l.clone(), in_r.clone());
    plug.run(
        &mut l,
        &mut r,
        None,
        &[(on, id, 1.0), (off, id, 0.0)],
        &[333],
    );
    let effect = through_plugin(kind, &values, &[333], n);
    for i in 0..n {
        let (want_l, want_r, what) = if i < on as usize {
            (effect[i], effect[n + i], "the effect before")
        } else if i >= on as usize + fade + 16 && i < off as usize {
            (in_l[i], in_r[i], "the input while bypassed")
        } else if i >= off as usize + fade + 16 {
            (effect[i], effect[n + i], "the effect after")
        } else {
            continue;
        };
        assert_eq!(l[i].to_bits(), want_l.to_bits(), "left at {i}: {what}");
        assert_eq!(r[i].to_bits(), want_r.to_bits(), "right at {i}: {what}");
    }
}

#[test]
fn every_plugin_runs_at_any_sample_rate_a_host_may_ask_for() {
    // clap-validator's rates, fractional ones included: each one is either refused at
    // activation or runs, finite.
    for kind in kinds() {
        for sr in [
            8_000.0,
            22_050.0,
            44_100.0,
            48_000.0,
            88_200.0,
            96_000.0,
            192_000.0,
            384_000.0,
            768_000.0,
            1_234.567_8,
            12_345.678,
            45_678.901,
            123_456.78,
        ] {
            let plug = Plug::new(kind);
            configure(&plug, &settings(kind));
            if !plug.activate_at(sr, 1024) {
                continue;
            }
            for block in 0..5u32 {
                let (mut l, mut r) = signal(1024, 3 + block);
                plug.process(&mut l, &mut r, None, &[], false);
                assert!(
                    l.iter().chain(&r).all(|x| x.is_finite()),
                    "{kind:?} at {sr} Hz is finite"
                );
            }
            plug.deactivate();
        }
    }
}

/// fxb:lenis — Lenis's latency is its quality's frame, reported to the host before it
/// activates (Fast 255, Standard 511, Precise 1 023 samples at 48 kHz, as the engine's
/// unit); a quality changed while it runs asks the host for a restart once, and on the
/// next activation the host is told the new latency.
#[test]
fn lenis_reports_its_latency_per_quality_and_a_change_re_reports_it() {
    use std::sync::atomic::Ordering::SeqCst;
    let kind = EffectKind::ResonanceSuppressor;
    for (qual, want) in [(0.0, 255), (1.0, 511), (2.0, 1_023)] {
        let values = [("qual", qual), ("depth", 0.6)];
        let plug = Plug::new(kind);
        configure(&plug, &values);
        plug.activate(512);
        assert_eq!(plug.latency(), want, "quality {qual}");
        assert_eq!(
            engine_unit(&spec_with(kind, &values)).latency(),
            want as usize,
            "the engine's unit agrees"
        );
    }
    // Standard to Precise while it plays.
    let plug = Plug::new(kind);
    plug.activate(512);
    assert_eq!(plug.latency(), 511);
    assert_eq!(plug.host.restarts.load(SeqCst), 0);
    let (id, v) = clap_value(kind, "qual", 2.0);
    let (mut l, mut r) = signal(512, 1);
    plug.process(&mut l, &mut r, None, &[(0, id, v)], false);
    assert_eq!(plug.host.restarts.load(SeqCst), 1, "a restart is asked for");
    plug.process(&mut l, &mut r, None, &[], false);
    assert_eq!(plug.host.restarts.load(SeqCst), 1, "once");
    let told = plug.host.latency_changed.load(SeqCst);
    plug.deactivate();
    plug.activate(512);
    assert_eq!(plug.latency(), 1_023, "Precise's frame");
    assert_eq!(
        plug.host.latency_changed.load(SeqCst),
        told + 1,
        "the host is told on activation"
    );
    // And back to Fast: asked again, told again.
    let (id, v) = clap_value(kind, "qual", 0.0);
    plug.process(&mut l, &mut r, None, &[(0, id, v)], false);
    assert_eq!(plug.host.restarts.load(SeqCst), 2, "a second restart");
    plug.deactivate();
    plug.activate(512);
    assert_eq!(plug.latency(), 255);
    assert_eq!(plug.host.latency_changed.load(SeqCst), told + 2);
}

/// fxb:lenis — the delta through CLAP: what is taken away, exactly the DSP's, and exactly
/// the input delayed by the latency less what the plug-in gives without the delta.
#[test]
fn lenis_s_delta_is_what_it_takes_away() {
    let kind = EffectKind::ResonanceSuppressor;
    // The output at 0 dB (the starting preset fits it): the delta is taken before it.
    let hard = [
        ("depth", 0.9),
        ("curve", 1.0),
        ("select", 0.3),
        ("att", 1.0),
        ("out", 0.0),
    ];
    let with_delta = [hard.as_slice(), &[("delta", 1.0)]].concat();
    let wet = through_plugin(kind, &hard, &[512, 333], N);
    let delta = through_plugin(kind, &with_delta, &[512, 333], N);
    let dsp = through_dsp(&spec_with(kind, &with_delta), N, true);
    assert_identical("the delta, plug-in and DSP", &delta, &dsp);
    let latency = 511;
    let (mut dry, dry_r) = signal(N, 7);
    dry.extend_from_slice(&dry_r);
    for ch in 0..2 {
        for i in 0..N {
            let x = if i >= latency {
                dry[ch * N + i - latency]
            } else {
                0.0
            };
            let want = x - wet[ch * N + i];
            assert_eq!(
                delta[ch * N + i].to_bits(),
                want.to_bits(),
                "channel {ch}, sample {i}: the delayed input less the output"
            );
        }
    }
    let taken = delta.iter().map(|v| v * v).sum::<f32>();
    assert!(taken > 1e-3, "something is taken away: {taken}");
}

/// fxb:cedo — Cedo's sidechain trigger keyed by the plug-in's sidechain input: with the
/// switch on the external key the input is ducked while the key plays, exactly as the DSP
/// keyed by it; on its own input nothing keys it and the input passes untouched.
#[test]
fn cedo_s_external_key_ducks_the_input_only_when_asked() {
    let kind = EffectKind::Ducker;
    let values = [
        ("trig", 0.0),
        ("depth", 24.0),
        ("thr", -30.0),
        ("att", 0.5),
        ("hold", 20.0),
        ("rel", 80.0),
        ("look", 1.0),
        // The starting preset's output was fitted to +1.4 dB (`preset_tool fit`,
        // c5e3b654); this test holds the key, so the output is set at unity.
        ("out", 0.0),
    ];
    let spec = spec_with(kind, &values);
    let n = 48_000;
    let (key_l, key_r) = signal(n, 99);
    let (quiet_l, quiet_r): (Vec<f32>, Vec<f32>) = {
        let (l, r) = signal(n, 3);
        (
            l.iter().map(|x| x * 0.01).collect(),
            r.iter().map(|x| x * 0.01).collect(),
        )
    };
    let mut outs = Vec::new();
    for keyed in [false, true] {
        let plug = Plug::new(kind);
        configure(&plug, &values);
        if keyed {
            configure(&plug, &[(SIDECHAIN_KEY, 1.0)]);
        }
        plug.activate(4096);
        let (mut l, mut r) = (quiet_l.clone(), quiet_r.clone());
        plug.run(&mut l, &mut r, Some((&key_l, &key_r)), &[], &[480]);

        let _guard = DenormalGuard::new();
        let mut d = Direct::new(&spec);
        let (mut el, mut er) = (quiet_l.clone(), quiet_r.clone());
        d.process(&mut el, &mut er, keyed.then_some((&key_l[..], &key_r[..])));
        assert_identical(&format!("Cedo keyed {keyed}, left"), &l, &el);
        assert_identical(&format!("Cedo keyed {keyed}, right"), &r, &er);
        outs.push(l);
    }
    let lat = 48;
    let delayed: Vec<f32> = (0..n)
        .map(|i| if i >= lat { quiet_l[i - lat] } else { 0.0 })
        .collect();
    assert_identical(
        "unkeyed: the input untouched (1 ms late)",
        &outs[0],
        &delayed,
    );
    let energy = |x: &[f32]| x[4_800..].iter().map(|v| v * v).sum::<f32>();
    assert!(
        energy(&outs[1]) < 0.1 * energy(&outs[0]),
        "the key ducks the input: {} keyed against {}",
        energy(&outs[1]),
        energy(&outs[0])
    );
}

/// fxb:cedo — Cedo's latency (the lookahead and the linear-phase split at its floor) is
/// reported before it activates; the crossover moved by a host (automation, a drag's
/// values) never moves it — the floor in force moves it once, when it is committed (the
/// editor commits it at a crossover's release; a host restoring a session sets it), and
/// then the host is asked for one restart and told the new latency.
#[test]
fn cedo_reports_its_latency_and_a_committed_floor_re_reports_it() {
    use std::sync::atomic::Ordering::SeqCst;
    let kind = EffectKind::Ducker;
    let lows = [("band", 1.0), ("xover", 1.0), ("depth", 12.0)];
    for (extra, want) in [
        (&[][..], 1_765u32),
        (&[("look", 2.0)][..], 1_765 + 96),
        (&[("lfloor", 1.0)][..], 2_655),
        (&[("lfloor", 4.0)][..], 880),
        (&[("lfhz", 100.0)][..], 1_060),
    ] {
        let values = [&lows[..], extra].concat();
        let plug = Plug::new(kind);
        configure(&plug, &values);
        plug.activate(512);
        assert_eq!(plug.latency(), want, "{extra:?}");
        assert_eq!(
            engine_unit(&spec_with(kind, &values)).latency(),
            want as usize,
            "the engine's unit agrees: {extra:?}"
        );
    }
    let plug = Plug::new(kind);
    configure(&plug, &lows);
    plug.activate(512);
    assert_eq!(plug.latency(), 1_765);
    let (mut l, mut r) = signal(512, 1);
    // The crossover swept as a drag sends it: no latency change, no restart.
    for hz in [140.0, 200.0, 260.0, 300.0] {
        let (id, v) = clap_value(kind, "xhz", hz);
        plug.process(&mut l, &mut r, None, &[(0, id, v)], false);
    }
    assert_eq!(
        plug.host.restarts.load(SeqCst),
        0,
        "no restart while it moves"
    );
    // The floor committed (an octave under 300 Hz): one restart, the new latency told.
    let (id, v) = clap_value(kind, "lfhz", 150.0);
    plug.process(&mut l, &mut r, None, &[(0, id, v)], false);
    plug.process(&mut l, &mut r, None, &[], false);
    assert_eq!(
        plug.host.restarts.load(SeqCst),
        1,
        "one restart at the commit"
    );
    let told = plug.host.latency_changed.load(SeqCst);
    plug.deactivate();
    plug.activate(512);
    let want = cantus_dsp::cedo::band::linear_latency(150.0, 48_000.0) as u32;
    assert_eq!(plug.latency(), want, "the 150 Hz floor's kernel");
    assert_eq!(plug.host.latency_changed.load(SeqCst), told + 1);
}

/// fxb:cedo — the drawn shape is content in the state, as the project saves it: a state
/// with a shape loads it (heard on the host's beat in `transport.rs`), saves it back
/// unchanged, and a state with none plays the classic pump again.
#[test]
fn cedo_s_shape_travels_in_the_state() {
    use cantus_core::fx::cedo::Shape;
    use cantus_core::fx::lapsus::Point;
    let kind = EffectKind::Ducker;
    let values = [("trig", 0.0), ("depth", 30.0), ("thr", -30.0)];
    let mut spec = spec_with(kind, &values);
    spec.cedo = Some(Box::new(Shape {
        points: vec![Point::new(0.0, 1.0).shape(2), Point::new(0.3, 0.2)],
    }));
    let bytes = serde_json::to_vec(&spec).unwrap();
    let plug = Plug::new(kind);
    assert!(plug.load(&bytes));
    let saved: EffectSpec = serde_json::from_slice(&plug.save()).unwrap();
    assert_eq!(saved.cedo, spec.cedo, "saved back as loaded");
    let reloaded = Plug::new(kind);
    assert!(reloaded.load(&plug.save()));
    let again: EffectSpec = serde_json::from_slice(&reloaded.save()).unwrap();
    assert_eq!(again.cedo, spec.cedo, "and again");
    // A state with no shape: the classic pump (none saved).
    let plain = Plug::new(kind);
    assert!(plain.load(&serde_json::to_vec(&spec_with(kind, &values)).unwrap()));
    let s: EffectSpec = serde_json::from_slice(&plain.save()).unwrap();
    assert_eq!(s.cedo, None);
}
