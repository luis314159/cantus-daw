//! The unit: the null, block-size independence, exact rest, the switch and its crossfade,
//! the loudness match, the reader's sync and its declick, the blind buttons.

use super::reader::Sync;
use super::testkit::*;
use super::*;

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

fn reference(secs: f64, seed: u32, rms: f32) -> OwnedRef {
    let n = (secs * FS as f64) as usize;
    OwnedRef {
        left: noise(n, seed).iter().map(|v| v * rms * 1.7).collect(),
        right: noise(n, seed ^ 0x77)
            .iter()
            .map(|v| v * rms * 1.7)
            .collect(),
        id: 1,
    }
}

fn mix(secs: f64) -> (Vec<f32>, Vec<f32>) {
    let n = (secs * FS as f64) as usize;
    let a = sine(220.0, -12.0, secs, 0.0);
    let nz = noise(n, 99);
    let l: Vec<f32> = a.iter().zip(&nz).map(|(s, z)| s + 0.05 * z).collect();
    let r: Vec<f32> = a.iter().zip(&nz).map(|(s, z)| 0.8 * s - 0.05 * z).collect();
    (l, r)
}

/// The null: with no reference, whatever is asked for, the mix passes to the bit; bypassed,
/// with a reference heard, it passes to the bit; with a reference loaded and the mix heard,
/// it passes to the bit.
#[test]
fn the_mix_passes_to_the_bit_when_it_is_heard() {
    let (l, r) = mix(3.0);
    let rf = reference(4.0, 5, 0.2);
    for listen in [
        Listen::Mix,
        Listen::Reference,
        Listen::X,
        Listen::One,
        Listen::Two,
    ] {
        let s = ComparatioSettings {
            listen,
            blind_seed: 3,
            ..ComparatioSettings::default()
        };
        let mut u = Comparatio::new(FS);
        let (ol, or) = drive(&mut u, (&l, &r), None, 256, 0, &[(0, s)]);
        assert_eq!(bits(&ol), bits(&l), "{listen:?} with no reference");
        assert_eq!(bits(&or), bits(&r));
        let mut u = Comparatio::new(FS);
        let by = ComparatioSettings { bypass: true, ..s };
        let (ol, or) = drive(&mut u, (&l, &r), Some(&rf), 256, 0, &[(0, by)]);
        assert_eq!(bits(&ol), bits(&l), "{listen:?} bypassed");
        assert_eq!(bits(&or), bits(&r));
    }
    for sync in [
        Sync::Free,
        Sync::Locked { offset: -4_000 },
        Sync::Loop {
            start: 1_000,
            end: 30_000,
        },
    ] {
        let s = ComparatioSettings {
            sync,
            ..ComparatioSettings::default()
        };
        let mut u = Comparatio::new(FS);
        let (ol, or) = drive(&mut u, (&l, &r), Some(&rf), 300, 0, &[(0, s)]);
        assert_eq!(bits(&ol), bits(&l), "{sync:?}, the mix heard");
        assert_eq!(bits(&or), bits(&r));
    }
}

/// Back on the mix after the reference, once the fade is done, the mix passes to the bit.
#[test]
fn back_on_the_mix_it_passes_to_the_bit_again() {
    let (l, r) = mix(4.0);
    let rf = reference(5.0, 5, 0.2);
    let on = ComparatioSettings {
        listen: Listen::Reference,
        ..ComparatioSettings::default()
    };
    let off = ComparatioSettings::default();
    let mut u = Comparatio::new(FS);
    let (ol, _) = drive(
        &mut u,
        (&l, &r),
        Some(&rf),
        128,
        0,
        &[(0, on), (96_000, off)],
    );
    let fade = (0.030 * FS) as usize;
    let from = 96_000 + fade + 1;
    assert_eq!(bits(&ol[from..]), bits(&l[from..]));
    assert_ne!(ol[96_000 + fade / 2], l[96_000 + fade / 2], "it does fade");
}

