//! Mackie Control and HUI (round 36, ADR 0095): the protocols byte for byte, and the whole
//! surface through the virtual units — banks, motor faders and their touch, V-pots, LEDs,
//! scribble strips, meters, time display, transport, jog, flip, extenders — with no hardware.

use cantus_core::external_emulation::{VirtualHui, VirtualMcu};
use cantus_core::external_hui as hui;
use cantus_core::external_mcu as mcu;
use cantus_core::external_surface::*;

fn view(n: usize) -> SurfaceView {
    SurfaceView {
        strips: (0..n)
            .map(|i| StripView {
                name: format!("Trk{}", i + 1),
                gain_db: 0.0,
                ..Default::default()
            })
            .collect(),
        master: StripView {
            name: "Master".into(),
            ..Default::default()
        },
        transport: TransportView {
            time: "001.01.01.000".into(),
            beats: true,
            ..Default::default()
        },
    }
}

// ---- Mackie Control: the bytes -----------------------------------------------------------

#[test]
fn every_host_message_of_the_mackie_protocol_has_its_bytes() {
    assert_eq!(mcu::fader(2, 0x2000), [0xE2, 0x00, 0x40]);
    assert_eq!(mcu::fader(8, 0x3FFF), [0xE8, 0x7F, 0x7F]);
    assert_eq!(mcu::led(0x5E, 0x7F), [0x90, 0x5E, 0x7F]);
    assert_eq!(mcu::led(0x00, 0x01), [0x90, 0x00, 0x01]);
    assert_eq!(mcu::ring(0, 3, 6, false), [0xB0, 0x30, 0x36]);
    assert_eq!(mcu::ring(7, 0, 6, true), [0xB0, 0x37, 0x46]);
    assert_eq!(mcu::meter(3, 9), [0xD0, 0x39]);
    assert_eq!(mcu::meter(7, 0xF), [0xD0, 0x7F]);
    assert_eq!(
        mcu::lcd(Kind::Mcu, 0, b"Hello"),
        [0xF0, 0x00, 0x00, 0x66, 0x14, 0x12, 0x00, 0x48, 0x65, 0x6C, 0x6C, 0x6F, 0xF7]
    );
    assert_eq!(
        mcu::lcd(Kind::McuExtender(0), 0x38, b"A"),
        [0xF0, 0x00, 0x00, 0x66, 0x15, 0x12, 0x38, 0x41, 0xF7]
    );
    // TouchMCU: timecode digits on CC 64-73 (0x40 the rightmost), the assignment on 74-75;
    // bit 6 the dot.
    assert_eq!(mcu::time_digit(0, b'1', false), [0xB0, 0x40, 0x31]);
    assert_eq!(mcu::time_digit(9, b'9', true), [0xB0, 0x49, 0x79]);
    assert_eq!(mcu::assignment_digit(0, b'2', false), [0xB0, 0x4A, 0x32]);
    assert_eq!(mcu::assignment_digit(1, b'0', false), [0xB0, 0x4B, 0x30]);
    assert_eq!(
        mcu::device_query(Kind::Mcu),
        [0xF0, 0x00, 0x00, 0x66, 0x14, 0x00, 0xF7]
    );
}

#[test]
fn every_unit_message_of_the_mackie_protocol_is_read() {
    use SurfaceEvent::*;
    assert_eq!(
        mcu::decode(&[0xE3, 0x7F, 0x7F]),
        Some(Fader {
            channel: 3,
            pos: 1.0
        })
    );
    assert_eq!(
        mcu::decode(&[0xE0, 0, 0]),
        Some(Fader {
            channel: 0,
            pos: 0.0
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x5E, 0x7F]),
        Some(Button {
            button: self::Button::Play,
            down: true
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x5E, 0x00]),
        Some(Button {
            button: self::Button::Play,
            down: false
        })
    );
    assert_eq!(
        mcu::decode(&[0x80, 0x5D, 0x40]),
        Some(Button {
            button: self::Button::Stop,
            down: false
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x08, 0x7F]),
        Some(Button {
            button: self::Button::Solo(0),
            down: true
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x17, 0x7F]),
        Some(Button {
            button: self::Button::Mute(7),
            down: true
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x68, 0x7F]),
        Some(Touch {
            channel: 0,
            on: true
        })
    );
    assert_eq!(
        mcu::decode(&[0x90, 0x70, 0x00]),
        Some(Touch {
            channel: 8,
            on: false
        })
    );
    assert_eq!(
        mcu::decode(&[0xB0, 0x10, 0x41]),
        Some(VPot {
            channel: 0,
            delta: -1
        })
    );
    assert_eq!(
        mcu::decode(&[0xB0, 0x17, 0x05]),
        Some(VPot {
            channel: 7,
            delta: 5
        })
    );
    assert_eq!(mcu::decode(&[0xB0, 0x3C, 0x03]), Some(Jog { delta: 3 }));
    assert_eq!(mcu::decode(&[0xB0, 0x3C, 0x42]), Some(Jog { delta: -2 }));
    assert_eq!(mcu::decode(&[0xB0, 0x05, 0x05]), None);
    assert_eq!(mcu::decode(&[]), None);
    // Every note maps to a button and back.
    for n in 0..=0x65u8 {
        let b = mcu::button_of(n);
        assert_eq!(mcu::note_of(b), Some(n), "note {n:#x}");
    }
}

