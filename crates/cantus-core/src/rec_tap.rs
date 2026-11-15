//! r40:pads — where a track's recording taps its signal (ADR 0118, schema 62).
//!
//! *Before FX* (the default, and every song before schema 62) records the track's input as
//! it arrives — after its trim, before anything else. *After FX* records it as heard
//! through the strip's inserts (Logic's and Studio One's "record with effects"): the
//! strip's signal after its insert chain, before its EQ, width, pan and fader, its latency
//! taken out so the take lands where it was played. A choice per track, kept in the song.

use serde::{Deserialize, Serialize};

/// Where a track's recording takes its signal.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecordTap {
    /// The input as it arrives (after its trim): today's, and the default.
    #[default]
    BeforeFx,
    /// The track's strip after its inserts, before its EQ and fader: what the inserts make
    /// of everything the strip carries then.
    AfterFx,
}

impl RecordTap {
    /// The default (serde's `skip_serializing_if`).
    pub fn is_before(&self) -> bool {
        *self == RecordTap::BeforeFx
    }

    pub const ALL: [RecordTap; 2] = [RecordTap::BeforeFx, RecordTap::AfterFx];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::freeze::{content_key, FreezeTarget};
    use crate::model::Project;

    #[test]
    fn the_command_has_its_exact_inverse_and_changes_no_freeze_key() {
        let mut p = Project::new("tap");
        let t = p.make_track("Voice");
        let id = t.id;
        p.tracks.push(t);
        let before = p.clone();
        let key = content_key(&p, FreezeTarget::Track(id));
        let undo = Command::SetTrackRecordTap {
            id,
            tap: RecordTap::AfterFx,
        }
        .apply(&mut p)
        .unwrap();
        assert_eq!(p.track(id).unwrap().record_tap, RecordTap::AfterFx);
        // Where a take is tapped is not what plays: a frozen track stays frozen.
        assert_eq!(content_key(&p, FreezeTarget::Track(id)), key);
        undo.apply(&mut p).unwrap();
        assert_eq!(p, before);
    }
}
