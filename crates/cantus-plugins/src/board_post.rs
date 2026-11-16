//! fx3:tabula — Tabula's board in the exported plug-in (ADR 0134 §4): the pedals are
//! content, not parameters (a host sees the board's twelve: the eight macros, `in`, `out`,
//! `mix`, `ab`), so the board travels beside the values in the state, as the project saves
//! it (`EffectSpec::board`).
//!
//! The main thread keeps the board (the editor's edits, a state loaded) and posts a copy to
//! the running processor through a lock-free ring, with a unit built here for each pedal the
//! processor does not hold yet; the processor adopts them (`EffectUnit::adopt`, as the
//! engine does), swaps the board into its spec, and hands back through a second ring the
//! old board and anything it refused, and through a third every pedal that faded out. The
//! audio thread never allocates, frees or locks for it.

use cantus_core::fx::tabula::{Board, PedalId};
use cantus_core::{EffectId, EffectKind, EffectSpec};
use cantus_engine::EffectUnit;
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

/// A board posted (`None`: an empty board) and the units of its new pedals.
#[derive(Debug, Default)]
pub struct Post {
    pub board: Option<Box<Board>>,
    pub kids: Vec<(PedalId, Box<EffectUnit>)>,
}

/// A ring's two ends, each kept while nobody has it.
type Ends<T> = (Option<Producer<T>>, Option<Consumer<T>>);

/// Room in each ring: an edit posts one board; the processor takes them all each block.
const ROOM: usize = 32;

/// The processor's ends: posts in, posts back, pedals let go.
pub type Lent = (Consumer<Post>, Producer<Post>, Producer<Box<EffectUnit>>);

#[derive(Debug)]
pub struct BoardPost {
    main: Mutex<Option<Box<Board>>>,
    /// Bumped at every new board: an editor's copy tells it is stale.
    gen: AtomicU64,
    /// The pedals the processor holds (or has been sent), by id and kind.
    known: Mutex<HashSet<(PedalId, EffectKind)>>,
    /// A processor is running: new pedals are built for it.
    live: AtomicBool,
    to_audio: Mutex<Ends<Post>>,
    back: Mutex<Ends<Post>>,
    gone: Mutex<Ends<Box<EffectUnit>>>,
}

impl Default for BoardPost {
    fn default() -> Self {
        let (tx, rx) = RingBuffer::new(ROOM);
        let (btx, brx) = RingBuffer::new(ROOM);
        let (gtx, grx) = RingBuffer::new(cantus_core::fx::tabula::MAX_PEDALS * 2);
        BoardPost {
            main: Mutex::new(None),
            gen: AtomicU64::new(0),
            known: Mutex::new(HashSet::new()),
            live: AtomicBool::new(false),
            to_audio: Mutex::new((Some(tx), Some(rx))),
            back: Mutex::new((Some(btx), Some(brx))),
            gone: Mutex::new((Some(gtx), Some(grx))),
        }
    }
}

impl BoardPost {
    /// The board as the main thread holds it (main thread).
    pub fn get(&self) -> Option<Box<Board>> {
        self.main.lock().ok().and_then(|b| b.clone())
    }

    /// Changes at every new board.
    pub fn generation(&self) -> u64 {
        self.gen.load(Ordering::Acquire)
    }

    /// A new board (a state loaded, an edit): sanitized, its macro targets stored, kept,
    /// and posted to the processor with a unit for each pedal it has not got (main thread,
    /// `sample_rate` the processor's). What the processor handed back is freed here.
    pub fn set(&self, board: Option<Box<Board>>, sample_rate: f32) {
        self.collect();
        let board = board
            .map(|mut b| {
                b.sanitize();
                b
            })
            .filter(|b| !b.pedals.is_empty() || !b.macros.is_empty());
        let board = board.map(|b| {
            let mut spec = EffectSpec::new(EffectKind::Board);
            spec.board = Some(b);
            cantus_core::fx::tabula::ensure_macro_keys(&mut spec);
            spec.board.expect("the board")
        });
        if let Ok(mut m) = self.main.lock() {
            m.clone_from(&board);
        }
        self.gen.fetch_add(1, Ordering::AcqRel);
        if !self.live.load(Ordering::Acquire) {
            return;
        }
        let mut kids = Vec::new();
        if let Ok(mut known) = self.known.lock() {
            let pedals = board.as_deref().map_or(&[][..], |b| &b.pedals[..]);
            known.retain(|(id, k)| pedals.iter().any(|p| p.id == *id && p.spec.kind == *k));
            for p in pedals {
                if known.insert((p.id, p.spec.kind)) {
                    let u = cantus_engine::fx::tabula::pedal_unit(EffectId(0), p, sample_rate);
                    kids.push((p.id, u));
                }
            }
        }
        if let Ok(mut r) = self.to_audio.lock() {
            if let Some(tx) = r.0.as_mut() {
                if let Err(rtrb::PushError::Full(post)) = tx.push(Post { board, kids }) {
                    // No room (a processor gone quiet): what it was not sent is not known.
                    if let Ok(mut known) = self.known.lock() {
                        for (id, u) in &post.kids {
                            known.remove(&(*id, u.kind));
                        }
                    }
                }
            }
        }
    }

