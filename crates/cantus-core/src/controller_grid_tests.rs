//! The grids' bytes against their references (ADR 0077): recorded presses decoded, the
//! mode messages, and the lighting messages for known pictures — no hardware needed.

use super::*;

const MINI: GridDevice = GridDevice::LaunchpadMiniMk3;
const X: GridDevice = GridDevice::LaunchpadX;
const PRO: GridDevice = GridDevice::LaunchpadProMk3;
const APCM: GridDevice = GridDevice::ApcMiniMk2;
const APC40: GridDevice = GridDevice::Apc40Mk2;

#[test]
fn ports_are_found_by_the_name_of_the_interface_each_mode_talks_on() {
    // Windows: the Mini's and the X's first interface is the DAW's; the second the MIDI's.
    assert_eq!(GridDevice::detect("LPMiniMK3 MIDI"), None);
    assert_eq!(GridDevice::detect("MIDIIN2 (LPMiniMK3 MIDI)"), Some(MINI));
    assert_eq!(GridDevice::detect("MIDIIN2 (LPX MIDI)"), Some(X));
    assert_eq!(GridDevice::detect("LPX MIDI"), None);
    // macOS and Linux name the interface.
    assert_eq!(
        GridDevice::detect("Launchpad Mini MK3 LPMiniMK3 MIDI Out"),
        Some(MINI)
    );
    assert_eq!(
        GridDevice::detect("Launchpad Mini MK3 LPMiniMK3 DAW Out"),
        None
    );
    assert_eq!(
        GridDevice::detect("Launchpad Mini MK3:Launchpad Mini MK3 LPMiniMK3 MI 20:1"),
        Some(MINI)
    );
    assert_eq!(
        GridDevice::detect("Launchpad Mini MK3:Launchpad Mini MK3 LPMiniMK3 DA 20:0"),
        None
    );
    assert_eq!(
        GridDevice::detect("Launchpad X:Launchpad X LPX MIDI In 24:1"),
        Some(X)
    );
    // The Pro's programmer mode is on its first interface; the DIN and DAW ports are not.
    assert_eq!(GridDevice::detect("LPProMK3 MIDI"), Some(PRO));
    assert_eq!(GridDevice::detect("MIDIIN2 (LPProMK3 MIDI)"), None);
    assert_eq!(GridDevice::detect("MIDIIN3 (LPProMK3 MIDI)"), None);
    assert_eq!(
        GridDevice::detect("Launchpad Pro MK3 LPProMK3 MIDI In"),
        Some(PRO)
    );
    assert_eq!(
        GridDevice::detect("Launchpad Pro MK3 LPProMK3 DAW In"),
        None
    );
    assert_eq!(GridDevice::detect("APC mini mk2"), Some(APCM));
    assert_eq!(GridDevice::detect("MIDIIN2 (APC mini mk2)"), None);
    assert_eq!(GridDevice::detect("APC mini mk2 Control"), Some(APCM));
    assert_eq!(GridDevice::detect("APC mini mk2 Notes"), None);
    assert_eq!(GridDevice::detect("APC40 mkII"), Some(APC40));
    assert_eq!(GridDevice::detect("nanoKONTROL2"), None);
    assert_eq!(GridDevice::detect("Launchkey Mini MK3 MIDI"), None);
}

#[test]
fn the_mode_messages_are_the_references_bytes() {
    assert_eq!(
        MINI.hello(),
        vec![vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x0E, 0x01, 0xF7]]
    );
    assert_eq!(
        X.hello(),
        vec![vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x0C, 0x0E, 0x01, 0xF7]]
    );
    assert_eq!(
        PRO.hello(),
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x0E, 0x00, 0x11, 0x00, 0x00, 0xF7
        ]]
    );
    assert_eq!(
        APCM.hello(),
        vec![vec![
            0xF0, 0x47, 0x7F, 0x4F, 0x60, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0xF7
        ]]
    );
    assert_eq!(
        APC40.hello(),
        vec![vec![
            0xF0, 0x47, 0x7F, 0x29, 0x60, 0x00, 0x04, 0x41, 0x00, 0x00, 0x00, 0xF7
        ]]
    );
    // Let go: the lights dark, then the device's own mode.
    let bye = MINI.goodbye();
    assert_eq!(
        bye.last().unwrap(),
        &vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x0E, 0x00, 0xF7]
    );
    assert!(bye.len() >= 2);
    assert_eq!(
        APC40.goodbye().last().unwrap(),
        &vec![0xF0, 0x47, 0x7F, 0x29, 0x60, 0x00, 0x04, 0x40, 0x00, 0x00, 0x00, 0xF7]
    );
}

