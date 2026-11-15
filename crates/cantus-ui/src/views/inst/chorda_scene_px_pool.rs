//! r40:scene-px — the bake's workers: **one pool for the app's life**, made when the first
//! photographic picture is baked, sized to leave the machine's cores to the audio and the
//! interface (its physical cores less two, at most six), its threads **below normal
//! priority** (`cantus_rt::thread::lower_background_thread`: never MMCSS, never above the
//! interface), and let go after half a minute with no bake — so nothing of it is left at
//! rest. A bake runs wholly on the pool's threads: the caller (the scene's worker) only
//! waits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the pool outlives its last bake.
pub const IDLE: Duration = Duration::from_secs(30);

struct Held {
    pool: Arc<rayon::ThreadPool>,
    last: Instant,
}

static POOL: Mutex<Option<Held>> = Mutex::new(None);
static REAPER: AtomicBool = AtomicBool::new(false);

/// The pool's size: the physical cores (the logical ones halved, SMT assumed) less two —
/// one for the audio, one for the interface — at least one, at most six.
pub fn workers() -> usize {
    let logical = std::thread::available_parallelism().map_or(2, |n| n.get());
    (logical / 2).saturating_sub(2).clamp(1, 6)
}

/// The pool, made if there is none; its last use is now.
pub fn pool() -> Option<Arc<rayon::ThreadPool>> {
    let mut g = POOL.lock().ok()?;
    if g.is_none() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers())
            .thread_name(|i| format!("cantus-scene-px-{i}"))
            .start_handler(|_| {
                cantus_rt::thread::lower_background_thread();
            })
            .build()
            .ok()?;
        *g = Some(Held {
            pool: Arc::new(pool),
            last: Instant::now(),
        });
        reap();
    }
    let held = g.as_mut()?;
    held.last = Instant::now();
    Some(held.pool.clone())
}

/// Whether the pool stands now (tests: it goes once idle).
pub fn alive() -> bool {
    POOL.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Let the pool go now if no bake holds it (tests; the idle reaper does it after `IDLE`).
pub fn release() {
    if let Ok(mut g) = POOL.lock() {
        if g.as_ref().is_some_and(|h| Arc::strong_count(&h.pool) == 1) {
            *g = None;
        }
    }
}

/// A watcher that lets the pool go after `IDLE` with no bake, then ends (one at a time).
fn reap() {
    if REAPER.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("cantus-scene-px-reaper".into())
        .spawn(|| loop {
            std::thread::sleep(IDLE / 3);
            let Ok(mut g) = POOL.lock() else {
                REAPER.store(false, Ordering::Release);
                return;
            };
            let gone = match g.as_ref() {
                None => true,
                Some(h) if h.last.elapsed() >= IDLE && Arc::strong_count(&h.pool) == 1 => {
                    *g = None;
                    true
                }
                Some(_) => false,
            };
            if gone {
                REAPER.store(false, Ordering::Release);
                return;
            }
        });
    if spawned.is_err() {
        REAPER.store(false, Ordering::Release);
    }
}
