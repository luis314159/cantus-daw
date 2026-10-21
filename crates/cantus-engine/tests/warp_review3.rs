//! The warp engine under what round 29's third review found (ADR 0061 §12): a swap's
//! crossfade once per swap — never again after a wrap or a seek, kept across installs that
//! come together, equal in gain, and for a render laid again under an edited tempo; a
//! render never played on another project's clip, a following warp's input never laid
//! where its warp does not put it, a fallback laid where its content begins, a transposed
//! window's render kept through a tempo edit's re-anchoring, and a fallback kept while it
//! plays.

use cantus_core::snapshot::FadeFrom;
use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpMarker};
use cantus_core::*;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

fn write_wav(path: &Path, x: &[f32], rate: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in x {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// A sine of `hz` for `seconds` at `rate`.
fn sine(rate: u32, hz: f64, seconds: f64) -> Vec<f32> {
    (0..(rate as f64 * seconds) as usize)
        .map(|i| {
            (0.5 * cantus_dsp::math::sin(2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64))
                as f32
        })
        .collect()
}

/// A project at 120 BPM with one clip of `path` at `start`, `offset` in, `length` long.
fn project(
    path: &Path,
    rate: u32,
    frames: usize,
    start: Ticks,
    offset: Ticks,
    length: Ticks,
    warp: Option<Warp>,
) -> Project {
    let mut p = Project::new("warp");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Loop");
    t.strip = None;
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "loop".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: rate,
        channels: 1,
        frames: frames as u64,
        sha256: None,
        recorded: false,
    });
    let mut c = p.make_audio_clip("loop", id, start, length);
    c.offset = offset;
    c.audio.as_mut().unwrap().warp = warp.map(Box::new);
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn point(tick: Ticks, bpm: f64) -> TempoPoint {
    TempoPoint {
        tick,
        bpm,
        ramp: false,
    }
}

/// A transposed warp that does not follow the tempo.
fn transposed(rate: u32) -> Warp {
    let mut w = Warp::new(rate, 120.0);
    w.follow = false;
    w.cents = 200;
    w
}

/// A following warp at 97 BPM with a marker a bar apart: it moves the audio.
fn at_97(rate: u32) -> Warp {
    let mut w = Warp::new(rate, 97.0);
    let fpt = rate as f64 * 60.0 / 97.0 / PPQ as f64;
    for b in 0..40 {
        let t = Ticks::beats(4 * b);
        w.markers.push(WarpMarker::at(t.0 as f64 * fpt, t));
    }
    w
}

/// A background cache (renders on the service's workers).
fn live_cache() -> SampleCache {
    let mut c = SampleCache::new();
    c.set_background(true);
    c.set_pitch_background(true);
    c
}

/// Attach `p` until its source is loaded (a background cache).
fn attach_loaded(cache: &mut SampleCache, p: &Project) -> Snapshot {
    let t = std::time::Instant::now();
    loop {
        cache.poll_loaded();
        let mut s = Snapshot::compile(p, 1);
        cache.attach_audio(&mut s, 48_000);
        if s.tracks[0].audio.iter().all(|c| c.data.is_some()) || t.elapsed().as_secs() > 30 {
            return s;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Wait for every final render on its way.
fn wait_finals(cache: &mut SampleCache) {
    let t = std::time::Instant::now();
    while cache.warp_pending() > 0 && t.elapsed().as_secs() < 60 {
        let _ = cache.wait_warp(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn arc(v: &[f32]) -> Arc<SampleData> {
    Arc::new(SampleData {
        stream: None,
        rate: 48_000,
        channels: 1,
        frames: v.to_vec().into(),
    })
}

/// A processor at 48 kHz and the queue that feeds it.
fn processor() -> (rtrb::Producer<EngineMessage>, Processor) {
    let (tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    std::mem::forget(_rx);
    (tx, Processor::without_midi(48_000.0, rx_engine, tx_engine))
}

/// Play `blocks` blocks of `n` frames, the left channel appended to `out`.
fn play(proc: &mut Processor, blocks: usize, n: usize, out: &mut Vec<f32>) {
    let mut buf = vec![0f32; n * 2];
    for _ in 0..blocks {
        proc.process(&mut buf, 2);
        out.extend(buf.chunks_exact(2).map(|f| f[0]));
    }
}

/// The largest step between two samples from `from` on.
fn steepest(x: &[f32], from: usize) -> f32 {
    x.windows(2)
        .skip(from)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max)
}

/// A snapshot of a clip of the 220 Hz tone at 0 (eight beats), its data `data` laid on the
/// window's start, looping beats 0–4 when `looped` (two seconds: 440 whole cycles), with a
/// swap from `from` as swap `id`.
fn tone_snapshot(
    data: &Arc<SampleData>,
    from: Option<(&Arc<SampleData>, u64)>,
    looped: bool,
) -> Arc<Snapshot> {
    let p = project(
        Path::new("C:/nowhere/tone.wav"),
        48_000,
        48_000 * 4,
        Ticks::ZERO,
        Ticks::ZERO,
        Ticks::beats(8),
        None,
    );
    let mut s = Snapshot::compile(&p, 1);
    if looped {
        s.loop_region = Some(LoopRegion {
            start: Ticks::ZERO,
            end: Ticks::beats(4),
        });
        s.loop_enabled = true;
    }
    let c = &mut s.tracks[0].audio[0];
    c.data = Some(data.clone());
    c.zero_at = Some((Ticks::ZERO, 0.0));
    if let Some((d, id)) = from {
        c.fade_from = Some(FadeFrom {
            data: d.clone(),
            id,
        });
        s.swaps = vec![(0, 0)];
    }
    Arc::new(s)
}

/// Findings 18, 11, 27 (the high one): a swap's crossfade plays once — after the loop wraps
/// and after a seek back, the counter passing the swap's sample again, the output is the
/// one a snapshot without the swap gives, to the bit.
#[test]
fn a_swap_fades_once_never_again_after_a_wrap_or_a_seek() {
    let x = sine(48_000, 220.0, 4.0);
    let turned: Vec<f32> = x.iter().map(|v| -v).collect();
    let (x, turned) = (arc(&x), arc(&turned));
    let run = |fade: bool| {
        let (mut tx, mut proc) = processor();
        tx.push(EngineMessage::SetSnapshot(tone_snapshot(&x, None, true)))
            .unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut out = Vec::new();
        play(&mut proc, 20, 256, &mut out);
        let from = fade.then_some((&x, 1));
        tx.push(EngineMessage::SetSnapshot(tone_snapshot(
            &turned, from, true,
        )))
        .unwrap();
        // Through the swap, the loop's wrap (two seconds) and the swap's sample again.
        play(&mut proc, 420, 256, &mut out);
        tx.push(EngineMessage::Seek(Ticks::ZERO)).unwrap();
        play(&mut proc, 40, 256, &mut out);
        out
    };
    let (faded, plain) = (run(true), run(false));
    let (swap, seek) = (20 * 256, 440 * 256);
    let tone = steepest(&plain[..swap], 100);
    assert!(
        steepest(&plain[..swap + 300], 100) > 4.0 * tone,
        "the swap without a fade clicks"
    );
    // Up to the seek (which jumps the tone's phase): the swap and the wrap are smooth.
    let step = steepest(&faded[..seek], 100);
    assert!(
        step <= 2.0 * tone,
        "a step of {step} (the tone's own {tone})"
    );
    let n = (0.005f64 * 48_000.0).round() as usize;
    let after = swap + n;
    let differ = faded[after..]
        .iter()
        .zip(&plain[after..])
        .position(|(a, b)| a != b);
    assert_eq!(differ, None, "the fade came back after the swap");
}

/// Finding 19: two installs in one callback — the swap's snapshot and the next, carrying
/// the same swap — still fade; and the same swap installed again mid-fade (a copy with the
/// previous zones) never starts it over.
#[test]
fn a_swap_carried_by_the_next_installs_fades_once() {
    let x = sine(48_000, 220.0, 4.0);
    let turned: Vec<f32> = x.iter().map(|v| -v).collect();
    let (x, turned) = (arc(&x), arc(&turned));
    let (mut tx, mut proc) = processor();
    tx.push(EngineMessage::SetSnapshot(tone_snapshot(&x, None, false)))
        .unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    play(&mut proc, 20, 256, &mut out);
    let tone = steepest(&out, 100);
    tx.push(EngineMessage::SetSnapshot(tone_snapshot(
        &turned,
        Some((&x, 1)),
        false,
    )))
    .unwrap();
    tx.push(EngineMessage::SetSnapshot(tone_snapshot(
        &turned,
        Some((&x, 1)),
        false,
    )))
    .unwrap();
    play(&mut proc, 20, 256, &mut out);
    let step = steepest(&out, 100);
    assert!(
        step <= 2.0 * tone,
        "a step of {step} (the tone's own {tone})"
    );
    // Mid-fade, in blocks of 64: the same swap again changes nothing.
    let run = |again: bool| {
        let (mut tx, mut proc) = processor();
        tx.push(EngineMessage::SetSnapshot(tone_snapshot(&x, None, false)))
            .unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut out = Vec::new();
        play(&mut proc, 80, 64, &mut out);
        tx.push(EngineMessage::SetSnapshot(tone_snapshot(
            &turned,
            Some((&x, 7)),
            false,
        )))
        .unwrap();
        play(&mut proc, 2, 64, &mut out);
        if again {
            tx.push(EngineMessage::SetSnapshot(tone_snapshot(
                &turned,
                Some((&x, 7)),
                false,
            )))
            .unwrap();
        }
        play(&mut proc, 20, 64, &mut out);
        out
    };
    assert_eq!(run(true), run(false), "a re-install started the fade over");
}

/// Finding 16: a swap of two versions of the same audio (correlated) never swells: the
/// crossfade's gains sum to one.
#[test]
fn a_swap_of_the_same_audio_never_swells() {
    let x = sine(48_000, 220.0, 4.0);
    let (a, b) = (arc(&x), arc(&x));
    let (mut tx, mut proc) = processor();
    tx.push(EngineMessage::SetSnapshot(tone_snapshot(&a, None, false)))
        .unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut out = Vec::new();
    play(&mut proc, 20, 256, &mut out);
    tx.push(EngineMessage::SetSnapshot(tone_snapshot(
        &b,
        Some((&a, 1)),
        false,
    )))
    .unwrap();
    play(&mut proc, 20, 256, &mut out);
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak <= 0.5 + 1e-4, "a swell to {peak} (the tone's 0.5)");
}

/// Finding 12: a render laid again under an edited tempo (the same data, its window's
/// start moved under the playhead) crossfades from where it was playing.
#[test]
fn a_render_laid_again_under_an_edited_tempo_crossfades() {
    let x = sine(48_000, 220.0, 4.0);
    let d = arc(&x);
    let with_tempo = |bpm: f64, from: Option<(&Arc<SampleData>, u64)>| {
        let s = tone_snapshot(&d, from, false);
        let mut s = (*s).clone();
        let mut p = Project::new("t");
        p.tempo_points = vec![point(Ticks::ZERO, 120.0), point(Ticks::beats(1), bpm)];
        s.tempo = Arc::new(p.tempo_map());
        Arc::new(s)
    };
    for fade in [false, true] {
        let (mut tx, mut proc) = processor();
        tx.push(EngineMessage::SetSnapshot(with_tempo(120.0, None)))
            .unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut out = Vec::new();
        play(&mut proc, 200, 256, &mut out);
        let tone = steepest(&out, 100);
        tx.push(EngineMessage::SetSnapshot(with_tempo(
            121.3,
            fade.then_some((&d, 1)),
        )))
        .unwrap();
        play(&mut proc, 20, 256, &mut out);
        let step = steepest(&out, 100);
        if fade {
            assert!(
                step <= 2.0 * tone,
                "a step of {step} (the tone's own {tone})"
            );
        } else {
            assert!(
                step > 4.0 * tone,
                "laid again without a fade, it jumps: {step}"
            );
        }
    }
}

/// Findings 12 and 19 at the sample cache: a live compile that lays a piece's data again
/// (a tempo edit inside a following window while its new render is on its way) carries a
/// swap, and the next live compile of the moment carries the same one.
#[test]
fn a_piece_laid_again_carries_its_swap_for_a_moment() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = sine(48_000, 220.0, 20.0);
    write_wav(&path, &x, 48_000);
    let mut p = project(
        &path,
        48_000,
        x.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        Ticks::beats(16),
        Some(at_97(48_000)),
    );
    let mut cache = live_cache();
    attach_loaded(&mut cache, &p);
    wait_finals(&mut cache);
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, 48_000);
    let first = s.tracks[0].audio[0].data.clone().expect("its render");
    cache.warp().hold_finals(true);
    p.tempo_points = vec![point(Ticks::ZERO, 120.0), point(Ticks::beats(4), 121.0)];
    let mut s = Snapshot::compile(&p, 2);
    cache.attach_audio(&mut s, 48_000);
    let c = &s.tracks[0].audio[0];
    assert!(
        Arc::ptr_eq(c.data.as_ref().unwrap(), &first),
        "its render, laid again"
    );
    let f = c
        .fade_from
        .clone()
        .expect("a swap: laid again under another map");
    assert_eq!(s.swaps, vec![(0, 0)]);
    let mut s2 = Snapshot::compile(&p, 3);
    cache.attach_audio(&mut s2, 48_000);
    assert_eq!(
        s2.tracks[0].audio[0].fade_from.as_ref().map(|g| g.id),
        Some(f.id),
        "the same swap, carried"
    );
    cache.warp().hold_finals(false);
}

/// Finding 10: a project opened after another never plays the other's render on its clip
/// of the same id.
#[test]
fn a_render_of_the_last_project_never_plays_on_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.wav"), dir.path().join("b.wav"));
    let x = sine(48_000, 220.0, 8.0);
    let y = sine(48_000, 330.0, 6.0);
    write_wav(&a, &x, 48_000);
    write_wav(&b, &y, 48_000);
    let len = Ticks::beats(8);
    let pa = project(
        &a,
        48_000,
        x.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        len,
        Some(transposed(48_000)),
    );
    let pb = project(
        &b,
        48_000,
        y.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        len,
        Some(transposed(48_000)),
    );
    assert_eq!(
        pa.tracks[0].clips[0].id, pb.tracks[0].clips[0].id,
        "ids start again"
    );
    let mut cache = live_cache();
    attach_loaded(&mut cache, &pa);
    wait_finals(&mut cache);
    let mut s = Snapshot::compile(&pa, 1);
    cache.attach_audio(&mut s, 48_000);
    let rendered = s.tracks[0].audio[0].data.clone().unwrap();
    cache.warp().hold_finals(true);
    let s = attach_loaded(&mut cache, &pb);
    let c = &s.tracks[0].audio[0];
    let d = c.data.as_ref().expect("its own input");
    assert!(!Arc::ptr_eq(d, &rendered), "the last project's render");
    assert_eq!(d.len_frames(), y.len(), "its own source");
    cache.warp().hold_finals(false);
}

/// Finding 13: a following warp that moves the audio, nothing made for it yet (a project
/// just opened), plays nothing until its bridge — never its input laid unwarped, at the
/// wrong bar and speed.
#[test]
fn a_following_warp_with_nothing_made_never_plays_its_input_laid_unwarped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let x = sine(48_000, 220.0, 40.0);
    write_wav(&path, &x, 48_000);
    let p = project(
        &path,
        48_000,
        x.len(),
        Ticks::beats(4),
        Ticks::beats(32),
        Ticks::beats(8),
        Some(at_97(48_000)),
    );
    let mut cache = live_cache();
    cache.warp().hold_finals(true);
    // Until the source is in, nothing; then the first compile with it.
    let t = std::time::Instant::now();
    let s = loop {
        cache.poll_loaded();
        let mut s = Snapshot::compile(&p, 1);
        cache.attach_audio(&mut s, 48_000);
        if cache.warp_pending() > 0 || t.elapsed().as_secs() > 30 {
            break s;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let c = &s.tracks[0].audio[0];
    assert!(
        c.data.is_none() || c.zero_at.is_some(),
        "its input, laid where the warp does not put it"
    );
    cache.warp().hold_finals(false);
}

/// Finding 14: a left trim of a playing warped clip, its new render on its way, plays the
/// render of the window before the trim where that render's content begins — not one trim
/// early.
#[test]
fn a_left_trim_plays_the_last_render_where_its_content_is() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = sine(48_000, 220.0, 20.0);
    write_wav(&path, &x, 48_000);
    for warp in [at_97(48_000), transposed(48_000)] {
        let mut p = project(
            &path,
            48_000,
            x.len(),
            Ticks::beats(8),
            Ticks::beats(2),
            Ticks::beats(16),
            Some(warp.clone()),
        );
        let mut cache = live_cache();
        attach_loaded(&mut cache, &p);
        wait_finals(&mut cache);
        cache.warp().hold_finals(true);
        let trim = Ticks::beats(4);
        let c = &mut p.tracks[0].clips[0];
        c.start += trim;
        c.offset += trim;
        c.length -= trim;
        let mut s = Snapshot::compile(&p, 2);
        cache.attach_audio(&mut s, 48_000);
        let c = &s.tracks[0].audio[0];
        assert!(c.data.is_some(), "the render before the trim");
        let (tick, frames) = c.zero_at.expect("laid by the cache");
        if warp.follow {
            assert_eq!(
                (tick, frames),
                (Ticks::beats(8), 0.0),
                "where its content begins"
            );
        } else {
            // Two seconds of source (four beats at 120) at the engine's rate.
            assert_eq!(tick, Ticks::beats(12));
            assert!((frames - 96_000.0).abs() < 1.0, "{frames} frames back");
        }
        cache.warp().hold_finals(false);
    }
}

/// Findings 15 and 29: tempo edits before a transposed window that does not follow (a
/// 44.1 kHz take whose start falls between frames), the bus re-anchoring it each time, keep
/// the render it plays — a window alone, and a linked pair whose offsets move by up to
/// half a tick.
#[test]
fn tempo_edits_before_a_transposed_window_keep_the_render_it_plays() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let x = sine(44_100, 220.0, 10.0);
    write_wav(&path, &x, 44_100);
    for linked in [false, true] {
        let mut p = project(
            &path,
            44_100,
            x.len(),
            Ticks::beats(16),
            Ticks(5_774),
            Ticks::beats(4),
            Some(transposed(44_100)),
        );
        if linked {
            let mut c = p.tracks[0].clips[0].clone();
            c.id = ClipId(p.alloc_id());
            c.start = Ticks::beats(24);
            let link = LinkId(p.alloc_id());
            c.link = Some(link);
            p.tracks[0].clips[0].link = Some(link);
            p.tracks[0].clips.push(c);
        }
        let mut cache = SampleCache::new();
        let mut bus = CommandBus::new(p);
        let mut s = Snapshot::compile(bus.project(), 1);
        cache.attach_audio(&mut s, 48_000);
        let first: Vec<_> = s.tracks[0]
            .audio
            .iter()
            .map(|c| c.data.clone().unwrap())
            .collect();
        let mut seed = 0x9e37_79b9u32;
        for k in 0..20 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let bpm = 60.0 + (seed >> 8) as f64 / (1u32 << 24) as f64 * 120.0;
            bus.execute(
                Command::SetTempoTrack {
                    bpm: 120.0,
                    points: vec![
                        point(Ticks::ZERO, 120.0),
                        point(Ticks::beats(4) + Ticks(k * 7), bpm),
                        point(Ticks::beats(12), 120.0),
                    ],
                },
                Origin::User,
            )
            .unwrap();
            let mut s = Snapshot::compile(bus.project(), 2 + k as u64);
            cache.attach_audio(&mut s, 48_000);
            for (c, f) in s.tracks[0].audio.iter().zip(&first) {
                assert!(
                    Arc::ptr_eq(c.data.as_ref().unwrap(), f),
                    "rendered again at edit {k} ({bpm:.2} BPM), linked {linked}"
                );
            }
        }
    }
}

/// Finding 34: a piece playing its fallback (its new render on its way) keeps it however
/// little the budget leaves — through the attach, not a protected set made by hand.
#[test]
fn a_fallback_that_plays_is_kept_past_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = sine(48_000, 220.0, 8.0);
    write_wav(&path, &x, 48_000);
    let mut p = project(
        &path,
        48_000,
        x.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        Ticks::beats(8),
        Some(transposed(48_000)),
    );
    let mut cache = live_cache();
    cache.set_warp_budget(1);
    attach_loaded(&mut cache, &p);
    wait_finals(&mut cache);
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, 48_000);
    let rendered = s.tracks[0].audio[0].data.clone().unwrap();
    cache.warp().hold_finals(true);
    if let Some(w) = p.tracks[0].clips[0].audio.as_mut().unwrap().warp.as_mut() {
        w.cents = 500;
    }
    for k in 0..3 {
        let mut s = Snapshot::compile(&p, 2 + k);
        cache.attach_audio(&mut s, 48_000);
        let c = &s.tracks[0].audio[0];
        assert!(
            c.data.as_ref().is_some_and(|d| Arc::ptr_eq(d, &rendered)),
            "attach {k}: its fallback was let go"
        );
    }
    cache.warp().hold_finals(false);
}

