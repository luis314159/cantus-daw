//! TEMPORARY: why the complex units do not come to rest. Deleted before the round ends.

use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::*;

#[test]
fn why() {
    let spec = NativeSpec::new(InstrumentKind::Bass);
    let mut u = einst::bass::Unit::new(48_000.0);
    u.configure(&spec, &[]);
    let mut buf = (vec![0.0f32; 256], vec![0.0f32; 256]);
    for (i, key) in [36u8, 40, 43, 36].into_iter().enumerate() {
        u.note_on(NoteOn { key, velocity: 0.8, id: i as u64 + 1, channel: 0 });
        u.controller(Controller::PitchBend, 0.7);
        for _ in 0..40 {
            buf.0.fill(0.0);
            buf.1.fill(0.0);
            u.process(&mut buf.0, &mut buf.1);
        }
        u.note_off(key, i as u64 + 1, 0.5);
        u.controller(Controller::PitchBend, 0.0);
    }
    for b in 0..(20 * 48_000 / 256) {
        buf.0.fill(0.0);
        buf.1.fill(0.0);
        u.process(&mut buf.0, &mut buf.1);
        if b % 375 == 0 {
            eprintln!("{:.1} s: rest {} {:?}", b as f32 * 256.0 / 48_000.0, u.at_rest(), u.dsp().rest_debug());
        }
    }
}
