//! Notes into the plug-ins that hear them (round 28): a CLAP note port for every kind that
//! takes notes (`EffectKind::takes_notes` — Tonus's targets, Porta's MIDI trigger), in the
//! CLAP and MIDI dialects, read from the host's event list at their sample.
//!
//! A release addresses what it names. In CLAP each of an off's key, note id and channel may
//! be −1, a wildcard of its own: an off with key −1 and a note id releases that note, with
//! key −1 and a channel every key on that channel, and only with all three −1 everything.
//! A MIDI all-notes-off (CC 120/123) is its channel's. [`Held`] remembers, from the notes
//! on, which keys each channel holds and which note id is which key.

use super::Instance;
use crate::instance::{push_note, NoteIn};
use cantus_engine::FxNote;
use clap_sys::events::*;
use clap_sys::ext::note_ports::*;
use clap_sys::plugin::clap_plugin;

pub(crate) static NOTE_PORTS: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(ports_count),
    get: Some(ports_get),
};

/// Whether this instance's kind hears notes (and so offers a note port).
pub(crate) unsafe fn hears_notes(plugin: *const clap_plugin) -> bool {
    Instance::from(plugin).is_some_and(|i| i.shared.desc.kind.takes_notes())
}

unsafe extern "C" fn ports_count(plugin: *const clap_plugin, is_input: bool) -> u32 {
    u32::from(is_input && hears_notes(plugin))
}

unsafe extern "C" fn ports_get(
    plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if info.is_null() || !is_input || index != 0 || !hears_notes(plugin) {
        return false;
    }
    let info = &mut *info;
    info.id = 0;
    info.supported_dialects = CLAP_NOTE_DIALECT_CLAP | CLAP_NOTE_DIALECT_MIDI;
    info.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    super::ext::copy_str(&mut info.name, &crate::text::t("export.bus.notes"));
    true
}

/// Note ids remembered at once; past them the oldest is forgotten (an off by that id then
/// finds nothing, which only a host holding more than this many notes could see).
const IDS: usize = 128;

/// What the host holds, as far as the notes it sent tell: the channels each key is held
/// on, and the key of each note id held. Fixed size: it lives on the audio thread.
#[derive(Clone, Debug)]
pub struct Held {
    /// Per key, a bit per MIDI channel holding it.
    channels: [u16; 128],
    /// (note id, key); an id of −1 is a free slot.
    ids: [(i32, u8); IDS],
    /// The slot taken next when every one is in use.
    next: usize,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            channels: [0; 128],
            ids: [(-1, 0); IDS],
            next: 0,
        }
    }
}

/// A channel's bit; a channel outside 0..16 (CLAP's −1 on a note on) is channel 0.
fn bit(channel: i16) -> u16 {
    1 << if (0..16).contains(&channel) {
        channel
    } else {
        0
    }
}

impl Held {
    /// Forget everything (the plug-in was activated or reset).
    pub fn clear(&mut self) {
        *self = Held::default();
    }

    fn on(
        &mut self,
        note_id: i32,
        channel: i16,
        key: u8,
        velocity: u8,
        emit: &mut impl FnMut(FxNote),
    ) {
        self.channels[key as usize] |= bit(channel);
        if note_id >= 0 {
            let same = self.ids.iter().position(|&(id, _)| id == note_id);
            let slot = match same.or_else(|| self.ids.iter().position(|&(id, _)| id < 0)) {
                Some(i) => i,
                None => {
                    let i = self.next;
                    self.next = (i + 1) % IDS;
                    i
                }
            };
            self.ids[slot] = (note_id, key);
        }
        emit(FxNote::On {
            pitch: key,
            velocity,
        });
    }

    /// `key` released on the channels in `mask`.
    fn release(&mut self, key: u8, mask: u16, emit: &mut impl FnMut(FxNote)) {
        let k = key as usize;
        self.channels[k] &= !mask;
        if self.channels[k] == 0 {
            for slot in self
                .ids
                .iter_mut()
                .filter(|(id, kk)| *id >= 0 && *kk == key)
            {
                *slot = (-1, 0);
            }
        }
        emit(FxNote::Off { pitch: key });
    }

