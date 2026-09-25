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
