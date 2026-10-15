//! Speculum through the whole engine (ADR 0056): a pure pass-through whatever it is set
//! to; and its data path — every frame of a watched Speculum's own signal reaches its scope
//! ring, with the song's position as heard at the slot, several at once, a full ring saying
//! what it lost, a reused ring giving only its new owner's frames, nothing allocated on
//! the audio thread.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::speculum::{ScopeChunk, MAX_SCOPES};
use cantus_engine::{EngineHandle, Processor};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use support::*;

// ---- allocations on the audio threads (the callback's and the pool's workers') --------

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static WORKER: Cell<bool> = const { Cell::new(false) };
}

/// Armed while the measured blocks run: the callback's allocations (its `ON`) and any pool
/// worker's (its `WORKER`) are counted here.
static ARMED: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicU64 = AtomicU64::new(0);

fn mark_worker(_slot: usize, serving: bool) {
    WORKER.with(|w| w.set(serving));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mine = ON.try_with(Cell::get).unwrap_or(false);
        let worker = WORKER.try_with(Cell::get).unwrap_or(false) && ARMED.load(Relaxed);
        if mine || worker {
            COUNT.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn speculum_set_any_way_renders_exactly_as_no_speculum() {
    let reference = render_of(&demo_project());
    for at_max in [false, true] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Analyzer);
        if at_max {
            let s = strip_of(&mut p, 0);
            let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
            for info in EffectKind::Analyzer.params() {
                slot.spec.set(info.key, info.max);
            }
        }
        assert_null(&reference, &render_of(&p), 0.0, &format!("at max {at_max}"));
    }
}

fn engine(p: &Project) -> (EngineHandle, Processor) {
    let (mut h, mut proc) = EngineHandle::manual(48_000.0);
    let s = h.compile(p, 1);
    h.set_snapshot(s);
    h.play();
    // Let the processor take the snapshot and the units.
    let mut buf = vec![0f32; 2 * 64];
    proc.process(&mut buf, 2);
    (h, proc)
}

/// Review 1: a master Speculum keeps measuring when the master's effects are switched off
/// or all bypassed — the moment a mastering engineer compares — every frame, the same
/// frames the master plays.
#[test]
fn a_master_speculum_measures_with_the_chain_off_or_bypassed() {
    for case in ["off", "bypassed"] {
        let mut p = demo_project();
        let slot = p.make_effect(EffectKind::Analyzer);
        let id = slot.id;
        p.master.effects.push(slot);
        if case == "off" {
            p.master.fx_enabled = false;
        } else {
            p.master.effects[0].bypass = true;
        }
        let (mut h, mut proc) = engine(&p);
        assert!(h.watch_scope(id));
        let mut played = 0usize;
        let mut seen = 0usize;
        let mut loud = 0f32;
        for k in 0..20 {
            let mut buf = vec![0f32; 2 * 512];
            proc.process(&mut buf, 2);
            if k > 0 {
                played += 512;
            }
            h.drain_scope(id, |c: &ScopeChunk| {
                if k > 0 {
                    seen += c.frames as usize;
                    loud = loud.max(
                        c.l[..c.frames as usize]
                            .iter()
                            .fold(0f32, |m, v| m.max(v.abs())),
                    );
                }
            });
        }
        assert_eq!(seen, played, "{case}: every frame");
        assert!(loud > 0.01, "{case}: the music, not silence");
    }
}

/// Everything a ring gave, in order: frames (interleaved), each chunk's (beat, frames),
/// and the frames it said it lost.
#[derive(Default)]
struct Drained {
    frames: Vec<f32>,
    beats: Vec<(f64, usize)>,
    lost: u64,
}

impl Drained {
    fn take(&mut self, h: &mut EngineHandle, id: EffectId) {
        let (frames, beats) = (&mut self.frames, &mut self.beats);
        self.lost += h.drain_scope(id, |c: &ScopeChunk| {
            for i in 0..c.frames as usize {
                frames.push(c.l[i]);
                frames.push(c.r[i]);
            }
            if let Some(b) = c.beat {
                beats.push((b, c.frames as usize));
            }
        });
    }
}

/// Run `blocks` blocks of irregular sizes, keeping the watched ones watched and draining
/// them; returns what was played and what each ring gave.
fn play(
    h: &mut EngineHandle,
    proc: &mut Processor,
    ids: &[EffectId],
    blocks: usize,
) -> (Vec<f32>, Vec<Drained>) {
    let sizes = [256usize, 333, 1024, 96, 512, 17];
    let mut played = Vec::new();
    let mut got: Vec<Drained> = ids.iter().map(|_| Drained::default()).collect();
    for k in 0..blocks {
        let n = sizes[k % sizes.len()];
        let mut buf = vec![0f32; 2 * n];
        proc.process(&mut buf, 2);
        played.extend_from_slice(&buf);
        for (d, id) in got.iter_mut().zip(ids) {
            d.take(h, *id);
            h.watch_scope(*id);
        }
        h.release_unwatched_scopes();
    }
    (played, got)
}