#[test]
fn the_connection_handshake_is_answered_and_the_unit_accepts_it() {
    let mut set = SurfaceSet::new(&[Kind::Mcu]);
    let v = view(8);
    let mut unit = VirtualMcu::new(false);
    for (_, m) in set.output(&v) {
        for q in unit.receive(&m) {
            let reply = set.input(0, &q, &v).reply;
            assert_eq!(reply.len(), 1, "a reply to the connection query");
            for r in reply {
                unit.receive(&r);
            }
        }
    }
    assert!(unit.verified, "the unit accepted the host's response");
}

// ---- Mackie Control: the whole surface through the virtual unit ---------------------------

struct Rig {
    set: SurfaceSet,
    unit: VirtualMcu,
    view: SurfaceView,
}

impl Rig {
    fn new(n: usize) -> Rig {
        let mut r = Rig {
            set: SurfaceSet::new(&[Kind::Mcu]),
            unit: VirtualMcu::new(false),
            view: view(n),
        };
        r.sync();
        r
    }

    /// The host renders; the unit receives it (and the replies go back).
    fn sync(&mut self) {
        for (_, m) in self.set.output(&self.view) {
            for q in self.unit.receive(&m) {
                let _ = self.set.input(0, &q, &self.view);
            }
        }
    }

    fn hand(&mut self, msgs: Vec<Vec<u8>>) -> Vec<SurfaceAction> {
        msgs.iter()
            .flat_map(|m| self.set.input(0, m, &self.view).actions)
            .collect()
    }
}

#[test]
fn the_unit_shows_the_mixer_names_values_leds_rings_and_meters() {
    let mut r = Rig::new(10);
    r.view.strips[1].name = "Kick".into();
    r.view.strips[1].gain_db = -6.0;
    r.view.strips[1].mute = true;
    r.view.strips[2].solo = true;
    r.view.strips[3].arm = true;
    r.view.strips[4].selected = true;
    r.view.strips[5].pan = -1.0;
    r.view.strips[6].meter = 1.0;
    r.view.strips[7].meter = 0.001;
    r.view.transport.playing = true;
    r.view.transport.looping = true;
    r.sync();
    let u = &r.unit;
    assert_eq!(u.scribble(0, 1), "Kick   ");
    assert_eq!(u.scribble(1, 1), "-6.0dB ");
    assert_eq!(u.scribble(0, 0), "Trk1   ");
    assert_eq!(u.leds[0x10 + 1], 0x7F, "mute");
    assert_eq!(u.leds[0x08 + 2], 0x7F, "solo");
    assert_eq!(u.leds[3], 0x7F, "rec");
    assert_eq!(u.leds[0x18 + 4], 0x7F, "select");
    assert_eq!(u.leds[0x10], 0, "an unmuted strip's LED is off");
    assert_eq!(u.leds[0x5E], 0x7F, "play");
    assert_eq!(u.leds[0x5D], 0, "stop is off while playing");
    assert_eq!(u.leds[0x56], 0x7F, "cycle");
    assert_eq!(u.rings[5], 1, "hard left: the first LED");
    assert_eq!(
        u.rings[0],
        6 | 0x40,
        "centred: the middle LED and the centre light"
    );
    assert_eq!(u.meters[6], 12);
    assert_eq!(u.meters[7], 0, "a peak under -60 dB is dark");
    assert_eq!(u.time_text().trim(), "001.01.01.000");
    assert_eq!(u.assignment(), "01");
}

