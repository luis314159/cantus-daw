//! The recording mode's benchmark (round 36, ADR 0092; `docs/round36/record.md`): a session
//! of 64 tracks, each with a synth and four effects, played live and then baked by the mode,
//! block by block at the buffer sizes an interface offers. What it measures is the
//! processor's own time per block against the block's budget — the CPU cost of the engine,
//! not of a device: the lowest *stable* buffer on a real interface is the owner's to
//! measure (BACKLOG §4c). Run it by hand (it takes a while and wants a quiet machine):
//!
//! `cargo test -p cantus-engine --test record_mode_bench -- --ignored --nocapture`

mod support;
use cantus_core::freeze;
use cantus_core::record_mode;
use cantus_core::*;
use cantus_engine::freeze::render::prepare;
use cantus_engine::*;
use std::time::Instant;

const SR: f64 = 48_000.0;
const TRACKS: usize = 64;

fn session() -> Project {
    session_of(TRACKS, 16)
}

/// `tracks` tracks, each `beats` beats long at 120 BPM (a note a beat).
fn session_of(tracks: usize, beats: i64) -> Project {
    let mut p = Project::new("bench");
    p.tempo_bpm = 120.0;
    p.tracks.clear();
    for i in 0..tracks {
        let mut t = p.make_track(format!("Track {i}"));
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(beats));
        for n in 0..beats as usize {
            let pitch = 40 + ((i * 3 + n * 5) % 36) as u8;
            c.notes
                .push(p.make_note(Ticks::beats(n as i64), Ticks::beats(1), pitch, 90));
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    // A strip each (the project's default strips run out at a few: make the rest).
    for i in 0..tracks {
        let strip = match p.tracks[i].strip {
            Some(s) if p.tracks_on_strip(s).count() == 1 => s,
            _ => {
                let s = p.make_strip(format!("Strip {i}"));
                let id = s.id;
                p.strips.push(s);
                p.tracks[i].strip = Some(id);
                id
            }
        };
        for kind in [
            EffectKind::Eq,
            EffectKind::Compressor,
            EffectKind::Delay,
            EffectKind::Space,
        ] {
            let fx = p.make_effect(kind);
            p.strip_mut(strip).unwrap().effects.push(fx);
        }
    }
    p
}

struct Stats {
    mean: f64,
    p99: f64,
    max: f64,
    over: usize,
    /// Frames a render's block was not read in time (disk underruns: silence in their place).
    late: u64,
    /// The blocks (indices) where frames came late.
    late_at: Vec<usize>,
    /// Whether the slowest block waited for the reader thread (an offline pass only).
    worst_waited: bool,
    /// The slowest block among those that never waited for the reader, against the budget.
    worst_dry: f64,
    /// Blocks that waited for the reader.
    waited: usize,
}

/// Process `blocks` blocks of `block` frames after `warm` ones, timing each call, in the
/// application's way (offline: the readers are waited for, so only the work is measured).
fn run(
    snap: &std::sync::Arc<Snapshot>,
    workers: Workers,
    block: usize,
    blocks: usize,
    realtime: bool,
) -> Stats {
    let effects = snap.effect_slots().count();
    let instruments = cantus_engine::inst::units_for(snap, SR as f32, |_, _| false);
    let (mut tx, rx_engine) =
        rtrb::RingBuffer::<EngineMessage>::new(64 + effects + instruments.len());
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(8192);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    p.set_workers(workers);
    // Real time: the readers are not waited for (a block late is silence, counted), and
    // the blocks are paced at the device rate.
    p.set_offline(!realtime);
    for slot in snap.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    for unit in instruments {
        tx.push(EngineMessage::AddInstrument(unit)).ok();
    }
    tx.push(EngineMessage::Automation(AutomationRuntime::build(snap)))
        .ok();
    tx.push(EngineMessage::SetSnapshotPlanned(
        snap.clone(),
        cantus_engine::graph::compile(snap),
    ))
    .ok();
    p.settle();
    while let Ok(ev) = rx.pop() {
        if let EngineEvent::PdcNeed { count, delay } = ev {
            tx.push(EngineMessage::PdcLines(cantus_engine::pdc::lines_for(
                count, delay,
            )))
            .ok();
            p.settle();
        }
    }
    tx.push(EngineMessage::Seek(Ticks::ZERO)).ok();
    let mut buf = vec![0f32; block * 2];
    p.process(&mut buf, 2);
    // Stopped a moment first, as a musician does before pressing play: the reader reads
    // where the playhead waits.
    std::thread::sleep(std::time::Duration::from_millis(50));
    tx.push(EngineMessage::Play).ok();
    let budget = block as f64 / SR * 1e6;
    // The warm-up at the device's pace too in real time: unpaced, the playhead ran a
    // second ahead of the reader in a few milliseconds (the first run's one underrun).
    let warm = 200;
    let start = Instant::now();
    // Paced by sleeping, never spinning (the thread may be in MMCSS, which throttles a
    // spinning thread): a block goes at most a millisecond and a half early.
    let pace = |i: usize| {
        if realtime {
            let due = std::time::Duration::from_secs_f64(i as f64 * budget / 1e6);
            loop {
                let now = start.elapsed();
                if now + std::time::Duration::from_micros(1500) >= due {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    };
    for i in 0..warm {
        pace(i);
        p.process(&mut buf, 2);
        while rx.pop().is_ok() {}
    }
    let mut times = Vec::with_capacity(blocks);
    let mut waits = Vec::with_capacity(blocks);
    let mut late_at = Vec::new();
    let late0 = p.stats().stream_late;
    for i in 0..blocks {
        pace(warm + i);
        let w0 = cantus_engine::freeze::offline_waits();
        let l0 = p.stats().stream_late;
        let t0 = Instant::now();
        p.process(&mut buf, 2);
        times.push(t0.elapsed().as_secs_f64() * 1e6);
        waits.push(cantus_engine::freeze::offline_waits() > w0);
        if p.stats().stream_late > l0 && late_at.len() < 8 {
            late_at.push(i);
        }
        while rx.pop().is_ok() {}
    }
    let worst_i = (0..times.len())
        .max_by(|a, b| times[*a].partial_cmp(&times[*b]).unwrap())
        .unwrap_or(0);
    let worst_waited = waits.get(worst_i).copied().unwrap_or(false);
    let worst_dry = times
        .iter()
        .zip(&waits)
        .filter(|(_, w)| !**w)
        .map(|(t, _)| *t)
        .fold(0.0, f64::max)
        / budget;
    let waited = waits.iter().filter(|w| **w).count();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let at = |q: f64| times[((times.len() as f64 * q) as usize).min(times.len() - 1)];
    Stats {
        mean: mean / budget,
        p99: at(0.99) / budget,
        max: times[times.len() - 1] / budget,
        over: times.iter().filter(|t| **t > budget).count(),
        late: p.stats().stream_late - late0,
        late_at,
        worst_waited,
        worst_dry,
        waited,
    }
}

/// The best of `n` runs by the 99th percentile (a quiet machine's figure).
fn best(snap: &std::sync::Arc<Snapshot>, workers: Workers, block: usize, blocks: usize) -> Stats {
    (0..3)
        .map(|_| run(snap, workers.clone(), block, blocks, false))
        .min_by(|a, b| a.p99.partial_cmp(&b.p99).unwrap())
        .unwrap()
}

#[test]
#[ignore = "a benchmark: run by hand on a quiet machine"]
fn sixty_four_tracks_with_effects_live_and_baked() {
    let (mut h, _p) = EngineHandle::manual(SR);
    let dir = std::env::temp_dir().join(format!("cantus-record-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    h.frozen.set_app_dir(dir.clone());
    let mut p = session();
    let live = h.compile(&p, 1);
    let plan = record_mode::plan(&p);
    println!("plan: {} baked, {} live", plan.bake.len(), plan.live.len());
    let t0 = Instant::now();
    for target in plan.bake.clone() {
        let job = prepare(&mut h, &p, target).expect("prepared");
        job.run(Vec::new(), Vec::new(), &RenderProgress::default())
            .expect("rendered");
        h.frozen.forget_missing();
        freeze::freeze_commands(&p, target)
            .unwrap()
            .apply(&mut p)
            .unwrap();
    }
    println!(
        "prepare to record: {:.1} s for {} renders",
        t0.elapsed().as_secs_f64(),
        plan.bake.len()
    );
    fn walk(d: &std::path::Path) -> u64 {
        std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| {
                let p = e.path();
                if p.is_dir() {
                    walk(&p)
                } else {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                }
            })
            .sum()
    }
    let bytes = walk(&dir);
    println!("renders on disk: {:.0} MB", bytes as f64 / 1_048_576.0);
    let baked = h.compile(&p, 2);
    assert!(baked
        .strips
        .iter()
        .any(|s| s.frozen.as_ref().is_some_and(|f| f.plays())));
    // The master baked: one render for the whole mix.
    let job = cantus_engine::record_mode::prepare_master(&mut h, &p, &[]);
    let t1 = Instant::now();
    let key = job.key.clone();
    job.run(&RenderProgress::default())
        .expect("the master rendered");
    println!("master render: {:.1} s", t1.elapsed().as_secs_f64());
    h.frozen.forget_missing();
    h.frozen.set_master(Some(key));
    let master = h.compile(&p, 3);
    assert!(master.master_frozen.as_ref().is_some_and(|f| f.plays()));
    h.frozen.set_master(None);
    let only_baked = std::env::var_os("CANTUS_BENCH_ONLY_BAKED").is_some();
    println!("\n| buffer | mode | mean load | p99 load | worst | worst waited for the reader | worst of blocks that did not wait | blocks that waited | blocks over budget |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let pool = Workers::parallel((cores - 1).clamp(1, 15));
    for block in [32usize, 64, 128, 256, 512] {
        let blocks = (SR as usize * 6 / block).max(2000);
        let mut rows = vec![
            ("baked, serial", &baked, Workers::SERIAL),
            ("baked master, serial", &master, Workers::SERIAL),
        ];
        if !only_baked {
            rows.insert(0, ("live, pool", &live, pool.clone()));
            rows.insert(0, ("live, serial", &live, Workers::SERIAL));
        }
        for (name, snap, workers) in rows {
            let s = best(snap, workers, block, blocks);
            println!(
                "| {block} ({:.2} ms) | {name} | {:.1} % | {:.1} % | {:.1} % | {} | {:.1} % | {} | {} of {blocks} |",
                block as f64 / SR * 1000.0,
                s.mean * 100.0,
                s.p99 * 100.0,
                s.max * 100.0,
                if s.worst_waited { "yes" } else { "no" },
                s.worst_dry * 100.0,
                s.waited,
                s.over
            );
        }
    }
    // Real time: paced at the device rate, the readers never waited for, 64 streams (or
    // one, the master baked): blocks over budget and frames a disk block came too late for.
    println!("\n| buffer | mode, real time (10 s) | p99 load | worst | blocks over budget | frames late (disk) |");
    println!("|---|---|---|---|---|---|");
    for block in [32usize, 64, 128, 256] {
        let blocks = SR as usize * 10 / block;
        for (name, snap) in [("baked", &baked), ("baked master", &master)] {
            let s = run(snap, Workers::SERIAL, block, blocks, true);
            println!(
                "| {block} | {name} | {:.1} % | {:.1} % | {} of {blocks} | {} |",
                s.p99 * 100.0,
                s.max * 100.0,
                s.over,
                s.late
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real-time run the recording mode is judged by (round 36): `CANTUS_BENCH_STREAMS`
/// tracks (64 by default; 128 for headroom), each a 64-second song with four effects, baked;
/// then 60 s at each buffer size, paced at the device's rate, the readers never waited for:
/// blocks over budget and frames a disk block came too late for (underruns).
///
/// `CANTUS_BENCH_STREAMS=128 cargo test -p cantus-engine --test record_mode_bench real_time -- --ignored --nocapture`
#[test]
#[ignore = "a benchmark: run by hand on a quiet machine"]
fn real_time_streams_for_a_minute() {
    let streams: usize = std::env::var("CANTUS_BENCH_STREAMS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64);
    let seconds: usize = std::env::var("CANTUS_BENCH_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let (mut h, _p) = EngineHandle::manual(SR);
    // Kept between runs (a 128-stream preparation takes minutes): the renders are found by
    // their keys. `CANTUS_BENCH_CLEAN` starts afresh.
    let dir = std::env::temp_dir().join(format!("cantus-record-rt-{streams}"));
    if std::env::var_os("CANTUS_BENCH_CLEAN").is_some() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    std::fs::create_dir_all(&dir).unwrap();
    h.frozen.set_app_dir(dir.clone());
    // 128 beats at 120 BPM: 64 s of song, so a minute reads the disk all the way.
    let mut p = session_of(streams, 128);
    let plan = record_mode::plan(&p);
    let t0 = Instant::now();
    for target in plan.bake.clone() {
        let key = freeze::content_key(&p, target).unwrap();
        if h.frozen.find(&key, SR as u32).is_none() {
            let job = prepare(&mut h, &p, target).expect("prepared");
            job.run(Vec::new(), Vec::new(), &RenderProgress::default())
                .expect("rendered");
            h.frozen.forget_missing();
        }
        freeze::freeze_commands(&p, target)
            .unwrap()
            .apply(&mut p)
            .unwrap();
    }
    println!(
        "{streams} streams: prepared in {:.1} s ({} renders)",
        t0.elapsed().as_secs_f64(),
        plan.bake.len()
    );
    let baked = h.compile(&p, 2);
    // This thread as the audio callback is (MMCSS "Pro Audio" on Windows, the real-time
    // policy where allowed): what the device's thread gets in the program.
    let promoted = cantus_rt::thread::promote_callback(
        cantus_rt::thread::AudioHost::from_driver_name("WASAPI"),
        std::time::Duration::from_millis(1),
    );
    println!("the bench thread: {promoted}");
    println!("\n| buffer | streams | p99 load | worst | blocks over budget | frames late (disk) | first late blocks |");
    println!("|---|---|---|---|---|---|---|");
    for block in [32usize, 64, 128, 256, 512, 1024] {
        let blocks = SR as usize * seconds / block;
        let s = run(&baked, Workers::SERIAL, block, blocks, true);
        println!(
            "| {block} | {streams} | {:.1} % | {:.1} % | {} of {blocks} | {} | {:?} |",
            s.p99 * 100.0,
            s.max * 100.0,
            s.over,
            s.late,
            s.late_at
        );
    }
}