    /// Every key held on the channels in `mask`; every channel's: everything.
    fn release_channels(&mut self, mask: u16, emit: &mut impl FnMut(FxNote)) {
        if mask == u16::MAX {
            self.clear();
            emit(FxNote::AllOff);
            return;
        }
        for key in 0..128u8 {
            if self.channels[key as usize] & mask != 0 {
                self.release(key, mask, emit);
            }
        }
    }

    /// A CLAP note on, off or choke, as the notes it means.
    pub fn clap(&mut self, type_: u16, e: &clap_event_note, mut emit: impl FnMut(FxNote)) {
        let key = (0..128).contains(&e.key).then_some(e.key as u8);
        let mask = if (0..16).contains(&e.channel) {
            bit(e.channel)
        } else {
            u16::MAX
        };
        match (type_, key) {
            (CLAP_EVENT_NOTE_ON, Some(k)) => {
                let velocity = (e.velocity * 127.0).round().clamp(1.0, 127.0) as u8;
                self.on(e.note_id, e.channel, k, velocity, &mut emit);
            }
            (CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE, Some(k)) => {
                self.release(k, mask, &mut emit);
            }
            (CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE, None) if e.key < 0 => {
                if e.note_id >= 0 {
                    // That note: the key its on had, if it is still held.
                    if let Some(&(_, k)) = self.ids.iter().find(|&&(id, _)| id == e.note_id) {
                        self.release(k, mask, &mut emit);
                    }
                } else {
                    self.release_channels(mask, &mut emit);
                }
            }
            _ => {}
        }
    }

    /// A MIDI channel message as the notes it means: note on (velocity 0 is an off), note
    /// off, and the all-notes-off and all-sound-off controllers (of their channel).
    pub fn midi(&mut self, data: [u8; 3], mut emit: impl FnMut(FxNote)) {
        let channel = (data[0] & 0x0f) as i16;
        let key = data[1] & 0x7f;
        match data[0] & 0xf0 {
            0x90 if data[2] > 0 => self.on(-1, channel, key, data[2] & 0x7f, &mut emit),
            0x90 | 0x80 => self.release(key, bit(channel), &mut emit),
            0xb0 if data[1] == 120 || data[1] == 123 => {
                self.release_channels(bit(channel), &mut emit)
            }
            _ => {}
        }
    }
}

