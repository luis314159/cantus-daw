//! Control surfaces on the interface (round 36, ADR 0095): Mackie Control, its extenders
//! and HUI, as devices of the studio.
//!
//! The protocols and the meaning of every button are in `cantus_core::external_surface`
//! (pure, tested with virtual units). This module is the part that touches the application:
//! [`view`] reads the mixer as a surface shows it, [`apply`] turns what a surface asks into
//! ordinary commands and actions, [`service`] owns the devices, takes their ports' messages
//! before the generic controller map sees them and writes what they should show.

pub mod apply;
pub mod service;
pub mod view;

pub use service::{SurfaceChoice, SurfaceService};
