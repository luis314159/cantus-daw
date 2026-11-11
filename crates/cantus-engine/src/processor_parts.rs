//! The parts a track slot holds only while its track uses them, on the audio thread
//! (ADR 0107, `crate::parts`): a Pulsus kit while the track hosts Pulsus, a note-effect
//! chain while it has note effects. Parts are built off the audio thread and sent in
//! (`EngineMessage::AddPart`) before the snapshot that uses them; the processor keeps every
//! part it is given — in a slot, or spare — and moves parts between slots as tracks start
//! and stop using them. Nothing here allocates or frees: the spares' vectors have room for
//! one part of each kind per track slot from the start, and a part only ever moves.

use super::*;
use crate::notefx::Chain;
use crate::parts::{Needed, SlotPart};
use cantus_core::InstrumentSpec;

/// The parts in no slot, and what the installed snapshot still waits for.
pub(crate) struct Spares {
    kits: Vec<Box<Pulsus>>,
    chains: Vec<Box<Chain>>,
    /// Tracks of the installed snapshot still waiting for a part.
    short: Needed,
    /// Parts asked for (`EngineEvent::PartsNeed`) and not arrived yet: a shortfall is
    /// asked for once.
    asked: Needed,
}

impl Spares {
    pub fn new() -> Self {
        Spares {
            kits: Vec::with_capacity(MAX_TRACKS),
            chains: Vec::with_capacity(MAX_TRACKS),
            short: Needed::default(),
            asked: Needed::default(),
        }
    }

    /// A copy, room kept (a range render's fork, ADR 0083). Allocates.
    pub fn fork(&self) -> Self {
        let mut s = Spares::new();
        s.kits.extend(self.kits.iter().cloned());
        s.chains.extend(self.chains.iter().cloned());
        s.short = self.short;
        s.asked = self.asked;
        s
    }
}

impl Processor {
    /// A part arrived: kept, and given at once to a track that waits for one.
    pub(super) fn part_arrived(&mut self, part: SlotPart) {
        let a = &mut self.parts.asked;
        match &part {
            SlotPart::Kit(_) => a.kits = a.kits.saturating_sub(1),
            SlotPart::NoteFx(_) => a.note_fx = a.note_fx.saturating_sub(1),
        }
        self.keep_part(part);
        if self.parts.short != Needed::default() {
            if let Some(snapshot) = self.snapshot.clone() {
                self.place_parts(&snapshot);
            }
        }
    }

    /// Every part where `snapshot` wants it, once its tracks have their slots: a slot whose
    /// track does not use a part gives it back (silenced, emptied), and a track that uses
    /// one it lacks takes a spare, configured from the snapshot as the install would have.
    /// A track left without one is asked for (`EngineEvent::PartsNeed`), once per
    /// shortfall: a drum track plays nothing, a chain is bypassed, until it comes.
    pub(super) fn place_parts(&mut self, snapshot: &Snapshot) {
        let nodes = self.nodes.clone();
        let mut drum = [false; MAX_TRACKS];
        let mut fx = [false; MAX_TRACKS];
        for (ti, t) in snapshot.tracks.iter().take(MAX_TRACKS).enumerate() {
            if let Some(&slot) = self.track_to_slot.get(ti) {
                drum[slot] = crate::parts::is_drum(t);
                fx[slot] = crate::parts::has_note_fx(t);
            }
        }
        // Given back first, so a track that just started using a part can take it.
        for slot in 0..nodes.tracks.len().min(MAX_TRACKS) {
            let (kit, chain) = match nodes.track(slot) {
                Some(mut s) => (
                    if drum[slot] { None } else { s.drums.take() },
                    if fx[slot] { None } else { s.notefx.take() },
                ),
                None => (None, None),
            };
            if let Some(mut kit) = kit {
                kit.reset();
                self.keep_part(SlotPart::Kit(kit));
            }
            if let Some(mut chain) = chain {
                // As a new one: no stage, nothing held (`Chain::configure` starts afresh).
                chain.configure(&[], None);
                chain.reset();
                self.keep_part(SlotPart::NoteFx(chain));
            }
        }
        let mut short = Needed::default();
        for (ti, t) in snapshot.tracks.iter().take(MAX_TRACKS).enumerate() {
            let Some(&slot) = self.track_to_slot.get(ti) else {
                continue;
            };
            if !drum[slot] && !fx[slot] {
                continue;
            }
            let Some(mut s) = nodes.track(slot) else {
                continue;
            };
            if let (true, InstrumentSpec::Pulsus(params)) = (drum[slot], &t.instrument) {
                if !s.drums.is_some() {
                    match self.parts.kits.pop() {
                        Some(kit) => {
                            s.drums.put(kit);
                            if let Some(d) = s.drums.get() {
                                crate::pulsus_kit::configure(d, params, &t.samples);
                            }
                        }
                        None => short.kits += 1,
                    }
                }
            }
            if fx[slot] && s.notefx.is_none() {
                match self.parts.chains.pop() {
                    Some(chain) => {
                        s.notefx = Some(chain);
                        s.configure_notefx(&t.note_fx, snapshot.key);
                    }
                    None => short.note_fx += 1,
                }
            }
        }
        self.parts.short = short;
        self.stats.kits_waiting = short.kits as u64;
        self.stats.note_fx_waiting = short.note_fx as u64;
        // Asked for once: only what is not already on its way.
        let asked = self.parts.asked;
        let ask = Needed {
            kits: short.kits.saturating_sub(asked.kits),
            note_fx: short.note_fx.saturating_sub(asked.note_fx),
        };
        if ask != Needed::default()
            && self
                .tx
                .push(EngineEvent::PartsNeed {
                    kits: ask.kits,
                    note_fx: ask.note_fx,
                })
                .is_ok()
        {
            self.parts.asked = Needed {
                kits: asked.kits + ask.kits,
                note_fx: asked.note_fx + ask.note_fx,
            };
        }
    }