/// Read the host's note events into `out` (sorted by time, as the host sends them), as
/// `held` says they mean. Allocation-free: `out` was reserved in `activate`.
///
/// # Safety
/// `list` must be a valid host event list for this call.
pub(crate) unsafe fn read(list: *const clap_input_events, out: &mut Vec<NoteIn>, held: &mut Held) {
    out.clear();
    if list.is_null() {
        return;
    }
    let (Some(size), Some(get)) = ((*list).size, (*list).get) else {
        return;
    };
    for i in 0..size(list) {
        let h = get(list, i);
        if h.is_null() || (*h).space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        let offset = (*h).time;
        let mut emit = |note| push_note(out, NoteIn { offset, note });
        match (*h).type_ {
            t @ (CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE) => {
                held.clap(t, &*(h as *const clap_event_note), &mut emit);
            }
            CLAP_EVENT_MIDI => held.midi((*(h as *const clap_event_midi)).data, &mut emit),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(note_id: i32, channel: i16, key: i16, velocity: f64) -> clap_event_note {
        clap_event_note {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: 0,
                flags: 0,
            },
            note_id,
            port_index: 0,
            channel,
            key,
            velocity,
        }
    }

    fn clap(h: &mut Held, type_: u16, e: clap_event_note) -> Vec<FxNote> {
        let mut out = Vec::new();
        h.clap(type_, &e, |n| out.push(n));
        out
    }

    fn midi(h: &mut Held, data: [u8; 3]) -> Vec<FxNote> {
        let mut out = Vec::new();
        h.midi(data, |n| out.push(n));
        out
    }

    const OFF: u16 = CLAP_EVENT_NOTE_OFF;

    #[test]
    fn midi_bytes_read_as_notes() {
        let mut h = Held::default();
        let on = |pitch, velocity| vec![FxNote::On { pitch, velocity }];
        assert_eq!(midi(&mut h, [0x93, 60, 100]), on(60, 100));
        assert_eq!(midi(&mut h, [0x90, 60, 0]), vec![FxNote::Off { pitch: 60 }]);
        assert_eq!(
            midi(&mut h, [0x8f, 61, 40]),
            vec![FxNote::Off { pitch: 61 }]
        );
        assert_eq!(midi(&mut h, [0xb0, 7, 100]), vec![]);
        assert_eq!(midi(&mut h, [0xe0, 0, 64]), vec![]);
        // All notes off is its channel's.
        midi(&mut h, [0x93, 60, 100]);
        midi(&mut h, [0x95, 64, 100]);
        midi(&mut h, [0x93, 67, 100]);
        assert_eq!(
            midi(&mut h, [0xb3, 123, 0]),
            vec![FxNote::Off { pitch: 60 }, FxNote::Off { pitch: 67 }]
        );
        assert_eq!(midi(&mut h, [0xb3, 120, 0]), vec![], "nothing left on 3");
        assert_eq!(
            midi(&mut h, [0xb5, 120, 0]),
            vec![FxNote::Off { pitch: 64 }]
        );
    }

    #[test]
    fn clap_notes_read_as_notes() {
        let mut h = Held::default();
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_ON, note(-1, 0, 64, 1.0)),
            vec![FxNote::On {
                pitch: 64,
                velocity: 127
            }]
        );
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_ON, note(-1, 0, 64, 0.0)),
            vec![FxNote::On {
                pitch: 64,
                velocity: 1
            }]
        );
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_ON, note(-1, 0, -1, 1.0)),
            vec![]
        );
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_CHOKE, note(-1, -1, -1, 0.0)),
            vec![FxNote::AllOff]
        );
        assert_eq!(
            clap(&mut h, OFF, note(-1, 0, 64, 0.0)),
            vec![FxNote::Off { pitch: 64 }]
        );
    }

    #[test]
    fn a_clap_off_with_key_minus_one_releases_only_what_it_addresses() {
        let mut h = Held::default();
        clap(&mut h, CLAP_EVENT_NOTE_ON, note(5, 0, 60, 0.8));
        clap(&mut h, CLAP_EVENT_NOTE_ON, note(6, 3, 64, 0.8));
        clap(&mut h, CLAP_EVENT_NOTE_ON, note(7, 3, 67, 0.8));
        // By note id: that note only.
        assert_eq!(
            clap(&mut h, OFF, note(5, -1, -1, 0.0)),
            vec![FxNote::Off { pitch: 60 }]
        );
        assert_eq!(
            clap(&mut h, OFF, note(5, -1, -1, 0.0)),
            vec![],
            "already off"
        );
        assert_eq!(
            clap(&mut h, OFF, note(99, -1, -1, 0.0)),
            vec![],
            "an unknown id"
        );
        // By channel: that channel's keys only.
        assert_eq!(clap(&mut h, OFF, note(-1, 0, -1, 0.0)), vec![]);
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_CHOKE, note(-1, 3, -1, 0.0)),
            vec![FxNote::Off { pitch: 64 }, FxNote::Off { pitch: 67 }]
        );
        assert_eq!(
            clap(&mut h, OFF, note(6, -1, -1, 0.0)),
            vec![],
            "the id went with it"
        );
        // Every field a wildcard: everything.
        assert_eq!(
            clap(&mut h, OFF, note(-1, -1, -1, 0.0)),
            vec![FxNote::AllOff]
        );
    }

    #[test]
    fn note_ids_past_the_table_forget_the_oldest() {
        let mut h = Held::default();
        for i in 0..IDS as i32 + 1 {
            clap(
                &mut h,
                CLAP_EVENT_NOTE_ON,
                note(i, 0, (i % 128) as i16, 0.8),
            );
        }
        assert_eq!(clap(&mut h, OFF, note(0, -1, -1, 0.0)), vec![], "forgotten");
        assert_eq!(
            clap(&mut h, OFF, note(IDS as i32, -1, -1, 0.0)),
            vec![FxNote::Off { pitch: 0 }]
        );
    }
}
