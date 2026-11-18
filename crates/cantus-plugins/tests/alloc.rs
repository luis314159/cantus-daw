//! The audio thread never allocates (ADR 0005): every plug-in's `process`, through the
//! CLAP entry point, with every parameter automated inside the block, is watched by a
//! counting allocator.

mod common;

use common::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.realloc(ptr, layout, new)
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn process_never_allocates_even_under_automation_of_every_parameter() {
    for kind in kinds() {
        let plug = Plug::new(kind);
        plug.activate(1024);
        let table = cantus_plugins::params::table(kind);
        // Every parameter moved to a quarter and then three quarters of its range, at
        // offsets spread through the block.
        let mut blocks: Vec<Vec<Ev>> = Vec::new();
        for frac in [0.25, 0.75] {
            let evs: Vec<Ev> = table
                .params
                .iter()
                .enumerate()
                .filter(|(_, p)| p.role == cantus_plugins::params::Role::Spec)
                .map(|(i, p)| {
                    let v = p.min + (p.max - p.min) * frac;
                    ((i * 7 % 1000) as u32, p.id, p.to_clap(v))
                })
                .collect();
            let mut evs = evs;
            evs.sort_by_key(|e| e.0);
            blocks.push(evs);
        }
        let (mut l, mut r) = signal(1024, 3);
        let (kl, kr) = signal(1024, 4);
        // Build every host-side buffer before watching: only the plug-in is counted.
        let lists: Vec<EventList> = blocks.iter().map(|b| EventList::new(b)).collect();
        for list in &lists {
            WATCHING.with(|w| w.set(true));
            COUNT.with(|c| c.set(0));
            plug.process_list(&mut l, &mut r, Some((&kl, &kr)), list);
            WATCHING.with(|w| w.set(false));
            let n = COUNT.with(|c| c.get());
            assert_eq!(n, 0, "{kind:?} allocated {n} times in process");
        }
    }
}

/// fx/final — the snap where the host's song jumps (a start from stop, a seek, an offline
/// render begun) allocates nothing either, for every plug-in, with every parameter moved in
/// the block it jumps in.
#[test]
fn a_jump_snaps_without_allocating() {
    use clap_sys::events::*;
    use clap_sys::fixedpoint::CLAP_BEATTIME_FACTOR;
    let transport = |beats: f64, playing: bool| clap_event_transport {
        header: clap_event_header {
            size: std::mem::size_of::<clap_event_transport>() as u32,
            time: 0,
            space_id: CLAP_CORE_EVENT_SPACE_ID,
            type_: CLAP_EVENT_TRANSPORT,
            flags: 0,
        },
        flags: CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
            | if playing {
                CLAP_TRANSPORT_IS_PLAYING
            } else {
                0
            },
        song_pos_beats: (beats * CLAP_BEATTIME_FACTOR as f64).round() as i64,
        song_pos_seconds: 0,
        tempo: 120.0,
        tempo_inc: 0.0,
        loop_start_beats: 0,
        loop_end_beats: 0,
        loop_start_seconds: 0,
        loop_end_seconds: 0,
        bar_start: 0,
        bar_number: 0,
        tsig_num: 4,
        tsig_denom: 4,
    };
    for kind in kinds() {
        let plug = Plug::new(kind);
        plug.activate(1024);
        let table = cantus_plugins::params::table(kind);
        let evs: Vec<Ev> = table
            .params
            .iter()
            .filter(|p| p.role == cantus_plugins::params::Role::Spec)
            .map(|p| (0, p.id, p.to_clap(p.min + (p.max - p.min) * 0.6)))
            .collect();
        let moved = EventList::new(&evs);
        let still = EventList::new(&[]);
        let (mut l, mut r) = signal(1024, 3);
        // Stopped, playing (a start), on (no jump), sought (a jump), then rendered offline.
        let steps: [(f64, bool, &EventList); 4] = [
            (0.0, false, &still),
            (0.0, true, &moved),
            (32.0, true, &moved),
            (32.0 + 1024.0 * 2.0 / 48_000.0, true, &moved),
        ];
        for (i, (beats, playing, list)) in steps.iter().enumerate() {
            plug.transport.set(Some(transport(*beats, *playing)));
            if i == 3 {
                assert!(plug.set_render(true));
            }
            WATCHING.with(|w| w.set(true));
            COUNT.with(|c| c.set(0));
            plug.process_list(&mut l, &mut r, None, list);
            WATCHING.with(|w| w.set(false));
            let n = COUNT.with(|c| c.get());
            assert_eq!(n, 0, "{kind:?} allocated {n} times at step {i}");
        }
    }
}