/// Offline, at `rate`, a manual cache rendering at once: the left channel.
fn bounce(p: &Project, rate: u32) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    let mut cache = SampleCache::new();
    assert!(cache.attach_audio(&mut s, rate).is_empty());
    let opts = RenderOptions {
        sample_rate: rate,
        tail_seconds: 0.0,
        block_frames: 256,
        ..Default::default()
    };
    let r = render(Arc::new(s), &opts).unwrap();
    r.samples.chunks_exact(2).map(|f| f[0]).collect()
}

/// Finding 30: a following, untransposed warp that maps as the unwarped clip does over its
/// window — Follow pressed on a trimmed window (its start between frames) under a tempo
/// step in its hidden part, or a marker placed and not moved — plays its source to the
/// bit, in every mode and at every rate: nothing rendered.
#[test]
fn a_following_warp_on_the_natural_line_plays_the_source_to_the_bit() {
    let dir = tempfile::tempdir().unwrap();
    for rate in [44_100u32, 48_000, 96_000] {
        let path = dir.path().join(format!("take {rate}.wav"));
        let x = sine(rate, 233.0, 12.0);
        write_wav(&path, &x, rate);
        let (start, offset, length) =
            (Ticks::beats(8), Ticks::beats(3) + Ticks(7), Ticks::beats(6));
        let mut plain = project(&path, rate, x.len(), start, offset, length, None);
        plain.tempo_points = vec![point(Ticks::ZERO, 120.0), point(Ticks::beats(6), 97.0)];
        plain.tempo_bpm = 120.0;
        let want = bounce(&plain, rate);
        let map = plain.tempo_map();
        for mode in cantus_core::warp::WarpMode::ALL {
            let mut natural = Warp::natural_of(&map, start, offset, length, 0, rate);
            natural.mode = mode;
            let placed = natural.add_marker(offset + Ticks::beats(2)).unwrap();
            for (what, w) in [("natural", natural.clone()), ("a marker placed", placed)] {
                let mut p = plain.clone();
                p.tracks[0].clips[0].audio.as_mut().unwrap().warp = Some(Box::new(w));
                let s = Snapshot::compile(&p, 1);
                assert!(
                    s.tracks[0].audio[0].warp.is_none(),
                    "{rate} Hz, {mode:?}, {what}: rendered"
                );
                let got = bounce(&p, rate);
                let differ = got.iter().zip(&want).position(|(a, b)| a != b);
                assert_eq!(differ, None, "{rate} Hz, {mode:?}, {what}");
            }
        }
    }
}

