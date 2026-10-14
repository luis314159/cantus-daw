//! Streaming (ADR 0050): the pool's seqlock, `halve_span`, and above all **streamed playback
//! equals in-memory playback to the bit** — at unity, transposed, across a mip level, with
//! a bend that changes level, stereo, on the parallel render — and a late block in real time
//! fades the voice out and is counted, while offline it waits.

use super::resample::{halve, halve_reach, halve_span, mip_chain};
use super::stream::{Cursor, Pool, Stream, BLOCK};
use super::*;
use std::sync::Arc;

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            ((s >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.8
        })
        .collect()
}

/// The levels of `data` (level 0 and `mips` more).
fn levels_of(data: &[f32], ch: usize, mips: usize) -> Vec<Vec<f32>> {
    let mut v = vec![data.to_vec()];
    v.extend(mip_chain(data, ch, mips, false).into_iter().take(mips));
    v
}

fn memory_source(levels: &[Vec<f32>], ch: usize) -> Source {
    let mut s = Source {
        channels: ch,
        rate: 48_000.0,
        frames: levels[0].len() / ch,
        ..Source::default()
    };
    for (k, l) in levels.iter().enumerate() {
        s.levels[k] = Some(Arc::from(l.as_slice()));
    }
    s
}

fn idle() {
    std::thread::yield_now();
}

/// A stream of these levels whose heads are `head` frames of level 0 (halved per level).
fn stream_of(levels: &[Vec<f32>], ch: usize, head: usize, id: u32) -> Arc<Stream> {
    let pool = Arc::new(Pool::new(8, idle));
    let heads = levels
        .iter()
        .enumerate()
        .map(|(k, l)| {
            let h = (head >> k).min(l.len() / ch);
            Arc::from(&l[..h * ch])
        })
        .collect();
    Arc::new(Stream::new(
        id,
        ch,
        48_000.0,
        levels[0].len() / ch,
        heads,
        pool,
    ))
}

/// Write every block of every level into the pool, as the reader would.
fn publish(st: &Stream, levels: &[Vec<f32>], blocks: Option<usize>) {
    let ch = st.channels;
    let mut slot = 0u32;
    for (k, l) in levels.iter().enumerate() {
        let n = st.blocks(k).min(blocks.unwrap_or(usize::MAX));
        for b in 0..n {
            let from = b * BLOCK * ch;
            let to = ((b + 1) * BLOCK * ch).min(l.len());
            while slot as usize >= st.pool.slots() {
                assert!(st.pool.grow());
            }
            let seq = st.pool.write(slot, &l[from..to]).unwrap();
            st.publish(k, b, slot, seq);
            slot += 1;
        }
    }
}

fn engine(src: Source, root: u8) -> Exemplar {
    let mut e = Exemplar::new(48_000.0);
    let g = Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 20.0],
        ..Globals::default()
    };
    e.set_globals(&g);
    e.put_zone(
        0,
        &ZoneParams {
            root,
            ..ZoneParams::default()
        },
        &src,
    );
    e
}

/// Play `key` for `frames`, bending at `bend_at` when asked, in blocks of `block`.
fn play(
    e: &mut Exemplar,
    key: u8,
    frames: usize,
    block: usize,
    bend: Option<(usize, f32)>,
    par: &dyn crate::par::Parallel,
) -> Vec<f32> {
    e.note_on(key, 0.9, 1);
    let mut out = Vec::with_capacity(frames * 2);
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    let mut done = 0;
    while done < frames {
        let mut n = block.min(frames - done);
        if let Some((at, v)) = bend {
            // The bend lands on its own sample, whatever the block size.
            if done == at {
                e.controller(2, v);
            } else if done < at && at < done + n {
                n = at - done;
            }
        }
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        e.process_par(&mut l[..n], &mut r[..n], par);
        for i in 0..n {
            out.push(l[i]);
            out.push(r[i]);
        }
        done += n;
    }
    out
}

#[test]
fn the_pool_hands_back_what_was_written_and_never_a_recycled_block() {
    let data = noise(3 * BLOCK * 2, 9);
    let levels = vec![data.clone()];
    let st = stream_of(&levels, 2, BLOCK, 1);
    publish(&st, &levels, None);
    let mut out = vec![0.0; 2 * BLOCK * 2];
    assert_eq!(st.copy(0, BLOCK / 2, &mut out), 2 * BLOCK);
    assert!(out == data[BLOCK..BLOCK * 5], "across a block edge");
    // The slot of block 1 written again for something else: block 1 is no longer there.
    let other = vec![0.25f32; BLOCK * 2];
    st.pool.write(1, &other).unwrap();
    let mut out = vec![0.0; BLOCK * 2];
    assert_eq!(
        st.copy(0, BLOCK, &mut out),
        0,
        "a recycled block is never used"
    );
    assert_eq!(st.copy(0, 0, &mut out), BLOCK, "block 0 is still good");
}

