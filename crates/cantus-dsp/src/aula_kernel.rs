//! Aula's impulse response as the convolver wants it (ADR 0112): its first [`B`] taps in
//! direct form, the rest as spectra of partitions that grow eightfold from level to level,
//! and, per level, the schedule that spreads one partition's work evenly over the blocks
//! its slack allows. Built off the audio thread, immutable, shared (`Arc`).
//!
//! | Level | Partition | Covers (taps) | Partitions |
//! |---|---|---|---|
//! | head | — | 0 … 64 | direct form |
//! | 0 | 64 | 64 … 1 024 | 15 |
//! | 1 | 512 | 1 024 … 8 192 | 14 |
//! | 2 | 4 096 | 8 192 … 65 536 | 14 |
//! | 3 | 32 768 | 65 536 … | as many as the response needs |
//!
//! A level of size `L` starts at `2L` (level 0 at `L`): its block's result is needed `L`
//! samples after the block completes, so its work — a forward transform, the products, an
//! inverse — is cut into pieces and spread over the `L / 64` blocks before then. Level 0 has
//! no slack and runs whole at each 64-sample boundary. Zero added latency, constant cost.

use crate::aula_fft::Fft;

/// The convolver's grid: the head's length, level 0's partition, the block every level's
/// work is scheduled on.
pub const B: usize = 64;
pub const LEVELS: usize = 4;
/// Each level's partition, samples.
pub const SIZES: [usize; LEVELS] = [64, 512, 4_096, 32_768];
/// Where each level's first partition starts, taps.
pub const STARTS: [usize; LEVELS] = [64, 1_024, 8_192, 65_536];
/// Where each level ends: the next one's start; the last runs on.
const ENDS: [usize; LEVELS] = [1_024, 8_192, 65_536, usize::MAX];
/// The most a piece of work does at once: elements, butterflies or bins.
const CHUNK: usize = 1_024;

/// How an impulse response maps inputs to outputs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Format {
    /// One response: left to left, right to right.
    #[default]
    Mono,
    /// Two: left to left, right to right (each side its own room).
    Stereo,
    /// Four (left→left, left→right, right→left, right→right): each input heard on both
    /// sides, as a room hears a source off centre.
    TrueStereo,
}

impl Format {
    /// Responses it holds (paths).
    pub fn paths(self) -> usize {
        match self {
            Format::Mono => 1,
            Format::Stereo => 2,
            Format::TrueStereo => 4,
        }
    }

    /// The format of a file of `channels` channels: 1 mono, 2 stereo, 4 true stereo (any
    /// other count reads its first two as stereo, or its one as mono).
    pub fn of_channels(channels: usize) -> Format {
        match channels {
            0 | 1 => Format::Mono,
            4 => Format::TrueStereo,
            _ => Format::Stereo,
        }
    }

    /// Products per bin (and taps per sample in the head) for both outputs.
    fn products(self) -> usize {
        match self {
            Format::TrueStereo => 4,
            _ => 2,
        }
    }
}

/// Which levels exist and how many partitions each has: of a response, or of the room a
/// convolver's state has (its capacity).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Layout {
    pub levels: usize,
    pub parts: [usize; LEVELS],
}

impl Layout {
    /// The layout that holds `taps` taps.
    pub fn for_taps(taps: usize) -> Layout {
        let mut l = Layout::default();
        for k in 0..LEVELS {
            if taps > STARTS[k] {
                l.parts[k] = (taps.min(ENDS[k]) - STARTS[k]).div_ceil(SIZES[k]);
                l.levels = k + 1;
            }
        }
        l
    }

    /// The taps it holds.
    pub fn taps(&self) -> usize {
        match self.levels {
            0 => B,
            n => STARTS[n - 1] + self.parts[n - 1] * SIZES[n - 1],
        }
    }

    /// Whether a response of layout `other` fits in this room.
    pub fn holds(&self, other: &Layout) -> bool {
        other.levels <= self.levels && (0..LEVELS).all(|k| other.parts[k] <= self.parts[k])
    }

