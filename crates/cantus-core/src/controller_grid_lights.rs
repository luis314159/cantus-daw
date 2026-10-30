//! A grid's lights as bytes (ADR 0077): the Launchpads' LED SysEx (03h, colour specs of
//! type 0 static, 1 flashing, 2 pulsing, 3 RGB); the APC mini mk2's RGB SysEx (24h) and
//! note-ons whose channel sets a light's behaviour; the APC40 mkII's note-ons.

use super::{addresses, launchpad_led, on, Addr};
use crate::controller::grid::{GridDevice, GridFrame, Light, Rgb};

/// The most colour specs one Launchpad SysEx carries (the Mini's 81 LEDs fit; the Pro's
/// 106 take two).
const SPECS_PER_MESSAGE: usize = 80;

/// The palette entry nearest `c` in the Launchpads' and APCs' shared 128-colour palette
/// (Ableton's): the bright entry of its hue (5 red, 9 orange, 13 yellow, 17 lime, 21
/// green, 29 spring, 33 turquoise, 37 cyan, 41 sky, 45 blue, 49 indigo, 53 violet, 57
/// magenta), its darker neighbour (+2) below half brightness, white (3) or grey (1) for a
/// colour with no hue, 0 for black. What flashing and pulsing take: they name colours by
/// palette entry only.
pub fn palette_index(c: Rgb) -> u8 {
    let (r, g, b) = (f32::from(c.0), f32::from(c.1), f32::from(c.2));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max < 16.0 {
        return 0;
    }
    if max - min < 0.2 * max {
        return if max >= 128.0 { 3 } else { 1 };
    }
    let d = max - min;
    let h = if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    const HUES: [(f32, u8); 14] = [
        (0.0, 5),
        (30.0, 9),
        (60.0, 13),
        (90.0, 17),
        (120.0, 21),
        (150.0, 29),
        (170.0, 33),
        (190.0, 37),
        (210.0, 41),
        (240.0, 45),
        (260.0, 49),
        (280.0, 53),
        (310.0, 57),
        (360.0, 5),
    ];
    let mut best = (f32::MAX, 5u8);
    for (hue, idx) in HUES {
        let dist = (h - hue).abs();
        if dist < best.0 {
            best = (dist, idx);
        }
    }
    if max < 128.0 {
        best.1 + 2
    } else {
        best.1
    }
}

/// 0…255 to the Launchpads' 0…127.
fn seven(v: u8) -> u8 {
    v >> 1
}

pub(crate) fn lights(d: GridDevice, old: Option<&GridFrame>, new: &GridFrame) -> Vec<Vec<u8>> {
    let now = addresses(d, new);
    let before = old.map(|o| addresses(d, o));
    let changed: Vec<(Addr, Light)> = now
        .into_iter()
        .enumerate()
        .filter(|(i, (_, l))| before.as_ref().is_none_or(|b| b[*i].1 != *l))
        .map(|(_, x)| x)
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    match d {
        GridDevice::LaunchpadMiniMk3 | GridDevice::LaunchpadX | GridDevice::LaunchpadProMk3 => {
            launchpad(d, &changed)
        }
        GridDevice::ApcMiniMk2 => apc_mini(&changed),
        GridDevice::Apc40Mk2 => apc40(&changed),
    }
}

/// One LED SysEx per 80 lights: `F0 00 20 29 02 <id> 03 <spec>… F7`.
fn launchpad(d: GridDevice, changed: &[(Addr, Light)]) -> Vec<Vec<u8>> {
    let id = match d {
        GridDevice::LaunchpadMiniMk3 => 0x0D,
        GridDevice::LaunchpadX => 0x0C,
        _ => 0x0E,
    };
    let mut out = Vec::new();
    for chunk in changed.chunks(SPECS_PER_MESSAGE) {
        let mut m = vec![0xF0, 0x00, 0x20, 0x29, 0x02, id, 0x03];
        for (a, l) in chunk {
            let led = launchpad_led(d, *a);
            match *l {
                Light::Off => m.extend_from_slice(&[0, led, 0]),
                Light::On(c) => m.extend_from_slice(&[3, led, seven(c.0), seven(c.1), seven(c.2)]),
                // Colour B (the event's), then colour A (dark): flashes between them.
                Light::Flash(c) => m.extend_from_slice(&[1, led, palette_index(c), 0]),
                Light::Pulse(c) => m.extend_from_slice(&[2, led, palette_index(c)]),
            }
        }
        m.push(0xF7);
        out.push(m);
    }
    out
}

