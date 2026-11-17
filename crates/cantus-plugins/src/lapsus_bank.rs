//! fx:lapsus — Lapsus's banks in the exported plug-in (ADR 0132, provisional): the
//! patterns are content, not parameters (a host sees eleven), so they travel beside the
//! values. The main thread keeps the bank (the editor's edits, a state loaded) and posts a
//! copy to the running processor through a lock-free ring; the processor swaps it into its
//! spec at its next block and hands the old one back through a second ring, to be freed on
//! the main thread. The audio thread never allocates, frees or locks for it. The bank is
//! saved in the state as the project saves it (`EffectSpec::lapsus`), so a preset moves
//! between Cantus and any host, both ways.

//!
//! fxa:fractus — the same post carries Fractus's lanes (`ContentPost<fractus::Pattern>`):
//! the content a spec holds beside its values, generic over what it is. fxb:cedo — and
//! Cedo's drawn shape (`ContentPost<cedo::Shape>`, copied from fx/wave-a's generalisation;
//! its Fractus arm arrives with the merge).

use cantus_core::fx::lapsus::{Bank, Lane, Pattern, SLOTS};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Content a spec holds beside its values (Lapsus's bank, Fractus's lanes): an empty one
/// is posted as `None`.
pub trait Content: Clone + Send + 'static {
    fn is_empty(&self) -> bool;
}

impl Content for Bank {
    fn is_empty(&self) -> bool {
        Bank::is_empty(self)
    }
}

/// fxb:cedo — a shape is never the factory's: an empty one ducks nothing (`None`, the
/// classic pump, is posted as `None`).
impl Content for cantus_core::fx::cedo::Shape {
    fn is_empty(&self) -> bool {
        false
    }
}

/// Content posted: `None` is the factory's (no bank of its own, no lanes painted).
pub type Posted<T> = Option<Box<T>>;

/// A bank posted: `None` is the factory's.
pub type Post = Posted<Bank>;

/// A ring's two ends, each kept while nobody has it.
type Ends<T> = (Option<Producer<Posted<T>>>, Option<Consumer<Posted<T>>>);

/// Room in each ring: a drag posts one bank a frame; the processor takes one a block.
const ROOM: usize = 32;

/// The main thread's bank and the two rings' ends kept here while nobody has them.
#[derive(Debug)]
pub struct ContentPost<T> {
    main: Mutex<Posted<T>>,
    /// Bumped at every new bank: an editor's copy tells it is stale.
    gen: AtomicU64,
    to_audio: Mutex<Ends<T>>,
    back: Mutex<Ends<T>>,
}

/// Lapsus's banks.
pub type BankPost = ContentPost<Bank>;

impl<T: Content> Default for ContentPost<T> {
    fn default() -> Self {
        let (tx, rx) = RingBuffer::new(ROOM);
        let (gtx, grx) = RingBuffer::new(ROOM);
        ContentPost {
            main: Mutex::new(None),
            gen: AtomicU64::new(0),
            to_audio: Mutex::new((Some(tx), Some(rx))),
            back: Mutex::new((Some(gtx), Some(grx))),
        }
    }
}

impl<T: Content> ContentPost<T> {
    /// The bank as the main thread holds it (main thread).
    pub fn get(&self) -> Posted<T> {
        self.main.lock().ok().and_then(|b| b.clone())
    }

