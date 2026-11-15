//! Exemplar's outputs (round 40 phase 3, ADR 0123 and 0125; `chorda-src`): a group's
//! output, each output on its own lane, mixed by its `o{n}.` keys, on Main or on its bus —
//! null at the defaults, and the split summing to the unsplit to the bit.

use cantus_core::inst_out::{key, OutputFamily, OutputField, OutputId, OutputSlot};
use cantus_core::{InstrumentKind, NativeSpec, StripId};
use cantus_engine::inst::{self as einst, InstrumentUnit};

/// Two built-in sounds on every key, in groups 0 and 1 (and a third group on 2 when
/// `three`), each group on the output `outs` says.
fn spec(outs: [f32; 3], three: bool) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (n, (gen, group)) in [(1.0, 0.0), (2.0, 1.0), (3.0, 2.0)]
        .into_iter()
        .take(if three { 3 } else { 2 })
        .enumerate()
    {
        s.set(&format!("z{n}.gen"), gen);
        s.set(&format!("z{n}.group"), group);
        s.set(&format!("z{n}.lo"), 0.0);
        s.set(&format!("z{n}.hi"), 127.0);
        s.set(&format!("z{n}.gain"), -6.0);
        s.set_text(&format!("z{n}.name"), Some(format!("z{n}")))
            .unwrap();
    }
    for (m, o) in outs.iter().enumerate() {
        if *o != 0.0 {
            s.set(&format!("g{m}.out"), *o);
        }
    }
    s
}

fn unit(spec: &NativeSpec) -> Box<dyn InstrumentUnit> {
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    u.configure(spec, &[]);
    for (k, id) in [(48u8, 1u64), (60, 2), (67, 3)] {
        u.note_on(einst::NoteOn {
            key: k,
            velocity: 0.8,
            id,
            channel: 0,
        });
    }
    u
}

/// `frames` in blocks of `block`: main, and each bus's sum.
fn run(u: &mut dyn InstrumentUnit, frames: usize, block: usize, buses: bool) -> Vec<Vec<f32>> {
    let mut out = vec![vec![0.0f32; 2 * frames]; 4];
    let mut at = 0;
    while at < frames {
        let n = block.min(frames - at);
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        if buses {
            let mut taps = vec![(vec![0.0f32; n], vec![0.0f32; n]); 3];
            u.process_buses(&mut l, &mut r, &mut |b, off, a, c| {
                let t = &mut taps[usize::from(b) - 1];
                for i in 0..a.len() {
                    t.0[off + i] += a[i];
                    t.1[off + i] += c[i];
                }
            });
            for (k, t) in taps.iter().enumerate() {
                for i in 0..n {
                    out[k + 1][2 * (at + i)] = t.0[i];
                    out[k + 1][2 * (at + i) + 1] = t.1[i];
                }
            }
        } else {
            u.process(&mut l, &mut r);
        }
        for i in 0..n {
            out[0][2 * (at + i)] = l[i];
            out[0][2 * (at + i) + 1] = r[i];
        }
        at += n;
    }
    out
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| (v + 0.0).to_bits()).collect()
}

#[test]
fn a_default_output_touches_no_sample() {
    // Every group on output 0, stored or not, and every `o0.` key at its default: the
    // instrument renders as it always did.
    let plain = spec([0.0; 3], true);
    let mut stored = plain.clone();
    for m in 0..3 {
        stored.params.insert(format!("g{m}.out"), 0.0);
    }
    for f in OutputField::ALL {
        let k = key(OutputId(0), f);
        let d = cantus_core::inst_out::param(&k).unwrap().default;
        stored.params.insert(k, d);
    }
    let a = run(unit(&plain).as_mut(), 9_000, 256, false);
    let b = run(unit(&stored).as_mut(), 9_000, 256, false);
    assert!(a[0].iter().any(|x| *x != 0.0));
    assert_eq!(bits(&a[0]), bits(&b[0]));
}

#[test]
fn outputs_at_unity_sum_to_what_one_output_plays_within_rounding() {
    // Three groups on three outputs, mixed at unity onto Main: the same layers, summed by
    // output instead of by layer — the same sound to within a float's rounding.
    let one = run(unit(&spec([0.0; 3], true)).as_mut(), 6_000, 512, false);
    let three = run(
        unit(&spec([0.0, 1.0, 2.0], true)).as_mut(),
        6_000,
        512,
        false,
    );
    let worst = one[0]
        .iter()
        .zip(&three[0])
        .fold(0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(worst < 1e-6, "{worst}");
}

#[test]
fn the_split_sums_to_the_unsplit_to_the_bit_at_any_block() {
    let base = spec([0.0, 1.0, 2.0], true);
    let mut split = base.clone();
    for (id, strip) in [(1u16, 11u64), (2, 12)] {
        split.outputs.push(OutputSlot {
            id: OutputId(id),
            name: format!("o{id}"),
            family: OutputFamily::Perspective,
            strip: StripId(strip),
        });
    }
    for block in [512, 333, 64] {
        let unsplit = run(unit(&base).as_mut(), 7_000, block, false);
        let parts = run(unit(&split).as_mut(), 7_000, block, true);
        // Main, then each strip in output order, as the desk adds them.
        let sum: Vec<f32> = (0..unsplit[0].len())
            .map(|i| parts[0][i] + parts[1][i] + parts[2][i])
            .collect();
        assert_eq!(bits(&sum), bits(&unsplit[0]), "block {block}");
        assert!(parts[1].iter().any(|x| *x != 0.0) && parts[2].iter().any(|x| *x != 0.0));
    }
    // And one block size renders what another does.
    let a = run(unit(&split).as_mut(), 7_000, 512, true);
    let b = run(unit(&split).as_mut(), 7_000, 97, true);
    assert_eq!(
        a.iter().map(|x| bits(x)).collect::<Vec<_>>(),
        b.iter().map(|x| bits(x)).collect::<Vec<_>>()
    );
}

#[test]
fn an_outputs_mix_moves_only_its_lane() {
    let base = spec([0.0, 1.0, 0.0], false);
    let mut quiet = base.clone();
    quiet
        .params
        .insert(key(OutputId(1), OutputField::Level), -120.0);
    let a = run(unit(&quiet).as_mut(), 4_000, 256, false);
    // Output 1 down by 120 dB is near silence: what is left is output 0 alone.
    let mut solo = base.clone();
    solo.params.insert(key(OutputId(0), OutputField::Solo), 1.0);
    let b = run(unit(&solo).as_mut(), 4_000, 256, false);
    let worst = a[0]
        .iter()
        .zip(&b[0])
        .fold(0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(worst < 1e-5, "{worst}");
    // Muting both is silence.
    let mut mute = base.clone();
    for o in 0..2 {
        mute.params.insert(key(OutputId(o), OutputField::Mute), 1.0);
    }
    let c = run(unit(&mute).as_mut(), 4_000, 256, false);
    assert!(c[0].iter().all(|x| *x == 0.0));
}
