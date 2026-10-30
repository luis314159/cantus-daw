//! MPE in the application (round 33, ADR 0080): the recorded take's per-note curves
//! ([`record`]), the input ports and zones from the settings to the engine ([`input`]),
//! and the piano roll's expression lane ([`lane`]).

#[path = "mpe_record.rs"]
pub mod record;

#[path = "mpe_input.rs"]
pub mod input;

#[path = "mpe_lane.rs"]
pub mod lane;
