//! Memory an exported effect asks for while it runs (round 39, ADR 0112 with ADR 0022's
//! path): Aula's kernels, built when the host automates its response or a shape. Inside
//! Cantus the interface's thread answers the engine; a plug-in has no such thread to lend,
//! so it keeps one of its own: the audio thread pushes a need into a ring, the builder
//! makes the memory and pushes it back, and what the unit hands back goes to the builder
//! to be freed. Nothing is allocated, freed, locked or waited for on the audio thread.

use cantus_engine::{EffectMemory, EffectUnit, NeedTicket};
use rtrb::{Consumer, Producer, RingBuffer};

/// Room in each ring: needs are asked once each, so a handful is plenty.
const ROOM: usize = 16;

/// The audio thread's ends.
pub struct Memory {
    // In this order, so they are dropped before the builder is joined in `Drop`.
    needs: Option<Producer<NeedTicket>>,
    built: Option<Consumer<EffectMemory>>,
    garbage: Option<Producer<EffectMemory>>,
    /// The builder, joined when the processor goes (main thread): a host may unload the
    /// library right after, and a thread still running its code would crash it.
    thread: Option<std::thread::JoinHandle<()>>,
    /// Memory that found the garbage ring full: kept (never freed here) until it has room.
    held: Option<EffectMemory>,
}

impl std::fmt::Debug for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Memory")
    }
}

impl Memory {
    /// The rings and the builder thread, for a unit at `sample_rate`. Main thread.
    pub fn start(sample_rate: f32) -> Memory {
        let (needs, mut need_rx) = RingBuffer::<NeedTicket>::new(ROOM);
        let (mut built_tx, built) = RingBuffer::<EffectMemory>::new(ROOM);
        let (garbage, mut garbage_rx) = RingBuffer::<EffectMemory>::new(ROOM);
        let spawned = std::thread::Builder::new()
            .name("cantus-plugin-memory".into())
            .spawn(move || loop {
                let mut idle = true;
                while let Ok(need) = need_rx.pop() {
                    idle = false;
                    let m = EffectMemory::build(need.need(), sample_rate);
                    // The audio thread takes one each block; a full ring waits here.
                    let mut m = Some(m);
                    while let Some(x) = m.take() {
                        match built_tx.push(x) {
                            Ok(()) => {}
                            Err(rtrb::PushError::Full(x)) => {
                                if built_tx.is_abandoned() {
                                    break;
                                }
                                m = Some(x);
                                std::thread::sleep(std::time::Duration::from_millis(2));
                            }
                        }
                    }
                }
                while let Ok(g) = garbage_rx.pop() {
                    idle = false;
                    drop(g);
                }
                if need_rx.is_abandoned() && garbage_rx.is_abandoned() {
                    while let Ok(g) = garbage_rx.pop() {
                        drop(g);
                    }
                    return;
                }
                if idle {
                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
            });
        let thread = match spawned {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("cantus-plugins: no memory builder ({e}); responses stay as they start");
                None
            }
        };
        Memory {
            needs: Some(needs),
            built: Some(built),
            garbage: Some(garbage),
            thread,
            held: None,
        }
    }

    /// Ask for what the unit's settings need (after a configure). Audio thread.
    pub fn ask(&mut self, unit: &mut EffectUnit) {
        let Some(needs) = self.needs.as_mut() else {
            return;
        };
        if let Some(need) = unit.take_request() {
            if needs.push(need).is_err() {
                unit.forget_request();
            }
        }
    }

    /// Hand the unit what was built; what it gives back goes to be freed. Audio thread.
    pub fn deliver(&mut self, unit: &mut EffectUnit) {
        let (Some(built), Some(garbage)) = (self.built.as_mut(), self.garbage.as_mut()) else {
            return;
        };
        if let Some(h) = self.held.take() {
            if let Err(rtrb::PushError::Full(h)) = garbage.push(h) {
                self.held = Some(h);
                return;
            }
        }
        let mut asked = false;
        while let Ok(m) = built.pop() {
            if let Some(back) = unit.supply(m) {
                if let Err(rtrb::PushError::Full(b)) = garbage.push(back) {
                    self.held = Some(b);
                    return;
                }
            }
            asked = true;
        }
        // The need may have moved on meanwhile: ask again.
        if asked {
            self.ask(unit);
        }
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        // The rings' ends first (the builder sees them abandoned), then the builder.
        self.needs = None;
        self.built = None;
        self.garbage = None;
        drop(self.held.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