/// What came out of a ring is every frame in order — all but the chunk still being filled
/// (fewer than 64 frames) — and `scale` times the reference.
fn every_frame(what: &str, seen: &[f32], reference: &[f32], scale: f32) {
    assert!(
        seen.len() <= reference.len(),
        "{what}: more than was played"
    );
    assert!(
        reference.len() - seen.len() < 2 * 64,
        "{what}: {} frames missing",
        (reference.len() - seen.len()) / 2
    );
    for (i, (a, b)) in seen.iter().zip(reference).enumerate() {
        assert!(
            (a * scale - b).abs() <= 1e-6,
            "{what}: sample {i}: {a} vs {b}"
        );
    }
}

/// The scale from a strip's signal to the master's output: the reference's loudest sample
/// over the ring's at the same place.
fn scale_of(seen: &[f32], reference: &[f32]) -> f32 {
    let i = (0..seen.len())
        .max_by(|a, b| seen[*a].abs().total_cmp(&seen[*b].abs()))
        .unwrap();
    reference[i] / seen[i]
}

/// The lead alone on a strip at 0 dB and centred, a Speculum on it (bypassed or not), one
/// on the master: the master's output is the strip's signal times a constant.
fn lead_and_master(bypassed: bool) -> (Project, EffectId, EffectId) {
    let mut p = modfx::lead_only();
    {
        let s = strip_of(&mut p, 0);
        s.gain_db = 0.0;
        s.pan = 0.0;
    }
    let strip = add_effect(&mut p, EffectKind::Analyzer);
    strip_of(&mut p, 0).effects[0].bypass = bypassed;
    let slot = p.make_effect(EffectKind::Analyzer);
    let master = slot.id;
    p.master.effects.push(slot);
    (p, master, strip)
}

/// Every frame a watched Speculum sees reaches its ring, in order, once — on the master and
/// on a strip, bypassed or not, several at once — and the song's position runs on with the
/// frames.
#[test]
fn every_frame_of_a_watched_speculum_reaches_its_ring() {
    for bypassed in [false, true] {
        let (p, master, strip) = lead_and_master(bypassed);
        let (mut h, mut proc) = engine(&p);
        assert!(h.watch_scope(master) && h.watch_scope(strip), "two at once");
        let (played, got) = play(&mut h, &mut proc, &[master, strip], 60);
        assert!(played.iter().any(|v| v.abs() > 0.01), "the demo plays");
        // The watch lands at the start of the first block: the rings begin with it.
        every_frame("master", &got[0].frames, &played, 0.999_999_94);
        let s = &got[1].frames;
        let reference = &played[..];
        every_frame(
            &format!("strip (bypassed {bypassed})"),
            s,
            reference,
            scale_of(s, reference),
        );
        let per_frame = 128.0 / 60.0 / 48_000.0;
        for d in &got {
            assert_eq!(d.lost, 0);
            for w in d.beats.windows(2) {
                let ((b0, n0), (b1, _)) = (w[0], w[1]);
                assert!(
                    (b1 - b0 - n0 as f64 * per_frame).abs() < 1e-9,
                    "{b0} + {n0} → {b1}"
                );
            }
        }
    }
}

/// What nobody watches is let go — its ring receives nothing more, looked at in the ring
/// itself — and a ninth Speculum is refused while eight are watched.
#[test]
fn an_unwatched_speculum_is_let_go_and_eight_is_the_most() {
    let (p, master, strip) = lead_and_master(false);
    let (mut h, mut proc) = engine(&p);
    assert!(h.watch_scope(master));
    let mut buf = vec![0f32; 2 * 512];
    proc.process(&mut buf, 2);
    assert!(h.scope_ring_len(0) > 0, "watched: the ring fills");
    h.release_unwatched_scopes();
    // Not asked for since: released, and what it held is thrown away when it is reused.
    h.release_unwatched_scopes();
    let before = h.scope_ring_len(0);
    proc.process(&mut buf, 2);
    proc.process(&mut buf, 2);
    assert_eq!(h.scope_ring_len(0), before, "nothing more goes in");
    for i in 0..MAX_SCOPES as u64 {
        assert!(h.watch_scope(EffectId(1_000 + i)));
    }
    assert!(!h.watch_scope(strip), "a ninth");
}

