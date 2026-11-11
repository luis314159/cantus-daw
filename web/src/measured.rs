//! Measured, not claimed: the scorecard (ADR 0102) on the home. Round 39 draws it as paired
//! bars, like against like, the lost row as plainly as the wins — `pages/measured.rs`, which
//! `/measured` shares.

use yew::prelude::*;

#[function_component(Measured)]
pub fn measured() -> Html {
    html! { <crate::pages::measured::MeasuredSection /> }
}