    /// A part kept spare — or, past one per slot (a sender that sent too many), handed back
    /// to be freed off the audio thread.
    fn keep_part(&mut self, part: SlotPart) {
        let s = &mut self.parts;
        let back = match part {
            SlotPart::Kit(k) if s.kits.len() < s.kits.capacity() => {
                s.kits.push(k);
                None
            }
            SlotPart::NoteFx(c) if s.chains.len() < s.chains.capacity() => {
                s.chains.push(c);
                None
            }
            other => Some(other),
        };
        if let Some(part) = back {
            if self.tx.push(EngineEvent::GarbagePart(part)).is_err() {
                // The queue full too: freed here and counted, as any garbage the queue
                // cannot take (never within the track limit).
                self.stats.dropped_events += 1;
            }
        }
    }

    /// The parts this processor holds, in its slots and spare (the tests' count).
    pub fn parts_held(&self) -> Needed {
        let mut n = Needed {
            kits: self.parts.kits.len(),
            note_fx: self.parts.chains.len(),
        };
        for i in 0..self.nodes.tracks.len() {
            if let Some(s) = self.nodes.track(i) {
                n.kits += usize::from(s.drums.is_some());
                n.note_fx += usize::from(s.notefx.is_some());
            }
        }
        n
    }

    /// Hold every part and slot `snapshot` uses, built here (ADR 0107): for a processor no audio
    /// thread runs — an offline render, a test that installs snapshots by hand — called
    /// before (or after) the snapshot is installed. **Allocates**: the application's
    /// engine is sent its parts instead (`EngineMessage::AddPart`).
    pub fn hold_parts_for(&mut self, snapshot: &Snapshot) {
        // Its slots first (ADR 0107): a snapshot waiting for them installs here.
        self.hold_cells_for(snapshot);
        let want = crate::parts::needed(snapshot);
        let held = self.parts_held();
        let sr = self.sample_rate as f32;
        for _ in held.kits..want.kits {
            if self.parts.kits.len() < self.parts.kits.capacity() {
                self.parts.kits.push(Box::new(Pulsus::new(sr)));
            }
        }
        for _ in held.note_fx..want.note_fx {
            if self.parts.chains.len() < self.parts.chains.capacity() {
                self.parts.chains.push(Box::default());
            }
        }
        if self.parts.short != Needed::default() {
            if let Some(installed) = self.snapshot.clone() {
                self.place_parts(&installed);
            }
        }
    }

    /// Compensation lines the pool holds (ADR 0107: it starts with
    /// [`crate::pdc::START_LINES`] and grows only as projects need).
    pub fn lines_held(&self) -> usize {
        self.pdc.held()
    }
}

// ---- Slots that grow with the song (ADR 0107) ---------------------------------------------

impl Processor {
    /// Whether `snapshot`'s tracks and strips all fit in the provisioned slots. A slot whose
    /// track went is given to a new one before a fresh slot is, so a snapshot of `n` tracks
    /// needs `n` slots whatever the last one held.
    pub(super) fn fits(&self, snapshot: &Snapshot) -> bool {
        let (t, s) = crate::parts::cells_needed(snapshot);
        t <= self.ready.0 && s <= self.ready.1
    }

    /// `snapshot` needs more slots than are provisioned: it waits — the one before it plays
    /// on and every other message is handled — and they are asked for once. A later
    /// snapshot takes its place.
    pub(super) fn wait_for_cells(&mut self, snapshot: Arc<Snapshot>, plan: Option<Box<GraphPlan>>) {
        if let Some((old, old_plan)) = self.cells_wait.take() {
            self.dispose(old);
            if let Some(p) = old_plan {
                self.dispose_plan(p);
            }
        }
        let need = crate::parts::cells_needed(&snapshot);
        self.cells_wait = Some((snapshot, plan));
        self.stats.cells_waiting = 1;
        if need != self.cells_asked
            && self
                .tx
                .push(EngineEvent::CellsNeed {
                    tracks: need.0,
                    strips: need.1,
                })
                .is_ok()
        {
            self.cells_asked = need;
        }
    }