#[test]
fn a_launchpad_pads_and_buttons_decode_from_the_programmer_layout() {
    // The reference's examples: 11 the lower left, 81 the upper left, 18 the lower right.
    assert_eq!(
        MINI.decode(0x90, 11, 100, false),
        Some(GridInput::Pad {
            col: 0,
            row: 7,
            velocity: 100
        })
    );
    assert_eq!(
        MINI.decode(0x90, 81, 64, false),
        Some(GridInput::Pad {
            col: 0,
            row: 0,
            velocity: 64
        })
    );
    assert_eq!(
        MINI.decode(0x90, 18, 0, false),
        Some(GridInput::Pad {
            col: 7,
            row: 7,
            velocity: 0
        })
    );
    assert_eq!(
        X.decode(0x80, 88, 0, false),
        Some(GridInput::Pad {
            col: 7,
            row: 0,
            velocity: 0
        })
    );
    // The right column: 89 the top scene, 19 the Mini's stop mode.
    assert_eq!(
        MINI.decode(0xB0, 89, 127, false),
        Some(GridInput::Scene {
            row: 0,
            pressed: true
        })
    );
    assert_eq!(
        MINI.decode(0xB0, 29, 0, false),
        Some(GridInput::Scene {
            row: 6,
            pressed: false
        })
    );
    assert_eq!(
        MINI.decode(0xB0, 19, 127, false),
        Some(GridInput::StopMode { pressed: true })
    );
    assert_eq!(
        PRO.decode(0xB0, 19, 127, false),
        Some(GridInput::Scene {
            row: 7,
            pressed: true
        })
    );
    // The top row's arrows, the Session button (stop all).
    for (cc, arrow) in [
        (91, Arrow::Up),
        (92, Arrow::Down),
        (93, Arrow::Left),
        (94, Arrow::Right),
    ] {
        assert_eq!(
            X.decode(0xB0, cc, 127, false),
            Some(GridInput::Arrow {
                arrow,
                pressed: true
            })
        );
    }
    assert_eq!(
        MINI.decode(0xB0, 95, 127, false),
        Some(GridInput::StopAll { pressed: true })
    );
    // The Pro: its arrows, track buttons as stops, Stop Clip, Shift.
    assert_eq!(
        PRO.decode(0xB0, 80, 127, false),
        Some(GridInput::Arrow {
            arrow: Arrow::Up,
            pressed: true
        })
    );
    assert_eq!(
        PRO.decode(0xB0, 92, 127, false),
        Some(GridInput::Arrow {
            arrow: Arrow::Right,
            pressed: true
        })
    );
    // The Pro's track control row (CC 1–8): Stop Clip (8) held makes the track select row
    // (CC 101–108) the stops; with Shift it stops all; the rest is no part of the grid.
    assert_eq!(
        PRO.decode(0xB0, 8, 127, false),
        Some(GridInput::StopMode { pressed: true })
    );
    assert_eq!(
        PRO.decode(0xB0, 8, 127, true),
        Some(GridInput::StopAll { pressed: true })
    );
    assert_eq!(
        PRO.decode(0xB0, 103, 127, false),
        Some(GridInput::TrackSelect {
            col: 2,
            pressed: true
        })
    );
    assert_eq!(PRO.decode(0xB0, 3, 127, false), None);
    // Session, Note, Chord (CC 93–95) are the Pro's mode buttons, not arrows or stop all.
    for cc in [93, 94, 95, 96, 97, 98] {
        assert_eq!(PRO.decode(0xB0, cc, 127, false), None, "CC {cc}");
    }
    // What is no part of the grid: aftertouch, the logo, a button Cantus leaves alone.
    assert_eq!(MINI.decode(0xA0, 11, 40, false), None);
    assert_eq!(MINI.decode(0xB0, 99, 127, false), None);
    assert_eq!(MINI.decode(0x90, 9, 127, false), None);
    // Of those, a pad's (a note, its aftertouch, channel pressure) stays the grid's; a
    // fader's, a knob's, the wheel, a program change go on to learn and the mappings.
    for status in [0x80, 0x90, 0xA0, 0xD0] {
        assert!(!GridDevice::passes_on(status), "{status:#x}");
    }
    for status in [0xB0, 0xB5, 0xC0, 0xE0] {
        assert!(GridDevice::passes_on(status), "{status:#x}");
    }
}

