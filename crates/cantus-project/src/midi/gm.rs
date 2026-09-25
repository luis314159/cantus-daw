//! General MIDI's two tables: the percussion key map (channel 10) and the 128 programs.
//! What an imported file's tracks are called, and where a kit's pads land in one.

use cantus_core::{DrumPad, PadVoice, Pitch, PulsusParams};

/// The GM percussion map, keys 35–81 (GM Level 1).
const DRUMS: [&str; 47] = [
    "Acoustic bass drum",
    "Bass drum",
    "Side stick",
    "Acoustic snare",
    "Hand clap",
    "Electric snare",
    "Low floor tom",
    "Closed hi-hat",
    "High floor tom",
    "Pedal hi-hat",
    "Low tom",
    "Open hi-hat",
    "Low-mid tom",
    "Hi-mid tom",
    "Crash cymbal 1",
    "High tom",
    "Ride cymbal 1",
    "Chinese cymbal",
    "Ride bell",
    "Tambourine",
    "Splash cymbal",
    "Cowbell",
    "Crash cymbal 2",
    "Vibraslap",
    "Ride cymbal 2",
    "Hi bongo",
    "Low bongo",
    "Mute hi conga",
    "Open hi conga",
    "Low conga",
    "High timbale",
    "Low timbale",
    "High agogo",
    "Low agogo",
    "Cabasa",
    "Maracas",
    "Short whistle",
    "Long whistle",
    "Short guiro",
    "Long guiro",
    "Claves",
    "Hi wood block",
    "Low wood block",
    "Mute cuica",
    "Open cuica",
    "Mute triangle",
    "Open triangle",
];

pub const FIRST_DRUM: Pitch = 35;
pub const LAST_DRUM: Pitch = 81;

/// The GM name of a percussion key, if it has one.
pub fn drum_name(key: Pitch) -> Option<&'static str> {
    (FIRST_DRUM..=LAST_DRUM)
        .contains(&key)
        .then(|| DRUMS[(key - FIRST_DRUM) as usize])
}

/// Which of Pulsus's shapes stands in for a GM percussion key: kicks are the kick, metal
/// is the hi-hat's partials, and everything with a skin or a body — snares, toms, hand
/// percussion — is the snare's shape, which is what the shipped toms already are.
pub fn drum_voice(key: Pitch) -> PadVoice {
    match key {
        35 | 36 => PadVoice::Kick,
        42 | 44 | 46 | 49 | 51 | 52 | 53 | 54 | 55 | 57 | 59 | 69 | 70 | 80 | 81 => PadVoice::HiHat,
        _ => PadVoice::Snare,
    }
}

/// A pad for a GM key the kit does not have, named after it, with its shape's defaults
/// and the shipped kit's hats choked together.
pub fn pad_for(key: Pitch) -> DrumPad {
    let name = drum_name(key)
        .map(str::to_string)
        .unwrap_or_else(|| cantus_core::pitch_name(key));
    let mut pad = DrumPad::new(key, &name, drum_voice(key));
    match key {
        // The hats: closed, pedal and open cut each other off, like the shipped pair.
        42 | 44 | 46 => {
            pad.choke = 1;
            pad.tone = 9_500.0;
            pad.decay = if key == 46 { 0.42 } else { 0.055 };
        }
        // Cymbals ring.
        49 | 51 | 52 | 55 | 57 | 59 => {
            pad.tone = 8_000.0;
            pad.decay = 1.2;
        }
        // Toms: the snare shape, all body, tuned by where they sit.
        41 | 43 | 45 | 47 | 48 | 50 => {
            pad.mix = 0.06;
            pad.decay = 0.4;
            pad.tune = (key as f32 - 50.0) * 1.5;
        }
        _ => {}
    }
    pad
}

/// The GM key a pad is written to in a MIDI file.
///
/// A pad on a GM percussion key whose sound is of that key's kind is already there (the
/// shipped kit is laid out on the GM map). A pad anywhere else — off the map, or a kick
/// sitting on the bongo's key — is moved to its shape's GM key (a kick to 36, a snare to
/// 38, a hi-hat to 42) when no other pad of the kit is on that key; otherwise, and for a
/// sample, whose sound nobody can guess, it keeps its own note.
pub fn export_key(kit: &PulsusParams, pitch: Pitch) -> Pitch {
    let Some(pad) = kit.pads.iter().find(|p| p.pitch == pitch) else {
        return pitch;
    };
    if pad.voice == PadVoice::Sample
        || ((FIRST_DRUM..=LAST_DRUM).contains(&pitch) && drum_voice(pitch) == pad.voice)
    {
        return pitch;
    }
    let target = match pad.voice {
        PadVoice::Kick => 36,
        PadVoice::Snare => 38,
        PadVoice::HiHat => 42,
        _ => return pitch,
    };
    if kit.pads.iter().any(|p| p.pitch == target) {
        pitch
    } else {
        target
    }
}

/// GM Level 1's program names, for naming a track imported from a type 0 file by what the
/// channel plays.
const PROGRAMS: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavinet",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Nylon Guitar",
    "Steel Guitar",
    "Jazz Guitar",
    "Clean Guitar",
    "Muted Guitar",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Fingered Bass",
    "Picked Bass",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Square Lead",
    "Sawtooth Lead",
    "Calliope Lead",
    "Chiff Lead",
    "Charang Lead",
    "Voice Lead",
    "Fifths Lead",
    "Bass + Lead",
    "New Age Pad",
    "Warm Pad",
    "Polysynth Pad",
    "Choir Pad",
    "Bowed Pad",
    "Metallic Pad",
    "Halo Pad",
    "Sweep Pad",
    "Rain",
    "Soundtrack",
    "Crystal",
    "Atmosphere",
    "Brightness",
    "Goblins",
    "Echoes",
    "Sci-fi",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bagpipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

pub fn program_name(program: u8) -> &'static str {
    PROGRAMS[(program & 0x7F) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_are_where_general_midi_puts_them() {
        assert_eq!(drum_name(35), Some("Acoustic bass drum"));
        assert_eq!(drum_name(38), Some("Acoustic snare"));
        assert_eq!(drum_name(42), Some("Closed hi-hat"));
        assert_eq!(drum_name(49), Some("Crash cymbal 1"));
        assert_eq!(drum_name(81), Some("Open triangle"));
        assert_eq!(drum_name(34), None);
        assert_eq!(program_name(0), "Acoustic Grand Piano");
        assert_eq!(program_name(33), "Fingered Bass");
        assert_eq!(program_name(127), "Gunshot");
    }

    #[test]
    fn the_shipped_kit_is_already_on_the_gm_map_and_stray_pads_move_to_their_shape() {
        let kit = PulsusParams::default();
        for pad in &kit.pads {
            assert_eq!(export_key(&kit, pad.pitch), pad.pitch, "{}", pad.name);
            assert!(drum_name(pad.pitch).is_some(), "{}", pad.name);
        }
        // A kick on C4 in a kit with no pad on 36 goes to 36; a snare on C4 in the shipped
        // kit (which has 38) keeps its note.
        let stray = PulsusParams::of(vec![DrumPad::new(60, "Kick", PadVoice::Kick)]);
        assert_eq!(export_key(&stray, 60), 36);
        let mut crowded = PulsusParams::default();
        crowded
            .pads
            .push(DrumPad::new(84, "Snare 2", PadVoice::Snare));
        assert_eq!(export_key(&crowded, 84), 84);
    }
}
