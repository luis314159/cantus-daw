//! Problems with the audio, said in words, each with the click that fixes it (round 20).
//!
//! `diagnose` reads what the engine reports — a device gone, a driver that stopped, an
//! input that would not open and why, dropouts — and returns what is wrong in the order
//! that matters (no sound first). Each problem has its sentence and one or two fixes; a
//! fix is applied by `super::fix` the same way the popover applies a choice.

use super::Live;
use crate::i18n::{t, tf};
use crate::settings::BUFFER_SIZES;

/// What a fix button does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    /// Open the device again with the same settings.
    RestartAudio,
    /// Use the system's default output and input.
    UseDefault,
    /// Run the output at this rate (the input's), so both directions agree.
    RunAt(u32),
    /// Record from this input device (the interface's own).
    UseInput(String),
    /// A larger buffer.
    RaiseBuffer(u32),
}

impl Fix {
    /// The button's words.
    pub fn label(&self) -> String {
        match self {
            Fix::RestartAudio => t("devices.fix.restart"),
            Fix::UseDefault => t("devices.fix.default"),
            Fix::RunAt(r) => tf("devices.fix.run_at", &[("rate", &khz(*r))]),
            Fix::UseInput(_) => t("devices.fix.own_input"),
            Fix::RaiseBuffer(b) => tf("devices.fix.buffer", &[("n", b)]),
        }
    }
}

/// What is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The interface was unplugged (or its stream died).
    Lost { name: String },
    /// The output stopped calling back while the device is still there.
    Stalled { asio: bool },
    /// The device could not be opened at all.
    OpenFailed { error: String },
    /// Another application holds the device exclusively.
    Busy { name: String },
    /// The input runs at another rate than the output and cannot follow it.
    RateMismatch { input: u32, output: u32 },
    /// The input did not open, for another reason.
    InputFailed { error: String },
    /// The sound broke up this many times lately.
    Dropouts { count: u64, buffer: u32 },
}

/// "44.1", "48": a rate in kHz for a sentence.
pub fn khz(rate: u32) -> String {
    if rate.is_multiple_of(1000) {
        format!("{}", rate / 1000)
    } else {
        format!("{:.1}", rate as f64 / 1000.0)
    }
}

/// The rate an input error names: devices.rs says "the input runs at 44100 Hz and cannot
/// run at 48000 Hz".
fn input_rate(error: &str) -> Option<u32> {
    let i = error.find("runs at ")? + "runs at ".len();
    error[i..]
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn busy(error: &str) -> bool {
    let e = error.to_lowercase();
    error.contains(cantus_engine::devices::BUSY_TAG)
        || e.contains("exclusive")
        || e.contains("in use")
        || e.contains("busy")
}

impl Problem {
    /// The sentence.
    pub fn text(&self) -> String {
        match self {
            Problem::Lost { name } => tf("devices.problem.lost", &[("name", name)]),
            Problem::Stalled { asio: true } => t("devices.problem.stalled_asio"),
            Problem::Stalled { asio: false } => t("devices.problem.stalled"),
            Problem::OpenFailed { error } => tf("devices.problem.open", &[("error", error)]),
            Problem::Busy { name } => tf("devices.problem.busy", &[("name", name)]),
            Problem::RateMismatch { input, output } => tf(
                "devices.problem.rates",
                &[("input", &khz(*input)), ("output", &khz(*output))],
            ),
            Problem::InputFailed { error } => tf("devices.problem.input", &[("error", error)]),
            Problem::Dropouts { count, buffer } => tf(
                "devices.problem.dropouts",
                &[("n", count), ("buffer", buffer)],
            ),
        }
    }

    /// Whether there is no sound at all (red) rather than a warning (amber).
    pub fn is_silent(&self) -> bool {
        matches!(
            self,
            Problem::Lost { .. } | Problem::Stalled { .. } | Problem::OpenFailed { .. }
        )
    }
}

/// What the engine's state says is wrong, most serious first, each with its fixes.
/// `lost` is the interface the device watcher saw disappear; `own_input` the input of the
/// interface in use (for a rate mismatch between two interfaces); `dropouts` how many
/// times the sound broke up in the last half minute.
pub fn diagnose(
    live: &Live,
    lost: Option<&str>,
    own_input: Option<&str>,
    dropouts: u64,
) -> Vec<(Problem, Vec<Fix>)> {
    let mut v: Vec<(Problem, Vec<Fix>)> = Vec::new();
    if let Some(name) = lost {
        v.push((
            Problem::Lost {
                name: name.to_string(),
            },
            vec![Fix::UseDefault],
        ));
    } else if live.lost {
        v.push((
            Problem::Lost {
                name: live.device.clone(),
            },
            vec![Fix::RestartAudio, Fix::UseDefault],
        ));
    } else if live.stalled {
        let asio = live.driver.eq_ignore_ascii_case("ASIO");
        v.push((Problem::Stalled { asio }, vec![Fix::RestartAudio]));
    }
    if let Some(e) = live.open_error.as_deref() {
        if busy(e) {
            v.push((
                Problem::Busy {
                    name: live.wanted.clone(),
                },
                vec![Fix::RestartAudio, Fix::UseDefault],
            ));
        } else if !live.live {
            v.push((
                Problem::OpenFailed {
                    error: e.to_string(),
                },
                vec![Fix::UseDefault, Fix::RestartAudio],
            ));
        }
    }
    if let Some(e) = live.input_error.as_deref() {
        if let Some(input) = input_rate(e) {
            let mut fixes = vec![Fix::RunAt(input)];
            if let Some(own) = own_input.filter(|o| live.input.as_deref() != Some(*o)) {
                fixes.push(Fix::UseInput(own.to_string()));
            }
            v.push((
                Problem::RateMismatch {
                    input,
                    output: live.rate.round() as u32,
                },
                fixes,
            ));
        } else if busy(e) {
            v.push((
                Problem::Busy {
                    name: live.input.clone().unwrap_or_default(),
                },
                vec![Fix::RestartAudio],
            ));
        } else {
            v.push((
                Problem::InputFailed {
                    error: e.to_string(),
                },
                vec![Fix::RestartAudio],
            ));
        }
    }
    if dropouts >= 3 && live.live && !live.lost {
        let buffer = live.block.max(1);
        let next = BUFFER_SIZES.iter().copied().find(|b| *b > buffer);
        v.push((
            Problem::Dropouts {
                count: dropouts,
                buffer,
            },
            next.map(Fix::RaiseBuffer).into_iter().collect(),
        ));
    }
    v
}
