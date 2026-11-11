//! The engine's constructors fit a small stack (round 39's start-up crash).
//!
//! The integrated tree's release build overflowed Windows' 1 MB main thread at start
//! (`thread 'main' has overflowed its stack`, right after the EcoQoS line): the processor
//! (63.6 KB inline) was built on the thread that opens the device and moved by value through
//! `EngineHandle::start` and cpal's generic stream builders — 1 MB of frames between them in
//! the release build. Tests run on 2 MB threads, so the suite never saw it. These build the
//! engine as the application does, on a thread with a quarter of Windows' main-thread stack:
//! a value that grows the frames back fails here, in either profile.
//!
//! What only a device can run (`EngineHandle::start`'s cpal path) is held by the size bounds
//! in `processor.rs` (`stack_size`): the driver it moves holds the processor boxed.

use cantus_engine::{EngineHandle, Processor};

/// A quarter of Windows' main-thread stack: the engine's constructors get no more of it.
const STACK: usize = 256 << 10;

fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("small-stack".into())
        .stack_size(STACK)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the constructor overflowed a 256 KB stack")
}

#[test]
fn the_engine_is_built_on_a_small_stack() {
    on_small_stack(|| {
        // What `EngineHandle::start` builds, without a device: the handle, the processor
        // with its pool, and the processor boxed as the output driver holds it.
        let (handle, processor) = EngineHandle::manual(48_000.0);
        let boxed = Box::new(processor);
        drop(boxed);
        drop(handle);
        let (_h, p) = EngineHandle::manual(44_100.0);
        let p: Box<Processor> = Box::new(p);
        drop(p);
    });
}

#[test]
fn a_headless_handle_is_built_on_a_small_stack() {
    on_small_stack(|| drop(EngineHandle::dummy()));
}