    /// A snapshot that installs drops the one that waited for slots (it is older).
    pub(super) fn forget_waiting(&mut self) {
        if let Some((old, plan)) = self.cells_wait.take() {
            self.dispose(old);
            if let Some(p) = plan {
                self.dispose_plan(p);
            }
        }
        self.stats.cells_waiting = 0;
    }

    /// Swap `cells` into the next bare slots — between passes, allocation-free — leaving the
    /// bare ones in `cells` to be handed back.
    pub(super) fn provision(&mut self, cells: &mut crate::parts::SlotCells) {
        let nodes = self.nodes.clone();
        for p in cells.tracks.iter_mut() {
            let Some(c) = nodes.tracks.get(self.ready.0) else {
                break;
            };
            match (
                c.state.try_borrow_mut(),
                c.out.try_borrow_mut(),
                c.outbox.try_borrow_mut(),
            ) {
                (Some(mut s), Some(mut o), Some(mut b)) => {
                    std::mem::swap(&mut *s, &mut p.state);
                    std::mem::swap(&mut *o, &mut p.out);
                    std::mem::swap(&mut *b, &mut p.outbox);
                    self.ready.0 += 1;
                }
                _ => {
                    nodes.conflict();
                    break;
                }
            }
        }
        for p in cells.strips.iter_mut() {
            let Some(c) = nodes.strips.get(self.ready.1) else {
                break;
            };
            match (
                c.state.try_borrow_mut(),
                c.out.try_borrow_mut(),
                c.key.try_borrow_mut(),
            ) {
                (Some(mut s), Some(mut o), Some(mut k)) => {
                    std::mem::swap(&mut *s, &mut p.state);
                    std::mem::swap(&mut *o, &mut p.out);
                    std::mem::swap(&mut *k, &mut p.key);
                    self.ready.1 += 1;
                }
                _ => {
                    nodes.conflict();
                    break;
                }
            }
        }
    }

    /// Slots arrived: provisioned, the bare ones handed back, and a snapshot that waited
    /// for them installed (on the automation's grid while playing, as any snapshot is).
    pub(super) fn cells_arrived(&mut self, mut cells: Box<crate::parts::SlotCells>) {
        self.provision(&mut cells);
        if self.tx.push(EngineEvent::GarbageCells(cells)).is_err() {
            self.stats.dropped_events += 1;
        }
        let fits = self.cells_wait.as_ref().is_some_and(|(s, _)| self.fits(s));
        if fits {
            if let Some((snapshot, plan)) = self.cells_wait.take() {
                self.stats.cells_waiting = 0;
                self.cells_asked = (0, 0);
                if self.defer_install() {
                    self.deferred = Some((snapshot, plan));
                } else {
                    self.install_snapshot(snapshot, plan);
                }
            }
        }
    }

    /// Provision slots for `snapshot` here (ADR 0107), off any audio thread: for
    /// [`Self::hold_parts_for`]. Allocates.
    pub(super) fn hold_cells_for(&mut self, snapshot: &Snapshot) {
        let (t, s) = crate::parts::cells_needed(snapshot);
        let more = (
            t.saturating_sub(self.ready.0),
            s.saturating_sub(self.ready.1),
        );
        if more == (0, 0) {
            return;
        }
        let mut cells = crate::parts::SlotCells::new(self.sample_rate as f32, more.0, more.1);
        self.provision(&mut cells);
        drop(cells);
        let fits = self.cells_wait.as_ref().is_some_and(|(s, _)| self.fits(s));
        if fits {
            if let Some((snapshot, plan)) = self.cells_wait.take() {
                self.stats.cells_waiting = 0;
                self.install_snapshot(snapshot, plan);
            }
        }
    }

    /// Track and strip slots provisioned (ADR 0107: [`crate::parts::START_TRACKS`] and
    /// [`crate::parts::START_STRIPS`] at first, more only as songs need).
    pub fn slots_ready(&self) -> (usize, usize) {
        self.ready
    }
}

impl Processor {
    /// Every place that now holds a line has its edge buffers (ADR 0107): a place given its
    /// first line takes the buffers the line brought, between passes, allocation-free.
    pub(super) fn edges_for_lines(&mut self) {
        let nodes = self.nodes.clone();
        for i in 0..nodes.edges.len().min(nodes.lines.len()) {
            let Some(mut line) = nodes.lines.get(i) else {
                continue;
            };
            if line.is_empty() || line.out[0].is_empty() {
                continue;
            }
            match nodes.edges[i].try_borrow_mut() {
                Some(mut e) if e[0].is_empty() => std::mem::swap(&mut *e, &mut line.out),
                Some(_) => {}
                None => nodes.conflict(),
            }
        }
    }
}