/// A ring handed to another Speculum gives it only its own frames, though a block of the
/// last one's was still going in when it changed hands.
#[test]
fn a_reused_ring_gives_only_its_new_owners_frames() {
    let (p, master, strip) = lead_and_master(false);
    let (mut h, mut proc) = engine(&p);
    assert!(h.watch_scope(master));
    let mut buf = vec![0f32; 2 * 500];
    proc.process(&mut buf, 2);
    // The master's chunks are in ring 0, undrained; ring 0 goes to the strip.
    h.release_unwatched_scopes();
    h.release_unwatched_scopes();
    assert!(h.watch_scope(strip));
    proc.process(&mut buf, 2);
    let mut owners = Vec::new();
    h.drain_scope(strip, |c| owners.push(c.owner));
    assert!(!owners.is_empty());
    assert!(owners.iter().all(|o| *o == strip), "{owners:?}");
}

/// The beat a chunk carries is the beat its frames are heard at: behind a latent insert on
/// the same strip (Tonsor at 16× with the guard, 93 samples) the stamps are 93 samples'
/// worth earlier than without it.
#[test]
fn the_beat_stamp_counts_the_compensation_the_slot_waits() {
    let stamps = |latent: bool| {
        let mut p = modfx::lead_only();
        if latent {
            let fx = add_effect(&mut p, EffectKind::Clipper);
            modfx::set_all(&mut p, fx, &[("os", 4.0), ("tp", 1.0)]);
        }
        let id = add_effect(&mut p, EffectKind::Analyzer);
        let (mut h, mut proc) = engine(&p);
        assert!(h.watch_scope(id));
        let (_, got) = play(&mut h, &mut proc, &[id], 20);
        got.into_iter().next().unwrap().beats
    };
    let (plain, late) = (stamps(false), stamps(true));
    let per_frame = 128.0 / 60.0 / 48_000.0;
    let lat = cantus_dsp::tonsor::latency_for(&cantus_dsp::tonsor::TonsorSettings {
        oversampling: 16,
        true_peak: true,
        ..Default::default()
    });
    assert_eq!(lat, 93);
    for ((a, _), (b, _)) in plain.iter().zip(&late).take(50) {
        assert!((a - b - lat as f64 * per_frame).abs() < 1e-9, "{a} vs {b}");
    }
}

/// A ring that is not drained in time says how many frames it lost, with the next chunk
/// that gets in.
#[test]
fn a_full_ring_says_what_it_lost() {
    let (p, master, _) = lead_and_master(false);
    let (mut h, mut proc) = engine(&p);
    assert!(h.watch_scope(master));
    let mut buf = vec![0f32; 2 * 1_000];
    // 100 000 frames, never drained: the ring holds 65 536.
    for _ in 0..100 {
        proc.process(&mut buf, 2);
    }
    let mut first = Drained::default();
    first.take(&mut h, master);
    assert_eq!(
        first.frames.len() / 2,
        65_536,
        "the ring's capacity, in frames"
    );
    for _ in 0..2 {
        proc.process(&mut buf, 2);
    }
    let mut next = Drained::default();
    next.take(&mut h, master);
    // What went missing between: everything played after the ring filled, but for the
    // chunk that was being filled.
    let lost = first.lost + next.lost;
    assert!(
        (100_000 - 65_536 - 64..=100_000 - 65_536).contains(&lost),
        "lost {lost}"
    );
}

/// Copying watched Speculums' signals allocates nothing on the audio thread — the callback's
/// or the pool's workers', which run the strips.
#[test]
fn the_copy_allocates_nothing() {
    cantus_engine::processor::on_worker_start(mark_worker);
    let (p, master, strip) = lead_and_master(false);
    let (mut h, mut proc) = engine(&p);
    proc.set_workers(cantus_engine::processor::Workers::live(2));
    h.watch_scope(master);
    h.watch_scope(strip);
    let mut buf = vec![0f32; 2 * 512];
    // Warm: the messages land, the pool starts, the first blocks run.
    for _ in 0..16 {
        proc.process(&mut buf, 2);
        h.drain_scope(master, |_| {});
        h.drain_scope(strip, |_| {});
    }
    COUNT.store(0, Relaxed);
    ARMED.store(true, Relaxed);
    ON.with(|on| on.set(true));
    for _ in 0..30 {
        proc.process(&mut buf, 2);
    }
    ON.with(|on| on.set(false));
    ARMED.store(false, Relaxed);
    assert_eq!(COUNT.load(Relaxed), 0, "allocations on the audio threads");
}
