//! **cantus-rt** — the real-time ground the engine stands on, and the only crate of the engine
//! where `unsafe` and OS calls live (ADR 0031 §2.7; `cantus-engine` keeps
//! `#![forbid(unsafe_code)]`).
//!
//! - [`thread`]: priority — the callback's promotion on WASAPI (MMCSS "Pro Audio" at HIGH;
//!   nothing on ASIO, whose driver owns its thread), pool workers' promotion and placement,
//!   macOS time constraint and audio workgroups, Linux rtkit / `SCHED_FIFO`, and the
//!   process-wide EcoQoS opt-out.
//! - [`topology`]: physical cores, performance and efficiency classes, last-level-cache
//!   domains, and where each kind of pool thread goes.
//! - [`pad`]: 128-byte cache padding.
//! - [`cell`]: try-only `AtomicRefCell`s for the state the engine's pool shares
//!   ([`cell::NodeCell`], [`cell::ShareCell`]).
//! - [`fork`]: fork–join inside a task — jobs that borrow the forker's stack, taken by the
//!   pool's idle threads (ADR 0038: an instrument's voices on several cores).
//! - [`memory`]: pre-faulting and best-effort locking, so the audio thread never takes a
//!   page fault.
//! - [`disk`]: a volume's size and free space (the sample cache's limit).
//! - [`files`]: the open-files limit, raised at start-up (the stream cache's handle budget).
//! - [`fpenv`] and [`denormal`]: the floating-point environment — FTZ/DAZ for a block and its
//!   re-assert after a hosted plug-in, and the interim CRT FMA3 switch (R9).
//!
//! Rules (ADR 0031 §2.7): every `unsafe` block is minimal and has a `// SAFETY:` line; the
//! pure types run under Miri; nothing here knows about audio; the crate stays under 2 000
//! lines.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod cell;
pub mod denormal;
pub mod disk;
pub mod files;
pub mod fork;
pub mod fpenv;
pub mod memory;
pub mod pad;
pub mod thread;
pub mod topology;

pub use cell::{NodeCell, ShareCell};
pub use denormal::DenormalGuard;
pub use pad::CachePadded;
pub use thread::{promote_callback, AudioHost, Promotion};
pub use topology::Topology;

/// What [`init_process`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessInit {
    pub crt_fma3: fpenv::CrtFma3,
    pub power_throttling: thread::PowerThrottling,
    /// The open-files limit after raising it ([`files::raise_limit`]).
    pub open_files: u64,
}

impl std::fmt::Display for ProcessInit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}; {}; {} open files",
            self.crt_fma3, self.power_throttling, self.open_files
        )
    }
}

/// The process-wide settings, once, at start-up before any audio: the CRT's single code path
/// (R9 (2), interim) and the EcoQoS opt-out (R1). Idempotent: later calls return the first
/// call's result.
pub fn init_process() -> ProcessInit {
    static ONCE: std::sync::OnceLock<ProcessInit> = std::sync::OnceLock::new();
    *ONCE.get_or_init(|| ProcessInit {
        crt_fma3: fpenv::disable_crt_fma3(),
        power_throttling: thread::opt_out_of_power_throttling(),
        open_files: files::raise_limit(),
    })
}
