//! The user's own starting presets (ADR 0135): for any unit, the sound an insert
//! starts on can be the person's choice instead of the factory's (ADR 0071) — chosen in the
//! unit's preset menu (*Use as starting preset*), undone there (*Reset to factory starting
//! preset*), kept in the user's settings (not in a project: a song made with it holds the
//! values it inserted, so it opens the same everywhere). // fx3:start
//!
//! The application hands the choices here at start and whenever they change; every way an
//! effect is inserted goes through [`crate::starting::apply`] (the picker, the browser, a
//! drag, a swap, Tabula's *Add pedal*, the agents' tools), which asks [`get`] first. A
//! program that never sets them — the exported plug-ins, the tests — sees the factory's.

use crate::effect::{EffectKind, EffectSpec};
use std::sync::RwLock;

/// A unit's chosen start: its whole spec (values, and a board's pedals or a bank where the
/// unit has them) and the name it is shown with.
#[derive(Clone, Debug, PartialEq)]
pub struct UserStart {
    pub spec: EffectSpec,
    pub label: String,
}

static CHOSEN: RwLock<Vec<UserStart>> = RwLock::new(Vec::new());

/// Whether a kind's start can be chosen: every native unit (a hosted plug-in starts as its
/// maker made it).
pub fn choosable(kind: EffectKind) -> bool {
    kind != EffectKind::External
}

/// The person's start for `kind`, if they chose one.
pub fn get(kind: EffectKind) -> Option<UserStart> {
    let all = CHOSEN.read().unwrap_or_else(|e| e.into_inner());
    all.iter().find(|u| u.spec.kind == kind).cloned()
}

/// Choose `spec` (its kind's) as that kind's start, shown as `label`.
pub fn set(spec: EffectSpec, label: String) {
    if !choosable(spec.kind) {
        return;
    }
    let mut all = CHOSEN.write().unwrap_or_else(|e| e.into_inner());
    all.retain(|u| u.spec.kind != spec.kind);
    all.push(UserStart { spec, label });
}

/// Back to the factory's start for `kind`.
pub fn clear(kind: EffectKind) {
    let mut all = CHOSEN.write().unwrap_or_else(|e| e.into_inner());
    all.retain(|u| u.spec.kind != kind);
}

/// Every choice at once (the settings, read at start): the old ones are forgotten.
pub fn replace_all(starts: impl IntoIterator<Item = UserStart>) {
    let mut v: Vec<UserStart> = Vec::new();
    for u in starts {
        if choosable(u.spec.kind) {
            v.retain(|x| x.spec.kind != u.spec.kind);
            v.push(u);
        }
    }
    *CHOSEN.write().unwrap_or_else(|e| e.into_inner()) = v;
}

/// Every choice, for the settings to keep.
pub fn all() -> Vec<UserStart> {
    CHOSEN.read().unwrap_or_else(|e| e.into_inner()).clone()
}
