//! Voices on several threads, with the bits of one (ADR 0038).
//!
//! A heavy instrument's voices are independent of each other between two events: each
//! reads the instance's settings and writes only its own state. [`Parallel`] is how an
//! instrument hands them to whoever can help — the engine's real-time pool, or nobody
//! ([`Serial`]) — and [`Lanes`] is where each voice renders: **its own buffer**, summed
//! into the instance's afterwards, by one thread, **in voice order**.
//!
//! Why the bits do not move: the serial loop adds voice after voice into one buffer, so
//! each sample is `((a + v₀) + v₁) + …` for whatever `a` the buffer held. A lane starts at
//! **`−0.0`** ([`clear_lane`]) — the exact additive identity, `−0 + x == x` for every `x`,
//! `−0` and `+0` included (`+0.0` is not: `+0 + −0 = +0`) — and takes exactly one addition
//! per sample from its voice, so it holds exactly `vₖ`; a sample the voice never wrote (it
//! ended) holds `−0`, which leaves the sum as the serial loop leaves it. Adding the lanes
//! in voice order is therefore the serial loop's own sequence of additions, into any
//! buffer — one the caller zeroed, or one that already holds `−0.0` or music. What is
//! *not* allowed is summing a group of voices first and adding the group (floating-point
//! addition is not associative): one lane per voice, always.
//!
//! An instrument takes the parallel path only when [`Parallel::width`] says someone could
//! help; otherwise it runs its serial loop untouched. Both give the same bits, which the
//! instruments' tests check against a runner that executes the jobs backwards
//! ([`Reversed`]) and one that runs them on real threads.

/// One unit of work: run once, on any thread. `None` once taken.
pub type Job<'a> = Option<&'a mut (dyn FnMut() + Send + 'a)>;

/// The most jobs one [`Parallel::run`] takes; [`run_each`] splits longer lists.
pub const MAX_JOBS: usize = 64;

/// Something that can run jobs, possibly on other threads.
///
/// **Contract.** `run` runs every `Some` job exactly once and returns only after every
/// one of them has returned (on whatever thread ran it). It never allocates, locks or
/// blocks on the OS (ADR 0005): it may wait only for jobs that are already running.
pub trait Parallel: Sync {
    /// How many threads could take part right now, the caller's included (1: only the
    /// caller — run serially). A hint, read once per render.
    fn width(&self) -> usize;
    /// Run the jobs; see the trait's contract.
    fn run(&self, jobs: &mut [Job<'_>]);
}

/// Nobody helps: the jobs run on the caller, in order.
#[derive(Clone, Copy, Debug, Default)]
pub struct Serial;

impl Parallel for Serial {
    fn width(&self) -> usize {
        1
    }

    fn run(&self, jobs: &mut [Job<'_>]) {
        for j in jobs.iter_mut() {
            if let Some(f) = j.take() {
                f();
            }
        }
    }
}

/// For tests: claims a width of `width` and runs the jobs on the caller **last first** —
/// a result that depended on the order jobs run in would show.
#[derive(Clone, Copy, Debug)]
pub struct Reversed(pub usize);

impl Parallel for Reversed {
    fn width(&self) -> usize {
        self.0.max(2)
    }

    fn run(&self, jobs: &mut [Job<'_>]) {
        for j in jobs.iter_mut().rev() {
            if let Some(f) = j.take() {
                f();
            }
        }
    }
}

/// Run `f` once for every item, through `par`: each item becomes a job (at most
/// [`MAX_JOBS`] per [`Parallel::run`]; a longer list is run in several). No allocation:
/// the jobs live on this stack frame.
pub fn run_each<T, I, F>(par: &dyn Parallel, items: I, f: &F)
where
    T: Send,
    I: IntoIterator<Item = T>,
    F: Fn(T) + Sync,
{
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        // One closure expression, so every element has the same type; each runs its item
        // at most once.
        let mut work: [Option<_>; MAX_JOBS] = std::array::from_fn(|_| {
            it.next().map(|t| {
                let mut item = Some(t);
                move || {
                    if let Some(t) = item.take() {
                        f(t);
                    }
                }
            })
        });
        let mut jobs: [Job<'_>; MAX_JOBS] = [const { None }; MAX_JOBS];
        let mut n = 0;
        for (j, w) in jobs.iter_mut().zip(work.iter_mut()) {
            if let Some(w) = w.as_mut() {
                *j = Some(w);
                n += 1;
            }
        }
        par.run(&mut jobs[..n]);
    }
}

/// Per-voice stereo buffers ("lanes"), `count` of them with room for `frames` samples
/// each. Allocated off the audio thread, with the instrument.
#[derive(Clone, Debug)]
pub struct Lanes {
    frames: usize,
    l: Vec<f32>,
    r: Vec<f32>,
}

impl Lanes {
    /// Allocates, and touches every page once so the audio thread never takes their first
    /// faults (ADR 0031 R11): build it with the instrument.
    pub fn new(count: usize, frames: usize) -> Self {
        let mut lanes = Lanes {
            frames,
            l: vec![0.0; count * frames],
            r: vec![0.0; count * frames],
        };
        for v in [&mut lanes.l, &mut lanes.r] {
            v.fill(f32::MIN_POSITIVE);
            std::hint::black_box(&mut *v);
            v.fill(0.0);
        }
        lanes
    }

    pub fn count(&self) -> usize {
        self.l.len().checked_div(self.frames).unwrap_or(0)
    }

    /// Room per lane, samples.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Every lane's first `n` samples, left and right, mutably, in lane order.
    pub fn split(&mut self, n: usize) -> impl Iterator<Item = (&mut [f32], &mut [f32])> {
        let n = n.min(self.frames);
        let f = self.frames.max(1);
        self.l
            .chunks_mut(f)
            .zip(self.r.chunks_mut(f))
            .map(move |(l, r)| (&mut l[..n], &mut r[..n]))
    }

    /// Lane `k`'s first `n` samples.
    pub fn lane(&self, k: usize, n: usize) -> (&[f32], &[f32]) {
        let n = n.min(self.frames);
        let a = k * self.frames;
        (&self.l[a..a + n], &self.r[a..a + n])
    }
}

/// Make a lane ready for its voice: every sample `−0.0`, the additive identity (the
/// module's notes say why not `+0.0`).
#[inline]
pub fn clear_lane(l: &mut [f32], r: &mut [f32]) {
    l.fill(-0.0);
    r.fill(-0.0);
}

/// `acc[i] += lane[i]`: one lane into the instance's sum (in voice order, by the caller).
#[inline]
pub fn add_into(acc: &mut [f32], lane: &[f32]) {
    for (a, x) in acc.iter_mut().zip(lane) {
        *a += *x;
    }
}
