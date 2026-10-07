//! The seam to `cantus-rt` (ADR 0031 §2.7, §5.2, amendment R1): the engine's only door to
//! OS thread priority and the floating-point environment. `cantus-rt` holds the `unsafe`;
//! the engine stays `forbid(unsafe_code)`.

pub use cantus_rt::{AudioHost, DenormalGuard};

/// Raise the audio callback's own thread — MMCSS "Pro Audio" at high priority on WASAPI,
/// nothing on ASIO (whose driver owns the thread) — once, on the first block after a
/// stream starts. Returns whether the OS was asked (`EngineStats::wakes`): granted or
/// refused, not a thread left alone on purpose.
#[inline]
pub(super) fn promote_callback(host: AudioHost, period: std::time::Duration) -> bool {
    !matches!(
        cantus_rt::promote_callback(host, period),
        cantus_rt::Promotion::LeftAsIs(_)
    )
}
