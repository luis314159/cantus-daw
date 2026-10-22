//! The run loop a plug-in's editor needs on Linux (ADR 0065): there is no system message
//! loop to hang timers and descriptors on, so the host keeps them — CLAP's `timer-support`
//! and `posix-fd-support`, VST3's `Linux::IRunLoop` — and serves them from the main thread
//! once a frame ([`crate::backend::MainThread::idle`]), and in the sandbox's helper from its
//! own loop. A toolkit's X connection is such a descriptor: its events are read when the
//! plug-in is told the descriptor is ready.
//!
//! Main thread only. Every handler is called with no borrow of this loop held, so a
//! handler may register or unregister (itself included) while it runs.

use std::time::{Duration, Instant};

/// Readiness, as CLAP spells it (`CLAP_POSIX_FD_READ`, `_WRITE`, `_ERROR`).
pub const FD_READ: u32 = 1 << 0;
pub const FD_WRITE: u32 = 1 << 1;
pub const FD_ERROR: u32 = 1 << 2;

/// The shortest period a timer is served at: one a plug-in asks for faster is served at
/// this (the frame loop's own pace is about this anyway).
pub const MIN_PERIOD: Duration = Duration::from_millis(10);

struct Timer<T> {
    id: u32,
    period: Duration,
    due: Instant,
    handler: T,
}

struct Fd<T> {
    fd: i32,
    flags: u32,
    handler: T,
}

/// Timers and descriptors registered by one plug-in instance.
pub struct RunLoop<T> {
    timers: Vec<Timer<T>>,
    fds: Vec<Fd<T>>,
    next_id: u32,
}

impl<T> Default for RunLoop<T> {
    fn default() -> Self {
        RunLoop {
            timers: Vec::new(),
            fds: Vec::new(),
            next_id: 1,
        }
    }
}

