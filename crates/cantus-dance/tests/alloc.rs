//! What drawing a character allocates (round 35: the stage builder measured it; the
//! songbird made up to 779 allocations a frame before the rig drew through pooled pens).
//! A counting allocator around the system one, as the frame budget's; this file holds one
//! test so nothing else allocates while it counts.
//!
//! What remains is tiny-skia's own: every `fill_path` allocates inside (its edge list and
//! coverage runs — 3 for a bare fill of a ready path, more for a long path). The counting
//! allocator asks the rig whether it is inside a fill (`rig::draw::in_fill`): every
//! allocation outside one is the rig's own, and a frame must make none.

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, pose_at, Character, DanceFrame, Mood, Move, Pose};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
/// Allocations made outside tiny-skia's fills (the rig's own: must be none).
static OURS: AtomicUsize = AtomicUsize::new(0);

fn counted() {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    if !cantus_dance::rig::draw::in_fill() {
        OURS.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        counted();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        counted();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed) - before
}

#[test]
fn drawing_a_character_allocates_little() {
    // One bare fill of a ready path with no transform: what tiny-skia itself allocates.
    let mut pm = tiny_skia::Pixmap::new(600, 600).expect("a stage");
    let path = tiny_skia::PathBuilder::from_circle(300.0, 300.0, 80.0).expect("a circle");
    let paint = tiny_skia::Paint::default();
    let fill = count(|| {
        pm.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        )
    });
    let stage = Stage::new(600, 600, Background::Transparent);
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let base = Pose {
            from: None,
            blend: 1.0,
            ..pose_at(
                &DanceFrame {
                    playing: true,
                    ..DanceFrame::default()
                },
                ch,
                0.0,
            )
        };
        let mut worst = (0, 0);
        for mv in Move::ALL {
            for phase in [0.1, 0.6] {
                let pose = Pose {
                    mv,
                    phase,
                    energy: 1.0,
                    mood: Mood::Excited,
                    ..base
                };
                render_into(&mut pm, &stage, &[(ch, pose)]);
                let fills0 = cantus_dance::rig::draw::fills_made();
                let ours0 = OURS.load(Ordering::Relaxed);
                let n = count(|| render_into(&mut pm, &stage, &[(ch, pose)]));
                let fills = cantus_dance::rig::draw::fills_made() - fills0;
                let ours = OURS.load(Ordering::Relaxed) - ours0;
                // Everything allocated is tiny-skia's, inside its fills; the rig's paths
                // come from the pool, so nothing of ours, frame after frame.
                assert_eq!(
                    ours,
                    0,
                    "{}: {mv:?} allocated {ours} times outside tiny-skia's fills ({n} in all, {fills} fills)",
                    ch.info().id
                );
                worst = worst.max((n, fills));
            }
        }
        println!(
            "{}: at most {} allocations a frame for {} fills (a bare tiny-skia fill makes {fill})",
            ch.info().id,
            worst.0,
            worst.1
        );
    }
}