#[test]
fn faders_are_motorised_in_the_console_scale_and_never_fight_a_hand() {
    let mut r = Rig::new(8);
    // 0 dB sits at 80 % of the travel, the mixer's own scale.
    assert_eq!(r.unit.faders[0], to14(0.8));
    r.view.strips[0].gain_db = -24.0;
    r.sync();
    assert_eq!(r.unit.faders[0], to14(0.4));
    // A hand moves it: the host hears a gain; its echo is not sent back.
    let moved = r.unit.move_fader(2, to14(0.8), false);
    let a = r.hand(moved);
    assert!(a.contains(&SurfaceAction::Touch {
        target: Target::Strip(2),
        on: true
    }));
    assert!(a.contains(&SurfaceAction::Gain {
        target: Target::Strip(2),
        db: 0.0
    }));
    // The mixer takes the move; while touched, a host change does not move the motor.
    r.view.strips[2].gain_db = -10.0;
    r.sync();
    assert_eq!(
        r.unit.faders[2],
        to14(0.8),
        "the hand wins while it touches"
    );
    // On release the fader lands where the mixer is.
    let rel = r.unit.release(2);
    let a = r.hand(rel);
    assert!(a.contains(&SurfaceAction::Touch {
        target: Target::Strip(2),
        on: false
    }));
    r.sync();
    assert_eq!(r.unit.faders[2], fader_value(-10.0));
    // The master fader.
    let m = r.unit.move_fader(8, to14(1.0), true);
    let a = r.hand(m);
    assert!(a.contains(&SurfaceAction::Gain {
        target: Target::Master,
        db: 12.0
    }));
    // The bottom of the travel is minus infinity.
    let m = r.unit.move_fader(0, 0, true);
    let a = r.hand(m);
    assert!(a.contains(&SurfaceAction::Gain {
        target: Target::Strip(0),
        db: -96.0
    }));
}

#[test]
fn the_gain_a_fader_sets_is_the_gain_the_motor_returns_to() {
    // A round trip through the 14 bits never drifts: set a gain, send it, read it back.
    for tenth in -600..=120 {
        let db = tenth as f32 / 10.0;
        let pos = from14(fader_value(db));
        let back = snap_db(cantus_core::automation::range::fader_db_at(pos));
        assert!((back - db).abs() <= 0.11, "{db}: {back}");
    }
}

#[test]
fn buttons_toggle_the_mixer_and_light_their_leds() {
    let mut r = Rig::new(8);
    for (note, want) in [
        (
            0x10 + 3,
            SurfaceAction::Mute {
                target: Target::Strip(3),
                on: true,
            },
        ),
        (
            0x08 + 4,
            SurfaceAction::Solo {
                target: Target::Strip(4),
                on: true,
            },
        ),
        (
            5,
            SurfaceAction::Arm {
                target: Target::Strip(5),
                on: true,
            },
        ),
        (0x18 + 6, SurfaceAction::Select(Target::Strip(6))),
    ] {
        let p = r.unit.press(note, true);
        assert_eq!(r.hand(p), vec![want], "note {note:#x}");
    }
    // A pressed-and-held mute that is on turns off.
    r.view.strips[3].mute = true;
    let p = r.unit.press(0x10 + 3, true);
    assert_eq!(
        r.hand(p),
        vec![SurfaceAction::Mute {
            target: Target::Strip(3),
            on: false
        }]
    );
    // The release alone asks for nothing.
    assert!(r.hand(vec![vec![0x90, 0x10, 0x00]]).is_empty());
}

#[test]
fn the_vpot_pans_and_pushes_to_centre_and_shift_makes_it_finer() {
    let mut r = Rig::new(8);
    let t = r.unit.turn(1, 5);
    let a = r.hand(vec![t]);
    assert_eq!(
        a,
        vec![SurfaceAction::Pan {
            target: Target::Strip(1),
            pan: 5.0 * 0.04
        }]
    );
    r.view.strips[1].pan = 0.5;
    let t = r.unit.turn(1, -3);
    let a = r.hand(vec![t]);
    assert!(matches!(a[0], SurfaceAction::Pan { pan, .. } if (pan - 0.38).abs() < 1e-6));
    // Shift (held) divides the step by ten.
    r.hand(r.unit.press(0x46, false));
    let t = r.unit.turn(1, 10);
    let a = r.hand(vec![t]);
    assert!(matches!(a[0], SurfaceAction::Pan { pan, .. } if (pan - 0.54).abs() < 1e-6));
    r.hand(vec![vec![0x90, 0x46, 0x00]]);
    // Push: centre. The ring follows the mixer.
    let p = r.unit.press(0x20 + 1, true);
    assert_eq!(
        r.hand(p),
        vec![SurfaceAction::Pan {
            target: Target::Strip(1),
            pan: 0.0
        }]
    );
    r.view.strips[1].pan = 1.0;
    r.sync();
    assert_eq!(r.unit.rings[1], 11);
    // Never past the ends.
    let t = r.unit.turn(1, 63);
    let a = r.hand(vec![t]);
    assert!(matches!(a[0], SurfaceAction::Pan { pan, .. } if pan == 1.0));
}

