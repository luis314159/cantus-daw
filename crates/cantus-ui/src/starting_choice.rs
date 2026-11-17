//! The user's own starting presets (ADR 0135), the application's side: the
//! choices kept in the settings (`settings.json`, never a song), handed to
//! `cantus_core::starting_user` at start and on every change, so every insert path — the
//! picker, the browser, a drag, a swap, Tabula's *Add pedal*, the agents' tools — starts a
//! unit on the person's choice. The global *Insert effects at their neutral settings* keeps
//! its rule: neutral inserts take no starting preset, the person's or the factory's.
//! // fx3:start

use crate::app::CantusApp;
use cantus_core::starting_user::{self, UserStart};
use cantus_core::{EffectKind, EffectSpec};
use serde::{Deserialize, Serialize};

/// One unit's chosen start, as the settings keep it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartingChoice {
    /// The name the preset menu shows it by (the preset's, or "Your settings").
    pub label: String,
    /// The whole sound: values, and a board's pedals or a bank where the unit has them.
    pub spec: EffectSpec,
}

impl CantusApp {
    /// Hand the settings' choices to the insert paths (at start, and after a change).
    pub fn apply_starting_choices(&mut self) {
        starting_user::replace_all(self.settings.starting.iter().map(|c| UserStart {
            spec: c.spec.clone(),
            label: c.label.clone(),
        }));
    }

    /// The person's start for `kind`, if any.
    pub fn starting_choice(&self, kind: EffectKind) -> Option<&StartingChoice> {
        self.settings.starting.iter().find(|c| c.spec.kind == kind)
    }

    /// New inserts of `spec`'s unit start on `spec` (kept in the settings).
    pub fn use_as_starting(&mut self, spec: EffectSpec, label: String) {
        if !starting_user::choosable(spec.kind) {
            return;
        }
        let kind = spec.kind;
        self.settings.starting.retain(|c| c.spec.kind != kind);
        self.settings.starting.push(StartingChoice {
            label: label.clone(),
            spec,
        });
        self.apply_starting_choices();
        self.store_settings();
        let unit = crate::i18n::t(kind.name_key());
        self.set_status(crate::i18n::tf(
            "start.used",
            &[("unit", &unit), ("name", &label)],
        ));
    }

    /// `kind`'s inserts start on its factory starting preset again.
    pub fn reset_starting(&mut self, kind: EffectKind) {
        self.settings.starting.retain(|c| c.spec.kind != kind);
        self.apply_starting_choices();
        self.store_settings();
        let unit = crate::i18n::t(kind.name_key());
        self.set_status(crate::i18n::tf("start.reset_done", &[("unit", &unit)]));
    }
}
