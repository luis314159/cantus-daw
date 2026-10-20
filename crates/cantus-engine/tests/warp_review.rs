//! The warp engine under what round 29's review found untested (ADR 0061): every mode's
//! markers to the sample at mixed rates and under a ramp, the identity through the render
//! path, the bridge (its length, its place, its pitch), renders kept while a clip leaves
//! the snapshot, a loop's pieces, finals nothing plays any more, tempo points in the render
//! map, and parallel = serial.

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpMode};
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
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

/// A project at `bpm` with one track per clip `(path, rate, frames, warp)`, each clip from
/// beat 0 for `beats`.
fn project(clips: &[(&Path, u32, usize, Option<Warp>)], bpm: f64, beats: i64) -> Project {
    let mut p = Project::new("warp");
    p.tempo_bpm = bpm;
    for (i, (path, rate, frames, warp)) in clips.iter().enumerate() {
        let mut t = p.make_track(format!("Loop {i}"));
        t.strip = None;
        let id = AudioSourceId(p.alloc_id());
        p.audio_pool.push(AudioSource {
            id,
            name: format!("loop {i}"),
            path: path.to_string_lossy().into_owned(),
            sample_rate: *rate,
            channels: 1,
            frames: *frames as u64,
            sha256: None,
            recorded: false,
        });
        let mut c = p.make_audio_clip("loop", id, Ticks::ZERO, Ticks::beats(beats));
        c.audio.as_mut().unwrap().warp = warp.clone().map(Box::new);
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

fn ramp(p: &mut Project, to_bpm: f64, at_beats: i64) {
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: p.tempo_bpm,
            ramp: true,
        },
        TempoPoint {
            tick: Ticks::beats(at_beats),
            bpm: to_bpm,
            ramp: false,
        },
    ];
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

/// Where a click is, fractionally: its largest sample in the window, refined by a parabola
/// through it and its neighbours (a click read slower is wider, but still centred there).
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

/// One click a second (each on its own frame), four seconds at `rate`.
fn clicks(rate: u32) -> Vec<f32> {
    let mut x = vec![0.0f32; rate as usize * 4];
    for k in 0..4 {
        x[k * rate as usize] = 0.9;
    }
    x
}

/// Every mode puts a marker's moment on its tick to the sample — at a file's rate and the
/// engine's alike or not, and under a ramp — measured against the click's own place in an
/// unwarped bounce at the same rates (so a rate change's own rounding cancels).
#[test]
fn every_mode_puts_a_marker_on_its_tick() {
    let dir = tempfile::tempdir().unwrap();
    for (file, engine) in [
        (44_100u32, 48_000u32),
        (48_000, 48_000),
        (96_000, 44_100),
        (44_100, 96_000),
        (48_000, 96_000),
    ] {
        let path = dir.path().join(format!("clicks {file}.wav"));
        let x = clicks(file);
        write_wav(&path, &x, file);
        // The click at one second, unwarped: its own rise at the engine's rate.
        let plain = bounce(&project(&[(&path, file, x.len(), None)], 120.0, 8), engine);
        let at = engine as f64;
        let own = rise(&plain, at as usize - 400, at as usize + 400) - at;
        for mode in WarpMode::ALL {
            let mut w = Warp::identity(file, 120.0, 0);
            w = w.add_marker(Ticks::beats(2)).unwrap();
            w = w.move_marker(1, Ticks::beats(3));
            w.mode = mode;
            for ramped in [false, true] {
                let mut p = project(&[(&path, file, x.len(), Some(w.clone()))], 120.0, 8);
                if ramped {
                    ramp(&mut p, 150.0, 8);
                }
                let out = bounce(&p, engine);
                let want = p
                    .tempo_map()
                    .sample_at(Ticks::beats(3).0 as f64, engine as f64);
                let got = rise(&out, want as usize - 400, want as usize + 400);
                let err = got - want - own;
                assert!(
                    err.abs() <= 1.0,
                    "{mode:?}, {file} Hz file at {engine} Hz, ramp {ramped}: {err:+.2} samples"
                );
            }
        }
    }
}

/// Where the tempo changes elsewhere in the song, a warp that maps as the clip plays
/// unwarped is kept — and rendered — yet plays the source to the bit, in every mode.
#[test]
fn an_identity_warp_through_the_render_path_is_the_source_to_the_bit() {
    let dir = tempfile::tempdir().unwrap();
    for rate in [44_100u32, 48_000, 96_000] {
        let path = dir.path().join(format!("loop {rate}.wav"));
        let m = cantus_dsp::beat::testkit::drum_loop(rate as f32, 120.0, 2, 0.0, 0.0, 3);
        write_wav(&path, &m.audio, rate);
        let mut plain = project(&[(&path, rate, m.audio.len(), None)], 120.0, 8);
        // A tempo change far after the clip: the map is not constant.
        plain.tempo_points = vec![
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(64),
                bpm: 140.0,
                ramp: false,
            },
        ];
        let want = bounce(&plain, rate);
        for mode in WarpMode::ALL {
            let mut w = Warp::identity(rate, 120.0, 0);
            w.mode = mode;
            let w = w.add_marker(Ticks::beats(4)).unwrap();
            let mut p = plain.clone();
            p.tracks[0].clips[0].audio.as_mut().unwrap().warp = Some(Box::new(w));
            let s = Snapshot::compile(&p, 1);
            assert!(
                s.tracks[0].audio[0].warp.is_some(),
                "{mode:?}: kept under a tempo map that moves"
            );
            assert_null(&want, &bounce(&p, rate), 0.0, "an identity warp, rendered");
        }
    }
}