    /// Free what the processor handed back (main thread).
    pub fn collect(&self) {
        if let Ok(mut r) = self.back.lock() {
            if let Some(rx) = r.1.as_mut() {
                while let Ok(p) = rx.pop() {
                    // A pedal refused (no room) is no longer known: sent again next time.
                    if let Ok(mut known) = self.known.lock() {
                        for (id, u) in &p.kids {
                            known.remove(&(*id, u.kind));
                        }
                    }
                }
            }
        }
        if let Ok(mut g) = self.gone.lock() {
            if let Some(rx) = g.1.as_mut() {
                while rx.pop().is_ok() {}
            }
        }
    }

    /// The processor's ends (main thread, when it is made, its board built whole from
    /// `board`); `None` while another has them.
    pub fn lend(&self, board: Option<&Board>) -> Option<Lent> {
        let rx = self.to_audio.lock().ok()?.1.take()?;
        let btx = self.back.lock().ok().and_then(|mut b| b.0.take());
        let gtx = self.gone.lock().ok().and_then(|mut b| b.0.take());
        let (Some(btx), Some(gtx)) = (btx, gtx) else {
            if let Ok(mut r) = self.to_audio.lock() {
                r.1 = Some(rx);
            }
            return None;
        };
        if let Ok(mut known) = self.known.lock() {
            known.clear();
            for p in board.map_or(&[][..], |b| &b.pedals[..]) {
                known.insert((p.id, p.spec.kind));
            }
        }
        self.live.store(true, Ordering::Release);
        Some((rx, btx, gtx))
    }

    /// The processor's ends back (main thread, when it goes).
    pub fn give_back(&self, lent: Lent) {
        let (rx, btx, gtx) = lent;
        self.live.store(false, Ordering::Release);
        if let Ok(mut r) = self.to_audio.lock() {
            r.1 = Some(rx);
        }
        if let Ok(mut b) = self.back.lock() {
            b.0 = Some(btx);
        }
        if let Ok(mut g) = self.gone.lock() {
            g.0 = Some(gtx);
        }
        // Posts the processor never took: freed with the rest.
        if let Ok(mut r) = self.to_audio.lock() {
            if let Some(rx) = r.1.as_mut() {
                while rx.pop().is_ok() {}
            }
        }
        self.collect();
    }
}

/// The processor's side: every board posted, adopted into `unit` and swapped into `spec`;
/// the old boards and what was refused go back, and so does every pedal the board let go.
/// Audio thread: no allocation, no free, no lock. Whether the board changed.
pub fn take_newest(
    lent: &mut Lent,
    held: &mut Option<Post>,
    unit: &mut EffectUnit,
    spec: &mut EffectSpec,
) -> bool {
    let (rx, back, gone) = lent;
    // What the board let go since the last block.
    while gone.slots() > 0 {
        let Some(u) = unit.take_retired() else { break };
        let _ = gone.push(u);
    }
    if let Some(h) = held.take() {
        if let Err(rtrb::PushError::Full(h)) = back.push(h) {
            *held = Some(h);
            return false;
        }
    }
    let mut changed = false;
    while let Ok(mut post) = rx.pop() {
        // Adopted where there is room; what is refused goes back in the same post.
        let mut i = 0;
        while i < post.kids.len() {
            let (id, _) = post.kids[i];
            let (_, u) = post.kids.swap_remove(i);
            match unit.adopt(id, u) {
                None => {}
                Some(u) => {
                    post.kids.push((id, u));
                    let last = post.kids.len() - 1;
                    post.kids.swap(i, last);
                    i += 1;
                }
            }
        }
        std::mem::swap(&mut spec.board, &mut post.board);
        changed = true;
        if let Err(rtrb::PushError::Full(p)) = back.push(post) {
            *held = Some(p);
            break;
        }
    }
    if changed {
        cantus_core::fx::tabula::apply_macros(spec);
    }
    changed
}
