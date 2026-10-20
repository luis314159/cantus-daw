//! The real-time rules on Cantus's side of an isolated plug-in (ADR 0005, ADR 0063): a
//! counting allocator proves that the audio half — the copy into the slot, the lock-step
//! exchange, the copy back — allocates and frees nothing, in a normal block, a late one, a
//! dry one after its helper died, and in a reset and a stop.

mod support;

use cantus_host::{Backend, Loaded, NoteEvent, NoteKind, ParamChange, TransportInfo};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use support::*;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: forwards to the system allocator; only counts on the thread under test.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: the caller's layout, passed on.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: the caller's pointer and layout, passed on.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: as above.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations_in(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    ALLOCATIONS.load(Ordering::SeqCst) - before
}

#[test]
fn the_audio_half_of_an_isolated_plugin_never_allocates() {
    let _g = serial();
    let pool = pool();
    let transport = TransportInfo {
        playing: true,
        recording: false,
        tempo_bpm: 120.0,
        tempo_inc: 0.0,
        position_beats: 4.0,
        position_samples: 96_000,
        position_seconds: 2.0,
        bar_start_beats: 4.0,
        bar_number: 1,
        time_sig_num: 4,
        time_sig_den: 4,
        loop_beats: None,
        loop_seconds: None,
    };
    for (id, latency_id) in [(GAIN_ID, Some(P_LATENCY)), (SYNTH_ID, None)] {
        let info = info(id);
        let Loaded {
            mut main,
            mut audio,
        } = bridged(&pool, info.format, &format!("own-rt-{id}"), false)
            .load(&info, setup())
            .unwrap();
        if let Some(l) = latency_id {
            // With a latency (a restart brings it in), so the dry path's line is in use.
            let mut out = cantus_host::ParamOut::new();
            main.flush(
                &[ParamChange {
                    offset: 0,
                    id: l,
                    value: 64.0,
                }],
                &mut out,
            );
            drop(audio);
            audio = loop {
                match main.restart() {
                    Ok(Some(a)) => break a,
                    Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                    Err(e) => panic!("restart: {e}"),
                }
            };
            assert_eq!(main.latency(), 64);
        }
        let (mut l, mut r) = (noise(512, 1), noise(512, 2));
        let key = (noise(512, 3), noise(512, 4));
        let params = [ParamChange {
            offset: 7,
            id: 0,
            value: -1.5,
        }];
        let notes = [NoteEvent {
            offset: 3,
            note_id: 1,
            channel: 0,
            key: 60,
            kind: NoteKind::On { velocity: 0.5 },
        }];
        let mut out = cantus_host::ParamOut::new();
        // Warm: the first call of anything may touch lazily made statics.
        audio.process(
            cantus_host::Audio {
                left: &mut l,
                right: &mut r,
                key: Some((&key.0, &key.1)),
                transport: Some(&transport),
            },
            &params,
            &notes,
            &mut out,
        );
        let n = allocations_in(|| {
            for _ in 0..50 {
                out.clear();
                audio.process(
                    cantus_host::Audio {
                        left: &mut l,
                        right: &mut r,
                        key: Some((&key.0, &key.1)),
                        transport: Some(&transport),
                    },
                    &params,
                    &notes,
                    &mut out,
                );
            }
            audio.reset();
        });
        assert_eq!(n, 0, "{id}: {n} allocations in normal blocks");
        // The helper dies: the dry blocks after it allocate nothing either.
        let fault_id = if id == GAIN_ID { P_FAULT } else { S_FAULT };
        audio.process(
            cantus_host::Audio {
                left: &mut l,
                right: &mut r,
                key: None,
                transport: None,
            },
            &fault(fault_id, ABORT_PROCESS),
            &[],
            &mut out,
        );
        std::thread::sleep(Duration::from_millis(50));
        let n = allocations_in(|| {
            for _ in 0..50 {
                out.clear();
                audio.process(
                    cantus_host::Audio {
                        left: &mut l,
                        right: &mut r,
                        key: None,
                        transport: None,
                    },
                    &params,
                    &notes,
                    &mut out,
                );
            }
            audio.stop();
        });
        assert_eq!(n, 0, "{id}: {n} allocations once its helper is gone");
    }
}
