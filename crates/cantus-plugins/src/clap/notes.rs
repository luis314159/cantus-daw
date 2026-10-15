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

/// Note ids remembered at once. Past them the oldest id is forgotten but not its note: it
/// becomes a hold of its key on its channel, which an off by key or channel still releases.
const IDS: usize = 128;

/// What the host holds, as far as the notes it sent tell, and so what the unit holds: the
/// units hear a pitch, not a channel or a note id, so a key is released in the unit (one
/// `Off`) only when its **last** holder lets go. Kept in step with what reached the unit:
/// a note on the block's buffer had no room for is not held, and a release that had no room
/// becomes an all-notes-off (which always gets in) and forgets everything — so no note is
/// ever left stuck. Fixed size: it lives on the audio thread.
#[derive(Clone, Debug)]
pub struct Held {
    /// Per key, a bit per MIDI channel holding it without a note id.
    channels: [u16; 128],
    /// (note id, key, channel) of each note held by id; an id of −1 is a free slot.
    ids: [(i32, u8, u8); IDS],
    /// The slot taken next when every one is in use.
    next: usize,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            channels: [0; 128],
            ids: [(-1, 0, 0); IDS],
            next: 0,
        }
    }
}

/// A channel's number; one outside 0..16 (CLAP's −1 on a note on) is channel 0.
fn chan(channel: i16) -> u8 {
    if (0..16).contains(&channel) {
        channel as u8
    } else {
        0
    }
}

/// A channel's bit.
fn bit(channel: u8) -> u16 {
    1 << (channel & 15)
}

impl Held {
    /// Forget everything (the plug-in was activated or reset).
    pub fn clear(&mut self) {
        *self = Held::default();
    }

    /// Whether anything holds `key`.
    pub fn holds(&self, key: u8) -> bool {
        self.channels[key as usize] != 0
            || self.ids.iter().any(|&(id, k, _)| id >= 0 && k == key)
    }

    /// The note `emit` could not deliver was a release: the unit is told to let go of
    /// everything (an all-notes-off always gets in), and so is this.
    fn overflow(&mut self, emit: &mut impl FnMut(FxNote) -> bool) {
        self.clear();
        emit(FxNote::AllOff);
    }

    /// Release the unit's `key` if this took its last holder; `was`: it was held before.
    fn settle(&mut self, key: u8, was: bool, emit: &mut impl FnMut(FxNote) -> bool) -> bool {
        if was && !self.holds(key) && !emit(FxNote::Off { pitch: key }) {
            self.overflow(emit);
            return false;
        }
        true
    }

    fn on(
        &mut self,
        note_id: i32,
        channel: u8,
        key: u8,
        velocity: u8,
        emit: &mut impl FnMut(FxNote) -> bool,
    ) {
        if !emit(FxNote::On {
            pitch: key,
            velocity,
        }) {
            // No room: the unit never heard it, so nothing holds it.
            return;
        }
        if note_id < 0 {
            self.channels[key as usize] |= bit(channel);
            return;
        }
        let same = self.ids.iter().position(|&(id, _, _)| id == note_id);
        let slot = match same.or_else(|| self.ids.iter().position(|&(id, _, _)| id < 0)) {
            Some(i) => i,
            None => {
                let i = self.next;
                self.next = (i + 1) % IDS;
                i
            }
        };
        // An id forgotten (or reused) keeps its note held, by key and channel.
        let (old, k, c) = self.ids[slot];
        if old >= 0 {
            self.channels[k as usize] |= bit(c);
        }
        self.ids[slot] = (note_id, key, channel);
    }

    /// An off for `key` on the channels in `mask`: by its note id when that is known (that
    /// note only), else every hold of the key on those channels. `false`: the buffer was
    /// full and everything was released instead.
    fn release(
        &mut self,
        key: u8,
        mask: u16,
        note_id: i32,
        emit: &mut impl FnMut(FxNote) -> bool,
    ) -> bool {
        let was = self.holds(key);
        let k = key as usize;
        let by_id = (note_id >= 0)
            .then(|| {
                self.ids
                    .iter()
                    .position(|&(id, kk, _)| id == note_id && kk == key)
            })
            .flatten();
        match by_id {
            Some(i) => self.ids[i] = (-1, 0, 0),
            // An id this forgot (its note is a channel hold now), or none: the key on
            // those channels — the holds without an id, and with no id named, those with.
            None => {
                self.channels[k] &= !mask;
                if note_id < 0 {
                    for slot in self.ids.iter_mut() {
                        if slot.0 >= 0 && slot.1 == key && bit(slot.2) & mask != 0 {
                            *slot = (-1, 0, 0);
                        }
                    }
                }
            }
        }
        self.settle(key, was, emit)
    }

