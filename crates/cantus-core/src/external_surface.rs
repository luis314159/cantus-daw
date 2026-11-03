//! Control surfaces (round 36, ADR 0095): what a Mackie Control or HUI unit does to the
//! mixer and what the mixer shows it, whatever the bytes.
//!
//! The protocols (`external_mcu`, `external_hui`) only turn bytes into [`SurfaceEvent`]s and
//! a [`SurfaceView`] into bytes. Everything about *meaning* lives here, once: banks and
//! channel shifts, the fader's scale (the console scale of the mixer's own faders and of
//! automation, `automation::range::fader_db_at`), the V-pot's pan, flip, mute/solo/arm/select,
//! transport, jog, the scribble strips and the meters. The state machine is pure: it never
//! touches a project or a port — the interface thread hands it the mixer as a
//! [`SurfaceView`] and applies the [`SurfaceAction`]s it returns as ordinary commands, so a
//! surface can never do what the screen cannot, and every move undoes like the mouse's.
//!
//! **Motor faders.** A fader the user touches is never moved by the host (it would fight
//! the hand); one that is not touched follows the mixer, and the feedback sends only what
//! changed. When a touch ends the current value is sent once, so the fader lands where the
//! mixer is.

use crate::automation::range::{fader_db_at, fader_pos_of};
use crate::model::{MAX_GAIN_DB, MIN_GAIN_DB};
use std::collections::BTreeMap;

/// Channel strips on one unit.
pub const STRIPS: usize = 8;
/// The master fader's channel on a unit.
pub const MASTER: usize = 8;

/// Which kind of unit a device is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A Mackie Control (Universal) main unit: strips, master, transport, jog, displays.
    Mcu,
    /// A Mackie Control extender (XT): eight more strips, nothing else. `0` is the one
    /// next to the main unit.
    McuExtender(u8),
    /// A Digidesign HUI: strips, transport, jog.
    Hui,
}

impl Kind {
    /// The id the settings and the agents name it by (forever).
    pub fn id(self) -> String {
        match self {
            Kind::Mcu => "mcu".into(),
            Kind::McuExtender(i) => format!("mcu-xt{}", i + 1),
            Kind::Hui => "hui".into(),
        }
    }

    pub fn from_id(id: &str) -> Option<Kind> {
        match id {
            "mcu" => Some(Kind::Mcu),
            "hui" => Some(Kind::Hui),
            _ => id
                .strip_prefix("mcu-xt")
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| (1..=7).contains(n))
                .map(|n| Kind::McuExtender(n - 1)),
        }
    }

    /// Whether it has a master fader, transport and jog.
    pub fn is_main(self) -> bool {
        !matches!(self, Kind::McuExtender(_))
    }

    /// How many strips past the bank's first it starts at.
    pub fn strip_offset(self) -> usize {
        match self {
            Kind::McuExtender(i) => STRIPS * (usize::from(i) + 1),
            _ => 0,
        }
    }
}

/// A button of a unit, by what it means (not by its note).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Button {
    Rec(u8),
    Solo(u8),
    Mute(u8),
    Select(u8),
    VPush(u8),
    BankLeft,
    BankRight,
    ChannelLeft,
    ChannelRight,
    Flip,
    NameValue,
    TimeFormat,
    Rewind,
    Forward,
    Stop,
    Play,
    Record,
    Cycle,
    Click,
    Scrub,
    Undo,
    Save,
    Marker,
    /// HUI's |<RTZ: the playhead to the song's start.
    ReturnToZero,
    Shift,
    Option,
    Control,
    Alt,
    Left,
    Right,
    /// Anything else the unit has (its note or port, kept for the log).
    Other(u8),
}

/// What a unit tells the host.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SurfaceEvent {
    /// A fader moved: channel 0–7 (8 is the master), position 0…1.
    Fader {
        channel: u8,
        pos: f32,
    },
    /// A fader touched or let go.
    Touch {
        channel: u8,
        on: bool,
    },
    Button {
        button: Button,
        down: bool,
    },
    /// A V-pot turned: signed ticks (clockwise positive).
    VPot {
        channel: u8,
        delta: i8,
    },
    /// The jog wheel: signed ticks.
    Jog {
        delta: i8,
    },
}