#[test]
fn halve_span_equals_halve_over_any_range() {
    let data = noise(2 * 20_000, 4);
    let whole = halve(&data, 2);
    let reach = halve_reach() as i64;
    let total = 20_000i64;
    for (first_out, n) in [(0usize, 100usize), (37, 1_000), (4_096, 4_096), (9_950, 50)] {
        let lo = (2 * first_out as i64 - reach).max(0);
        let hi = (2 * (first_out + n) as i64 + reach).min(total);
        let src = &data[lo as usize * 2..hi as usize * 2];
        let mut out = vec![0.0f32; n * 2];
        halve_span(src, lo, total, 2, first_out, &mut out);
        assert!(
            out == whole[first_out * 2..(first_out + n) * 2],
            "frames {first_out}..+{n}"
        );
    }
}

#[test]
fn streamed_playback_equals_in_memory_to_the_bit() {
    let frames = 60_000;
    for (ch, key, bend, par) in [
        (1usize, 60u8, None, false),         // unity: the samples themselves
        (2, 60, None, false),                // stereo at unity
        (2, 65, None, false),                // the resampler, level 0
        (1, 79, None, false),                // nineteen semitones up: level 1
        (2, 71, Some((20_000, 2.0)), false), // a bend that crosses into level 1
        (2, 64, None, true),                 // on the parallel render
    ] {
        let data = noise(frames * ch, 7 + key as u32);
        let levels = levels_of(&data, ch, 2);
        // The reference renders in memory, serially (parallel = serial, ADR 0038).
        let reference = {
            let mut e = engine(memory_source(&levels, ch), 60);
            play(&mut e, key, 50_000, 512, bend, &crate::par::Serial)
        };
        let st = stream_of(&levels, ch, BLOCK, 3);
        publish(&st, &levels, None);
        for block in [512usize, 37, 4_096] {
            let mut e = engine(Source::from_stream(&st), 60);
            let rp = crate::par::Reversed(4);
            let p: &dyn crate::par::Parallel = if par { &rp } else { &crate::par::Serial };
            let out = play(&mut e, key, 50_000, block, bend, p);
            assert!(reference.iter().any(|x| *x != 0.0));
            assert!(
                out == reference,
                "ch {ch} key {key} bend {bend:?} block {block}: streamed differs"
            );
            assert_eq!(e.take_late(), 0);
        }
    }
}

#[test]
fn a_late_block_in_real_time_fades_the_voice_out_and_is_counted() {
    let data = noise(60_000, 11);
    let levels = levels_of(&data, 1, 1);
    let reference = play(
        &mut engine(memory_source(&levels, 1), 60),
        60,
        40_000,
        256,
        None,
        &crate::par::Serial,
    );
    // Only the head (one block) and the next block are there.
    let st = stream_of(&levels, 1, BLOCK, 4);
    publish(&st, &levels, Some(2));
    let mut e = engine(Source::from_stream(&st), 60);
    let out = play(&mut e, 60, 40_000, 256, None, &crate::par::Serial);
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    let refl: Vec<f32> = reference.iter().step_by(2).copied().collect();
    let edge = 2 * BLOCK;
    assert!(
        left[..edge - 400] == refl[..edge - 400],
        "the same until the missing block nears"
    );
    assert!(left[edge..].iter().all(|x| *x == 0.0), "then silence");
    // The fade: the voice's own samples scaled by a falling gain, over at most 5 ms.
    let first = left.iter().zip(&refl).position(|(a, b)| a != b).unwrap();
    let last = left.iter().rposition(|x| *x != 0.0).unwrap();
    assert!(last - first <= 241, "a 5 ms fade: {}", last - first);
    for i in first..=last {
        assert!(
            left[i].abs() <= refl[i].abs() + 1e-6,
            "never louder than the voice"
        );
    }
    assert_eq!(e.take_late(), 1, "counted once");
    assert_eq!(e.active_layers(), 0);
}

#[test]
fn offline_a_voice_waits_for_its_blocks_and_plays_them() {
    let data = noise(2 * 50_000, 12);
    let levels = levels_of(&data, 2, 1);
    let reference = play(
        &mut engine(memory_source(&levels, 2), 60),
        62,
        45_000,
        512,
        None,
        &crate::par::Serial,
    );
    let st = stream_of(&levels, 2, BLOCK, 5);
    // The blocks arrive slowly, from another thread, while the voice plays.
    let writer = {
        let st = st.clone();
        let levels = levels.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            publish(&st, &levels, None);
        })
    };
    let mut e = engine(Source::from_stream(&st), 60);
    e.set_blocking(true);
    let out = play(&mut e, 62, 45_000, 512, None, &crate::par::Serial);
    writer.join().unwrap();
    assert!(out == reference, "offline equals in memory");
    assert_eq!(e.take_late(), 0);
}

