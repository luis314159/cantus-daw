//! The COM objects handed to `IAudioProcessor::process`: parameter changes in and out
//! (`IParameterChanges` of `IParamValueQueue`s) and note events (`IEventList`).
//!
//! All of them are built on the main thread at load with fixed capacities and never grow:
//! on the audio thread they are cleared and refilled in place (ADR 0005). What does not fit
//! is counted, never allocated for. They are touched only by the thread that calls
//! `process` and by the plug-in inside that call, which is why the interior mutability
//! below is sound although the types say `Sync`.

use std::cell::UnsafeCell;

use ::vst3::Steinberg::Vst::{
    Event, IEventList, IEventListTrait, IParamValueQueue, IParamValueQueueTrait, IParameterChanges,
    IParameterChangesTrait, ParamID, ParamValue,
};
use ::vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultOk, tresult};
use ::vst3::{Class, ComWrapper};

/// Distinct parameters one block can carry, each way.
pub const MAX_QUEUES: usize = 128;
/// Points per parameter per block.
pub const QUEUE_POINTS: usize = 32;
/// Note events per block, each way.
pub const MAX_EVENTS: usize = 512;

struct QueueData {
    id: ParamID,
    len: usize,
    points: [(i32, ParamValue); QUEUE_POINTS],
}

pub struct ParamQueue {
    data: UnsafeCell<QueueData>,
}

// SAFETY: see the module documentation — one thread at a time, by construction.
unsafe impl Send for ParamQueue {}
unsafe impl Sync for ParamQueue {}

impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}

impl ParamQueue {
    fn new() -> Self {
        ParamQueue {
            data: UnsafeCell::new(QueueData {
                id: 0,
                len: 0,
                points: [(0, 0.0); QUEUE_POINTS],
            }),
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn data(&self) -> &mut QueueData {
        // SAFETY: single-threaded use (module docs); no reference outlives a call.
        unsafe { &mut *self.data.get() }
    }

    pub fn id(&self) -> ParamID {
        self.data().id
    }

    pub fn points(&self) -> &[(i32, ParamValue)] {
        let d = self.data();
        &d.points[..d.len]
    }

    fn reset(&self, id: ParamID) {
        let d = self.data();
        d.id = id;
        d.len = 0;
    }

    /// Keep points ordered by offset; a point at an offset already present replaces it.
    /// When full, a point at or after the last replaces the last, so the block still ends
    /// on the newest value. Returns the index, or `None` when dropped.
    pub fn add(&self, offset: i32, value: ParamValue) -> Option<usize> {
        let d = self.data();
        let mut at = d.len;
        while at > 0 && d.points[at - 1].0 > offset {
            at -= 1;
        }
        if at > 0 && d.points[at - 1].0 == offset {
            d.points[at - 1].1 = value;
            return Some(at - 1);
        }
        if d.len == QUEUE_POINTS {
            if at == d.len {
                d.points[d.len - 1] = (offset, value);
                return Some(d.len - 1);
            }
            return None;
        }
        d.points.copy_within(at..d.len, at + 1);
        d.points[at] = (offset, value);
        d.len += 1;
        Some(at)
    }
}

impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id()
    }

    unsafe fn getPointCount(&self) -> i32 {
        self.data().len as i32
    }

    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut ParamValue) -> tresult {
        let d = self.data();
        if index < 0 || index as usize >= d.len {
            return kInvalidArgument;
        }
        let (o, v) = d.points[index as usize];
        if !offset.is_null() {
            *offset = o;
        }
        if !value.is_null() {
            *value = v;
        }
        kResultOk
    }

    unsafe fn addPoint(&self, offset: i32, value: ParamValue, index: *mut i32) -> tresult {
        match self.add(offset, value) {
            Some(i) => {
                if !index.is_null() {
                    *index = i as i32;
                }
                kResultOk
            }
            None => kResultFalse,
        }
    }
}

pub struct ParamChanges {
    queues: Vec<ComWrapper<ParamQueue>>,
    count: UnsafeCell<usize>,
}

// SAFETY: see the module documentation.
unsafe impl Send for ParamChanges {}
unsafe impl Sync for ParamChanges {}

impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}

impl ParamChanges {
    /// Main thread: allocates every queue it will ever use.
    pub fn new() -> Self {
        ParamChanges {
            queues: (0..MAX_QUEUES)
                .map(|_| ComWrapper::new(ParamQueue::new()))
                .collect(),
            count: UnsafeCell::new(0),
        }
    }

    fn count(&self) -> usize {
        // SAFETY: single-threaded use (module docs).
        unsafe { *self.count.get() }
    }

    pub fn clear(&self) {
        // SAFETY: as above.
        unsafe { *self.count.get() = 0 };
    }

    pub fn queues(&self) -> impl Iterator<Item = &ParamQueue> {
        self.queues[..self.count()].iter().map(|q| &**q)
    }

