//! What a message does to a mapping's target (ADR 0062 §3): the arithmetic of the modes,
//! the range, the bend and the takeover, pure and deterministic, so the interface only
//! reads messages and sends commands.

use super::{Control, MapMode, Mapping, Takeover};

/// One message as a mapping reads it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Input {
    /// Where the control is (0…1): a knob, a fader, a pad's velocity, the pitch wheel.
    Abs(f32),
    /// An encoder's steps (clockwise positive).
    Steps(i32),
    /// A button or a pad pressed (`true`) or let go.
    Press(bool),
}

/// What a mapping remembers between messages: the interface keeps one per mapping.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DriveState {
    /// The control and the target agree (*pick up* and *scale* move the target directly).
    pub picked: bool,
    /// The control's last position (absolute controls).
    pub last_in: Option<f32>,
    /// The position this mapping last gave the target: a target found elsewhere has been
    /// moved by something else, and must be taken over again.
    pub last_out: Option<f32>,
    /// A button is held (a press is counted once, however many messages say so).
    pub held: bool,
}

/// What a message asks.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Drive {
    /// Put the target at this position (0…1).
    Set(f32),
    /// Fire the target (an action).
    Fire,
    /// Nothing (a release, a knob still picking up, a repeated press).
    Nothing,
}

/// Within this of each other, two positions agree: a little more than one step of a 7-bit
/// control over the whole travel.
pub const AGREE: f32 = 1.5 / 127.0;

impl Mapping {
    /// The message `value` of the mapping's control (`on`: a note-on with velocity, or any
    /// other message; `false` for a note-off) as this mapping reads it.
    pub fn input(&self, value: u16, on: bool) -> Input {
        let max = f32::from(self.source.control.max());
        let x = (f32::from(value) / max).clamp(0.0, 1.0);
        let button = self.source.control.is_button();
        match self.mode {
            MapMode::Relative(enc) => Input::Steps(enc.steps(value.min(127) as u8)),
            MapMode::Absolute if button => {
                if on {
                    Input::Abs(x)
                } else {
                    // A pad let go leaves the value its velocity set.
                    Input::Press(false)
                }
            }
            MapMode::Absolute => Input::Abs(x),
            MapMode::Toggle | MapMode::Momentary | MapMode::Trigger => {
                if button {
                    Input::Press(
                        on && (value > 0 || matches!(self.source.control, Control::Program(_))),
                    )
                } else {
                    // A controller used as a button: pressed from its middle up.
                    Input::Press(value >= 64.max(self.source.control.max() / 2 + 1))
                }
            }
        }
    }

    /// What `input` does, the target being at `current` (0…1; `None` when unknown — a
    /// hosted plug-in not loaded, an action). Updates `state`.
    pub fn drive(&self, input: Input, current: Option<f32>, state: &mut DriveState) -> Drive {
        let out = if self.target.is_action() {
            self.press(input, state).then_some(Drive::Fire)
        } else {
            match (self.mode, input) {
                (MapMode::Relative(_), Input::Steps(n)) => {
                    let from = current.or(state.last_out).unwrap_or(self.min);
                    let x = self.unshape(from) + n as f32 * self.sensitivity / 128.0;
                    Some(Drive::Set(self.shape(x.clamp(0.0, 1.0))))
                }
                (_, Input::Abs(x)) => self.absolute(x, current, state),
                (MapMode::Toggle, i) => self.press(i, state).then(|| {
                    let now = current.or(state.last_out).unwrap_or(self.min);
                    let (lo, hi) = (self.shape(0.0), self.shape(1.0));
                    // To the far end from where it is.
                    let to = if (now - hi).abs() <= (now - lo).abs() {
                        lo
                    } else {
                        hi
                    };
                    Drive::Set(to)
                }),
                (MapMode::Momentary, Input::Press(p)) => {
                    let was = state.held;
                    state.held = p;
                    (p != was).then(|| Drive::Set(self.shape(if p { 1.0 } else { 0.0 })))
                }
                (MapMode::Trigger, i) => self.press(i, state).then(|| Drive::Set(self.shape(1.0))),
                _ => None,
            }
        };
        let out = out.unwrap_or(Drive::Nothing);
        if let Drive::Set(p) = out {
            state.last_out = Some(p);
        }
        out
    }

    /// A press counted once: `true` on the edge from let go to pressed.
    fn press(&self, input: Input, state: &mut DriveState) -> bool {
        let p = match input {
            Input::Press(p) => p,
            Input::Abs(x) => x >= 0.5,
            Input::Steps(n) => n != 0,
        };
        let edge = p && !state.held;
        // An encoder's step is a press and a release at once.
        state.held = p && !matches!(input, Input::Steps(_));
        edge
    }

    /// An absolute control at `x` under the mapping's takeover.
    fn absolute(&self, x: f32, current: Option<f32>, state: &mut DriveState) -> Option<Drive> {
        let want = self.shape(x);
        let prev = state.last_in.replace(x);
        let Some(cur) = current else {
            state.picked = true;
            return Some(Drive::Set(want));
        };
        // Moved by something else since this mapping last set it: take it over again.
        if state.last_out.is_some_and(|o| (o - cur).abs() > AGREE) || state.last_out.is_none() {
            state.picked = (want - cur).abs() <= AGREE;
        }
        let step = AGREE * (self.max - self.min).abs().max(1.0 / 127.0);
        if state.picked || self.takeover == Takeover::Jump {
            state.picked = true;
            return Some(Drive::Set(want));
        }
        match self.takeover {
            Takeover::Jump => unreachable!("handled above"),
            Takeover::Pickup => {
                // Picked up when the control reaches the value, or passes it between two
                // messages (a fast turn skips over it).
                let crossed = prev
                    .map(|p| self.shape(p))
                    .is_some_and(|p| (p - cur) * (want - cur) <= 0.0);
                if (want - cur).abs() <= step || crossed {
                    state.picked = true;
                    Some(Drive::Set(want))
                } else {
                    None
                }
            }
            Takeover::Scale => {
                let Some(xp) = prev else {
                    return None;
                };
                let (lo, hi) = (self.shape(0.0), self.shape(1.0));
                // The target covers what is left of its way in the same share of what is
                // left of the control's.
                let new = if x > xp && xp < 1.0 {
                    cur + (x - xp) / (1.0 - xp) * (hi - cur)
                } else if x < xp && xp > 0.0 {
                    cur - (xp - x) / xp * (cur - lo)
                } else {
                    return None;
                };
                if (new - want).abs() <= step {
                    state.picked = true;
                }
                Some(Drive::Set(new))
            }
        }
    }

    /// The value to send back on the control for a target at `pos` (feedback: motor
    /// faders and LED rings): a button lit when the target is on its top half.
    pub fn feedback_value(&self, pos: f32) -> u16 {
        let max = self.source.control.max();
        match self.mode {
            MapMode::Toggle | MapMode::Momentary | MapMode::Trigger => {
                let (lo, hi) = (self.shape(0.0), self.shape(1.0));
                if (pos - hi).abs() <= (pos - lo).abs() && hi != lo {
                    max
                } else {
                    0
                }
            }
            _ => (self.unshape(pos) * f32::from(max))
                .round()
                .clamp(0.0, f32::from(max)) as u16,
        }
    }
}
