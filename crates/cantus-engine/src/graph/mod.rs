//! The processor as a graph (ADR 0031 §1, Phase 0 of `docs/perf/plan.md`).
//!
//! The processor is a set of **nodes** — every track, every strip, the master — run in an
//! order a [`GraphPlan`] fixes, each gathering its inputs in a canonical order
//! ([`gather`]) from what its predecessors left for it. Phase 0 runs them on the callback's
//! one thread, in the plan's order; the next phase hands the same nodes to a pool of
//! threads, and because who adds what, in which order, is the plan's and never the
//! schedule's, the parallel engine will give the serial engine's bits.
//!
//! **Passes, not segments.** A block is cut only where *everything* must stop at once —
//! a loop wrap (every track releases and re-seeks), an automation control point (values
//! change everywhere), and every [`PASS_FRAMES`] frames (the size of a node's buffers) —
//! into **passes**; a live block of up to 1 024 frames is one pass. Inside a pass each node
//! runs at its own segments: a track splits its pass only at its own events, a strip and
//! the master run the whole pass, the click splits only itself at its beats. The serial
//! engine cut every node at every event of every track; every native unit is required to
//! give the same bits whatever its calls' lengths (`fx_contract.rs`, `inst_contract.rs`),
//! which is what makes the fewer, longer calls exact.
//!
//! Where the code is: [`plan`] (compiled on the UI thread, sent with the snapshot),
//! [`gather`] (the canonical sums), [`notes`] (track outboxes for notes into effects); the
//! nodes themselves are children of the processor, whose state they own today —
//! `track.rs`, `strip.rs`, `master.rs` in this folder.

pub mod gather;
pub(crate) mod notes;
pub mod plan;

pub use gather::Contribution;
pub use plan::{compile, topology_key, GraphPlan, Input, NodeRef, MAX_NODES};

/// The longest pass, and the length of every node's own buffers. Larger blocks (an
/// offline render's) are cut into passes of this length, which changes no sample.
pub const PASS_FRAMES: usize = 1024;