    /// Every key held on the channels in `mask`; every channel's: everything.
    fn release_channels(&mut self, mask: u16, emit: &mut impl FnMut(FxNote) -> bool) {
        if mask == u16::MAX {
            self.clear();
            emit(FxNote::AllOff);
            return;
        }
        for key in 0..128u8 {
            let k = key as usize;
            let on_mask = self.channels[k] & mask != 0
                || self
                    .ids
                    .iter()
                    .any(|&(id, kk, c)| id >= 0 && kk == key && bit(c) & mask != 0);
            if on_mask && !self.release(key, mask, -1, emit) {
                return;
            }
        }
    }

    /// A CLAP note on, off or choke, as the notes it means. `emit` says whether the note
    /// got into the block's buffer.
    pub fn clap(&mut self, type_: u16, e: &clap_event_note, mut emit: impl FnMut(FxNote) -> bool) {
        let key = (0..128).contains(&e.key).then_some(e.key as u8);
        let mask = if (0..16).contains(&e.channel) {
            bit(e.channel as u8)
        } else {
            u16::MAX
        };
        match (type_, key) {
            (CLAP_EVENT_NOTE_ON, Some(k)) => {
                let velocity = (e.velocity * 127.0).round().clamp(1.0, 127.0) as u8;
                self.on(e.note_id, chan(e.channel), k, velocity, &mut emit);
            }
            (CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE, Some(k)) => {
                self.release(k, mask, e.note_id, &mut emit);
            }
            (CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE, None) if e.key < 0 => {
                if e.note_id >= 0 {
                    // That note: the key its on had, if it is still held.
                    if let Some(&(_, k, _)) =
                        self.ids.iter().find(|&&(id, _, _)| id == e.note_id)
                    {
                        self.release(k, mask, e.note_id, &mut emit);
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
    pub fn midi(&mut self, data: [u8; 3], mut emit: impl FnMut(FxNote) -> bool) {
        let channel = data[0] & 0x0f;
        let key = data[1] & 0x7f;
        match data[0] & 0xf0 {
            0x90 if data[2] > 0 => self.on(-1, channel, key, data[2] & 0x7f, &mut emit),
            0x90 | 0x80 => {
                self.release(key, bit(channel), -1, &mut emit);
            }
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
        h.clap(type_, &e, |n| {
            out.push(n);
            true
        });
        out
    }

    fn midi(h: &mut Held, data: [u8; 3]) -> Vec<FxNote> {
        let mut out = Vec::new();
        h.midi(data, |n| {
            out.push(n);
            true
        });
        out
    }

    /// A MIDI message into a block whose buffer is full: only an all-notes-off gets in.
    fn midi_full(h: &mut Held, data: [u8; 3]) -> Vec<FxNote> {
        let mut out = Vec::new();
        h.midi(data, |n| {
            let kept = n == FxNote::AllOff;
            if kept {
                out.push(n);
            }
            kept
        });
        out
    }

    const OFF: u16 = CLAP_EVENT_NOTE_OFF;
    const ON: u16 = CLAP_EVENT_NOTE_ON;

    fn off(pitch: u8) -> Vec<FxNote> {
        vec![FxNote::Off { pitch }]
    }

    #[test]
    fn midi_bytes_read_as_notes() {
        let mut h = Held::default();
        let on = |pitch, velocity| vec![FxNote::On { pitch, velocity }];
        assert_eq!(midi(&mut h, [0x93, 60, 100]), on(60, 100));
        assert_eq!(midi(&mut h, [0x90, 60, 0]), vec![], "held on 3, not on 0");
        assert_eq!(midi(&mut h, [0x93, 60, 0]), off(60), "velocity 0 is an off");
        assert_eq!(midi(&mut h, [0x8f, 61, 40]), vec![], "nothing held 61");
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
        assert_eq!(midi(&mut h, [0xb5, 120, 0]), off(64));
    }

    #[test]
    fn clap_notes_read_as_notes() {
        let mut h = Held::default();
        assert_eq!(
            clap(&mut h, ON, note(-1, 0, 64, 1.0)),
            vec![FxNote::On {
                pitch: 64,
                velocity: 127
            }]
        );
        assert_eq!(
            clap(&mut h, ON, note(-1, 0, 64, 0.0)),
            vec![FxNote::On {
                pitch: 64,
                velocity: 1
            }]
        );
        assert_eq!(clap(&mut h, ON, note(-1, 0, -1, 1.0)), vec![]);
        assert_eq!(clap(&mut h, OFF, note(-1, 0, 64, 0.0)), off(64));
        clap(&mut h, ON, note(-1, 0, 64, 1.0));
        assert_eq!(
            clap(&mut h, CLAP_EVENT_NOTE_CHOKE, note(-1, -1, -1, 0.0)),
            vec![FxNote::AllOff]
        );
        assert_eq!(clap(&mut h, OFF, note(-1, 0, 64, 0.0)), vec![], "all gone");
    }

    #[test]
    fn a_clap_off_with_key_minus_one_releases_only_what_it_addresses() {
        let mut h = Held::default();
        clap(&mut h, ON, note(5, 0, 60, 0.8));
        clap(&mut h, ON, note(6, 3, 64, 0.8));
        clap(&mut h, ON, note(7, 3, 67, 0.8));
        // By note id: that note only.
        assert_eq!(clap(&mut h, OFF, note(5, -1, -1, 0.0)), off(60));
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

    /// The units hear a pitch: a key is released there only when its last holder lets go.
    #[test]
    fn a_key_held_on_two_channels_is_released_by_the_last_of_them() {
        let mut h = Held::default();
        // A plain off on one channel.
        midi(&mut h, [0x91, 60, 100]);
        midi(&mut h, [0x92, 60, 100]);
        assert_eq!(midi(&mut h, [0x82, 60, 0]), vec![], "channel 1 still holds it");
        assert_eq!(midi(&mut h, [0x81, 60, 0]), off(60));
        // A channel's all-notes-off.
        midi(&mut h, [0x93, 60, 100]);
        midi(&mut h, [0x95, 60, 100]);
        assert_eq!(midi(&mut h, [0xb3, 123, 0]), vec![], "channel 5 still holds it");
        assert_eq!(midi(&mut h, [0xb5, 123, 0]), off(60));
        // CLAP: an off on one channel, and a channel's off (key −1).
        clap(&mut h, ON, note(-1, 0, 62, 0.8));
        clap(&mut h, ON, note(-1, 1, 62, 0.8));
        assert_eq!(clap(&mut h, OFF, note(-1, 0, 62, 0.0)), vec![]);
        assert_eq!(clap(&mut h, OFF, note(-1, 1, -1, 0.0)), off(62));
    }

    #[test]
    fn two_note_ids_on_one_key_are_released_one_by_one() {
        let mut h = Held::default();
        clap(&mut h, ON, note(5, 0, 60, 0.8));
        clap(&mut h, ON, note(6, 0, 60, 0.8));
        // By id with key −1, and by id with its key named: that voice only.
        assert_eq!(clap(&mut h, OFF, note(5, -1, -1, 0.0)), vec![]);
        assert!(h.holds(60));
        assert_eq!(clap(&mut h, OFF, note(6, 0, 60, 0.0)), off(60));
        // An off for the key with no id names every voice on it.
        clap(&mut h, ON, note(7, 2, 61, 0.8));
        clap(&mut h, ON, note(8, 2, 61, 0.8));
        clap(&mut h, ON, note(9, 4, 61, 0.8));
        assert_eq!(clap(&mut h, OFF, note(-1, 2, 61, 0.0)), vec![], "9 holds it");
        assert_eq!(clap(&mut h, OFF, note(-1, -1, 61, 0.0)), off(61));
    }

    #[test]
    fn note_ids_past_the_table_forget_the_id_not_the_note() {
        let mut h = Held::default();
        for i in 0..IDS as i32 + 1 {
            clap(&mut h, ON, note(i, 0, (i % 128) as i16, 0.8));
        }
        assert_eq!(clap(&mut h, OFF, note(0, -1, -1, 0.0)), vec![], "forgotten");
        // Id 128 shares key 0 with the forgotten note, which still holds it.
        assert_eq!(clap(&mut h, OFF, note(IDS as i32, -1, -1, 0.0)), vec![]);
        // An off by key and channel releases what the forgotten id held.
        assert_eq!(clap(&mut h, OFF, note(0, 0, 0, 0.0)), off(0));
        assert_eq!(clap(&mut h, OFF, note(1, -1, -1, 0.0)), off(1));
    }

    /// A full block: a note on that did not get in is not held, and a release that did not
    /// get in becomes an all-notes-off: nothing is left held in the unit.
    #[test]
    fn a_release_with_no_room_becomes_an_all_notes_off() {
        let mut h = Held::default();
        midi(&mut h, [0x90, 60, 100]);
        midi(&mut h, [0x99, 72, 100]);
        assert_eq!(midi_full(&mut h, [0x95, 64, 100]), vec![]);
        assert!(!h.holds(64), "the unit never heard it");
        assert_eq!(midi_full(&mut h, [0xb9, 123, 0]), vec![FxNote::AllOff]);
        assert!(!h.holds(60) && !h.holds(72), "in step with the unit");
        assert_eq!(midi(&mut h, [0x80, 60, 0]), vec![], "nothing to release");
        // A plain note-off too.
        midi(&mut h, [0x90, 61, 100]);
        assert_eq!(midi_full(&mut h, [0x80, 61, 0]), vec![FxNote::AllOff]);
        // An off for a key nothing holds needs no room.
        assert_eq!(midi_full(&mut h, [0x80, 62, 0]), vec![]);
    }
}
