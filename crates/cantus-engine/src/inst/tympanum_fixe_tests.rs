//! fix-e's rules for Tympanum's unit (round 40, the review of 40a27bb7): a lane on a mix,
//! the Feel card or the bleed sets those alone — to the very bits a whole configure plays;
//! overlapping held rolls never strike early nor pile strokes on one frame; a unit owns a
//! delay line only for the outputs it was built for, and a line at 0 passes to the bit.

use super::*;
use crate::inst::{Assets, InstrumentUnit, NoteOn};
use cantus_core::inst::tympanum::{KitRef, KIT_KEY};
use cantus_core::inst::tympanum_kit::{
    self as kitdef, ArtHits, KitHits, MicTake, RecordedKit, Take, VelLayer,
};
use cantus_core::InstrumentKind;
use std::cell::RefCell;

thread_local! {
    /// Every stroke struck on this thread: (note id, key).
    pub(super) static STRUCK: RefCell<Vec<(u64, u8)>> = const { RefCell::new(Vec::new()) };
}

const FS: f32 = 48_000.0;

type Owned = Vec<(String, Option<Arc<SampleData>>)>;

fn spec(kit: &str, keys: &[(&str, f32)]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
    s.set_text(KIT_KEY, Some(kit.into())).unwrap();
    for (k, v) in keys {
        s.set(k, *v);
    }
    s
}

/// A decaying tone, `frames` long (a recorded strike).
fn tone(hz: f32, decay: f32, frames: usize) -> Arc<SampleData> {
    let x: Vec<f32> = (0..frames)
        .map(|t| {
            let ph = (t as f32 / FS * hz).fract();
            cantus_dsp::math::sin(ph * std::f32::consts::TAU)
                * cantus_dsp::math::exp(-(t as f32) / (decay * FS))
                * 0.5
        })
        .collect();
    Arc::new(SampleData {
        rate: FS as u32,
        channels: 1,
        frames: x.into(),
        stream: None,
    })
}

/// A recorded kit registered as `pack`: the studio kit's mics, a kick and a snare, each
/// strike a file per mic; its files as the unit's assets.
fn recorded(pack: &str, decay: f32) -> (String, Owned) {
    let mut def = kitdef::modelled_kit("studio").unwrap();
    def.kit = KitRef::Pack {
        pack: pack.into(),
        kit: "kit".into(),
    };
    let mics = def.mics.len();
    let pieces: [(u16, u8); 2] = [(0, 36), (1, 38)];
    let mut files = Vec::new();
    let mut assets: Owned = Vec::new();
    let mut arts = Vec::new();
    for (a, (piece, key)) in pieces.iter().enumerate() {
        let mut take = Take { mics: Vec::new() };
        for m in 0..mics {
            let i = files.len();
            files.push(format!("F:/{pack}/{a}_{m}.wav"));
            let hz = 80.0 * (1.0 + a as f32) * (1.0 + 0.11 * m as f32);
            assets.push((
                format!("t{i}"),
                Some(tone(hz, decay, (decay * 3.0 * FS) as usize)),
            ));
            take.mics.push(MicTake {
                mic: m as u8,
                file: i as u32,
                channel: None,
                gain: 1.0,
                pan: 0.0,
                offset: 0,
                cents: 0.0,
                highpass: 0.0,
            });
        }
        arts.push(ArtHits {
            piece: *piece,
            articulation: 0,
            key: *key,
            openness: None,
            layers: vec![VelLayer {
                lo: 1,
                hi: 127,
                curve: Vec::new(),
                power: None,
                takes: vec![take],
            }],
            chokes: Vec::new(),
            choke_ms: 70.0,
            polyphony: 0,
        });
    }
    let k = kitdef::register(RecordedKit {
        def,
        hits: KitHits { files, arts },
        log: Vec::new(),
    })
    .unwrap();
    cache::warm_recorded(&k).unwrap();
    (format!("pack:{pack}/kit"), assets)
}

fn hit(u: &mut Unit, key: u8, id: u64, ordinal: u32) {
    let note = NoteOn {
        key,
        velocity: 100.0 / 127.0,
        id,
        channel: 9,
    };
    u.note_on_seq(note, ordinal);
}

