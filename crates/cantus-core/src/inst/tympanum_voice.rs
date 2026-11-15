//! A modelled kit's voice (round 40, `drums-hyb`; ADR 0124's third amendment): *Generated*
//! — model 5 alone, the default — or *Hybrid*: a recorded residual (the first 30–200 ms of
//! each piece that the model does not explain, fitted off a recorded kit) played under the
//! model, at each piece's source, so the mics, the bleed and the outputs apply to it.
//!
//! The project holds the text key [`VOICE_KEY`] (`voice = "hybrid"`; absent: generated).
//! The residual is a downloaded content pack ([`hybrid_pack`], ADR 0030/0042): a song whose
//! pack is not installed plays the generated kit — never silence — and the kit card offers
//! *Get*.

use crate::command::Command;
use crate::model::{Project, TrackId};
use crate::NativeSpec;

/// The text key: [`HYBRID`] or absent.
pub const VOICE_KEY: &str = "voice";
/// The hybrid's text.
pub const HYBRID: &str = "hybrid";
/// The file in a hybrid pack that holds the residual (`CRS1`, `cantus_dsp::tympanum_residual`).
pub const RESIDUAL_FILE: &str = "residual.crs";

/// What a modelled kit's pieces play.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Voice {
    /// Model 5 alone (the default).
    #[default]
    Generated,
    /// Model 5 with the recorded residual under it.
    Hybrid,
}

impl Voice {
    pub const ALL: [Voice; 2] = [Voice::Generated, Voice::Hybrid];

    /// The spec's voice (anything but [`HYBRID`] is generated).
    pub fn of(spec: &NativeSpec) -> Voice {
        if spec.text(VOICE_KEY) == Some(HYBRID) {
            Voice::Hybrid
        } else {
            Voice::Generated
        }
    }

    /// Its stored text (`None`: the key absent).
    pub fn text(self) -> Option<&'static str> {
        match self {
            Voice::Generated => None,
            Voice::Hybrid => Some(HYBRID),
        }
    }

    /// Its name's locale key.
    pub fn label_key(self) -> &'static str {
        match self {
            Voice::Generated => "tympanum.voice.generated",
            Voice::Hybrid => "tympanum.voice.hybrid",
        }
    }
}

/// The content pack that holds a modelled kit's residual: the jazz kit's from Virtuosity
/// Drums (CC0), the studio kit's from DRSKit (CC BY 4.0). `None`: the kit has no hybrid.
pub fn hybrid_pack(kit_id: &str) -> Option<&'static str> {
    match kit_id {
        "jazz" => Some("tympanum-hybrid-jazz"),
        "studio" => Some("tympanum-hybrid-studio"),
        _ => None,
    }
}

/// The command that sets `track`'s Tympanum's voice: one `SetInstrumentText` (its exact
/// inverse by construction, one undo step). `None`: not a Tympanum, or already that voice.
pub fn set_voice(project: &Project, track: TrackId, voice: Voice) -> Option<Command> {
    let n = project.track(track)?.instrument.native()?;
    if n.kind != crate::InstrumentKind::AcousticDrums || Voice::of(n) == voice {
        return None;
    }
    Some(Command::SetInstrumentText {
        track,
        key: VOICE_KEY.into(),
        text: voice.text().map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InstrumentKind, InstrumentSpec};

    #[test]
    fn the_voice_is_generated_unless_the_key_says_hybrid() {
        let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
        assert_eq!(Voice::of(&spec), Voice::Generated);
        spec.set_text(VOICE_KEY, Some(HYBRID.into())).unwrap();
        assert_eq!(Voice::of(&spec), Voice::Hybrid);
        spec.set_text(VOICE_KEY, Some("something".into())).unwrap();
        assert_eq!(Voice::of(&spec), Voice::Generated);
        assert_eq!(hybrid_pack("jazz"), Some("tympanum-hybrid-jazz"));
        assert_eq!(hybrid_pack("nope"), None);
    }

    #[test]
    fn setting_the_voice_is_one_text_command_and_none_when_it_is_already_so() {
        let mut p = Project::new("t");
        let mut t = p.make_track("Drums");
        t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
        let id = t.id;
        p.tracks.push(t);
        assert!(set_voice(&p, id, Voice::Generated).is_none());
        match set_voice(&p, id, Voice::Hybrid) {
            Some(Command::SetInstrumentText { key, text, .. }) => {
                assert_eq!(key, VOICE_KEY);
                assert_eq!(text.as_deref(), Some(HYBRID));
            }
            other => panic!("{other:?}"),
        }
    }
}