    /// The largest partition in use (the head's block when there are no levels).
    pub fn top(&self) -> usize {
        match self.levels {
            0 => B,
            n => SIZES[n - 1],
        }
    }

    /// The input and output rings' length: four of the largest partition (a power of two).
    pub fn ring(&self) -> usize {
        4 * self.top()
    }

    /// Samples of silence after which every state is zero: the response, the longest
    /// level's slack and frame, and a block.
    pub fn settle(&self) -> usize {
        self.taps() + 3 * self.top() + 2 * B
    }
}

/// One piece of a level's work in a period.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    /// The frame's inputs, into the work buffer in bit-reversed order.
    Gather,
    /// A forward stage.
    Fwd(u32),
    /// The two inputs' spectra out of the one transform, into the delay line.
    Unpack,
    /// Partition `p`'s products, into the outputs' spectra.
    Mac(u32),
    /// The two outputs' spectra into one transform, bit-reversed.
    Pack,
    /// An inverse stage.
    Inv(u32),
    /// The valid half of the result, added into the output ring.
    Extract,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Unit {
    pub op: Op,
    pub from: u32,
    pub to: u32,
}

/// A level's work for one period, cut into units and spread over its ticks: tick `j` runs
/// `units[starts[j] .. starts[j + 1]]`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Schedule {
    pub units: Vec<Unit>,
    pub starts: Vec<u32>,
}

impl Schedule {
    pub fn tick(&self, j: usize) -> &[Unit] {
        &self.units[self.starts[j] as usize..self.starts[j + 1] as usize]
    }

    /// The schedule of level `k` with `parts` partitions in use, for a response of
    /// `format`. Without partitions only the forward half runs (the delay line stays
    /// current for a longer response to come).
    pub fn new(k: usize, parts: usize, format: Format) -> Schedule {
        let l = SIZES[k];
        let n = 2 * l;
        let stages = n.trailing_zeros();
        let mut work: Vec<(Unit, usize)> = Vec::new();
        let pieces = |op: Op, len: usize, cost: usize, work: &mut Vec<(Unit, usize)>| {
            let mut from = 0;
            while from < len {
                let to = (from + CHUNK).min(len);
                let u = Unit {
                    op,
                    from: from as u32,
                    to: to as u32,
                };
                work.push((u, (to - from) * cost));
                from = to;
            }
        };
        pieces(Op::Gather, n, 1, &mut work);
        for s in 1..=stages {
            pieces(Op::Fwd(s), n / 2, 2, &mut work);
        }
        pieces(Op::Unpack, l + 1, 2, &mut work);
        if parts > 0 {
            for p in 0..parts {
                pieces(Op::Mac(p as u32), l + 1, 2 * format.products(), &mut work);
            }
            pieces(Op::Pack, n, 1, &mut work);
            for s in 1..=stages {
                pieces(Op::Inv(s), n / 2, 2, &mut work);
            }
            pieces(Op::Extract, l, 1, &mut work);
        }
        let ticks = if k == 0 { 1 } else { l / B };
        let total: usize = work.iter().map(|w| w.1).sum::<usize>().max(1);
        let mut starts = vec![0u32; ticks + 1];
        let mut units = Vec::with_capacity(work.len());
        let mut cum = 0usize;
        let mut tick = 0usize;
        for (u, cost) in work {
            let want = ((cum as u128 * ticks as u128) / total as u128) as usize;
            while tick < want.min(ticks - 1) {
                tick += 1;
                starts[tick] = units.len() as u32;
            }
            units.push(u);
            cum += cost;
        }
        while tick < ticks {
            tick += 1;
            starts[tick] = units.len() as u32;
        }
        Schedule { units, starts }
    }
}

/// One level's partitions' spectra (bins `0 … L`, scaled by `1 / 2L`), path by path:
/// `re[(path · parts + p) · bins + bin]`.
#[derive(Clone, Debug)]
pub(crate) struct KLevel {
    pub parts: usize,
    pub re: Vec<f32>,
    pub im: Vec<f32>,
    pub sched: Schedule,
}

