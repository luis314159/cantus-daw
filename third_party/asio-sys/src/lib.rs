#![allow(non_camel_case_types)]

#[macro_use]
extern crate num_derive;
extern crate num_traits;

pub mod bindings;
pub use bindings::{
    errors::{AsioError, LoadDriverError},
    *,
};

/// Whether this build links the Steinberg ASIO SDK (CANTUS PATCH). Without it the crate
/// builds stub bindings: it compiles and runs, but no driver is ever listed or loaded.
pub const SDK_LINKED: bool = cfg!(asio_sdk);