#[test]
fn the_board_names_the_block_a_voice_reads_and_forgets_it_when_it_ends() {
    let data = noise(30_000, 13);
    let levels = levels_of(&data, 1, 1);
    let st = stream_of(&levels, 1, BLOCK, 6);
    publish(&st, &levels, None);
    let mut e = engine(Source::from_stream(&st), 60);
    let board = e.board().clone();
    play(&mut e, 60, 10_000, 500, None, &crate::par::Serial);
    let cell = (0..MAX_LAYERS)
        .map(|i| board.get(i))
        .find(|c| *c != 0)
        .unwrap();
    let c = Cursor::unpack(cell).unwrap();
    assert_eq!((c.stream, c.level), (6, 0));
    assert_eq!(c.block, 9_999 / BLOCK, "the block it reads now");
    assert!(
        !c.near_up && !c.near_down,
        "at unity no other level is near"
    );
    e.note_off(60, 1, 0.0);
    let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
    e.process(&mut l, &mut r);
    assert_eq!(e.active_layers(), 0);
    assert!((0..MAX_LAYERS).all(|i| board.get(i) == 0), "forgotten");
}

/// What a streamed voice costs against one in memory (`docs/round26/sampler.md`): 24
/// stereo voices transposed (the resampler) for eight seconds, every block already in the
/// pool — the window's copies are the only difference. Ignored: a measurement.
#[test]
#[ignore]
fn measure_cpu_per_voice_streamed_and_in_memory() {
    let frames = 48_000 * 9;
    let data = noise(frames * 2, 77);
    let levels = levels_of(&data, 2, 1);
    let st = stream_of(&levels, 2, 16_384, 9);
    publish(&st, &levels, None);
    let run = |src: Source, unity: bool| {
        let mut e = Exemplar::new(48_000.0);
        e.put_zone(
            0,
            &ZoneParams {
                root: 60,
                ..ZoneParams::default()
            },
            &src,
        );
        for i in 0..24u64 {
            let key = if unity { 60 } else { 55 + (i % 10) as u8 };
            e.note_on(key, 0.8, i + 1);
        }
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        let t = std::time::Instant::now();
        for _ in 0..(48_000 * 8 / 256) {
            l.fill(0.0);
            r.fill(0.0);
            e.process(&mut l, &mut r);
        }
        t.elapsed().as_secs_f64() / 24.0 / 8.0 * 100.0
    };
    // Best of fifteen, interleaved: the machine is shared.
    for unity in [false, true] {
        let (mut m, mut s) = (f64::MAX, f64::MAX);
        for _ in 0..15 {
            m = m.min(run(memory_source(&levels, 2), unity));
            s = s.min(run(Source::from_stream(&st), unity));
        }
        eprintln!(
            "{}: per voice in memory {m:.3} % of a core, streamed {s:.3} % ({:+.1} %)",
            if unity { "at unity" } else { "transposed" },
            (s / m - 1.0) * 100.0
        );
    }
}