    /// The queue for `id`, opened if this block has none yet.
    fn queue_for(&self, id: ParamID) -> Option<usize> {
        let n = self.count();
        if let Some(i) = self.queues[..n].iter().position(|q| q.id() == id) {
            return Some(i);
        }
        if n == MAX_QUEUES {
            return None;
        }
        self.queues[n].reset(id);
        // SAFETY: as above.
        unsafe { *self.count.get() = n + 1 };
        Some(n)
    }

    /// Host side: add one change (normalized value) at `offset`. A change with no room
    /// left is dropped.
    pub fn add(&self, id: ParamID, offset: i32, value: ParamValue) -> bool {
        self.queue_for(id)
            .and_then(|i| self.queues[i].add(offset, value))
            .is_some()
    }
}

impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> i32 {
        self.count() as i32
    }

    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index < 0 || index as usize >= self.count() {
            return std::ptr::null_mut();
        }
        self.queues[index as usize]
            .as_com_ref::<IParamValueQueue>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut i32,
    ) -> *mut IParamValueQueue {
        if id.is_null() {
            return std::ptr::null_mut();
        }
        match self.queue_for(*id) {
            Some(i) => {
                if !index.is_null() {
                    *index = i as i32;
                }
                self.queues[i]
                    .as_com_ref::<IParamValueQueue>()
                    .map_or(std::ptr::null_mut(), |r| r.as_ptr())
            }
            None => std::ptr::null_mut(),
        }
    }
}

pub struct EventList {
    events: UnsafeCell<Box<[Event]>>,
    len: UnsafeCell<usize>,
    dropped: UnsafeCell<usize>,
}

// SAFETY: see the module documentation.
unsafe impl Send for EventList {}
unsafe impl Sync for EventList {}

impl Class for EventList {
    type Interfaces = (IEventList,);
}

impl EventList {
    /// Main thread: allocates room for [`MAX_EVENTS`].
    pub fn new() -> Self {
        // SAFETY: `Event` is plain data (integers, floats and a union of such); all-zero
        // is a valid value of it.
        let blank: Event = unsafe { std::mem::zeroed() };
        EventList {
            events: UnsafeCell::new(vec![blank; MAX_EVENTS].into_boxed_slice()),
            len: UnsafeCell::new(0),
            dropped: UnsafeCell::new(0),
        }
    }

    pub fn clear(&self) {
        // SAFETY: single-threaded use (module docs).
        unsafe {
            *self.len.get() = 0;
            *self.dropped.get() = 0;
        }
    }

    pub fn len(&self) -> usize {
        // SAFETY: as above.
        unsafe { *self.len.get() }
    }

    /// Host side: append an event (the caller keeps them in offset order).
    pub fn push(&self, e: Event) -> bool {
        // SAFETY: as above.
        unsafe {
            let len = &mut *self.len.get();
            let events = &mut *self.events.get();
            if *len < events.len() {
                events[*len] = e;
                *len += 1;
                true
            } else {
                *self.dropped.get() += 1;
                false
            }
        }
    }
}

impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> i32 {
        self.len() as i32
    }

    unsafe fn getEvent(&self, index: i32, e: *mut Event) -> tresult {
        if e.is_null() || index < 0 || index as usize >= self.len() {
            return kInvalidArgument;
        }
        *e = (&*self.events.get())[index as usize];
        kResultOk
    }

    unsafe fn addEvent(&self, e: *mut Event) -> tresult {
        if e.is_null() {
            return kInvalidArgument;
        }
        if self.push(*e) {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_keep_offset_order_and_the_newest_value() {
        let q = ParamQueue::new();
        q.reset(7);
        assert_eq!(q.add(10, 0.5), Some(0));
        assert_eq!(q.add(0, 0.1), Some(0));
        assert_eq!(q.add(10, 0.6), Some(1), "same offset replaces");
        assert_eq!(q.points(), &[(0, 0.1), (10, 0.6)]);
        for i in 0..QUEUE_POINTS as i32 {
            q.add(100 + i, f64::from(i));
        }
        assert_eq!(q.points().len(), QUEUE_POINTS);
        let last = *q.points().last().unwrap();
        assert_eq!(
            last,
            (
                100 + QUEUE_POINTS as i32 - 1,
                f64::from(QUEUE_POINTS as i32 - 1)
            )
        );
    }

    #[test]
    fn changes_group_by_parameter_and_count_what_does_not_fit() {
        let c = ParamChanges::new();
        c.add(1, 0, 0.25);
        c.add(2, 5, 0.5);
        c.add(1, 9, 0.75);
        let got: Vec<(u32, Vec<(i32, f64)>)> =
            c.queues().map(|q| (q.id(), q.points().to_vec())).collect();
        assert_eq!(
            got,
            vec![(1, vec![(0, 0.25), (9, 0.75)]), (2, vec![(5, 0.5)])]
        );
        c.clear();
        for id in 0..(MAX_QUEUES as u32 + 3) {
            c.add(id, 0, 0.0);
        }
        assert_eq!(c.queues().count(), MAX_QUEUES);
        assert!(!c.add(MAX_QUEUES as u32 + 9, 0, 0.0), "no room is said");
    }
}