/// Every block size gives the same output and the same readings, to the bit: 1, 37, 64,
/// 512 and 4 096 (over the internal piece), the song locked with an offset, switched to the
/// reference with the integrated match moving, then back with the short-term match and an
/// offset — at 37·4 096 samples, a boundary of every size.
#[test]
fn every_block_size_gives_the_same_bits() {
    let (l, r) = mix(7.0);
    let rf = reference(9.0, 8, 0.4);
    let base = ComparatioSettings {
        sync: Sync::Locked { offset: 1_234 },
        ..ComparatioSettings::default()
    };
    let at = 37 * 4_096;
    let changes = [
        (0, base),
        (
            at,
            ComparatioSettings {
                listen: Listen::Reference,
                ..base
            },
        ),
        (
            2 * at,
            ComparatioSettings {
                listen: Listen::Mix,
                matching: MatchMode::ShortTerm,
                offset_db: -2.0,
                ..base
            },
        ),
    ];
    let mut want: Option<(Vec<u32>, Vec<u32>, String)> = None;
    for block in [4_096usize, 512, 64, 37, 1] {
        let mut u = Comparatio::new(FS);
        let (ol, or) = drive(&mut u, (&l, &r), Some(&rf), block, 10_000, &changes);
        let got = (bits(&ol), bits(&or), format!("{:?}", u.meter()));
        match &want {
            None => want = Some(got),
            Some(w) => {
                assert!(
                    w.0 == got.0 && w.1 == got.1,
                    "block {block}: the output moved"
                );
                assert_eq!(w.2, got.2, "block {block}: the readings moved");
            }
        }
    }
    let w = want.unwrap();
    assert_ne!(w.0, bits(&l), "the reference was heard");
}

/// Exact rest: a twin that skips whenever the unit says it rests hears and reads what one
/// that processes every block hears and reads, to the bit; and it does rest — past the
/// end of a locked reference, and with the song stopped.
#[test]
fn rest_is_exact_and_reached() {
    let n = (6.0 * FS) as usize;
    let mut l = sine(110.0, -10.0, 1.0, 0.0);
    l.resize(n, 0.0);
    let r = l.clone();
    let rf = reference(1.5, 3, 0.3);
    for (listen, sync) in [
        (Listen::Mix, Sync::Locked { offset: 0 }),
        (Listen::Reference, Sync::Locked { offset: 2_000 }),
        (Listen::Mix, Sync::Free),
    ] {
        let s = ComparatioSettings {
            listen,
            sync,
            ..ComparatioSettings::default()
        };
        let mut a = Comparatio::new(FS);
        let mut b = Comparatio::new(FS);
        a.set(&s);
        b.set(&s);
        let (mut la, mut ra) = (l.clone(), r.clone());
        let (mut lb, mut rb) = (l.clone(), r.clone());
        let mut rested = 0usize;
        let block = 480;
        for start in (0..n).step_by(block) {
            let m = block.min(n - start);
            // The song plays for four seconds, then stops.
            let t = if start < 4 * FS as usize {
                playing(start as i64)
            } else {
                stopped(start as i64)
            };
            a.set_transport(&t);
            b.set_transport(&t);
            let silent_in = la[start..start + m].iter().all(|v| *v == 0.0);
            a.process(
                &mut la[start..start + m],
                &mut ra[start..start + m],
                Some(rf.audio()),
            );
            if silent_in && b.at_rest() {
                b.skip(m);
                rested += m;
                lb[start..start + m].iter_mut().for_each(|v| *v = 0.0);
                rb[start..start + m].iter_mut().for_each(|v| *v = 0.0);
            } else {
                b.process(
                    &mut lb[start..start + m],
                    &mut rb[start..start + m],
                    Some(rf.audio()),
                );
            }
            assert_eq!(
                format!("{:?}", a.meter()),
                format!("{:?}", b.meter()),
                "{listen:?} at {start}"
            );
        }
        assert_eq!(bits(&la), bits(&lb), "{listen:?} {sync:?}: the output");
        assert_eq!(bits(&ra), bits(&rb));
        eprintln!(
            "{listen:?} {sync:?}: rested {:.2} s of 6",
            rested as f32 / FS
        );
        assert!(
            rested as f32 > 1.5 * FS,
            "{listen:?} {sync:?}: it rests ({rested}; {:?})",
            b.rest_blocker()
        );
    }
}