#[test]
fn banks_and_channel_shifts_move_the_strips_and_stop_at_the_ends() {
    let mut r = Rig::new(20);
    assert_eq!(r.unit.scribble(0, 0), "Trk1   ");
    r.hand(r.unit.press(0x2F, true)); // bank right: 8 more
    r.sync();
    assert_eq!(r.set.bank, 8);
    assert_eq!(r.unit.scribble(0, 0), "Trk9   ");
    assert_eq!(r.unit.assignment(), "02");
    r.hand(r.unit.press(0x2F, true));
    r.sync();
    assert_eq!(
        r.set.bank, 12,
        "stops where the last strip is on the last channel"
    );
    assert_eq!(r.unit.scribble(0, 7), "Trk20  ");
    r.hand(r.unit.press(0x30, true)); // channel left: one
    r.sync();
    assert_eq!(r.set.bank, 11);
    assert_eq!(r.unit.scribble(0, 0), "Trk12  ");
    for _ in 0..3 {
        r.hand(r.unit.press(0x2E, true));
    }
    assert_eq!(r.set.bank, 0);
    // A fader on the second page edits the strip that is shown there.
    r.hand(r.unit.press(0x2F, true));
    r.sync();
    let m = r.unit.move_fader(1, to14(0.8), true);
    let a = r.hand(m);
    assert!(a.contains(&SurfaceAction::Gain {
        target: Target::Strip(9),
        db: 0.0
    }));
    // Fewer strips than channels: the empty ones are dark and ask for nothing.
    let mut r = Rig::new(3);
    assert_eq!(r.unit.scribble(0, 5), "       ");
    let p = r.unit.press(0x10 + 6, true);
    assert!(r.hand(p).is_empty());
}

#[test]
fn transport_jog_scrub_markers_and_undo() {
    let mut r = Rig::new(8);
    for (note, want) in [
        (0x5E, vec![SurfaceAction::Play]),
        (0x5D, vec![SurfaceAction::Stop]),
        (0x5F, vec![SurfaceAction::Record]),
        (0x56, vec![SurfaceAction::Cycle]),
        (0x59, vec![SurfaceAction::Click]),
        (0x51, vec![SurfaceAction::Undo]),
        (0x50, vec![SurfaceAction::Save]),
        (0x54, vec![SurfaceAction::Marker]),
        (0x62, vec![SurfaceAction::PreviousMarker]),
        (0x63, vec![SurfaceAction::NextMarker]),
        (0x65, vec![SurfaceAction::Scrub(true)]),
    ] {
        let p = r.unit.press(note, true);
        assert_eq!(r.hand(p), want, "note {note:#x}");
    }
    // Rewind and forward are held.
    let p = r.unit.press(0x5B, true);
    assert_eq!(
        r.hand(p),
        vec![SurfaceAction::Rewind(true), SurfaceAction::Rewind(false)]
    );
    // The jog wheel.
    let j = r.unit.jog(-4);
    assert_eq!(r.hand(vec![j]), vec![SurfaceAction::Jog(-4)]);
    // Shift + Undo is redo.
    r.hand(r.unit.press(0x46, false));
    let p = r.unit.press(0x51, true);
    assert_eq!(r.hand(p), vec![SurfaceAction::Redo]);
}

#[test]
fn flip_swaps_the_fader_and_the_pan_and_the_name_value_button_swaps_the_second_line() {
    let mut r = Rig::new(8);
    r.view.strips[0].pan = -1.0;
    r.view.strips[0].gain_db = -6.0;
    r.sync();
    assert_eq!(r.unit.scribble(1, 0), "-6.0dB ");
    r.hand(r.unit.press(0x34, true));
    r.sync();
    assert_eq!(r.unit.scribble(1, 0), "  L100 ");
    assert_eq!(r.unit.leds[0x34], 0x7F);
    r.hand(r.unit.press(0x32, true)); // flip
    r.sync();
    assert_eq!(
        r.unit.faders[0], 0,
        "the fader now shows the pan (hard left is the bottom)"
    );
    let m = r.unit.move_fader(0, to14(0.75), true);
    let a = r.hand(m);
    assert!(a.contains(&SurfaceAction::Pan {
        target: Target::Strip(0),
        pan: 0.5
    }));
    let t = r.unit.turn(0, 2);
    assert!(matches!(
        r.hand(vec![t])[0],
        SurfaceAction::Gain { db, .. } if (db + 5.0).abs() < 1e-6
    ));
    assert_eq!(r.unit.leds[0x32], 0x7F);
}