/// A background cache (renders on the service's workers).
fn live_cache() -> SampleCache {
    let mut c = SampleCache::new();
    c.set_background(true);
    c.set_pitch_background(true);
    c
}

/// Attach `p` to `cache`; the first track's clip data.
fn attach(cache: &mut SampleCache, p: &Project, rev: u64) -> Option<Arc<SampleData>> {
    let mut s = Snapshot::compile(p, rev);
    cache.attach_audio(&mut s, 48_000);
    s.tracks
        .iter()
        .find(|t| !t.audio.is_empty())
        .and_then(|t| t.audio[0].data.clone())
}

fn settle(cache: &mut SampleCache) {
    let t = std::time::Instant::now();
    while cache.loading() && t.elapsed() < std::time::Duration::from_secs(120) {
        cache.poll_loaded();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!cache.loading());
}

/// Attach `p` again and again until nothing is on its way (its file loaded, its render
/// made): what it then plays.
fn ready(cache: &mut SampleCache, p: &Project) -> Arc<SampleData> {
    let t = std::time::Instant::now();
    loop {
        cache.poll_loaded();
        let d = attach(cache, p, 40);
        if !cache.loading() {
            return d.expect("a render");
        }
        assert!(
            t.elapsed() < std::time::Duration::from_secs(120),
            "never ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Poll until `p`'s clip plays something other than `not` (its bridge), at most 30 s.
fn next_data(cache: &mut SampleCache, p: &Project, not: &Arc<SampleData>) -> Arc<SampleData> {
    let t = std::time::Instant::now();
    loop {
        cache.poll_loaded();
        if let Some(d) = attach(cache, p, 50) {
            if !Arc::ptr_eq(&d, not) {
                return d;
            }
        }
        assert!(
            t.elapsed() < std::time::Duration::from_secs(30),
            "no bridge came"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// A 440 Hz tone with a click a second on top, four seconds at 48 kHz.
fn tone_and_clicks() -> Vec<f32> {
    let mut x: Vec<f32> = (0..48_000 * 4)
        .map(|i| {
            0.3 * cantus_dsp::math::sin(2.0 * std::f64::consts::PI * 440.0 * i as f64 / 48_000.0)
                as f32
        })
        .collect();
    for k in 1..4 {
        x[k * 48_000] += 0.9;
    }
    x
}

/// While a transposed clip's new render is on its way, its bridge plays: as long as the
/// render, the moved marker's moment on its tick, and at the clip's transposition — never
/// back at its source's pitch (round 29's review). Then the render, the same as one made at
/// once.
#[test]
fn the_bridge_plays_at_the_right_place_and_pitch_until_the_render_comes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let base = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap();
    let mut one = base.move_marker(1, Ticks::beats(3));
    one.cents = -300;
    let mut two = base.move_marker(1, Ticks::beats(5) / 2);
    two.cents = -300;
    let p1 = project(&[(&path, 48_000, x.len(), Some(one))], 120.0, 8);
    let p2 = project(&[(&path, 48_000, x.len(), Some(two))], 120.0, 8);
    let mut cache = live_cache();
    let first = ready(&mut cache, &p1);
    cache.warp().hold_finals(true);
    // Moved: at once the render it had; then its bridge.
    let meanwhile = attach(&mut cache, &p2, 3).expect("something plays");
    assert!(Arc::ptr_eq(&meanwhile, &first));
    let bridge = next_data(&mut cache, &p2, &first);
    assert!(cache.loading(), "the final is held back");
    // At the marker's tick: the click that sat at one second.
    let want = 2.5 * 0.5 * 48_000.0;
    let mono: Vec<f32> = bridge
        .frames
        .iter()
        .step_by(bridge.channels as usize)
        .copied()
        .collect();
    let got = rise(&mono, want as usize - 400, want as usize + 400);
    assert!(
        (got - want).abs() <= 1.0,
        "the bridge's click at {got}, its tick at {want}"
    );
    // At the transposition, between the clicks.
    let f = cantus_dsp::pitch::testkit::measured_f0(&mono[62_000..86_000], 48_000.0, 370.0);
    let to = 440.0 * cantus_dsp::math::exp2(-3.0f32 / 12.0);
    let cents = 1200.0 * cantus_dsp::math::log2(f / to);
    assert!(
        cents.abs() < 15.0,
        "the bridge plays at {f} Hz, the clip at {to} Hz"
    );
    cache.warp().hold_finals(false);
    settle(&mut cache);
    let second = attach(&mut cache, &p2, 4).unwrap();
    assert_eq!(
        second.len_frames(),
        bridge.len_frames(),
        "the bridge's length"
    );
    let mut sync = Snapshot::compile(&p2, 1);
    SampleCache::new().attach_audio(&mut sync, 48_000);
    assert_eq!(
        second.frames[..],
        sync.tracks[0].audio[0].data.as_ref().unwrap().frames[..],
        "the render, as one made at once"
    );
}

/// A tempo edit under a warped clip: its bridge plays at the new speed at once, as long as
/// the render that replaces it.
#[test]
fn a_tempo_edit_plays_its_bridge_then_its_render() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let w = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    let p1 = project(&[(&path, 48_000, x.len(), Some(w))], 120.0, 8);
    let mut p2 = p1.clone();
    ramp(&mut p2, 150.0, 8);
    let mut cache = live_cache();
    let first = ready(&mut cache, &p1);
    cache.warp().hold_finals(true);
    let bridge = next_data(&mut cache, &p2, &first);
    let want = p2.tempo_map().sample_at(Ticks::beats(3).0 as f64, 48_000.0);
    let mono: Vec<f32> = bridge
        .frames
        .iter()
        .step_by(bridge.channels as usize)
        .copied()
        .collect();
    let got = rise(&mono, want as usize - 400, want as usize + 400);
    assert!(
        (got - want).abs() <= 1.0,
        "the bridge's click at {got}, its tick at {want}"
    );
    cache.warp().hold_finals(false);
    settle(&mut cache);
    let second = attach(&mut cache, &p2, 3).unwrap();
    assert_eq!(second.len_frames(), bridge.len_frames());
}

/// Muted, auditioned out (a pattern), or absent from a foreign project's compile: a warped
/// clip's render is kept, and played at once when the clip is back.
#[test]
fn a_render_is_kept_while_its_clip_is_out_of_the_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let w = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    let p = project(&[(&path, 48_000, x.len(), Some(w))], 120.0, 8);
    let mut cache = live_cache();
    let r = ready(&mut cache, &p);
    let mut muted = p.clone();
    muted.tracks[0].clips[0].muted = true;
    let foreign = project(&[], 120.0, 8);
    for (i, out) in [muted, foreign].iter().enumerate() {
        attach(&mut cache, out, 10 + i as u64);
        let back = attach(&mut cache, &p, 20 + i as u64).unwrap();
        assert!(
            Arc::ptr_eq(&back, &r),
            "case {i}: the render played again at once"
        );
        assert!(!cache.loading(), "case {i}: nothing made again");
    }
}

/// While a marker is dragged, the clip plays the newest bridge made — not the render from
/// before the drag.
#[test]
fn a_drag_plays_the_newest_bridge() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let base = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap();
    let at = |t: i64| {
        project(
            &[(&path, 48_000, x.len(), Some(base.move_marker(1, Ticks(t))))],
            120.0,
            8,
        )
    };
    let mut cache = live_cache();
    let before = ready(&mut cache, &at(2_880));
    cache.warp().hold_finals(true);
    let b1 = next_data(&mut cache, &at(2_900), &before);
    // The next frame of the drag, before its own bridge is taken in.
    let now = attach(&mut cache, &at(2_920), 3).unwrap();
    assert!(
        Arc::ptr_eq(&now, &b1),
        "the newest bridge plays, not the pre-drag render"
    );
    cache.warp().hold_finals(false);
    settle(&mut cache);
}

/// A clip deleted while its render is on its way: nothing waits for that render.
#[test]
fn a_deleted_clips_render_is_not_waited_for() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let w = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    let p = project(&[(&path, 48_000, x.len(), Some(w.clone()))], 120.0, 8);
    let mut cache = live_cache();
    ready(&mut cache, &p);
    cache.warp().hold_finals(true);
    let moved = project(
        &[(
            &path,
            48_000,
            x.len(),
            Some(w.move_marker(1, Ticks::beats(4))),
        )],
        120.0,
        8,
    );
    attach(&mut cache, &moved, 1);
    assert!(cache.loading(), "its render is on its way");
    let gone = project(&[], 120.0, 8);
    attach(&mut cache, &gone, 2);
    assert_eq!(cache.warp_pending(), 0);
    assert!(!cache.loading(), "an export would not wait for it");
    cache.warp().hold_finals(false);
}

/// Two looped warped clips, each with a short last piece: through `compile_for_render` every
/// piece plays the render a cache makes at once — no piece cancels another's render
/// (round 29's review: the queue no longer replaces by clip).
#[test]
fn every_piece_of_looped_warped_clips_plays_its_own_render() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let w = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    let mut p = project(
        &[
            (&path, 48_000, x.len(), Some(w.clone())),
            (&path, 48_000, x.len(), Some(w)),
        ],
        120.0,
        28,
    );
    for t in &mut p.tracks {
        // Seven bars of a two-bar loop: pieces of 2, 2, 2 and 1 bars.
        t.clips[0].content_length = Some(Ticks::beats(8));
    }
    ramp(&mut p, 140.0, 28);
    let mut engine = EngineHandle::dummy();
    let rate = engine.sample_rate.round() as u32;
    let late = engine.renders_late();
    let snap = engine.compile_for_render(&p, 1);
    assert_eq!(engine.renders_late(), late, "every render was in");
    let mut sync = Snapshot::compile(&p, 1);
    SampleCache::new().attach_audio(&mut sync, rate);
    let mut pieces = 0;
    for (a, b) in snap.tracks.iter().zip(&sync.tracks) {
        assert_eq!(a.audio.len(), b.audio.len());
        for (x, y) in a.audio.iter().zip(&b.audio) {
            let (x, y) = (x.data.as_ref().unwrap(), y.data.as_ref().unwrap());
            assert_eq!(
                x.frames[..],
                y.frames[..],
                "piece at {:?}",
                a.audio[0].start
            );
            pieces += 1;
        }
    }
    assert!(pieces >= 8, "{pieces} pieces");
}

