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

    /// Do what `purpose` asked with `choice`, as one undo step, and close the picker.
    /// Contract version: the plain add/replace through the existing commands; the
    /// device-ux builder makes swapping keep what should be kept.
    pub fn apply_pick(&mut self, purpose: PickPurpose, choice: PickChoice) {
        use cantus_core::{InstrumentSpec, NativeSpec};
        self.picker = None;
        let spec = match &choice {
            PickChoice::Firmus => Some(InstrumentSpec::default()),
            PickChoice::Pulsus => Some(InstrumentSpec::Pulsus(Default::default())),
            PickChoice::Instrument(k) => Some(InstrumentSpec::Native(NativeSpec::new(*k))),
            _ => None,
        };
        match (purpose, choice) {
            // A hosted instrument (ADR 0028): through the host, which loads it first.
            (PickPurpose::ReplaceInstrument { track }, PickChoice::Hosted(info))
                if info.kind == cantus_host::PluginKind::Instrument =>
            {
                self.set_hosted_instrument(track, &info);
            }
            (PickPurpose::AddInstrument { .. }, PickChoice::Hosted(info))
                if info.kind == cantus_host::PluginKind::Instrument =>
            {
                self.add_track();
                if let Some(t) = self.project().tracks.last().map(|t| t.id) {
                    self.set_hosted_instrument(t, &info);
                }
            }
            (PickPurpose::ReplaceInstrument { track }, _) => {
                if let Some(spec) = spec {
                    self.set_instrument(track, spec);
                }
            }
            (PickPurpose::AddInstrument { .. }, _) => {
                if let Some(spec) = spec {
                    self.add_track();
                    if let Some(t) = self.project().tracks.last().map(|t| t.id) {
                        self.set_instrument(t, spec);
                    }
                }
            }
            (PickPurpose::AddEffect { strip }, PickChoice::Effect(kind)) => {
                self.add_effect(strip, kind);
            }
            (PickPurpose::ReplaceEffect { strip, effect }, PickChoice::Effect(kind)) => {
                self.remove_effect(strip, effect);
                self.add_effect(strip, kind);
            }
            _ => {}
        }
    }
}
