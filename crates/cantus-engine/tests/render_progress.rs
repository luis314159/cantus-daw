//! The offline render's progress and Stop (round 22, for the export dialog): the frames
//! done reach the total, the result is the plain render's to the bit, and a cancel stops it.

mod support;
use cantus_core::*;
use cantus_engine::*;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use support::*;

#[test]
fn a_render_reports_its_progress_and_stops_when_asked() {
    let snap = Arc::new(Snapshot::compile(&demo_project(), 1));
    let opts = RenderOptions {
        tail_seconds: 0.25,
        ..Default::default()
    };
    let plain = render(snap.clone(), &opts).unwrap();
    let progress = RenderProgress::default();
    let r = render_with_progress(snap.clone(), &opts, Vec::new(), Vec::new(), &progress).unwrap();
    assert_eq!(r.samples, plain.samples, "progress changes no sample");
    let total = progress.total.load(Relaxed);
    assert_eq!(total as usize, r.frames());
    assert_eq!(progress.done.load(Relaxed), total);
    assert_eq!(progress.fraction(), 1.0);
    let stopped = RenderProgress::default();
    stopped.cancel();
    assert!(matches!(
        render_with_progress(snap, &opts, Vec::new(), Vec::new(), &stopped),
        Err(RenderError::Cancelled)
    ));
}