impl<T: Clone> RunLoop<T> {
    /// A timer every `period` (at least [`MIN_PERIOD`]), first due one period from now.
    pub fn add_timer(&mut self, period: Duration, handler: T) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let period = period.max(MIN_PERIOD);
        self.timers.push(Timer {
            id,
            period,
            due: Instant::now() + period,
            handler,
        });
        id
    }

    /// Remove the timer `id`; false when there is none.
    pub fn remove_timer(&mut self, id: u32) -> bool {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id);
        self.timers.len() != before
    }

    /// Remove every timer whose handler matches; how many went.
    pub fn remove_timers_where(&mut self, mut matches: impl FnMut(&T) -> bool) -> usize {
        let before = self.timers.len();
        self.timers.retain(|t| !matches(&t.handler));
        before - self.timers.len()
    }

    /// Watch `fd` for `flags`; false when it is watched already (CLAP: `modify_fd` then).
    pub fn add_fd(&mut self, fd: i32, flags: u32, handler: T) -> bool {
        if fd < 0 || self.fds.iter().any(|f| f.fd == fd) {
            return false;
        }
        self.fds.push(Fd { fd, flags, handler });
        true
    }

    /// Watch `fd` for other `flags`; false when it is not watched.
    pub fn modify_fd(&mut self, fd: i32, flags: u32) -> bool {
        match self.fds.iter_mut().find(|f| f.fd == fd) {
            Some(f) => {
                f.flags = flags;
                true
            }
            None => false,
        }
    }

    /// Stop watching `fd`; false when it was not watched.
    pub fn remove_fd(&mut self, fd: i32) -> bool {
        let before = self.fds.len();
        self.fds.retain(|f| f.fd != fd);
        self.fds.len() != before
    }

    /// Stop watching every descriptor whose handler matches; how many went.
    pub fn remove_fds_where(&mut self, mut matches: impl FnMut(&T) -> bool) -> usize {
        let before = self.fds.len();
        self.fds.retain(|f| !matches(&f.handler));
        before - self.fds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.timers.is_empty() && self.fds.is_empty()
    }

    /// The timers due at `now`, each moved to its next due time. A timer late by more than
    /// a period (the frame loop paused) fires once, not once per period missed.
    pub fn due(&mut self, now: Instant) -> Vec<(u32, T)> {
        let mut out = Vec::new();
        for t in &mut self.timers {
            if t.due <= now {
                out.push((t.id, t.handler.clone()));
                t.due += t.period;
                if t.due <= now {
                    t.due = now + t.period;
                }
            }
        }
        out
    }

    /// How long until the next timer is due (zero when one is due now); `None` without
    /// timers. The frame loop asks for its next frame no later than this.
    pub fn next_due(&self, now: Instant) -> Option<Duration> {
        self.timers
            .iter()
            .map(|t| t.due.saturating_duration_since(now))
            .min()
    }

    /// The descriptors ready now (`poll` without waiting): each with what it is ready for,
    /// in CLAP's flags. A descriptor closed under us reports [`FD_ERROR`].
    pub fn ready(&self) -> Vec<(i32, u32, T)> {
        if self.fds.is_empty() {
            return Vec::new();
        }
        let mut polls: Vec<libc::pollfd> = self
            .fds
            .iter()
            .map(|f| libc::pollfd {
                fd: f.fd,
                events: (if f.flags & FD_READ != 0 {
                    libc::POLLIN
                } else {
                    0
                }) | (if f.flags & FD_WRITE != 0 {
                    libc::POLLOUT
                } else {
                    0
                }),
                revents: 0,
            })
            .collect();
        // SAFETY: `polls` is a live array of `polls.len()` entries; no wait (timeout 0).
        let n = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 0) };
        if n <= 0 {
            return Vec::new();
        }
        polls
            .iter()
            .zip(&self.fds)
            .filter_map(|(p, f)| {
                let mut got = 0;
                if p.revents & libc::POLLIN != 0 {
                    got |= FD_READ;
                }
                if p.revents & libc::POLLOUT != 0 {
                    got |= FD_WRITE;
                }
                if p.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                    got |= FD_ERROR;
                }
                // Errors are always reported; reads and writes only when asked for.
                let wanted = got & (f.flags | FD_ERROR);
                (wanted != 0).then(|| (f.fd, wanted, f.handler.clone()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_fire_when_due_and_once_when_late() {
        let mut l: RunLoop<&str> = RunLoop::default();
        let a = l.add_timer(Duration::from_millis(20), "a");
        let b = l.add_timer(Duration::from_millis(1), "b");
        assert_ne!(a, b);
        let now = Instant::now();
        assert!(l.due(now).is_empty(), "nothing is due at once");
        // Faster than the floor is served at the floor.
        assert!(l.next_due(now).unwrap() <= MIN_PERIOD);
        let later = now + Duration::from_secs(1);
        let fired: Vec<_> = l.due(later).into_iter().map(|(_, h)| h).collect();
        assert_eq!(fired, vec!["a", "b"]);
        // Late by many periods: once, then not again until a period on.
        assert!(l.due(later).is_empty());
        assert!(l.remove_timer(a));
        assert!(!l.remove_timer(a));
        assert_eq!(l.remove_timers_where(|h| *h == "b"), 1);
        assert!(l.is_empty() && l.next_due(later).is_none());
    }

    #[test]
    fn a_descriptor_is_ready_when_it_has_data() {
        let mut fds = [0; 2];
        // SAFETY: a fresh pipe; both ends closed at the end.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let (read, write) = (fds[0], fds[1]);
        let mut l: RunLoop<u8> = RunLoop::default();
        assert!(l.add_fd(read, FD_READ, 7));
        assert!(!l.add_fd(read, FD_READ, 7), "once per descriptor");
        assert!(l.ready().is_empty(), "nothing to read yet");
        // SAFETY: one byte from a live local into the pipe's write end.
        assert_eq!(unsafe { libc::write(write, [1u8].as_ptr().cast(), 1) }, 1);
        assert_eq!(l.ready(), vec![(read, FD_READ, 7)]);
        // Asked for writes only: a readable pipe is not reported.
        assert!(l.modify_fd(read, FD_WRITE));
        assert!(l.ready().is_empty());
        assert!(l.remove_fd(read) && !l.remove_fd(read));
        // SAFETY: closing the pipe this test opened.
        unsafe {
            libc::close(read);
            libc::close(write);
        }
    }
}