/// A mixer target of a surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// The mixer's strip at this index (the order the mixer shows them).
    Strip(usize),
    Master,
}

/// What a surface asks of the host, to become commands or actions.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceAction {
    Gain {
        target: Target,
        db: f32,
    },
    Pan {
        target: Target,
        pan: f32,
    },
    Mute {
        target: Target,
        on: bool,
    },
    Solo {
        target: Target,
        on: bool,
    },
    Arm {
        target: Target,
        on: bool,
    },
    Select(Target),
    /// A fader touched or released: the host groups the move into one undo step.
    Touch {
        target: Target,
        on: bool,
    },
    Play,
    Stop,
    Record,
    Cycle,
    Click,
    /// Rewind/forward held (`true`) or released.
    Rewind(bool),
    Forward(bool),
    Scrub(bool),
    /// The jog wheel, in ticks: the host moves the playhead.
    Jog(i32),
    Undo,
    Redo,
    Save,
    Marker,
    PreviousMarker,
    NextMarker,
    /// The playhead to the song's start.
    ReturnToZero,
}

/// One strip as the surface shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StripView {
    pub name: String,
    pub gain_db: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub arm: bool,
    pub selected: bool,
    /// 0…1 peak, linear.
    pub meter: f32,
}

/// The transport as the surface shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransportView {
    pub playing: bool,
    pub recording: bool,
    pub looping: bool,
    pub click: bool,
    pub scrubbing: bool,
    /// The position as the 10-digit display shows it (bars.beats.sub.ticks or time code),
    /// at most 10 characters of digits, '.' and ':' (a dot lights the digit before it).
    pub time: String,
    /// The time is bars and beats (otherwise a clock).
    pub beats: bool,
}

/// The mixer as a surface sees it: every strip (the surface banks over them), the master
/// and the transport.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceView {
    pub strips: Vec<StripView>,
    pub master: StripView,
    pub transport: TransportView,
}

impl SurfaceView {
    fn strip(&self, t: Target) -> Option<&StripView> {
        match t {
            Target::Strip(i) => self.strips.get(i),
            Target::Master => Some(&self.master),
        }
    }
}

/// What was last sent to a device, so only changes go out.
#[derive(Clone, Debug, Default)]
pub(crate) struct Sent {
    pub faders: [Option<u16>; STRIPS + 1],
    pub rings: [Option<u8>; STRIPS],
    pub meters: [Option<u8>; STRIPS],
    /// Button LEDs by note (MCU) or zone/port key (HUI).
    pub leds: BTreeMap<u16, u8>,
    pub lcd: [Vec<u8>; 2],
    pub time: [Option<u8>; 12],
    /// HUI's four-character names, by a checksum of what was sent.
    pub names: [Option<u32>; STRIPS],
    pub online: bool,
}

/// How often the host pings a HUI (its notes: "about every second").
pub const PING_EVERY_MS: u64 = 1_000;
/// A HUI that has not answered a ping for this long is said lost (it goes offline itself
/// after about 2 s without one).
pub const LOST_AFTER_MS: u64 = 3_000;
/// A Mackie meter decays by itself (about 300 ms a step): a steady level is sent again this
/// often so it stays lit.
pub const METER_REFRESH_MS: u64 = 150;
/// A fader without touch sensing: after a move from the unit, the motor waits this long
/// before following the mixer again (it would fight the hand).
pub const NO_TOUCH_HOLD_MS: u64 = 600;