#[test]
fn an_apc_decodes_its_notes_and_its_shift_layer() {
    assert_eq!(
        APCM.decode(0x90, 0, 127, false),
        Some(GridInput::Pad {
            col: 0,
            row: 7,
            velocity: 127
        })
    );
    assert_eq!(
        APCM.decode(0x90, 63, 127, false),
        Some(GridInput::Pad {
            col: 7,
            row: 0,
            velocity: 127
        })
    );
    assert_eq!(
        APCM.decode(0x80, 63, 0, false),
        Some(GridInput::Pad {
            col: 7,
            row: 0,
            velocity: 0
        })
    );
    assert_eq!(
        APCM.decode(0x90, 102, 127, false),
        Some(GridInput::Stop {
            col: 2,
            pressed: true
        })
    );
    assert_eq!(
        APCM.decode(0x90, 102, 127, true),
        Some(GridInput::Arrow {
            arrow: Arrow::Left,
            pressed: true
        })
    );
    assert_eq!(
        APCM.decode(0x90, 119, 127, true),
        Some(GridInput::StopAll { pressed: true })
    );
    assert_eq!(
        APCM.decode(0x90, 119, 127, false),
        Some(GridInput::Scene {
            row: 7,
            pressed: true
        })
    );
    // Shift's other layers: nothing (never a stop or a launch).
    assert_eq!(APCM.decode(0x90, 105, 127, true), None);
    assert_eq!(APCM.decode(0x90, 113, 127, true), None);
    assert_eq!(
        APCM.decode(0x90, 122, 127, false),
        Some(GridInput::Shift { pressed: true })
    );
    // A fader is no part of the grid.
    assert_eq!(APCM.decode(0xB0, 48, 90, false), None);
    // The APC40: the top row is 0x20–0x27, the clip stops ride the track's channel.
    assert_eq!(
        APC40.decode(0x90, 0x20, 127, false),
        Some(GridInput::Pad {
            col: 0,
            row: 0,
            velocity: 127
        })
    );
    assert_eq!(
        APC40.decode(0x90, 0x07, 127, false),
        Some(GridInput::Pad {
            col: 7,
            row: 4,
            velocity: 127
        })
    );
    assert_eq!(
        APC40.decode(0x93, 0x34, 127, false),
        Some(GridInput::Stop {
            col: 3,
            pressed: true
        })
    );
    assert_eq!(
        APC40.decode(0x80, 0x56, 0, false),
        Some(GridInput::Scene {
            row: 4,
            pressed: false
        })
    );
    assert_eq!(
        APC40.decode(0x90, 0x51, 127, false),
        Some(GridInput::StopAll { pressed: true })
    );
    assert_eq!(
        APC40.decode(0x90, 0x61, 127, false),
        Some(GridInput::Arrow {
            arrow: Arrow::Left,
            pressed: true
        })
    );
}

fn frame_with(pad: (usize, usize), l: Light) -> GridFrame {
    let mut f = GridFrame::dark();
    f.pads[pad.1][pad.0] = l;
    f
}

#[test]
fn a_launchpad_lights_what_changed_in_one_led_sysex() {
    let dark = GridFrame::dark();
    // Nothing changed: nothing sent.
    assert!(MINI.lights(Some(&dark), &dark).is_empty());
    // The lower left pad red, steady: an RGB spec (type 3) at LED 11, 0…127.
    let red = frame_with((0, 7), Light::On(Rgb(255, 0, 0)));
    assert_eq!(
        MINI.lights(Some(&dark), &red),
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x03, 3, 11, 127, 0, 0, 0xF7
        ]]
    );
    // The upper left flashing green: type 1, colour B the palette's green (21), A dark.
    let flash = frame_with((0, 0), Light::Flash(Rgb(0, 255, 0)));
    assert_eq!(
        X.lights(Some(&dark), &flash),
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x0C, 0x03, 1, 81, 21, 0, 0xF7
        ]]
    );
    // The lower right pulsing blue: type 2, the palette's blue (45).
    let pulse = frame_with((7, 7), Light::Pulse(Rgb(0, 0, 255)));
    assert_eq!(
        PRO.lights(Some(&dark), &pulse),
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x0E, 0x03, 2, 18, 45, 0xF7
        ]]
    );
    // Off again: type 0, entry 0.
    assert_eq!(
        MINI.lights(Some(&red), &dark),
        vec![vec![
            0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x03, 0, 11, 0, 0xF7
        ]]
    );
    // Everything unknown: every light the Mini has (64 pads, 7 scenes, stop mode, stop
    // all, four arrows = 77) in one message.
    let all = MINI.lights(None, &dark);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].len(), 7 + 77 * 3 + 1);
    // The Pro's 64 + 8 + 8 + 1 + 4 = 85 lights take two messages.
    assert_eq!(PRO.lights(None, &dark).len(), 2);
}