    /// A new bank (a state loaded, an edit): kept, and posted to the processor (main
    /// thread). What the processor handed back is freed here.
    pub fn set(&self, bank: Posted<T>) {
        self.collect();
        let bank = bank.filter(|b| !b.is_empty());
        if let Ok(mut m) = self.main.lock() {
            *m = bank.clone();
        }
        self.gen.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut r) = self.to_audio.lock() {
            if let Some(tx) = r.0.as_mut() {
                let _ = tx.push(bank);
            }
        }
    }

    /// Changes at every new content.
    pub fn generation(&self) -> u64 {
        self.gen.load(Ordering::Acquire)
    }

    /// Free what the processor handed back (main thread).
    pub fn collect(&self) {
        if let Ok(mut r) = self.back.lock() {
            if let Some(rx) = r.1.as_mut() {
                while rx.pop().is_ok() {}
            }
        }
    }

    /// The processor's ends (main thread, when it is made); `None` while another has them.
    #[allow(clippy::type_complexity)]
    pub fn lend(&self) -> Option<(Consumer<Posted<T>>, Producer<Posted<T>>)> {
        let rx = self.to_audio.lock().ok()?.1.take()?;
        let Some(gtx) = self.back.lock().ok().and_then(|mut b| b.0.take()) else {
            self.give_back(Some(rx), None);
            return None;
        };
        Some((rx, gtx))
    }

    /// The processor's ends back (main thread, when it goes).
    pub fn give_back(&self, rx: Option<Consumer<Posted<T>>>, gtx: Option<Producer<Posted<T>>>) {
        if let (Some(rx), Ok(mut r)) = (rx, self.to_audio.lock()) {
            r.1 = Some(rx);
        }
        if let (Some(gtx), Ok(mut b)) = (gtx, self.back.lock()) {
            b.0 = Some(gtx);
        }
        self.collect();
    }
}

impl BankPost {
    /// One slot of `lane` set (`None`: the factory's), as `Command::SetLapsusPattern` sets
    /// it in a project (main thread).
    pub fn set_pattern(&self, lane: Lane, slot: u8, pattern: Option<Pattern>) {
        if usize::from(slot) >= SLOTS {
            return;
        }
        let mut bank = self.get().unwrap_or_default();
        match pattern {
            Some(p) => {
                bank.lane_mut(lane).insert(slot, p.sanitized(lane));
            }
            None => {
                bank.lane_mut(lane).remove(&slot);
            }
        }
        self.set(Some(bank));
    }
}

/// The processor's side: the newest bank posted, swapped into `slot`; the old one handed
/// back. Audio thread: no allocation, no free, no lock. Whether the bank changed.
pub fn take_newest<T>(
    rx: &mut Consumer<Posted<T>>,
    back: &mut Producer<Posted<T>>,
    held: &mut Option<Posted<T>>,
    slot: &mut Posted<T>,
) -> bool {
    // What could not go back last time goes first; nothing new is taken until it has.
    if let Some(h) = held.take() {
        if let Err(rtrb::PushError::Full(h)) = back.push(h) {
            *held = Some(h);
            return false;
        }
    }
    let mut changed = false;
    while let Ok(newer) = rx.pop() {
        let old = std::mem::replace(slot, newer);
        changed = true;
        if let Err(rtrb::PushError::Full(old)) = back.push(old) {
            *held = Some(old);
            break;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::fx::lapsus::Point;

    #[test]
    fn a_bank_posted_reaches_the_processor_and_the_old_one_comes_back() {
        let post = BankPost::default();
        let (mut rx, mut gtx) = post.lend().expect("lent");
        assert!(post.lend().is_none(), "one processor at a time");
        post.set_pattern(
            Lane::Vol,
            4,
            Some(Pattern {
                len: 0,
                points: vec![Point::new(0.0, 0.5)],
                name: None,
            }),
        );
        let mut slot: Post = None;
        let mut held = None;
        assert!(take_newest(&mut rx, &mut gtx, &mut held, &mut slot));
        assert_eq!(slot, post.get());
        assert!(slot.as_ref().unwrap().vol.contains_key(&4));
        post.set_pattern(Lane::Vol, 4, None);
        assert!(take_newest(&mut rx, &mut gtx, &mut held, &mut slot));
        assert_eq!(slot, None, "an empty bank is the factory's");
        post.give_back(Some(rx), Some(gtx));
        assert!(post.lend().is_some(), "the ends came back");
    }
}