/// The channel of a note-on that pulses (1/4 note) or blinks (1/4 note) on the APCs: their
/// channels 0–6 set a brightness, 7–10 pulse and 11–15 blink, at 1/24 … 1/2.
const APC_PULSE: u8 = 0x99;
const APC_BLINK: u8 = 0x9E;

/// The APC mini mk2: steady pads by the RGB SysEx (`F0 47 7F 4F 24 <len> <start> <end>
/// <r> <g> <b> F7`, each colour 0…255 as two 7-bit bytes), flashing and pulsing pads by a
/// note-on on the behaviour's channel, the single-colour buttons by velocity (0 off, 1
/// on, 2 blinking).
fn apc_mini(changed: &[(Addr, Light)]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rgb: Vec<(u8, Rgb)> = Vec::new();
    for (a, l) in changed {
        match *a {
            Addr::Pad(col, row) => {
                let n = (7 - row) * 8 + col;
                match *l {
                    Light::Off => out.push(vec![0x90, n, 0]),
                    Light::On(c) => rgb.push((n, c)),
                    Light::Flash(c) => out.push(vec![APC_BLINK, n, palette_index(c)]),
                    Light::Pulse(c) => out.push(vec![APC_PULSE, n, palette_index(c)]),
                }
            }
            Addr::Scene(row) => out.push(vec![0x90, 112 + row, button(*l)]),
            Addr::Stop(col) => out.push(vec![0x90, 100 + col, button(*l)]),
            _ => {}
        }
    }
    for chunk in rgb.chunks(32) {
        let len = (chunk.len() * 8) as u16;
        let mut m = vec![
            0xF0,
            0x47,
            0x7F,
            0x4F,
            0x24,
            (len >> 7) as u8,
            (len & 0x7F) as u8,
        ];
        for (n, c) in chunk {
            m.extend_from_slice(&[*n, *n]);
            for v in [c.0, c.1, c.2] {
                m.extend_from_slice(&[v >> 7, v & 0x7F]);
            }
        }
        m.push(0xF7);
        out.push(m);
    }
    out
}

/// A single-colour button's velocity: 0 off, 1 on, 2 blinking.
fn button(l: Light) -> u8 {
    match l {
        _ if !on(l) => 0,
        Light::Flash(_) | Light::Pulse(_) => 2,
        _ => 1,
    }
}

/// The APC40 mkII: pads by palette entry, steady on channel 0, flashing and pulsing on
/// their behaviour's channel; scene launch, clip stop (on the track's channel), stop all
/// and the bank buttons by velocity.
fn apc40(changed: &[(Addr, Light)]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (a, l) in changed {
        match *a {
            Addr::Pad(col, row) => {
                let n = (4 - row) * 8 + col;
                out.push(match *l {
                    Light::Off => vec![0x90, n, 0],
                    Light::On(c) => vec![0x90, n, palette_index(c)],
                    Light::Flash(c) => vec![APC_BLINK, n, palette_index(c)],
                    Light::Pulse(c) => vec![APC_PULSE, n, palette_index(c)],
                });
            }
            Addr::Scene(row) => out.push(vec![0x90, 0x52 + row, button(*l)]),
            Addr::Stop(col) => out.push(vec![0x90 | col, 0x34, button(*l)]),
            Addr::StopAll => out.push(vec![0x90, 0x51, button(*l)]),
            Addr::Arrow(a) => {
                let n = match a {
                    crate::controller::grid::Arrow::Up => 0x5E,
                    crate::controller::grid::Arrow::Down => 0x5F,
                    crate::controller::grid::Arrow::Right => 0x60,
                    crate::controller::grid::Arrow::Left => 0x61,
                };
                out.push(vec![0x90, n, button(*l)]);
            }
            Addr::StopMode => {}
        }
    }
    out
}