/// One unit's state: where its strips are, what it has sent, what it holds down.
#[derive(Clone, Debug)]
pub struct Device {
    pub kind: Kind,
    pub(crate) touched: [bool; STRIPS + 1],
    pub(crate) sent: Sent,
    /// A fader move seen from the unit this tick: the echo of it is not sent back.
    pub(crate) from_unit: [Option<u16>; STRIPS + 1],
    /// HUI's decoder (the zone it selected, the high half of each fader).
    pub(crate) hui: crate::external_hui::Decoder,
    /// The faders sense touch (every MCU and HUI does; some MCU-mode controllers do not):
    /// when they do not, a fader the unit moved is held [`NO_TOUCH_HOLD_MS`].
    pub touch_sensing: bool,
    /// When each fader was last moved from the unit (ms of the set's clock).
    pub(crate) moved_ms: [Option<u64>; STRIPS + 1],
    /// The keep-alive (HUI): the last ping sent, the first, the last answer, whether the unit
    /// answers, and how many times it was lost.
    pub(crate) last_ping_ms: Option<u64>,
    pub(crate) first_ping_ms: Option<u64>,
    pub(crate) last_pong_ms: Option<u64>,
    pub alive: bool,
    pub lost: u32,
    /// When the meters were last sent again (MCU).
    pub(crate) last_meter_ms: u64,
}

impl Device {
    pub fn new(kind: Kind) -> Device {
        Device {
            kind,
            touched: [false; STRIPS + 1],
            sent: Sent::default(),
            from_unit: [None; STRIPS + 1],
            hui: Default::default(),
            touch_sensing: true,
            moved_ms: [None; STRIPS + 1],
            last_ping_ms: None,
            first_ping_ms: None,
            last_pong_ms: None,
            alive: true,
            lost: 0,
            last_meter_ms: 0,
        }
    }

    /// Whether the host may move fader `c`'s motor now: not while it is touched, and — for a
    /// unit without touch sensing — not within [`NO_TOUCH_HOLD_MS`] of the unit moving it.
    pub(crate) fn motor_may_move(&self, c: usize, now_ms: u64) -> bool {
        !self.touched[c]
            && (self.touch_sensing
                || self.moved_ms[c].is_none_or(|t| now_ms >= t + NO_TOUCH_HOLD_MS))
    }

    /// Forget what was sent: the next render sends everything (a unit just connected, or
    /// switched on).
    pub fn refresh(&mut self) {
        self.sent = Sent::default();
        self.from_unit = [None; STRIPS + 1];
    }
}

/// The surfaces together: the units, the bank they share and the modifiers.
#[derive(Clone, Debug)]
pub struct SurfaceSet {
    pub devices: Vec<Device>,
    /// The first strip (of the mixer's order) the main unit shows.
    pub bank: usize,
    pub flip: bool,
    /// The bottom scribble line shows pan instead of gain.
    pub show_pan: bool,
    pub shift: bool,
    /// The set's clock, in ms (the interface's, given with each [`SurfaceSet::output_at`]).
    pub now_ms: u64,
}

/// The longest a tick step of a V-pot is, on the pan: 2 % of the travel (a full sweep in
/// fifty ticks), ten times finer with Shift held.
const PAN_STEP: f32 = 0.04;

impl SurfaceSet {
    pub fn new(kinds: &[Kind]) -> SurfaceSet {
        SurfaceSet {
            devices: kinds.iter().map(|k| Device::new(*k)).collect(),
            bank: 0,
            flip: false,
            show_pan: false,
            shift: false,
            now_ms: 0,
        }
    }

    /// The mixer strip a device's channel `ch` shows now.
    pub fn target_of(&self, dev: usize, ch: u8, view: &SurfaceView) -> Option<Target> {
        let kind = self.devices.get(dev)?.kind;
        if usize::from(ch) == MASTER {
            return kind.is_main().then_some(Target::Master);
        }
        let i = self.bank + kind.strip_offset() + usize::from(ch);
        (i < view.strips.len()).then_some(Target::Strip(i))
    }

    fn last_bank(&self, view: &SurfaceView) -> usize {
        // The units together show this many strips; the bank stops where the last strip is
        // on the last unit's last channel.
        let shown = self
            .devices
            .iter()
            .map(|d| d.kind.strip_offset() + STRIPS)
            .max()
            .unwrap_or(STRIPS);
        view.strips.len().saturating_sub(shown)
    }

