//! The warp engine under what round 29's second review found (ADR 0061 §11): a trimmed
//! warped window's markers on their ticks across a tempo step, a transposed window that
//! does not follow keyed on what it plays (a tempo edit before it, loop pieces, a tempo
//! edit that keeps it), a first transposition never silent, a foreign compile never taking
//! the playing render, a negative offset across a tempo step, and a swap crossfaded.

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpMode};
use cantus_core::*;
use cantus_engine::warp::{render_key, Plan};
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

/// One click a second (each on its own frame), six seconds at `rate`.
fn clicks(rate: u32) -> Vec<f32> {
    let mut x = vec![0.0f32; rate as usize * 6];
    for k in 0..6 {
        x[k * rate as usize] = 0.9;
    }
    x
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

/// Where a click is, fractionally (its largest sample, refined by a parabola).
fn rise(x: &[f32], from: usize, to: usize) -> f64 {
    let to = to.min(x.len());
    let i = (from..to)
        .max_by(|a, b| x[*a].abs().total_cmp(&x[*b].abs()))
        .unwrap();
    let (a, b, c) = (x[i - 1] as f64, x[i] as f64, x[i + 1] as f64);
    let d = a - 2.0 * b + c;
    i as f64
        + if d.abs() > 1e-12 {
            0.5 * (a - c) / d
        } else {
            0.0
        }
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

fn points(p: &mut Project, pts: &[(i64, f64)]) {
    p.tempo_points = pts
        .iter()
        .map(|&(t, bpm)| TempoPoint {
            tick: Ticks(t),
            bpm,
            ramp: false,
        })
        .collect();
    p.tempo_bpm = pts[0].1;
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

/// Finding 39: a warped window trimmed in (start two beats and 97 ticks, three beats into
/// its content, a tempo step between its content origin and its start) plays a marker's
/// click on its tick to the sample — at a file's rate and the engine's, alike or not.
#[test]
fn a_trimmed_warped_window_plays_its_markers_on_their_ticks() {
    let dir = tempfile::tempdir().unwrap();
    for (file, engine) in [
        (44_100u32, 48_000u32),
        (48_000, 48_000),
        (96_000, 44_100),
        (44_100, 96_000),
    ] {
        let path = dir.path().join(format!("clicks {file}.wav"));
        let x = clicks(file);
        write_wav(&path, &x, file);
        // The click at one second, unwarped: its own rise at the engine's rate.
        let plain = project(
            &path,
            file,
            x.len(),
            Ticks::ZERO,
            Ticks::ZERO,
            Ticks::beats(8),
            None,
        );
        let plain = bounce(&plain, engine);
        let at = engine as f64;
        let own = rise(&plain, at as usize - 400, at as usize + 400) - at;
        // The click at one second (content beat 2 at the source's 120) moved to beat 5.
        let mut w = Warp::identity(file, 120.0, 0);
        w = w.add_marker(Ticks::beats(2)).unwrap();
        let i = w.marker_at(Ticks::beats(2)).unwrap();
        w = w.move_marker(i, Ticks::beats(5));
        for mode in [WarpMode::Repitch, WarpMode::Complex] {
            w.mode = mode;
            let (start, offset) = (Ticks::beats(2) + Ticks(97), Ticks::beats(3));
            let mut p = project(
                &path,
                file,
                x.len(),
                start,
                offset,
                Ticks::beats(8),
                Some(w.clone()),
            );
            points(&mut p, &[(0, 120.0), (960, 90.0)]);
            let out = bounce(&p, engine);
            let heard = start - offset + Ticks::beats(5);
            let want = p.tempo_map().sample_at(heard.0 as f64, engine as f64);
            let got = rise(&out, want as usize - 400, want as usize + 400);
            let err = got - want - own;
            assert!(
                err.abs() <= 1.0,
                "{mode:?}, {file} Hz file at {engine} Hz: {err:+.2} samples"
            );
        }
    }
}

/// The plan of `p`'s first clip, as the sample cache makes it.
fn plan_of(p: &Project, clip: usize) -> Plan {
    let s = Snapshot::compile(p, 1);
    let c = &s.tracks[0].audio[clip];
    Plan {
        warp: c.warp.clone().expect("a warp that renders"),
        map: s.tempo.clone(),
        start: c.start,
        end: c.end,
        offset: c.offset,
        slip_frames: c.slip * c.source_rate as f64,
        source_rate: c.source_rate,
        rate: 48_000,
        path: c.path.clone().into(),
        input_key: 1,
    }
}

/// A transposed warp that does not follow the tempo.
fn transposed(rate: u32) -> Warp {
    let mut w = Warp::new(rate, 120.0);
    w.follow = false;
    w.cents = 200;
    w
}

/// Findings 11, 17, 24: a transposed window that does not follow is keyed on what it
/// plays — a tempo edit before its start (inside its hidden part) renders it again, and
/// its first frame is the one the interface draws.
#[test]
fn a_tempo_edit_before_a_transposed_window_renders_it_again() {
    let path = Path::new("C:/nowhere/vox.wav");
    let (start, offset) = (Ticks::beats(8), Ticks::beats(6));
    let mut p = project(
        path,
        48_000,
        48_000 * 60,
        start,
        offset,
        Ticks::beats(4),
        Some(transposed(48_000)),
    );
    points(&mut p, &[(0, 120.0), (Ticks::beats(4).0, 100.0)]);
    let a = plan_of(&p, 0);
    // The point moved from beat 4 to beat 3: the tempo over the window is the same.
    points(&mut p, &[(0, 120.0), (Ticks::beats(3).0, 100.0)]);
    let b = plan_of(&p, 0);
    assert_eq!(
        a.map.span_key(start, start + Ticks::beats(4)),
        b.map.span_key(start, start + Ticks::beats(4))
    );
    assert_ne!(render_key(&a), render_key(&b), "a new render");
    let c = &p.tracks[0].clips[0];
    let drawn = audio_time::clip_frame_at(
        &p.tempo_map(),
        c.start,
        c.offset,
        c.audio.as_ref().unwrap(),
        48_000.0,
        c.start.0 as f64,
    );
    assert!(
        (b.source_at(0.0) - drawn).abs() <= 0.5,
        "{} against {drawn}",
        b.source_at(0.0)
    );
}

/// Finding 17: two pieces of a looped transposed window with equal windows but hidden
/// parts under other tempos play other frames — never one render.
#[test]
fn loop_pieces_whose_hidden_parts_differ_render_apart() {
    let path = Path::new("C:/nowhere/vox.wav");
    let mut p = project(
        path,
        48_000,
        48_000 * 60,
        Ticks::beats(20),
        Ticks::beats(4),
        Ticks::beats(16),
        Some(transposed(48_000)),
    );
    p.tracks[0].clips[0].content_length = Some(Ticks::beats(8));
    // 120 BPM until beat 18, then 100: the first piece's hidden part crosses the step.
    points(&mut p, &[(0, 120.0), (Ticks::beats(18).0, 100.0)]);
    let (a, b) = (plan_of(&p, 0), plan_of(&p, 1));
    assert_eq!(a.end - a.start, b.end - b.start);
    assert_ne!(render_key(&a), render_key(&b));
    assert!((a.source_at(0.0) - b.source_at(0.0)).abs() > 100.0);
}

/// Finding 24: a tempo edit that keeps a transposed window's audio at its start (the
/// command bus re-anchors it, ADR 0060 §4) keeps its render.
#[test]
fn a_tempo_edit_that_keeps_a_transposed_window_keeps_its_render() {
    let path = Path::new("C:/nowhere/vox.wav");
    let p = project(
        path,
        48_000,
        48_000 * 60,
        Ticks::beats(8) + Ticks(13),
        Ticks::beats(6),
        Ticks::beats(4),
        Some(transposed(48_000)),
    );
    let mut bus = CommandBus::new(p);
    let first = plan_of(bus.project(), 0);
    let before = render_key(&first);
    for bpm in [97.0, 133.3, 61.0] {
        bus.execute(
            Command::SetTempoTrack {
                bpm: 120.0,
                points: vec![
                    TempoPoint {
                        tick: Ticks::ZERO,
                        bpm: 120.0,
                        ramp: false,
                    },
                    TempoPoint {
                        tick: Ticks::beats(4),
                        bpm,
                        ramp: false,
                    },
                    TempoPoint {
                        tick: Ticks::beats(8),
                        bpm: 120.0,
                        ramp: false,
                    },
                ],
            },
            Origin::User,
        )
        .unwrap();
        let plan = plan_of(bus.project(), 0);
        assert_eq!(
            render_key(&plan),
            before,
            "re-rendered at {bpm} BPM before the window"
        );
        // And the frame heard at its start is the one it was (to the frame).
        let d = plan.source_at(0.0) - first.source_at(0.0);
        assert!(d.abs() < 1.0, "{d:+} frames at {bpm} BPM");
    }
}

/// A background cache (renders on the service's workers).
fn live_cache() -> SampleCache {
    let mut c = SampleCache::new();
    c.set_background(true);
    c.set_pitch_background(true);
    c
}

/// Finding 19: a clip transposed for the first time plays its input — the source, laid as
/// unwarped — while its bridge and render are made: never silence.
#[test]
fn a_first_transposition_plays_its_input_until_its_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = sine(48_000, 220.0, 20.0);
    write_wav(&path, &x, 48_000);
    let p = project(
        &path,
        48_000,
        x.len(),
        Ticks::beats(2),
        Ticks::beats(1),
        Ticks::beats(16),
        Some(transposed(48_000)),
    );
    let mut cache = live_cache();
    // The source first (loaded in the background).
    let t = std::time::Instant::now();
    loop {
        cache.poll_loaded();
        let mut s = Snapshot::compile(&p, 1);
        cache.attach_audio(&mut s, 48_000);
        let c = &s.tracks[0].audio[0];
        if let Some(d) = c.data.as_ref() {
            assert!(!c.zero_at_start, "its input, as unwarped");
            assert_eq!(d.len_frames(), x.len(), "the source itself");
            break;
        }
        assert!(t.elapsed().as_secs() < 30, "the source never came");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // Then its render.
    let t = std::time::Instant::now();
    while cache.warp_pending() > 0 && t.elapsed().as_secs() < 60 {
        let _ = cache.wait_warp(std::time::Duration::from_millis(20));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut s = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s, 48_000);
    assert!(s.tracks[0].audio[0].zero_at_start, "its render now");
}

/// Finding 23: a foreign compile (a bounce, a freeze, a comp merge) of a project without
/// the playing clip never takes that clip's render away, however small the budget.
#[test]
fn a_foreign_compile_never_takes_the_playing_render() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = sine(48_000, 220.0, 4.0);
    write_wav(&path, &x, 48_000);
    let p = project(
        &path,
        48_000,
        x.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        Ticks::beats(4),
        Some(transposed(48_000)),
    );
    let mut cache = SampleCache::new();
    cache.set_warp_budget(1);
    let mut s = Snapshot::compile(&p, 1);
    assert!(cache.attach_audio(&mut s, 48_000).is_empty());
    let first = s.tracks[0].audio[0].data.clone().unwrap();
    assert!(s.tracks[0].audio[0].zero_at_start, "rendered at once");
    // A foreign project: nothing of this clip in it.
    let mut other = p.clone();
    other.tracks[0].clips.clear();
    cache.set_offline(true);
    let mut s2 = Snapshot::compile(&other, 1);
    cache.attach_audio(&mut s2, 48_000);
    cache.set_offline(false);
    let mut s3 = Snapshot::compile(&p, 1);
    cache.attach_audio(&mut s3, 48_000);
    let again = s3.tracks[0].audio[0].data.clone().unwrap();
    assert!(
        Arc::ptr_eq(&first, &again),
        "the same render, never made again"
    );
}

/// Finding 21: a window whose offset is negative (its content origin after its start, past
/// a tempo step) starts where the map puts it — the held tempo never stretched past its end.
#[test]
fn a_negative_offset_across_a_tempo_step_starts_where_the_map_says() {
    let mut p = Project::new("t");
    points(&mut p, &[(0, 120.0), (Ticks::beats(36).0, 90.0)]);
    let map = Arc::new(p.tempo_map());
    let mut t = Transport::new(48_000.0);
    t.set_map(map.clone());
    let (start, offset) = (Ticks::beats(32), -Ticks::beats(8));
    let zero = t.clip_zero(start, offset, 0.0);
    let want = map.sample_at(start.0 as f64, 48_000.0)
        - audio_time::offset_seconds(&map, start, offset) * 48_000.0;
    assert!((zero as f64 - want).abs() <= 0.5, "{zero} against {want}");
}

/// Finding 22: a playing piece whose data a new snapshot swaps (a bridge giving way to its
/// render) crossfades: no sample steps further than the material's own steepest.
#[test]
fn a_swap_of_a_playing_pieces_data_is_crossfaded() {
    let path = Path::new("C:/nowhere/tone.wav");
    let x = sine(48_000, 220.0, 4.0);
    let p = project(
        path,
        48_000,
        x.len(),
        Ticks::ZERO,
        Ticks::ZERO,
        Ticks::beats(8),
        None,
    );
    let s = Snapshot::compile(&p, 1);
    let with = |data: Vec<f32>, from: Option<Vec<f32>>| {
        let mut s = s.clone();
        let c = &mut s.tracks[0].audio[0];
        let d = |v: Vec<f32>| {
            Arc::new(SampleData {
                stream: None,
                rate: 48_000,
                channels: 1,
                frames: v.into(),
            })
        };
        c.data = Some(d(data));
        c.zero_at_start = true;
        c.fade_from = from.map(|v| cantus_core::snapshot::FadeFrom {
            data: d(v),
            slip: 0.0,
            zero_at_start: true,
        });
        Arc::new(s)
    };
    // The bridge, then its render: the same tone, its phase turned half a cycle.
    let turned: Vec<f32> = x.iter().map(|v| -v).collect();
    let steepest = x
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    for fade in [false, true] {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, _rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        tx.push(EngineMessage::SetSnapshot(with(x.clone(), None)))
            .unwrap();
        tx.push(EngineMessage::Play).unwrap();
        let mut out = Vec::new();
        let mut buf = vec![0f32; 256 * 2];
        for _ in 0..20 {
            proc.process(&mut buf, 2);
            out.extend(buf.chunks_exact(2).map(|f| f[0]));
        }
        let next = with(turned.clone(), fade.then(|| x.clone()));
        tx.push(EngineMessage::SetSnapshot(next)).unwrap();
        for _ in 0..20 {
            proc.process(&mut buf, 2);
            out.extend(buf.chunks_exact(2).map(|f| f[0]));
        }
        let step = out
            .windows(2)
            .skip(100)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        if fade {
            assert!(
                step <= 2.0 * steepest,
                "a step of {step} (the tone's own {steepest})"
            );
        } else {
            assert!(
                step > 4.0 * steepest,
                "the swap without a fade clicks: {step}"
            );
        }
    }
}