/// A step in the tempo inside a warped clip's window is a breakpoint of its render's map:
/// the stretcher reads where the interface draws, to a millionth of a frame, beside it.
#[test]
fn a_tempo_step_inside_a_window_is_on_the_render_map() {
    use cantus_core::tempo::TempoMap;
    use cantus_core::time::TimeSignature;
    let map = Arc::new(TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(6),
                bpm: 180.0,
                ramp: false,
            },
        ],
        &[],
    ));
    let w = Warp::identity(44_100, 92.0, 0)
        .add_marker(Ticks::beats(4))
        .unwrap();
    let plan = cantus_engine::warp::Plan {
        warp: Arc::new(w),
        map: map.clone(),
        start: Ticks::beats(2) + Ticks(97),
        end: Ticks::beats(14),
        offset: Ticks::ZERO,
        slip_frames: 0.0,
        source_rate: 44_100,
        rate: 48_000,
        path: Default::default(),
        input_key: 1,
    };
    let step = map.seconds_between_f64(plan.start.0 as f64, Ticks::beats(6).0 as f64) * 48_000.0;
    let m = plan.time_map();
    for d in [-200.0, -50.0, -1.0, 0.0, 1.0, 50.0, 200.0] {
        let k = step + d;
        let err = m.source(k) - plan.source_at(k);
        assert!(err.abs() < 1e-6, "{d:+} from the step: {err}");
    }
}

