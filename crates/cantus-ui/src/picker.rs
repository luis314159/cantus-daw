//! The device picker (round 20, ADR 0029): one place to choose an instrument or an effect
//! — to add one, or to swap the one that is there — whether it is ours (native) or a
//! hosted CLAP/VST3 plug-in.
//!
//! **Contract (round 20).** The types and the two methods below are what every builder
//! codes against. The `browser` builder owns the picker's view (the gallery of every
//! device, `views/picker*.rs`) and this file's `open_picker`; the `device-ux` builder owns
//! `apply_pick` (what adding or swapping does: keeping notes, position, mix and bypass)
//! and every entry point that opens the picker (window chrome, track header, mixer slot,
//! inspector, rack). The `hosted-inst` builder makes `PickChoice::Hosted` work for
//! instruments.

use crate::app::CantusApp;
use cantus_core::{EffectId, EffectKind, InstrumentKind, StripId, TrackId};

/// What the picker was opened for: it decides which devices are offered (instruments or
/// effects) and what choosing one does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickPurpose {
    /// A new track with the chosen instrument, inserted at `index` (the end when `None`).
    AddInstrument { index: Option<usize> },
    /// Give this track another instrument; its clips and notes stay.
    ReplaceInstrument { track: TrackId },
    /// A new insert at the end of this strip's chain (`StripId::MASTER` for the master).
    AddEffect { strip: StripId },
    /// Swap this insert for another kind, in the same place.
    ReplaceEffect { strip: StripId, effect: EffectId },
}

impl PickPurpose {
    /// Whether this purpose chooses among instruments (else among effects).
    pub fn wants_instrument(&self) -> bool {
        matches!(
            self,
            PickPurpose::AddInstrument { .. } | PickPurpose::ReplaceInstrument { .. }
        )
    }
}

/// What was chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum PickChoice {
    /// The basic synth every track starts with.
    Firmus,
    /// The classic synthesized kit.
    Pulsus,
    /// A native instrument (ADR 0023).
    Instrument(InstrumentKind),
    /// A native effect.
    Effect(EffectKind),
    /// A hosted CLAP/VST3 plug-in, instrument or effect (ADR 0019, 0028).
    Hosted(Box<cantus_host::PluginInfo>),
}

impl CantusApp {
    /// Open the picker for `purpose` (it is drawn by `views::picker`, owned by the browser
    /// builder). Contract version: remembers the purpose.
    pub fn open_picker(&mut self, purpose: PickPurpose) {
        self.picker = Some(purpose);
    }

    /// Do what `purpose` asked with `choice`, as one undo step, and close the picker
    /// (round 20, device-ux; the work is in [`crate::swap`]):
    ///
    /// - **AddInstrument**: a track named after the instrument, at `index`, selected, its
    ///   window open.
    /// - **ReplaceInstrument**: the same track — clips, notes, strip, inserts, colour, and
    ///   its name unless it was a default one — with the new instrument; between native
    ///   instruments the matching settings come along (`CantusApp::swap_keep`).
    /// - **AddEffect**: at the end of the chain, its editor open.
    /// - **ReplaceEffect**: in the same place, with its mix, bypass, key and note source
    ///   where the new kind takes them (`Command::ReplaceEffect`).
    ///
    /// A choice that does not fit the purpose (an effect for a track, a hosted instrument
    /// before ADR 0028 lands: [`crate::swap::available`]) changes nothing and says why.
    pub fn apply_pick(&mut self, purpose: PickPurpose, choice: PickChoice) {
        self.picker = None;
        if !crate::swap::available(&purpose, &choice) {
            if matches!(&choice, PickChoice::Hosted(i) if i.kind == cantus_host::PluginKind::Instrument)
            {
                self.set_status(crate::i18n::t("swap.status.hosted_instrument"));
            }
            return;
        }
        match purpose {
            PickPurpose::AddInstrument { index } => {
                self.add_instrument_track(index, &choice);
            }
            PickPurpose::ReplaceInstrument { track } => {
                self.replace_instrument(track, &choice);
            }
            PickPurpose::AddEffect { strip } => {
                self.add_picked_effect(strip, &choice);
            }
            PickPurpose::ReplaceEffect { strip, effect } => {
                self.replace_effect_with(strip, effect, &choice);
            }
        }
    }
}
