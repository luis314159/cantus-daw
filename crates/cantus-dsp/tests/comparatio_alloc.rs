//! Comparatio on the audio path allocates nothing (ADR 0005): every listen button, every
//! sync, every match mode, settings changed between blocks, seeks, the song starting and
//! stopping, a reference swapped and removed, the tap, the meter and the histograms, rest,
//! `skip`, `reset` — and the tonal analyser's frames, its curve, the comparison and the
//! match suggestion, and an ABX session — under a counting allocator after `new`.

use cantus_dsp::comparatio::blind::{Abx, Source};
use cantus_dsp::comparatio::match_eq::{suggest, MatchOptions};
use cantus_dsp::comparatio::reader::{RefAudio, Sync};
use cantus_dsp::comparatio::tonal::{compare, Resolution, Tonal};
use cantus_dsp::comparatio::{Comparatio, ComparatioSettings, Listen, MatchMode, RefLoudness};
use cantus_dsp::modfx_clock::Transport;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) - 0.5
        })
        .collect()
}

#[test]
fn comparatio_allocates_nothing_after_new() {
    let sr = 48_000.0;
    let mix_l = noise(8 * 48_000, 1);
    let mix_r = noise(8 * 48_000, 2);
    let ref_l = noise(6 * 48_000, 3);
    let ref_r = noise(6 * 48_000, 4);
    let other = noise(48_000, 5);
    let mut u = Comparatio::new(sr);
    let mut tm = Tonal::new(sr, Resolution::Sixth);
    let mut tr = Tonal::new(sr, Resolution::Sixth);
    let mut buf_l = vec![0.0f32; 4_096];
    let mut buf_r = vec![0.0f32; 4_096];
    let base = ComparatioSettings::default();
    let plan = [
        ComparatioSettings {
            listen: Listen::Reference,
            sync: Sync::Locked { offset: 500 },
            ..base
        },
        ComparatioSettings {
            listen: Listen::X,
            matching: MatchMode::ShortTerm,
            blind_seed: 9,
            ..base
        },
        ComparatioSettings {
            listen: Listen::One,
            sync: Sync::Loop {
                start: 1_000,
                end: 60_000,
            },
            matching: MatchMode::Manual,
            offset_db: -3.0,
            ..base
        },
        ComparatioSettings {
            listen: Listen::Two,
            sync: Sync::Free,
            ref_loudness: RefLoudness::WholeFile,
            fade_ms: 120.0,
            ..base
        },
        ComparatioSettings {
            bypass: true,
            ..base
        },
        base,
    ];
    let mut abx = Abx::new(3, 8);

    COUNTING.store(true, Ordering::Relaxed);
    let mut pos = 0usize;
    let mut song = 0i64;
    let mut sink = 0f32;
    for (k, block) in [64usize, 37, 512, 4_096, 1, 300]
        .iter()
        .cycle()
        .take(900)
        .enumerate()
    {
        let n = *block;
        if pos + n > mix_l.len() {
            pos = 0;
        }
        if k % 150 == 0 {
            u.set(&plan[(k / 150) % plan.len()]);
            u.set_whole_file_loudness(Some(-14.0));
        }
        if k == 400 {
            song = 200_000; // a seek
            u.cue(20_000);
        }
        let playing = !(600..640).contains(&k);
        u.set_transport(&Transport {
            playing,
            tempo_bpm: 120.0,
            samples: song,
        });
        buf_l[..n].copy_from_slice(&mix_l[pos..pos + n]);
        buf_r[..n].copy_from_slice(&mix_r[pos..pos + n]);
        let audio = match k {
            0..700 => Some(RefAudio {
                left: &ref_l,
                right: &ref_r,
                id: 1,
            }),
            700..800 => Some(RefAudio {
                left: &other,
                right: &other,
                id: 2,
            }),
            _ => None,
        };
        u.process_tapped(&mut buf_l[..n], &mut buf_r[..n], audio, |t| {
            tm.push(t.mix.0, t.mix.1);
            tr.push(t.reference.0, t.reference.1);
        });
        let m = u.meter();
        sink += m.gain_db + u.histograms().0[30] as f32;
        if u.at_rest() {
            u.skip(n);
        }
        if k % 97 == 0 {
            u.restart_measurement();
        }
        if !abx.finished() && k % 50 == 0 {
            let x = abx.x();
            abx.answer(if k % 100 == 0 { x } else { Source::Mix });
        }
        pos += n;
        if playing {
            song += n as i64;
        }
    }
    let d = compare(&tm.curve(), &tr.curve());
    let s = suggest(&d, &MatchOptions::default());
    sink += s.makeup_db + abx.p_value() as f32;
    u.reset();
    tm.restart();
    tr.reset();
    COUNTING.store(false, Ordering::Relaxed);
    let allocs = ALLOCS.load(Ordering::Relaxed);
    assert!(sink.is_finite());
    assert!(tm.frames() == 0 && tr.frames() == 0);
    assert_eq!(allocs, 0, "allocations on the audio path");
}
