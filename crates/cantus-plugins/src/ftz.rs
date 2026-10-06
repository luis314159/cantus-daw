//! Denormals flushed to zero for the length of a block, the host's setting restored after.
//!
//! The guard moved to `cantus-rt` in round 21 (ADR 0031 §2.7, §5.2), where the engine uses
//! it too; this module re-exports it so the plug-ins' code and tests keep their paths.

pub use cantus_rt::denormal::{flushing, reassert, DenormalGuard};