/// The keys a lane moves, and where it has them at frame `t`.
const LANES: [&str; 5] = ["o2.level", "amb.level", "b0.1.db", "humanize", "o1.pan"];

fn lane_values(t: usize) -> [f32; 5] {
    let x = t as f32 / 48_000.0;
    [
        -12.0 * x,
        -6.0 * x,
        -9.0 * x,
        0.3 * x,
        (x * 3.0).fract() - 0.5,
    ]
}

/// A groove through a unit, the lanes `which` moving every 16 frames (the automation's
/// grid): each control point through `configure_keys` (`cheap`) or a whole `configure`.
fn drive(kit: &str, assets: &Assets, cheap: bool, frames: usize, which: &[usize]) -> Vec<u32> {
    cache::warm_modelled("studio");
    let mut s = spec(kit, &[("timing", 3.0)]);
    let mut u = Unit::new(FS);
    u.configure(&s, assets);
    let keys: Vec<String> = which.iter().map(|i| LANES[*i].to_string()).collect();
    let mut out = Vec::with_capacity(frames * 2);
    let mut ord = [0u32; 128];
    for t in (0..frames).step_by(16) {
        if t.is_multiple_of(6_000) {
            for key in [36u8, 38] {
                let o = ord[key as usize];
                ord[key as usize] += 1;
                hit(&mut u, key, (t + key as usize) as u64, o);
            }
        }
        let v = lane_values(t);
        for i in which {
            s.params.insert(LANES[*i].to_string(), v[*i]);
        }
        if cheap {
            assert!(u.configure_keys(&s, assets, &keys), "a lane on {keys:?}");
        } else {
            u.configure(&s, assets);
        }
        let (mut l, mut r) = ([0.0f32; 16], [0.0f32; 16]);
        u.process(&mut l, &mut r);
        for i in 0..16 {
            out.push(l[i].to_bits());
            out.push(r[i].to_bits());
        }
    }
    out
}

#[test]
fn a_lane_on_the_mix_the_feel_or_the_bleed_sets_them_alone_to_the_bits_of_a_whole_configure() {
    let (rec, assets) = recorded("fixe-lanes", 0.4);
    for (kit, a) in [("model:studio", &[][..]), (rec.as_str(), &assets[..])] {
        for which in [&[][..], &[0usize, 1, 4], &[3], &[2], &[0, 1, 2, 3, 4]] {
            let cheap = drive(kit, a, true, 48_000, which);
            let whole = drive(kit, a, false, 48_000, which);
            assert!(cheap.iter().any(|b| *b != 0), "{kit} plays");
            if let Some(i) = cheap.iter().zip(&whole).position(|(x, y)| x != y) {
                panic!(
                    "{kit}, lanes {which:?}: the lanes' path differs from a whole configure \
                     at frame {} ({} vs {})",
                    i / 2,
                    f32::from_bits(cheap[i]),
                    f32::from_bits(whole[i])
                );
            }
        }
    }
    // A key that changes what the unit is goes the whole way.
    let mut u = Unit::new(FS);
    u.configure(&spec("model:studio", &[]), &[]);
    let s = spec("model:studio", &[("p0.tune", 2.0)]);
    assert!(!u.configure_keys(&s, &[], &["p0.tune".to_string()]));
    assert!(!u.configure_keys(&s, &[], &["o1.on".to_string()]));
}