#[test]
fn an_apc_mini_lights_steady_pads_by_rgb_sysex_and_the_rest_by_note() {
    let dark = GridFrame::dark();
    let mut f = frame_with((0, 7), Light::On(Rgb(255, 128, 0)));
    f.pads[0][7] = Light::Pulse(Rgb(255, 0, 0));
    f.scenes[2] = Light::On(Rgb::GREEN);
    f.stops[1] = Light::Flash(Rgb::RED);
    let out = APCM.lights(Some(&dark), &f);
    // Pad 0 (lower left) steady orange: F0 47 7F 4F 24, length 8, start 0, end 0, then
    // each colour as two 7-bit bytes (255 = 01 7F, 128 = 01 00, 0 = 00 00).
    assert!(out.contains(&vec![
        0xF0, 0x47, 0x7F, 0x4F, 0x24, 0x00, 0x08, 0, 0, 0x01, 0x7F, 0x01, 0x00, 0x00, 0x00, 0xF7
    ]));
    // Pad 63 (upper right) pulsing red at a quarter: channel 9, the palette's red.
    assert!(out.contains(&vec![0x99, 63, 5]));
    // Scene 3 on, track 2's stop blinking.
    assert!(out.contains(&vec![0x90, 114, 1]));
    assert!(out.contains(&vec![0x90, 101, 2]));
    assert_eq!(out.len(), 4);
}

#[test]
fn an_apc40_lights_by_palette_and_channel() {
    let dark = GridFrame::dark();
    let mut f = frame_with((0, 0), Light::On(Rgb(255, 255, 0)));
    f.stops[3] = Light::On(Rgb::RED);
    f.stop_all = Light::On(Rgb::RED);
    f.arrows[1] = Light::On(Rgb::WHITE);
    // A row the APC40 does not have is not sent.
    f.pads[6][0] = Light::On(Rgb::WHITE);
    let out = APC40.lights(Some(&dark), &f);
    assert_eq!(
        out,
        vec![
            vec![0x90, 0x20, 13],
            vec![0x93, 0x34, 1],
            vec![0x90, 0x51, 1],
            vec![0x90, 0x5F, 1],
        ]
    );
}

#[test]
fn the_palette_names_the_hues_and_their_dark_neighbours() {
    assert_eq!(palette_index(Rgb(0, 0, 0)), 0);
    assert_eq!(palette_index(Rgb(255, 255, 255)), 3);
    assert_eq!(palette_index(Rgb(90, 90, 90)), 1);
    assert_eq!(palette_index(Rgb(255, 0, 0)), 5);
    assert_eq!(palette_index(Rgb(255, 128, 0)), 9);
    assert_eq!(palette_index(Rgb(255, 255, 0)), 13);
    assert_eq!(palette_index(Rgb(0, 255, 0)), 21);
    assert_eq!(palette_index(Rgb(0, 0, 255)), 45);
    assert_eq!(palette_index(Rgb(255, 0, 255)), 57);
    assert_eq!(palette_index(Rgb(100, 0, 0)), 7);
}

#[test]
fn the_ring_moves_inside_the_session_and_its_arrows_say_where_it_can() {
    let size = (12, 20);
    let view = (8, 8);
    let r = Ring::default();
    assert!(!r.can_move(Arrow::Up, size, view));
    assert!(!r.can_move(Arrow::Left, size, view));
    let r = r.moved(Arrow::Right, false, size, view);
    assert_eq!(r, Ring { track: 1, scene: 0 });
    let r = r.moved(Arrow::Right, true, size, view);
    // A page would pass the last track: the ring stops where it shows the last eight.
    assert_eq!(r.track, 4);
    let r = r.moved(Arrow::Down, true, size, view);
    assert_eq!(r.scene, 8);
    let r = r.moved(Arrow::Down, true, size, view);
    assert_eq!(r.scene, 12);
    assert!(!r.can_move(Arrow::Down, size, view));
    // A session smaller than the grid: the ring stays at its corner.
    assert_eq!(
        Ring::default().moved(Arrow::Right, false, (3, 2), view),
        Ring::default()
    );
    let lights = arrow_lights(
        Ring {
            track: 4,
            scene: 12,
        },
        size,
        view,
        Rgb::WHITE,
    );
    assert_eq!(lights[0], Light::On(Rgb::WHITE));
    assert_eq!(lights[1], Light::Off);
    assert_eq!(lights[2], Light::On(Rgb::WHITE));
    assert_eq!(lights[3], Light::Off);
}