#[test]
fn an_extender_continues_the_strips_and_follows_the_bank() {
    let mut set = SurfaceSet::new(&[Kind::Mcu, Kind::McuExtender(0)]);
    let v = view(24);
    let mut main = VirtualMcu::new(false);
    let mut xt = VirtualMcu::new(true);
    let feed =
        |set: &mut SurfaceSet, main: &mut VirtualMcu, xt: &mut VirtualMcu, v: &SurfaceView| {
            for (dev, m) in set.output(v) {
                if dev == 0 {
                    main.receive(&m);
                } else {
                    xt.receive(&m);
                }
            }
        };
    feed(&mut set, &mut main, &mut xt, &v);
    assert_eq!(main.scribble(0, 0), "Trk1   ");
    assert_eq!(xt.scribble(0, 0), "Trk9   ");
    assert_eq!(xt.scribble(0, 7), "Trk16  ");
    // The extender has no master fader and no transport; its play does nothing.
    assert!(set.input(1, &[0x90, 0x5E, 0x7F], &v).actions.is_empty());
    // Its fader edits its own strips.
    let m = xt.move_fader(3, to14(0.8), true);
    let acts: Vec<_> = m.iter().flat_map(|b| set.input(1, b, &v).actions).collect();
    assert!(acts.contains(&SurfaceAction::Gain {
        target: Target::Strip(11),
        db: 0.0
    }));
    // Bank right moves both units by sixteen strips, to where the last strip is on the last
    // channel (24 strips, sixteen shown: the bank stops at 8).
    set.input(0, &[0x90, 0x2F, 0x7F], &v);
    assert_eq!(set.bank, 8);
    feed(&mut set, &mut main, &mut xt, &v);
    assert_eq!(main.scribble(0, 0), "Trk9   ");
    assert_eq!(xt.scribble(0, 7), "Trk24  ");
}

#[test]
fn only_what_changed_is_sent() {
    let mut set = SurfaceSet::new(&[Kind::Mcu]);
    let mut v = view(8);
    assert!(!set.output(&v).is_empty());
    assert!(set.output(&v).is_empty(), "nothing changed: nothing sent");
    v.strips[2].mute = true;
    let out = set.output(&v);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].1, mcu::led(0x10 + 2, 0x7F));
    v.strips[3].gain_db = -3.0;
    let out = set.output(&v);
    // The fader and the scribble line that shows its value.
    assert_eq!(out.len(), 2, "{out:?}");
    set.refresh();
    assert!(
        set.output(&v).len() > 40,
        "a refresh sends everything again"
    );
}

// ---- HUI ----------------------------------------------------------------------------------

#[test]
fn every_host_message_of_hui_has_its_bytes() {
    assert_eq!(hui::ping(), [0x90, 0x00, 0x00]);
    assert_eq!(
        hui::fader(1, 0x2001),
        [vec![0xB0, 0x01, 0x40], vec![0xB0, 0x21, 0x01]]
    );
    assert_eq!(
        hui::led(3, 2, true),
        [vec![0xB0, 0x0C, 0x03], vec![0xB0, 0x2C, 0x42]]
    );
    assert_eq!(
        hui::led(0x0E, 3, false),
        [vec![0xB0, 0x0C, 0x0E], vec![0xB0, 0x2C, 0x03]]
    );
    assert_eq!(hui::ring(2, 11, true), [0xB0, 0x12, 0x4B]);
    assert_eq!(
        hui::name(3, "Kick!"),
        [0xF0, 0x00, 0x00, 0x66, 0x05, 0x00, 0x10, 0x03, b'K', b'i', b'c', b'k', 0xF7]
    );
}