/// An impulse response, partitioned for a convolver of capacity `cap`.
#[derive(Clone, Debug)]
pub struct Kernel {
    format: Format,
    taps: usize,
    cap: Layout,
    /// The head's taps, reversed, path by path (`B` each).
    pub(crate) head: Vec<f32>,
    pub(crate) levels: Vec<KLevel>,
}

impl Kernel {
    /// Partition `responses` (one per path of `format`; shorter ones are zero-padded, and
    /// what does not fit `cap` is cut). Allocates: off the audio thread.
    pub fn new(format: Format, responses: &[Vec<f32>], cap: Layout) -> Kernel {
        let paths = format.paths();
        assert!(
            responses.len() >= paths,
            "{format:?} needs {paths} responses"
        );
        let taps = responses[..paths]
            .iter()
            .map(|r| r.len())
            .max()
            .unwrap_or(0)
            .min(cap.taps());
        let tap = |p: usize, i: usize| -> f32 {
            if i < taps {
                responses[p].get(i).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        };
        let mut head = vec![0.0; paths * B];
        for p in 0..paths {
            for j in 0..B {
                head[p * B + j] = tap(p, B - 1 - j);
            }
        }
        let own = Layout::for_taps(taps);
        let mut levels = Vec::with_capacity(cap.levels);
        for k in 0..cap.levels {
            let l = SIZES[k];
            let n = 2 * l;
            let bins = l + 1;
            let parts = if k < own.levels { own.parts[k] } else { 0 };
            let mut re = vec![0.0; paths * parts * bins];
            let mut im = vec![0.0; paths * parts * bins];
            if parts > 0 {
                let fft = Fft::new(n);
                let scale = 1.0 / n as f32;
                let mut wr = vec![0.0f32; n];
                let mut wi = vec![0.0f32; n];
                // Two paths per transform: one in the real part, one in the imaginary.
                for pair in (0..paths).step_by(2) {
                    let second = pair + 1 < paths;
                    for p in 0..parts {
                        let start = STARTS[k] + p * l;
                        for i in 0..n {
                            wr[i] = if i < l { tap(pair, start + i) } else { 0.0 };
                            wi[i] = if i < l && second {
                                tap(pair + 1, start + i)
                            } else {
                                0.0
                            };
                        }
                        fft.run(&mut wr, &mut wi, false);
                        for b in 0..bins {
                            let (a, bb) = (wr[b], wi[b]);
                            let (c, d) = (wr[(n - b) & (n - 1)], wi[(n - b) & (n - 1)]);
                            let at = |path: usize| (path * parts + p) * bins + b;
                            re[at(pair)] = 0.5 * (a + c) * scale;
                            im[at(pair)] = 0.5 * (bb - d) * scale;
                            if second {
                                re[at(pair + 1)] = 0.5 * (bb + d) * scale;
                                im[at(pair + 1)] = 0.5 * (c - a) * scale;
                            }
                        }
                    }
                }
            }
            levels.push(KLevel {
                parts,
                re,
                im,
                sched: Schedule::new(k, parts, format),
            });
        }
        Kernel {
            format,
            taps,
            cap,
            head,
            levels,
        }
    }

    /// A response of silence for a convolver of capacity `cap`: what runs before the first
    /// real one arrives (its delay lines kept current).
    pub fn silent(cap: Layout) -> Kernel {
        Kernel::new(Format::Mono, &[Vec::new()], cap)
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Its length, taps.
    pub fn taps(&self) -> usize {
        self.taps
    }

    /// The capacity it was partitioned for.
    pub fn cap(&self) -> Layout {
        self.cap
    }

    /// Bytes it holds (spectra and head), for the memory report.
    pub fn bytes(&self) -> usize {
        4 * (self.head.len() + self.levels.iter().map(|l| 2 * l.re.len()).sum::<usize>())
    }
}