    /// An event of device `dev`, and the actions it asks for.
    pub fn handle(
        &mut self,
        dev: usize,
        ev: SurfaceEvent,
        view: &SurfaceView,
    ) -> Vec<SurfaceAction> {
        let mut out = Vec::new();
        let Some(kind) = self.devices.get(dev).map(|d| d.kind) else {
            return out;
        };
        match ev {
            SurfaceEvent::Fader { channel, pos } => {
                let Some(t) = self.target_of(dev, channel, view) else {
                    return out;
                };
                // The unit's own move is what it shows: never sent back to it.
                self.devices[dev].from_unit[usize::from(channel).min(MASTER)] = Some(to14(pos));
                self.devices[dev].moved_ms[usize::from(channel).min(MASTER)] = Some(self.now_ms);
                if self.flip && t != Target::Master {
                    out.push(SurfaceAction::Pan {
                        target: t,
                        // A hundredth of the travel: what a screen shows (and a 14-bit position's own
                        // rounding is not a pan of its own).
                        pan: (((pos * 2.0 - 1.0) * 100.0).round() / 100.0).clamp(-1.0, 1.0),
                    });
                } else {
                    out.push(SurfaceAction::Gain {
                        target: t,
                        db: snap_db(fader_db_at(pos)),
                    });
                }
            }
            SurfaceEvent::Touch { channel, on } => {
                if let Some(t) = self.target_of(dev, channel, view) {
                    self.devices[dev].touched[usize::from(channel).min(MASTER)] = on;
                    if !on {
                        // Let the fader land where the mixer is.
                        self.devices[dev].sent.faders[usize::from(channel).min(MASTER)] = None;
                    }
                    out.push(SurfaceAction::Touch { target: t, on });
                }
            }
            SurfaceEvent::VPot { channel, delta } => {
                if let Some(t @ Target::Strip(_)) = self.target_of(dev, channel, view) {
                    let step = if self.shift {
                        PAN_STEP / 10.0
                    } else {
                        PAN_STEP
                    };
                    if self.flip {
                        let db = view.strip(t).map_or(0.0, |s| s.gain_db) + f32::from(delta) * 0.5;
                        out.push(SurfaceAction::Gain {
                            target: t,
                            db: db.clamp(MIN_GAIN_DB, MAX_GAIN_DB),
                        });
                    } else {
                        let pan = view.strip(t).map_or(0.0, |s| s.pan) + f32::from(delta) * step;
                        out.push(SurfaceAction::Pan {
                            target: t,
                            pan: pan.clamp(-1.0, 1.0),
                        });
                    }
                }
            }
            SurfaceEvent::Jog { delta } => {
                if kind.is_main() {
                    out.push(SurfaceAction::Jog(i32::from(delta)));
                }
            }
            SurfaceEvent::Button { button, down } => self.button(dev, button, down, view, &mut out),
        }
        out
    }