/// Tempo's relay (its old 1/9): a transposed window that does not follow, a tempo point
/// moved inside its hidden part with nothing re-anchoring it, plays a new render — the
/// render kept through a re-anchoring is never kept past it.
#[test]
fn a_tempo_edit_in_the_hidden_part_renders_a_transposed_window_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let x = sine(48_000, 220.0, 12.0);
    write_wav(&path, &x, 48_000);
    let mut p = project(
        &path,
        48_000,
        x.len(),
        Ticks::beats(8),
        Ticks::beats(6),
        Ticks::beats(4),
        Some(transposed(48_000)),
    );
    p.tempo_points = vec![point(Ticks::ZERO, 120.0), point(Ticks::beats(4), 100.0)];
    let mut cache = SampleCache::new();
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, 48_000);
    let first = s.tracks[0].audio[0].data.clone().unwrap();
    p.tempo_points = vec![point(Ticks::ZERO, 120.0), point(Ticks::beats(3), 100.0)];
    let mut s = Snapshot::compile(&p, 2);
    cache.attach_audio(&mut s, 48_000);
    let c = &s.tracks[0].audio[0];
    assert!(
        !Arc::ptr_eq(c.data.as_ref().unwrap(), &first),
        "the render of the frame it played before"
    );
    assert_eq!(c.zero_at, Some((c.start, 0.0)), "its own render");
}