/// The bytes of theageman's *MACKIE HUI MIDI protocol* (2010, SSEI;
/// https://forum.bome.com/uploads/short-url/aX7AWqj6FpgMiEMQi0kACDDTSSi.pdf), page by page.
#[test]
fn every_unit_message_of_hui_is_read_as_its_published_notes_print_it() {
    use SurfaceEvent::*;
    let mut d = hui::Decoder::default();
    let mut feed = |msgs: &[&[u8]]| -> Vec<SurfaceEvent> {
        msgs.iter().flat_map(|m| d.feed(m).events).collect()
    };
    let b = |button, down| Button { button, down };
    // p. 9: "when the user presses the solo button on the fifth channelstrip, the HUI will
    // send: b0 0f 04 … b0 2f 43 … After the user releases the button … b0 0f 04, b0 2f 03".
    assert_eq!(
        feed(&[&[0xB0, 0x0F, 0x04], &[0xB0, 0x2F, 0x43]]),
        vec![b(self::Button::Solo(4), true)]
    );
    assert_eq!(
        feed(&[&[0xB0, 0x0F, 0x04], &[0xB0, 0x2F, 0x03]]),
        vec![b(self::Button::Solo(4), false)]
    );
    // p. 14, zones 00-07: "fader select mute solo auto v-sel insert rec/rdy".
    for (port, want) in [
        (1u8, self::Button::Select(2)),
        (2, self::Button::Mute(2)),
        (3, self::Button::Solo(2)),
        (5, self::Button::VPush(2)),
        (7, self::Button::Rec(2)),
    ] {
        assert_eq!(
            feed(&[&[0xB0, 0x0F, 0x02], &[0xB0, 0x2F, 0x40 | port]]),
            vec![b(want, true)],
            "port {port}"
        );
    }
    // p. 11: "touch fader: b0 0f 0z, b0 2f 40; release fader: b0 0f 0z, b0 2f 00".
    assert_eq!(
        feed(&[&[0xB0, 0x0F, 0x03], &[0xB0, 0x2F, 0x40]]),
        vec![Touch {
            channel: 3,
            on: true
        }]
    );
    assert_eq!(
        feed(&[&[0xB0, 0x0F, 0x03], &[0xB0, 0x2F, 0x00]]),
        vec![Touch {
            channel: 3,
            on: false
        }]
    );
    // p. 14, zone 0e: "talkback rewind fast fwd stop play record".
    for (port, want) in [
        (1u8, self::Button::Rewind),
        (2, self::Button::Forward),
        (3, self::Button::Stop),
        (4, self::Button::Play),
        (5, self::Button::Record),
    ] {
        assert_eq!(
            feed(&[&[0xB0, 0x0F, 0x0E], &[0xB0, 0x2F, 0x40 | port]]),
            vec![b(want, true)],
            "transport port {port}"
        );
    }
    // Zone 0a "<- chanl, <- bank, chanl ->, bank ->" (p. 14; p. 7: next bank is "zone 0a,
    // port 3"), zone 0f "|<rtz end>| on line loop", zone 08 "… undo … save", zone 0d
    // "down left mode right up scrub".
    for (zone, port, want) in [
        (0x0Au8, 0u8, self::Button::ChannelLeft),
        (0x0A, 1, self::Button::BankLeft),
        (0x0A, 2, self::Button::ChannelRight),
        (0x0A, 3, self::Button::BankRight),
        (0x0F, 0, self::Button::ReturnToZero),
        (0x0F, 3, self::Button::Cycle),
        (0x08, 1, self::Button::Shift),
        (0x08, 3, self::Button::Undo),
        (0x08, 7, self::Button::Save),
        (0x0D, 1, self::Button::Left),
        (0x0D, 3, self::Button::Right),
        (0x0D, 5, self::Button::Scrub),
    ] {
        assert_eq!(
            feed(&[&[0xB0, 0x0F, zone], &[0xB0, 0x2F, 0x40 | port]]),
            vec![b(want, true)],
            "zone {zone:#x} port {port}"
        );
    }
    // p. 11: "move fader: b0 0z hi, b0 2z lo … value = (hi << 7) + lo".
    assert_eq!(
        feed(&[&[0xB0, 0x04, 0x40], &[0xB0, 0x24, 0x00]]),
        vec![Fader {
            channel: 4,
            pos: from14(0x2000)
        }]
    );
    // p. 10: "b0 4p vv … If vv>40 then delta=vv-40. If vv<40 then delta=-vv."
    assert_eq!(
        feed(&[&[0xB0, 0x42, 0x43]]),
        vec![VPot {
            channel: 2,
            delta: 3
        }]
    );
    assert_eq!(
        feed(&[&[0xB0, 0x42, 0x03]]),
        vec![VPot {
            channel: 2,
            delta: -3
        }]
    );
    // p. 11: the jog wheel, "b0 0d vv", the same encoding.
    assert_eq!(feed(&[&[0xB0, 0x0D, 0x45]]), vec![Jog { delta: 5 }]);
    assert_eq!(feed(&[&[0xB0, 0x0D, 0x05]]), vec![Jog { delta: -5 }]);
    // A port nothing is mapped to is logged, not guessed at (zone 09, the window keys).
    let r = d.feed(&[0xB0, 0x0F, 0x09]);
    assert!(r.events.is_empty());
    let r = d.feed(&[0xB0, 0x2F, 0x41]);
    assert_eq!(r.unmapped, vec![(9, 1)]);
    // p. 4: "The HUI will respond to a ping with a ping-reply: 90 00 7f".
    assert!(d.feed(&[0x90, 0x00, 0x7F]).pong);
    assert!(!d.feed(&[0x90, 0x00, 0x00]).pong);
}