/// Warped clips in every mode, transposed and not, on several tracks: the multicore pool
/// renders them to the same bits as one thread.
#[test]
fn parallel_equals_serial_with_warped_clips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tone.wav");
    let x = tone_and_clicks();
    write_wav(&path, &x, 48_000);
    let warps: Vec<Warp> = WarpMode::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut w = Warp::identity(48_000, 120.0, 0)
                .add_marker(Ticks::beats(2))
                .unwrap()
                .move_marker(1, Ticks(2_400 + 240 * i as i64));
            w.mode = *m;
            w.cents = [0, -300, 500, 0][i];
            w
        })
        .collect();
    let clips: Vec<(&Path, u32, usize, Option<Warp>)> = warps
        .iter()
        .map(|w| (path.as_path(), 48_000u32, x.len(), Some(w.clone())))
        .collect();
    let p = project(&clips, 120.0, 8);
    let mut s = Snapshot::compile(&p, 1);
    assert!(SampleCache::new().attach_audio(&mut s, 48_000).is_empty());
    let snap = Arc::new(s);
    let play = |workers: Workers| {
        let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
        let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
        let mut proc = Processor::without_midi(48_000.0, rx_engine, tx_engine);
        proc.set_workers(workers);
        let plan = cantus_engine::graph::compile(&snap);
        tx.push(EngineMessage::SetSnapshotPlanned(snap.clone(), plan))
            .ok();
        proc.settle();
        tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
        tx.push(EngineMessage::Play).ok();
        let mut out = Vec::new();
        let mut buf = vec![0f32; 300 * 2];
        for _ in 0..640 {
            proc.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            while rx.pop().is_ok() {}
        }
        out
    };
    let serial = play(Workers::SERIAL);
    assert!(serial.iter().any(|v| v.abs() > 0.01), "it sounds");
    assert_null(&serial, &play(Workers::parallel(3)), 0.0, "three workers");
}

/// A warp that does not follow the tempo and does not transpose, under a tempo ramp: the
/// clip plays the bits it plays with no warp at all (round 29's review).
#[test]
fn a_natural_warp_under_a_ramp_plays_the_unwarped_bits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loop.wav");
    let m = cantus_dsp::beat::testkit::drum_loop(48_000.0, 120.0, 2, 0.0, 0.0, 9);
    write_wav(&path, &m.audio, 48_000);
    let mut plain = project(&[(&path, 48_000, m.audio.len(), None)], 120.0, 8);
    ramp(&mut plain, 150.0, 8);
    let want = bounce(&plain, 48_000);
    let mut w = Warp::identity(48_000, 100.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap()
        .move_marker(1, Ticks::beats(3));
    w.follow = false;
    let mut p = plain.clone();
    p.tracks[0].clips[0].audio.as_mut().unwrap().warp = Some(Box::new(w));
    assert_null(
        &want,
        &bounce(&p, 48_000),
        0.0,
        "a natural warp under a ramp",
    );
}
