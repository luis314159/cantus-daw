//! Learning a control (ADR 0062 §2): from the first messages a control sends after the
//! user armed a target, what it is — a knob, a 14-bit fader, an endless encoder and its
//! encoding, a button, a pad — and so how the mapping should read it. Where the hardware
//! does not make it clear, the plain reading wins (absolute), and the mappings view lets
//! the user say otherwise.

use super::{Control, Encoding, MapMode, MappingTarget};

/// What a learnt control is, and how a mapping to `target` should read it.
pub fn detect(heard: &[(Control, u16)], target: &MappingTarget) -> Option<(Control, MapMode)> {
    let (first, _) = *heard.first()?;
    let control = match first {
        // The coarse half of a 14-bit pair, when its fine half came too.
        Control::Cc(n) if n < 32 && heard.iter().any(|(c, _)| *c == Control::Cc(n + 32)) => {
            Control::Cc14(n)
        }
        // The fine half came first: the pair is the coarse one's.
        Control::Cc(n)
            if (32..64).contains(&n) && heard.iter().any(|(c, _)| *c == Control::Cc(n - 32)) =>
        {
            Control::Cc14(n - 32)
        }
        c => c,
    };
    let values: Vec<u16> = heard
        .iter()
        .filter(|(c, _)| *c == first)
        .map(|(_, v)| *v)
        .collect();
    let switchy = target.is_switch() || target.is_action();
    let mode = match control {
        Control::Note(_) | Control::Program(_) => {
            if target.is_action() {
                MapMode::Trigger
            } else if switchy || matches!(control, Control::Program(_)) {
                MapMode::Toggle
            } else {
                // A pad on a parameter: its velocity sets it.
                MapMode::Absolute
            }
        }
        Control::Cc(_) => {
            if let Some(enc) = encoder(&values) {
                MapMode::Relative(enc)
            } else if values.iter().all(|v| *v == 0 || *v == 127) {
                // A button that sends its two ends. Both heard: a momentary button (press,
                // release) toggles; one only: a button latching in the controller itself
                // (127 on one press, 0 on the next) says on or off as it is (review 1, #10).
                let both = values.contains(&0) && values.contains(&127);
                if target.is_action() {
                    MapMode::Trigger
                } else if both {
                    MapMode::Toggle
                } else {
                    MapMode::Absolute
                }
            } else if target.is_action() {
                MapMode::Trigger
            } else {
                MapMode::Absolute
            }
        }
        _ => {
            if target.is_action() {
                MapMode::Trigger
            } else {
                MapMode::Absolute
            }
        }
    };
    Some((control, mode))
}

/// An endless encoder, recognised by what one sends while turned: the same small step
/// again and again (an absolute knob never repeats a value while it moves), within one
/// encoding's band.
pub fn encoder(values: &[u16]) -> Option<Encoding> {
    if values.len() < 3 || values.iter().any(|v| *v > 127) {
        return None;
    }
    let repeats = values.windows(2).filter(|w| w[0] == w[1]).count();
    if repeats == 0 {
        return None;
    }
    let small_up = |v: u16| (1..=15).contains(&v);
    let twos_down = |v: u16| (113..=127).contains(&v);
    let offset_down = |v: u16| (49..=63).contains(&v);
    let high_small = |v: u16| (65..=79).contains(&v);
    if values.iter().all(|v| small_up(*v) || twos_down(*v)) {
        return Some(Encoding::TwosComplement);
    }
    if values.iter().all(|v| offset_down(*v) || high_small(*v)) {
        return Some(Encoding::BinaryOffset);
    }
    if values.iter().all(|v| small_up(*v) || high_small(*v)) {
        return Some(Encoding::SignBit);
    }
    None
}