/// The switch's click, measured: the output's energy above 2 kHz around a switch between
/// two low sines, hard (a one-sample fade) against the 30 ms equal-power crossfade.
#[test]
fn the_crossfade_does_not_click() {
    let secs = 2.0;
    let a = sine(100.0, -6.0, secs, 0.3);
    let b = sine(150.0, -6.0, secs + 1.0, 1.9);
    let rf = OwnedRef {
        left: b.clone(),
        right: b,
        id: 1,
    };
    let click = |fade_ms: f32| -> f64 {
        let base = ComparatioSettings {
            fade_ms,
            matching: MatchMode::Manual,
            sync: Sync::Locked { offset: 0 },
            ..ComparatioSettings::default()
        };
        let on = ComparatioSettings {
            listen: Listen::Reference,
            ..base
        };
        let mut u = Comparatio::new(FS);
        let sw = 48_000 + 123;
        let (ol, _) = drive(&mut u, (&a, &a), Some(&rf), 1, 0, &[(0, base), (sw, on)]);
        let mut hp = HighPass::new(2_000.0);
        let mut peak = 0f64;
        for (i, v) in ol.iter().enumerate() {
            let y = hp.tick(*v);
            if i > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        20.0 * math::log10(peak.max(1e-12))
    };
    let hard = click(1.0 / 48.0);
    let soft = click(30.0);
    eprintln!("click above 2 kHz: hard switch {hard:.1} dBFS, 30 ms crossfade {soft:.1} dBFS");
    assert!(hard > -50.0, "a hard switch clicks: {hard}");
    assert!(soft < -85.0, "the crossfade does not: {soft}");
}

/// Equal power: switching back and forth between two unrelated noises of the same power,
/// the output's power through the fades is the power outside them.
#[test]
fn the_crossfade_keeps_the_level_of_unrelated_signals() {
    let n = 1 << 19;
    let a = noise(n, 1);
    let rf = OwnedRef {
        left: noise(n + 48_000, 2),
        right: noise(n + 48_000, 3),
        id: 1,
    };
    let base = ComparatioSettings {
        fade_ms: 30.0,
        matching: MatchMode::Manual,
        sync: Sync::Locked { offset: 0 },
        ..ComparatioSettings::default()
    };
    let on = ComparatioSettings {
        listen: Listen::Reference,
        ..base
    };
    let mut changes = vec![(0, base)];
    let period = 4_800;
    for k in 1..(n / period) {
        changes.push((k * period, if k % 2 == 1 { on } else { base }));
    }
    let mut u = Comparatio::new(FS);
    let (ol, or) = drive(&mut u, (&a, &a), Some(&rf), 1_200, 0, &changes);
    let fade = (0.03 * FS) as usize;
    let (mut inside, mut ni, mut outside, mut no) = (0f64, 0usize, 0f64, 0usize);
    for i in period..n {
        let p = ol[i] as f64 * ol[i] as f64 + or[i] as f64 * or[i] as f64;
        if i % period < fade {
            inside += p;
            ni += 1;
        } else {
            outside += p;
            no += 1;
        }
    }
    let ratio = 10.0 * math::log10((inside / ni as f64) / (outside / no as f64));
    eprintln!("power through the fades against outside them: {ratio:+.3} dB");
    assert!(ratio.abs() < 0.15, "{ratio}");
}
