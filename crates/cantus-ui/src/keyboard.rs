//! Computer keyboard as a MIDI controller: the classic two-row layout
//! (`A W S E D F T G Y H U J K O L P ; '`) with `Z`/`X` shifting the octave.

use egui::Key;

const MAP: [(Key, u8); 18] = [
    (Key::A, 0),
    (Key::W, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
    (Key::P, 15),
    (Key::Semicolon, 16),
    (Key::Quote, 17),
];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    On(u8),
    Off(u8),
}

#[derive(Debug)]
pub struct KeyboardPiano {
    /// Octave of the `A` key (4 = middle C).
    pub octave: i32,
    /// Keys currently held → pitch they triggered (pitch is frozen at press time so an
    /// octave change never leaves a note hanging).
    held: Vec<(Key, u8)>,
    pub velocity: u8,
}

impl Default for KeyboardPiano {
    fn default() -> Self {
        KeyboardPiano {
            octave: 4,
            held: Vec::with_capacity(18),
            velocity: 100,
        }
    }
}

impl KeyboardPiano {
    /// Translate this frame's key presses/releases into note events. Call once per frame,
    /// only when no text field wants the keyboard.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<KeyEvent> {
        let mut out = Vec::new();
        ctx.input(|i| {
            if i.modifiers.any() {
                // Chords with modifiers are shortcuts, not notes — but always release.
                self.release_all_missing(i, &mut out);
                return;
            }
            if i.key_pressed(Key::Z) {
                self.octave = (self.octave - 1).max(-1);
            }
            if i.key_pressed(Key::X) {
                self.octave = (self.octave + 1).min(8);
            }
            for (key, semis) in MAP {
                if i.key_pressed(key) && !self.held.iter().any(|(k, _)| *k == key) {
                    let pitch = ((self.octave + 1) * 12 + semis as i32).clamp(0, 127) as u8;
                    self.held.push((key, pitch));
                    out.push(KeyEvent::On(pitch));
                }
            }
            self.release_all_missing(i, &mut out);
        });
        out
    }

    fn release_all_missing(&mut self, i: &egui::InputState, out: &mut Vec<KeyEvent>) {
        self.held.retain(|(key, pitch)| {
            if i.key_down(*key) {
                true
            } else {
                out.push(KeyEvent::Off(*pitch));
                false
            }
        });
    }

    /// How many keys are sounding a note now.
    pub fn held_count(&self) -> usize {
        self.held.len()
    }

    /// Release everything (focus lost, track changed).
    pub fn release_all(&mut self) -> Vec<KeyEvent> {
        self.held.drain(..).map(|(_, p)| KeyEvent::Off(p)).collect()
    }
}