/// p. 4: the host pings "every second or so"; without a reply the unit is said lost, and it
/// is refreshed when it answers again.
#[test]
fn a_hui_is_pinged_every_second_and_its_loss_is_said() {
    let mut set = SurfaceSet::new(&[Kind::Hui]);
    let v = view(8);
    let pings =
        |out: &[(usize, Vec<u8>)]| out.iter().filter(|(_, m)| m == &vec![0x90u8, 0, 0]).count();
    assert_eq!(pings(&set.output_at(&v, 0)), 1, "a ping at once");
    assert_eq!(pings(&set.output_at(&v, 500)), 0);
    assert_eq!(pings(&set.output_at(&v, 1_000)), 1, "and every second");
    set.input(0, &[0x90, 0x00, 0x7F], &v);
    assert!(set.devices[0].alive);
    let _ = set.output_at(&v, 2_000);
    let _ = set.output_at(&v, 3_500);
    assert!(
        set.devices[0].alive,
        "answered at 1 000: not lost yet at 3 500"
    );
    let _ = set.output_at(&v, 4_100);
    assert!(!set.devices[0].alive, "no answer for three seconds: lost");
    assert_eq!(set.devices[0].lost, 1);
    // It comes back: everything is sent again.
    set.now_ms = 5_000;
    set.input(0, &[0x90, 0x00, 0x7F], &v);
    assert!(set.devices[0].alive);
    assert!(
        set.output_at(&v, 5_000).len() > 20,
        "a unit back online is sent everything"
    );
}

/// TouchMCU (https://github.com/EMATech/TouchMCU/blob/main/doc/mackie_control_protocol.md):
/// the meters decay by themselves (~300 ms a division), so a steady level is sent again.
#[test]
fn a_steady_mackie_meter_is_sent_again_before_it_decays() {
    let mut set = SurfaceSet::new(&[Kind::Mcu]);
    let mut v = view(8);
    v.strips[0].meter = 0.5;
    let _ = set.output_at(&v, 0);
    let meters =
        |out: &[(usize, Vec<u8>)]| out.iter().filter(|(_, m)| m.first() == Some(&0xD0)).count();
    assert_eq!(
        meters(&set.output_at(&v, 50)),
        0,
        "unchanged and fresh: nothing"
    );
    assert_eq!(
        meters(&set.output_at(&v, 160)),
        1,
        "the lit meter again, not the dark ones"
    );
    v.strips[0].meter = 0.0;
    let _ = set.output_at(&v, 200);
    assert_eq!(
        meters(&set.output_at(&v, 400)),
        0,
        "a dark meter is not re-sent"
    );
}

/// A controller without touch-sensing faders: after it moves a fader, the motor holds.
#[test]
fn faders_without_touch_sensing_are_held_after_a_move() {
    let mut set = SurfaceSet::new(&[Kind::Mcu]);
    set.devices[0].touch_sensing = false;
    let mut v = view(8);
    let _ = set.output_at(&v, 0);
    set.now_ms = 1_000;
    set.input(0, &mcu::fader(0, to14(0.5)), &v);
    v.strips[0].gain_db = -3.0;
    let faders =
        |out: &[(usize, Vec<u8>)]| out.iter().filter(|(_, m)| m.first() == Some(&0xE0)).count();
    assert_eq!(faders(&set.output_at(&v, 1_100)), 0, "held");
    assert_eq!(
        faders(&set.output_at(&v, 1_700)),
        1,
        "then it follows the mixer"
    );
}

/// TouchMCU's *Host Connection Query* as a unit sends it (`F0 00 00 66 14 01`, a 7-byte ASCII
/// serial, a 4-byte challenge, `F7`) is answered with the *Host Connection Reply* (`02`, the
/// serial, the response by the document's formula).
#[test]
fn a_mackie_host_connection_query_is_answered_by_the_documented_formula() {
    let mut set = SurfaceSet::new(&[Kind::Mcu]);
    let v = view(8);
    let serial = *b"MCU0001";
    let c = [0x12u8, 0x34, 0x56, 0x78];
    let mut query = vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x01];
    query.extend_from_slice(&serial);
    query.extend_from_slice(&c);
    query.push(0xF7);
    // The document's pseudocode, verbatim in Rust.
    let c = c.map(i32::from);
    let r = [
        0x7F & (c[0] + (c[1] ^ 0x0A) - c[3]),
        0x7F & ((c[2] >> 4) ^ (c[0] + c[3])),
        0x7F & ((c[3] - (c[2] << 2)) ^ (c[0] | c[1])),
        0x7F & (c[1] - c[2] + (0xF0 ^ (c[3] << 4))),
    ];
    let mut want = vec![0xF0, 0x00, 0x00, 0x66, 0x14, 0x02];
    want.extend_from_slice(&serial);
    want.extend(r.iter().map(|x| *x as u8));
    want.push(0xF7);
    assert_eq!(set.input(0, &query, &v).reply, vec![want]);
}

