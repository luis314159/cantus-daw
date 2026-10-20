//! What a message does to a mapping's target (ADR 0062 §3): the arithmetic of the modes,
//! the range, the bend and the takeover, pure and deterministic, so the interface only
//! reads messages and sends commands.

use super::{Control, Encoding, MapMode, Mapping, Takeover};

/// What the interface knows of a target's values that the mapping alone does not
/// (review 3, #0, #1): whether it has two states (a switch: a mute, an LED, a band's On, a
/// hosted on/off), and how many steps a stepped one moves through (a choice of nine
/// styles has eight). `Default` is a continuous range.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Steps {
    pub switch: bool,
    pub steps: Option<u32>,
}

impl Steps {
    /// A stepped range from `min` to `max` (in its own units).
    pub fn of_range(stepped: bool, min: f64, max: f64) -> Steps {
        if !stepped {
            return Steps::default();
        }
        let n = (max - min).abs().round();
        let steps = (n >= 1.0 && n < f64::from(u32::MAX)).then_some(n as u32);
        Steps {
            switch: steps == Some(1),
            steps,
        }
    }
}

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
    /// The control's last raw value: a button that never sends a release repeats its press
    /// value, and each repeat is a press of its own (review 1, #0).
    pub last_raw: Option<u16>,
    /// The encoding an encoder turned out to speak when its learnt one read a message as a
    /// jump of half the range (review 1, #6): a sign-bit encoder learnt turning one way.
    pub encoding: Option<Encoding>,
    /// A relative control's own position (0…1 of the control's travel), unrounded: a
    /// stepped target or a switch keeps the steps it has not reached yet (review 1, #2).
    pub rel: Option<f32>,
    /// *Scale*'s own output, unrounded: a stepped target or a switch gathers what it has not
    /// reached yet, as an encoder's does (review 2, #8).
    pub scaled: Option<f32>,
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

/// An encoder message read as this many steps or more is a jump of half the range: another
/// encoding is tried (review 1, #6).
pub const JUMP: i32 = 40;

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

    /// A message as this mapping reads it, and what it does: [`Self::input`] and
    /// [`Self::drive`] with what a message alone cannot say — a button that never sends a
    /// release (a program change; a CC button that only sends its press value) presses
    /// anew each time, and an encoder whose learnt encoding reads a message as a jump of
    /// half the range is read in the encoding that makes it a step. What the interface calls.
    pub fn read(
        &self,
        value: u16,
        on: bool,
        current: Option<f32>,
        state: &mut DriveState,
    ) -> Drive {
        self.read_in(value, on, current, state, Steps::default())
    }

    /// [`Self::read`] for a target of the shape `shape` (a two-state parameter is a
    /// switch; a stepped one moves a step per encoder detent).
    pub fn read_in(
        &self,
        value: u16,
        on: bool,
        current: Option<f32>,
        state: &mut DriveState,
        shape: Steps,
    ) -> Drive {
        let input = match self.mode {
            MapMode::Relative(learnt) => {
                let v = value.min(127) as u8;
                let enc = state.encoding.unwrap_or(learnt);
                let mut n = enc.steps(v);
                if n.abs() >= JUMP {
                    if let Some(other) = enc.reinterpret(v) {
                        state.encoding = Some(other);
                        n = other.steps(v);
                    }
                }
                Input::Steps(n)
            }
            _ => self.input(value, on),
        };
        let release_less = matches!(self.source.control, Control::Program(_));
        // A CC button that sends its press value again with no release between: a new press.
        if !self.source.control.is_button()
            && matches!(input, Input::Press(true))
            && state.last_raw == Some(value)
        {
            state.held = false;
        }
        state.last_raw = Some(value);
        let out = self.drive_in(input, current, state, shape);
        if release_less {
            state.held = false;
        }
        out
    }

    /// What `input` does, the target being at `current` (0…1; `None` when unknown — a
    /// hosted plug-in not loaded, an action). Updates `state`.
    pub fn drive(&self, input: Input, current: Option<f32>, state: &mut DriveState) -> Drive {
        self.drive_in(input, current, state, Steps::default())
    }

    /// [`Self::drive`] for a target of the shape `shape`.
    pub fn drive_in(
        &self,
        input: Input,
        current: Option<f32>,
        state: &mut DriveState,
        shape: Steps,
    ) -> Drive {
        let switch = self.target.is_switch() || shape.switch;
        let out = if self.target.is_action() {
            self.press(input, state).then_some(Drive::Fire)
        } else {
            match (self.mode, input) {
                (MapMode::Relative(_), Input::Steps(0)) => None,
                (MapMode::Relative(_), Input::Steps(n)) if switch => {
                    // A switch on an encoder: a turn either way puts it that way.
                    state.rel = None;
                    Some(Drive::Set(self.shape(if n > 0 { 1.0 } else { 0.0 })))
                }
                (MapMode::Relative(_), Input::Steps(n)) => {
                    // From where this control left it — unrounded, so a stepped target
                    // gathers the steps it has not reached yet — unless something else
                    // moved the target since.
                    let untouched = match (current, state.last_out) {
                        (Some(c), Some(o)) => (c - o).abs() <= 1e-4,
                        _ => true,
                    };
                    let from = match state.rel {
                        Some(x) if untouched => x,
                        _ => self.unshape(current.or(state.last_out).unwrap_or(self.min)),
                    };
                    // A stepped target moves a step per detent (times the sensitivity),
                    // whatever its number of steps (review 3, #1); a range, 1/128.
                    let per = shape.steps.map_or(128.0, |s| s.max(1) as f32);
                    let x = (from + n as f32 * self.sensitivity / per).clamp(0.0, 1.0);
                    state.rel = Some(x);
                    Some(Drive::Set(self.shape(x)))
                }
                (_, Input::Abs(x)) => {
                    state.rel = None;
                    self.absolute(x, current, state, switch)
                }
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
    fn absolute(
        &self,
        x: f32,
        current: Option<f32>,
        state: &mut DriveState,
        switch: bool,
    ) -> Option<Drive> {
        let want = self.shape(x);
        let prev = state.last_in.replace(x);
        // A pad's velocity is where it puts the target, and a switch is on or off as the
        // control says: no takeover (review 1, #4; review 2, #2).
        let Some(cur) = current.filter(|_| !self.source.control.is_button() && !switch) else {
            state.picked = true;
            return Some(Drive::Set(want));
        };
        // Where *scale* left it, unrounded, while nothing else moved the target.
        let untouched = state.last_out.is_some_and(|o| (o - cur).abs() <= AGREE);
        let from = match state.scaled {
            Some(s) if untouched => s,
            _ => cur,
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
                let xp = prev?;
                let (lo, hi) = (self.shape(0.0), self.shape(1.0));
                // The target covers what is left of its way in the same share of what is
                // left of the control's.
                let new = if x > xp && xp < 1.0 {
                    from + (x - xp) / (1.0 - xp) * (hi - from)
                } else if x < xp && xp > 0.0 {
                    from - (xp - x) / xp * (from - lo)
                } else {
                    return None;
                };
                if (new - want).abs() <= step {
                    state.picked = true;
                    state.scaled = None;
                } else {
                    state.scaled = Some(new);
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