    fn button(
        &mut self,
        dev: usize,
        b: Button,
        down: bool,
        view: &SurfaceView,
        out: &mut Vec<SurfaceAction>,
    ) {
        let kind = self.devices[dev].kind;
        let strip = |s: &Self, ch: u8| s.target_of(dev, ch, view);
        match b {
            Button::Shift => self.shift = down,
            Button::Rewind if kind.is_main() => out.push(SurfaceAction::Rewind(down)),
            Button::Forward if kind.is_main() => out.push(SurfaceAction::Forward(down)),
            Button::Scrub if kind.is_main() && down => {
                out.push(SurfaceAction::Scrub(!view.transport.scrubbing))
            }
            _ if !down => {}
            Button::Rec(c) => {
                if let Some(t) = strip(self, c) {
                    let on = !view.strip(t).is_some_and(|s| s.arm);
                    out.push(SurfaceAction::Arm { target: t, on });
                }
            }
            Button::Solo(c) => {
                if let Some(t) = strip(self, c) {
                    let on = !view.strip(t).is_some_and(|s| s.solo);
                    out.push(SurfaceAction::Solo { target: t, on });
                }
            }
            Button::Mute(c) => {
                if let Some(t) = strip(self, c) {
                    let on = !view.strip(t).is_some_and(|s| s.mute);
                    out.push(SurfaceAction::Mute { target: t, on });
                }
            }
            Button::Select(c) => {
                if let Some(t) = strip(self, c) {
                    out.push(SurfaceAction::Select(t));
                }
            }
            Button::VPush(c) => {
                if let Some(t @ Target::Strip(_)) = strip(self, c) {
                    out.push(if self.flip {
                        SurfaceAction::Gain { target: t, db: 0.0 }
                    } else {
                        SurfaceAction::Pan {
                            target: t,
                            pan: 0.0,
                        }
                    });
                }
            }
            Button::BankLeft => self.shift_bank(-(STRIPS as isize) * self.span(), view),
            Button::BankRight => self.shift_bank(STRIPS as isize * self.span(), view),
            Button::ChannelLeft => self.shift_bank(-1, view),
            Button::ChannelRight => self.shift_bank(1, view),
            Button::Flip => {
                self.flip = !self.flip;
                self.refresh();
            }
            Button::NameValue => {
                self.show_pan = !self.show_pan;
                self.refresh();
            }
            Button::Marker if kind.is_main() => out.push(SurfaceAction::Marker),
            Button::ReturnToZero if kind.is_main() => out.push(SurfaceAction::ReturnToZero),
            Button::Left if kind.is_main() => out.push(SurfaceAction::PreviousMarker),
            Button::Right if kind.is_main() => out.push(SurfaceAction::NextMarker),
            Button::Stop if kind.is_main() => out.push(SurfaceAction::Stop),
            Button::Play if kind.is_main() => out.push(SurfaceAction::Play),
            Button::Record if kind.is_main() => out.push(SurfaceAction::Record),
            Button::Cycle if kind.is_main() => out.push(SurfaceAction::Cycle),
            Button::Click if kind.is_main() => out.push(SurfaceAction::Click),
            Button::Undo if kind.is_main() => out.push(if self.shift {
                SurfaceAction::Redo
            } else {
                SurfaceAction::Undo
            }),
            Button::Save if kind.is_main() => out.push(SurfaceAction::Save),
            _ => {}
        }
    }

    /// How many units' worth a bank step moves: every unit shows its eight strips.
    fn span(&self) -> isize {
        self.devices.len().max(1) as isize
    }

    fn shift_bank(&mut self, by: isize, view: &SurfaceView) {
        let last = self.last_bank(view) as isize;
        let next = (self.bank as isize + by).clamp(0, last.max(0));
        if next as usize != self.bank {
            self.bank = next as usize;
            self.refresh();
        }
    }

    /// Whether a fader of any unit is being touched now.
    pub fn any_touched(&self) -> bool {
        self.devices.iter().any(|d| d.touched.iter().any(|t| *t))
    }

    /// Every unit sends everything again at the next render.
    pub fn refresh(&mut self) {
        for d in &mut self.devices {
            d.refresh();
        }
    }
}

/// The position 0…1 as the 14 bits a unit sends.
pub fn to14(pos: f32) -> u16 {
    (pos.clamp(0.0, 1.0) * 16383.0).round() as u16
}

/// The 14 bits as 0…1.
pub fn from14(v: u16) -> f32 {
    f32::from(v.min(16383)) / 16383.0
}

/// A fader's dB rounded to a tenth (a unit's 14 bits are finer than a tenth of a dB, and a
/// mixer shows tenths): the value a surface's move sets.
pub fn snap_db(db: f32) -> f32 {
    if db <= MIN_GAIN_DB + 0.05 {
        MIN_GAIN_DB
    } else {
        ((db * 10.0).round() / 10.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB)
    }
}

/// The 14-bit fader position of a gain.
pub fn fader_value(db: f32) -> u16 {
    to14(fader_pos_of(db))
}

/// The pan as a V-pot ring position 1…11 (6 is the centre).
pub fn ring_of(pan: f32) -> u8 {
    (((pan.clamp(-1.0, 1.0) + 1.0) * 5.0).round() as u8 + 1).clamp(1, 11)
}

/// A peak as a meter's 0…12 steps (5 dB each from −60 dB), 0 for silence.
pub fn meter_step(peak: f32) -> u8 {
    if peak.is_nan() || peak <= 0.0 {
        return 0;
    }
    let db = 20.0 * cantus_dsp::math::log10(peak.min(4.0));
    (((db + 60.0) / 5.0).floor().clamp(0.0, 12.0)) as u8
}

