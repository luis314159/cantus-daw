//! Notes into the plug-ins that hear them (round 28): a CLAP note port for every kind that
//! takes notes (`EffectKind::takes_notes` — Tonus's targets, Porta's MIDI trigger), in the
//! CLAP and MIDI dialects, read from the host's event list at their sample.

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

/// A MIDI channel message as a note: note on (velocity 0 is an off), note off, and the
/// all-notes-off and all-sound-off controllers.
pub fn from_midi(data: [u8; 3]) -> Option<FxNote> {
    let pitch = data[1] & 0x7f;
    match data[0] & 0xf0 {
        0x90 if data[2] > 0 => Some(FxNote::On {
            pitch,
            velocity: data[2] & 0x7f,
        }),
        0x90 | 0x80 => Some(FxNote::Off { pitch }),
        0xb0 if data[1] == 120 || data[1] == 123 => Some(FxNote::AllOff),
        _ => None,
    }
}

/// A CLAP note event as a note: a key of −1 on an off or a choke is every key.
pub fn from_clap(type_: u16, key: i16, velocity: f64) -> Option<FxNote> {
    match type_ {
        CLAP_EVENT_NOTE_ON if (0..128).contains(&key) => Some(FxNote::On {
            pitch: key as u8,
            velocity: (velocity * 127.0).round().clamp(1.0, 127.0) as u8,
        }),
        CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE if (0..128).contains(&key) => {
            Some(FxNote::Off { pitch: key as u8 })
        }
        CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE if key < 0 => Some(FxNote::AllOff),
        _ => None,
    }
}

/// Read the host's note events into `out` (sorted by time, as the host sends them).
/// Allocation-free: `out` was reserved in `activate`.
///
/// # Safety
/// `list` must be a valid host event list for this call.
pub(crate) unsafe fn read(list: *const clap_input_events, out: &mut Vec<NoteIn>) {
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
        let note = match (*h).type_ {
            t @ (CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE) => {
                let e = &*(h as *const clap_event_note);
                from_clap(t, e.key, e.velocity)
            }
            CLAP_EVENT_MIDI => from_midi((*(h as *const clap_event_midi)).data),
            _ => None,
        };
        if let Some(note) = note {
            push_note(
                out,
                NoteIn {
                    offset: (*h).time,
                    note,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_bytes_read_as_notes() {
        assert_eq!(
            from_midi([0x93, 60, 100]),
            Some(FxNote::On {
                pitch: 60,
                velocity: 100
            })
        );
        assert_eq!(from_midi([0x90, 60, 0]), Some(FxNote::Off { pitch: 60 }));
        assert_eq!(from_midi([0x8f, 61, 40]), Some(FxNote::Off { pitch: 61 }));
        assert_eq!(from_midi([0xb0, 123, 0]), Some(FxNote::AllOff));
        assert_eq!(from_midi([0xb0, 7, 100]), None);
        assert_eq!(from_midi([0xe0, 0, 64]), None);
    }

    #[test]
    fn clap_notes_read_as_notes() {
        assert_eq!(
            from_clap(CLAP_EVENT_NOTE_ON, 64, 1.0),
            Some(FxNote::On {
                pitch: 64,
                velocity: 127
            })
        );
        assert_eq!(
            from_clap(CLAP_EVENT_NOTE_ON, 64, 0.0),
            Some(FxNote::On {
                pitch: 64,
                velocity: 1
            })
        );
        assert_eq!(
            from_clap(CLAP_EVENT_NOTE_CHOKE, -1, 0.0),
            Some(FxNote::AllOff)
        );
        assert_eq!(from_clap(CLAP_EVENT_NOTE_ON, -1, 1.0), None);
    }
}