struct HuiRig {
    set: SurfaceSet,
    unit: VirtualHui,
    view: SurfaceView,
}

impl HuiRig {
    fn new(n: usize) -> HuiRig {
        let mut r = HuiRig {
            set: SurfaceSet::new(&[Kind::Hui]),
            unit: VirtualHui::new(),
            view: view(n),
        };
        r.sync();
        r
    }

    fn sync(&mut self) {
        for (_, m) in self.set.output(&self.view) {
            self.unit.receive(&m);
        }
    }

    fn hand(&mut self, msgs: Vec<Vec<u8>>) -> Vec<SurfaceAction> {
        msgs.iter()
            .flat_map(|m| self.set.input(0, m, &self.view).actions)
            .collect()
    }
}

#[test]
fn a_hui_shows_the_mixer_and_answers_the_mixers_hand() {
    let mut r = HuiRig::new(10);
    assert!(r.unit.pinged, "the host pings on connect");
    assert_eq!(r.unit.names[0], "Trk1");
    assert_eq!(r.unit.faders[0], to14(0.8));
    r.view.strips[1].mute = true;
    r.view.strips[2].solo = true;
    r.view.strips[3].arm = true;
    r.view.strips[4].selected = true;
    r.view.strips[5].gain_db = -24.0;
    r.view.transport.playing = true;
    r.sync();
    assert!(r.unit.led(1, hui::port::MUTE));
    assert!(r.unit.led(2, hui::port::SOLO));
    assert!(r.unit.led(3, hui::port::REC));
    assert!(r.unit.led(4, hui::port::SELECT));
    assert!(!r.unit.led(0, hui::port::MUTE));
    assert_eq!(r.unit.faders[5], to14(0.4));
    assert!(r.unit.led(hui::TRANSPORT_ZONE, hui::transport::PLAY));
    assert!(!r.unit.led(hui::TRANSPORT_ZONE, hui::transport::STOP));
    // A hand: mute, a fader with its touch, a V-pot, play, the jog wheel.
    let p = r.unit.press(3, hui::port::MUTE, true);
    assert_eq!(
        r.hand(p),
        vec![SurfaceAction::Mute {
            target: Target::Strip(3),
            on: true
        },]
    );
    let m = r.unit.move_fader(2, to14(0.8), true);
    let a = r.hand(m);
    assert!(a.contains(&SurfaceAction::Gain {
        target: Target::Strip(2),
        db: 0.0
    }));
    assert!(a.contains(&SurfaceAction::Touch {
        target: Target::Strip(2),
        on: true
    }));
    let t = r.unit.turn(1, 4);
    assert_eq!(
        r.hand(vec![t]),
        vec![SurfaceAction::Pan {
            target: Target::Strip(1),
            pan: 0.16
        }]
    );
    let p = r
        .unit
        .press(hui::TRANSPORT_ZONE, hui::transport::PLAY, true);
    assert_eq!(r.hand(p), vec![SurfaceAction::Play]);
    let j = r.unit.jog(-2);
    assert_eq!(r.hand(vec![j]), vec![SurfaceAction::Jog(-2)]);
}

#[test]
fn a_hui_does_not_fight_a_touched_fader_and_banks_with_its_strips() {
    let mut r = HuiRig::new(12);
    let m = r.unit.move_fader(0, to14(0.5), false);
    r.hand(m);
    r.view.strips[0].gain_db = -3.0;
    r.sync();
    assert_eq!(r.unit.faders[0], to14(0.5), "touched: the motor waits");
    let rel = r.unit.release(0);
    r.hand(rel);
    r.sync();
    assert_eq!(r.unit.faders[0], fader_value(-3.0));
    // The HUI has no bank buttons here: the bank is the host's (the agents and the screen).
    r.set.bank = 4;
    r.set.refresh();
    r.sync();
    assert_eq!(r.unit.names[0], "Trk5");
}

#[test]
fn the_kinds_have_ids_that_never_change() {
    for (k, id) in [
        (Kind::Mcu, "mcu"),
        (Kind::McuExtender(0), "mcu-xt1"),
        (Kind::McuExtender(2), "mcu-xt3"),
        (Kind::Hui, "hui"),
    ] {
        assert_eq!(k.id(), id);
        assert_eq!(Kind::from_id(id), Some(k));
    }
    assert_eq!(Kind::from_id("mcu-xt0"), None);
    assert_eq!(Kind::from_id("mcu-xt9"), None);
    assert_eq!(Kind::from_id("x"), None);
}
