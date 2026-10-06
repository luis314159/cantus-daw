//! TEMPORARY: why the complex units do not come to rest. Deleted before the round ends.

mod support;
use cantus_core::*;
use cantus_engine::fx;
use support::heavy::{self, Flush};

fn feed(mut f: impl FnMut(&mut [f32], &mut [f32]), blocker: impl Fn() -> Option<&'static str>) {
    let (l, r) = heavy::signal(48_000);
    let (mut a, mut b) = (l.clone(), r.clone());
    for (x, y) in a.chunks_mut(128).zip(b.chunks_mut(128)) {
        f(x, y);
    }
    let mut last = None;
    for i in 0..(30 * 48_000 / 128) {
        let (mut x, mut y) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        f(&mut x, &mut y);
        let now = blocker();
        if now != last {
            eprintln!("  at {:.2} s: {:?}", i as f32 * 128.0 / 48_000.0, now);
            last = now;
        }
        if now.is_none() {
            break;
        }
    }
}

#[test]
fn why() {
    let _f = Flush::new();
    for (name, spec) in [("default", EffectSpec::new(EffectKind::Limiter)), ("heavy", heavy::heavy(EffectKind::Limiter))] {
        eprintln!("limiter {name}");
        let mut t = cantus_dsp::terminus::Terminus::new(48_000.0);
        t.set(&fx::limiter::settings(&spec));
        t.reset();
        let t = std::cell::RefCell::new(t);
        feed(|l, r| t.borrow_mut().process(l, r, None), || t.borrow().rest_blocker());
    }
    for (name, spec) in [("default", EffectSpec::new(EffectKind::Compressor)), ("heavy", heavy::heavy(EffectKind::Compressor))] {
        eprintln!("compressor {name}");
        let mut c = cantus_dsp::Comprimo::new(48_000.0);
        c.set(&cantus_engine::comp_settings(&spec));
        c.reset();
        let c = std::cell::RefCell::new(c);
        feed(|l, r| c.borrow_mut().process(l, r, None), || c.borrow().rest_blocker());
    }
    for (name, spec) in [("default", EffectSpec::new(EffectKind::Multiband)), ("heavy", heavy::heavy(EffectKind::Multiband))] {
        eprintln!("multiband {name}");
        let mut m = cantus_dsp::multiplex::Multiplex::new(48_000.0);
        m.set(&fx::multiband::settings(&spec));
        m.reset();
        let m = std::cell::RefCell::new(m);
        feed(|l, r| m.borrow_mut().process(l, r, None), || m.borrow().rest_blocker());
    }
    for (name, spec) in [("default", EffectSpec::new(EffectKind::Maximizer)), ("heavy", heavy::heavy(EffectKind::Maximizer))] {
        eprintln!("maximizer {name}");
        let mut m = cantus_dsp::culmen::Culmen::new(48_000.0);
        m.set(&fx::maximizer::settings(&spec));
        m.reset();
        let m = std::cell::RefCell::new(m);
        feed(|l, r| m.borrow_mut().process(l, r), || m.borrow().rest_blocker());
    }
    {
        eprintln!("eq heavy");
        let slot = EffectSlot {
            id: EffectId(1),
            bypass: false,
            mix: 1.0,
            sidechain: None,
            midi_from: None,
            spec: heavy::heavy(EffectKind::Eq),
        };
        let _ = slot;
        let spec = heavy::heavy(EffectKind::Eq);
        let mut e = cantus_dsp::Aequalis::new(48_000.0);
        let solo = spec.get("solo").round();
        e.set_options(cantus_dsp::aequalis::Options {
            output_db: spec.get("out"),
            mode: cantus_dsp::aequalis::Mode::from_index(spec.get("mode").round() as usize),
            resolution: cantus_dsp::aequalis::Resolution::from_index(spec.get("lp_res").round() as usize),
            auto_gain: spec.get("auto") >= 0.5,
            solo: (solo >= 1.0).then(|| solo as usize - 1),
        });
        e.set(&cantus_engine::eq_bands(&spec), spec.get("out"));
        e.reset();
        let e = std::cell::RefCell::new(e);
        feed(|l, r| e.borrow_mut().process(l, r), || e.borrow().rest_blocker());
    }
}
