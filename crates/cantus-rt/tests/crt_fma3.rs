//! The CRT's FMA3 switch (ADR 0031 R9 (2)). Process-wide, so alone in its own test binary.
//!
//! What it measures: whether std's transcendentals give different bits on this machine with
//! the CRT's FMA3 path on and off — the reason the switch exists. A difference is printed,
//! not asserted: a machine without FMA3 takes one path either way.

#![cfg(not(miri))]

use cantus_rt::fpenv::{crt_fma3_enabled, disable_crt_fma3, enable_crt_fma3_for_tests, CrtFma3};

/// A digest of std's transcendentals over a sweep, in f32 and f64.
fn digest() -> (u64, usize) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bits: u64| {
        h ^= bits;
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for i in 0..20_000 {
        let x = i as f64 * 0.000_731 - 7.3;
        let xf = x as f32;
        for v in [x.sin(), x.cos(), x.exp(), x.abs().ln_1p(), x.abs().powf(0.37), x.tanh(), x.atan()] {
            mix(v.to_bits());
        }
        for v in [xf.sin(), xf.cos(), xf.exp(), xf.abs().ln_1p(), xf.abs().powf(0.37), xf.tanh()] {
            mix(v.to_bits() as u64);
        }
    }
    (h, 20_000 * 13)
}

#[test]
fn the_switch_turns_the_fma3_path_off() {
    let before = crt_fma3_enabled();
    let r = disable_crt_fma3();
    if cfg!(all(windows, target_arch = "x86_64")) {
        assert_eq!(r, CrtFma3::Disabled);
        assert_eq!(crt_fma3_enabled(), Some(false));
        let (off, n) = digest();
        let on_now = enable_crt_fma3_for_tests();
        let (on, _) = digest();
        disable_crt_fma3();
        println!(
            "CRT FMA3 before: {before:?}; can enable: {on_now:?}; {n} results {} with the path on and off",
            if on == off { "identical" } else { "DIFFER" }
        );
        if on_now != Some(true) {
            assert_eq!(on, off, "without FMA3 there is one path");
        }
    } else {
        assert_eq!(r, CrtFma3::NotApplicable);
        assert_eq!(crt_fma3_enabled(), None);
    }
}
