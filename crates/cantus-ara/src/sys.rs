//! ARA's C API (`ARAInterface.h`, ARA_API release 2.3.0, Apache-2.0). The header packs its
//! structs at 1 byte on x86 and at 8 on ARM, so the bindings are generated once per
//! architecture and the right one is compiled in: one build per system, every release target.

#[cfg(target_arch = "aarch64")]
#[path = "sys_arm.rs"]
mod imp;
#[cfg(not(target_arch = "aarch64"))]
#[path = "sys_x86.rs"]
mod imp;

pub use imp::*;
