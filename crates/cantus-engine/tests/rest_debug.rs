//! TEMPORARY: why the complex units do not come to rest. Deleted before the round ends.

mod support;
use cantus_core::*;
use cantus_engine::fx;
use support::heavy::{self, Flush};

#[test]
fn why() {
    let _f = Flush::new();
    let spec = EffectSpec::new(EffectKind::Maximizer);
    let mut m = cantus_dsp::culmen::Culmen::new(48_000.0);
    m.set(&fx::maximizer::settings(&spec));
    m.reset();
    let (l, r) = heavy::signal(48_000);
    let (mut a, mut b) = (l.clone(), r.clone());
    for x in a.iter_mut().chain(b.iter_mut()) {
        *x *= 4.0;
    }
    for (x, y) in a.chunks_mut(128).zip(b.chunks_mut(128)) {
        m.process(x, y);
    }
    for i in 0..(30 * 48_000 / 128) {
        let (mut x, mut y) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        m.process(&mut x, &mut y);
        if i % 375 == 0 {
            let (g, t, k) = m.match_debug();
            eprintln!("{:.1} s: gain {g:e} target {t:e} k {k:e} blocker {:?}", i as f32 * 128.0 / 48_000.0, m.rest_blocker());
        }
    }
}