#[test]
fn overlapping_held_rolls_never_strike_early_nor_pile_up_on_one_frame() {
    cache::warm_modelled("studio");
    let mut u = Unit::new(FS);
    u.configure(&spec("model:studio", &[("timing", 12.0)]), &[]);
    let roll = (0u8..128)
        .find(|k| u.figures[*k as usize] == Figure::Roll)
        .expect("the studio kit has a roll");
    STRUCK.with(|s| s.borrow_mut().clear());
    // Three held rolls, 10 ms apart, each up to 32 strokes (96 > the 64 the queue holds).
    let starts = [0usize, 480, 960];
    let mut at_frame: Vec<Vec<(u64, u8)>> = Vec::new();
    let frames = 96_000;
    for t in 0..frames {
        for (i, s) in starts.iter().enumerate() {
            if t == *s {
                hit(&mut u, roll, 100 + i as u64, i as u32);
            }
        }
        let (mut l, mut r) = ([0.0f32; 1], [0.0f32; 1]);
        u.process(&mut l, &mut r);
        at_frame.push(STRUCK.with(|s| std::mem::take(&mut *s.borrow_mut())));
    }
    let mut count = [0usize; 3];
    for (t, struck) in at_frame.iter().enumerate() {
        assert!(
            struck.len() <= starts.len(),
            "{} strokes at frame {t}",
            struck.len()
        );
        for (id, _) in struck {
            let i = (*id - 100) as usize;
            // Stroke k comes no sooner than 55 ms × k after its roll began.
            let k = count[i];
            let earliest = starts[i] + (55.0 * k as f32 * 0.001 * FS) as usize;
            assert!(
                t >= earliest,
                "roll {i}'s stroke {k} at {t}, before {earliest}"
            );
            count[i] += 1;
        }
    }
    assert_eq!(count, [32, 32, 32], "every stroke of every roll");
}

#[test]
fn a_unit_owns_lines_only_for_its_outputs_and_a_line_at_zero_passes_to_the_bit() {
    cache::warm_modelled("studio");
    assert_eq!(Unit::new(FS).outs.delay_mask(), 0, "no line unless asked");
    assert_eq!(Unit::with(FS, 0, 0b100).outs.delay_mask(), 0b100);
    let groove = |mut u: Unit, s: &NativeSpec| {
        u.configure(s, &[]);
        hit(&mut u, 36, 1, 0);
        hit(&mut u, 38, 2, 0);
        let (mut l, mut r) = (vec![0.0f32; 24_000], vec![0.0f32; 24_000]);
        for (a, b) in l.chunks_mut(333).zip(r.chunks_mut(333)) {
            u.process(a, b);
        }
        l.iter().chain(&r).map(|x| x.to_bits()).collect::<Vec<_>>()
    };
    let plain = spec("model:studio", &[]);
    let none = groove(Unit::new(FS), &plain);
    // A line at 0 (a lane drives o2.delay, still at 0): the same bits.
    assert_eq!(groove(Unit::with(FS, 0, 0b100), &plain), none);
    // At 2.5 ms it delays (and only that mic).
    let delayed = spec("model:studio", &[("o2.delay", 2.5)]);
    assert_ne!(groove(Unit::with(FS, 0, 0b100), &delayed), none);
}

#[test]
fn each_pad_has_its_own_id_and_a_pad_changed_moves_only_its_own_voices() {
    let kit = cache::warm_modelled("studio").unwrap();
    // Unique, non-zero, by place: Pulsus follows, limits polyphony and chokes by them.
    let ids: Vec<u16> = kit.pads.iter().map(|p| p.2.id).collect();
    assert!(
        ids.iter().enumerate().all(|(i, id)| *id == i as u16 + 1),
        "{ids:?}"
    );
    let snare = kit.pads.iter().find(|p| p.0 == 38).unwrap().1;
    // A kick rings; a lane retunes the snare mid-ring: the kick's tail stays the kick's.
    let ring = |retune: bool| {
        let mut u = Unit::new(FS);
        u.configure(&spec("model:studio", &[]), &[]);
        hit(&mut u, 36, 1, 0);
        let (mut l, mut r) = (vec![0.0f32; 24_000], vec![0.0f32; 24_000]);
        u.process(&mut l[..2_400], &mut r[..2_400]);
        if retune {
            let key = format!("p{snare}.tune");
            u.configure(&spec("model:studio", &[(key.as_str(), 3.0)]), &[]);
        }
        u.process(&mut l[2_400..], &mut r[2_400..]);
        l.iter().chain(&r).map(|x| x.to_bits()).collect::<Vec<_>>()
    };
    assert_eq!(
        ring(true),
        ring(false),
        "the snare's pad took the kick's voice"
    );
}