/// Review 1 and 3: a read span wider than the window (Draft five octaves up, a 96 kHz file
/// at 48 kHz, past the top mip level, a +24-semitone bend) is never a late read: streamed
/// still equals in memory to the bit, in real time and offline, at any block size.
#[test]
fn extreme_ratios_stream_exactly_as_in_memory_and_are_never_late() {
    struct Case {
        ch: usize,
        rate: f32,
        q: Quality,
        root: u8,
        key: u8,
        bend: Option<(usize, f32)>,
        mips: usize,
    }
    let cases = [
        // Draft (linear, level 0) five octaves up: 32 frames a sample.
        Case {
            ch: 1,
            rate: 48_000.0,
            q: Quality::Draft,
            root: 36,
            key: 97,
            bend: None,
            mips: 0,
        },
        // A 96 kHz file four octaves up in Draft: about 32 frames a sample too.
        Case {
            ch: 2,
            rate: 96_000.0,
            q: Quality::Draft,
            root: 48,
            key: 96,
            bend: None,
            mips: 0,
        },
        // Past the top mip level: 80 semitones up with one level.
        Case {
            ch: 1,
            rate: 48_000.0,
            q: Quality::High,
            root: 20,
            key: 100,
            bend: None,
            mips: 1,
        },
        // A +24-semitone bend on top of a fourth.
        Case {
            ch: 2,
            rate: 48_000.0,
            q: Quality::High,
            root: 60,
            key: 65,
            bend: Some((7_000, 2.0)),
            mips: 1,
        },
    ];
    for c in cases {
        let frames = 400_000;
        let data = noise(frames * c.ch, 31 + c.key as u32);
        let levels = levels_of(&data, c.ch, c.mips);
        let mut mem = memory_source(&levels, c.ch);
        mem.rate = c.rate;
        let st = {
            let pool = Arc::new(Pool::new(8, idle));
            let heads = levels
                .iter()
                .enumerate()
                .map(|(k, l)| Arc::from(&l[..(BLOCK >> k).min(l.len() / c.ch) * c.ch]))
                .collect();
            Arc::new(Stream::new(8, c.ch, c.rate, frames, heads, pool))
        };
        publish(&st, &levels, None);
        let make = |src: Source, blocking: bool| {
            let mut e = engine(src, c.root);
            let mut g = e.globals().clone();
            g.quality = c.q;
            g.bend_range = 24.0;
            e.set_globals(&g);
            e.set_blocking(blocking);
            e
        };
        let reference = play(
            &mut make(mem.clone(), false),
            c.key,
            12_000,
            512,
            c.bend,
            &crate::par::Serial,
        );
        assert!(reference.iter().any(|x| *x != 0.0));
        for (blocking, block) in [(false, 512usize), (false, 37), (true, 512), (true, 1)] {
            let mut e = make(Source::from_stream(&st), blocking);
            let out = play(&mut e, c.key, 12_000, block, c.bend, &crate::par::Serial);
            let d = out.iter().zip(&reference).position(|(a, b)| a != b);
            assert!(
                d.is_none(),
                "{:?} key {} rate {} blocking {blocking} block {block}: first difference at {d:?}",
                c.q,
                c.key,
                c.rate
            );
            assert_eq!(e.take_late(), 0, "{:?} key {}: nothing is late", c.q, c.key);
        }
    }
}

/// Review 1: offline, a block that never comes is waited for a bounded time, not minutes —
/// then the voice fades out and is counted, as in real time.
#[test]
fn an_offline_wait_for_a_block_that_never_comes_is_bounded() {
    let data = noise(200_000, 41);
    let levels = levels_of(&data, 1, 0);
    let pool = Arc::new(Pool::new(1, idle).with_patience(std::time::Duration::from_millis(200)));
    let heads = vec![Arc::from(&levels[0][..BLOCK])];
    let st = Arc::new(Stream::new(9, 1, 48_000.0, 200_000, heads, pool));
    let mut e = engine(Source::from_stream(&st), 60);
    e.set_blocking(true);
    let t = std::time::Instant::now();
    play(&mut e, 60, 20_000, 512, None, &crate::par::Serial);
    assert!(
        t.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(e.take_late(), 1);
    assert_eq!(e.active_layers(), 0);
}

static WAKES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn count_wake() {
    WAKES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Fourth review #1: a voice at unity crossing blocks the reader already keeps ready wakes it
/// not once — the system call is not made in steady state — and one whose blocks are not
/// there wakes it, counted.
#[test]
fn a_voice_wakes_the_reader_only_when_it_is_needed() {
    let frames = 120_000;
    let data = noise(frames, 171);
    let levels = levels_of(&data, 1, 0);
    let heads = vec![Arc::from(&levels[0][..BLOCK])];
    let pool = Arc::new(Pool::new(8, idle).with_wake(count_wake));
    let st = Arc::new(Stream::new(21, 1, 48_000.0, frames, heads, pool));
    publish(&st, &levels, None);
    let mut e = engine(Source::from_stream(&st), 60);
    WAKES.store(0, std::sync::atomic::Ordering::Relaxed);
    play(&mut e, 60, 100_000, 512, None, &crate::par::Serial);
    assert_eq!(e.take_late(), 0);
    assert_eq!(e.take_wakes(), 0, "every block in the pool: no wake");
    assert_eq!(WAKES.load(std::sync::atomic::Ordering::Relaxed), 0);
    // Nothing past the head in the pool: the voice wakes the reader, and says so.
    let heads = vec![Arc::from(&levels[0][..BLOCK])];
    let pool = Arc::new(Pool::new(8, idle).with_wake(count_wake));
    let empty = Arc::new(Stream::new(22, 1, 48_000.0, frames, heads, pool));
    let mut e = engine(Source::from_stream(&empty), 60);
    play(&mut e, 60, 20_000, 512, None, &crate::par::Serial);
    let woke = e.take_wakes();
    assert!(woke > 0);
    assert_eq!(
        WAKES.load(std::sync::atomic::Ordering::Relaxed),
        woke as u64
    );
}
