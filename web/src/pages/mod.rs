//! The site's own pages beside the home (round 39): each a component that takes its language
//! from the context; `app.rs` picks one by the route (`route.rs`).
//!
//! - `/download` — [`download::DownloadPage`] (web-content)
//! - `/measured` — [`measured::MeasuredPage`] (web-content; the home's chapter is
//!   `measured::MeasuredSection`)
//! - `/instruments/<name>`, `/effects/<name>` — [`unit::UnitPage`]; `/instruments`,
//!   `/effects` — [`unit::UnitIndex`] (web-content)
//! - `/learn` — [`learn::Learn`] (web-extra)

pub mod download;
pub mod learn;
pub mod measured;
pub mod unit;