/// The text of a strip's gain for the scribble strip: at most 6 characters.
pub fn gain_text(db: f32) -> String {
    if db <= MIN_GAIN_DB + 0.05 {
        "  -inf".into()
    } else {
        let s = format!("{db:.1}dB");
        if s.len() > 6 {
            format!("{db:.0}dB")
        } else {
            format!("{s:>6}")
        }
    }
}

/// The text of a pan: `C`, `L40`, `R12` (percent of the travel).
pub fn pan_text(pan: f32) -> String {
    let p = (pan * 100.0).round() as i32;
    match p {
        0 => "     C".into(),
        p if p < 0 => format!("{:>6}", format!("L{}", -p)),
        p => format!("{:>6}", format!("R{p}")),
    }
}

/// A name for a scribble strip: 6 characters, ASCII only (the unit's character set), padded.
pub fn name_text(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_control() {
                c
            } else {
                '?'
            }
        })
        .take(6)
        .collect();
    while s.len() < 6 {
        s.push(' ');
    }
    s
}

/// What a device shows on a channel: the target it is bound to, if any.
pub(crate) fn bound<'a>(
    set: &SurfaceSet,
    dev: usize,
    ch: usize,
    view: &'a SurfaceView,
) -> Option<&'a StripView> {
    match set.target_of(dev, ch as u8, view)? {
        Target::Strip(i) => view.strips.get(i),
        Target::Master => Some(&view.master),
    }
}

/// What a device's bytes came to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Input {
    pub actions: Vec<SurfaceAction>,
    /// Bytes to send straight back to the device (a ping's answer, a handshake's reply).
    pub reply: Vec<Vec<u8>>,
    /// HUI zones and ports heard that nothing is mapped to, for the log.
    pub unmapped: Vec<(u8, u8)>,
}

impl SurfaceSet {
    /// Device `dev` sent `bytes`: the actions they ask for and what to answer.
    pub fn input(&mut self, dev: usize, bytes: &[u8], view: &SurfaceView) -> Input {
        let mut out = Input::default();
        let Some(kind) = self.devices.get(dev).map(|d| d.kind) else {
            return out;
        };
        let mut events = Vec::new();
        match kind {
            Kind::Mcu | Kind::McuExtender(_) => {
                if bytes.first() == Some(&0xF0) {
                    out.reply
                        .extend(crate::external_mcu::connection_reply(kind, bytes));
                } else {
                    events.extend(crate::external_mcu::decode(bytes));
                }
            }
            Kind::Hui => {
                let d = self.devices[dev].hui.feed(bytes);
                events.extend(d.events);
                out.unmapped = d.unmapped;
                if d.pong {
                    let now = self.now_ms;
                    let dv = &mut self.devices[dev];
                    dv.last_pong_ms = Some(now);
                    if !dv.alive {
                        // Back from a loss: it shows nothing of ours any more.
                        dv.alive = true;
                        dv.refresh();
                    }
                }
            }
        }
        for e in events {
            out.actions.extend(self.handle(dev, e, view));
        }
        out
    }

    /// What every device should be sent now (only what changed): `(device, bytes)`.
    pub fn output(&mut self, view: &SurfaceView) -> Vec<(usize, Vec<u8>)> {
        self.output_at(view, self.now_ms)
    }

    /// [`Self::output`] at `now_ms` on the interface's clock (the keep-alive, the meters'
    /// refresh and the no-touch hold count by it).
    pub fn output_at(&mut self, view: &SurfaceView, now_ms: u64) -> Vec<(usize, Vec<u8>)> {
        self.now_ms = self.now_ms.max(now_ms);
        let mut out = Vec::new();
        for dev in 0..self.devices.len() {
            let msgs = match self.devices[dev].kind {
                Kind::Mcu | Kind::McuExtender(_) => crate::external_mcu::render(self, dev, view),
                Kind::Hui => crate::external_hui::render(self, dev, view),
            };
            out.extend(msgs.into_iter().map(|m| (dev, m)));
        }
        out
    }
}
