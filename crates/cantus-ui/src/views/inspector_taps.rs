//! r40:pads — two rows the inspector sheet drew and the engine lacked (ADR 0118):
//!
//! - **Hear the key**, under each keyed insert of the Sidechain section: a switch that has
//!   the strip play the insert's key in place of its own signal — a compressor its
//!   detector's input, after the key's level and EQ — to tune the key by ear. Monitor state:
//!   never saved, never in a render, one insert at a time.
//! - **Record: Before FX · After FX**, in the Recording section: where the track's takes
//!   are tapped — its input as it arrives (the default) or its strip after the inserts.

use super::sheet::Sheet;
use super::Act;
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{segmented, Function, State};
use cantus_core::{Command, EffectId, RecordTap, StripId, TrackId};
use egui::{Id, Sense};

/// *Hear the key* for insert `effect` on strip `strip`: the des-mixer switch at the row's
/// right, lit while its key is heard. A click turns it on (any other insert's off) or off.
pub fn hear_key_row(sh: &mut Sheet<'_>, app: &CantusApp, strip: StripId, effect: EffectId) {
    let on = app.engine.hear_key() == Some(effect);
    let row = sh.row();
    sh.label(row.label, "pads.hear_key", false);
    let r = sh.ui.interact(
        row.full,
        Id::new(("insp-hear-key", strip.0, effect.0)),
        Sense::click(),
    );
    let st = State::of(&r, on);
    let m = sh.s.s(super::sheet::MARGIN);
    let sw = super::look::switch(
        sh.b,
        &sh.s,
        row.full.right() - m,
        row.full.center().y,
        on,
        st,
    );
    sh.last_led = sw;
    if r.hovered() {
        hint::set_text(
            sh.ui.ctx(),
            crate::views::clips_chrome::word("pads.hear_key_tip"),
            "",
            Caps::NONE,
        );
    }
    if r.clicked() {
        sh.acts
            .push(Act::HearKey(if on { None } else { Some(effect) }));
    }
}

const TAPS: [RecordTap; 2] = RecordTap::ALL;

fn tap_key(t: RecordTap) -> &'static str {
    match t {
        RecordTap::BeforeFx => "pads.rec_before",
        RecordTap::AfterFx => "pads.rec_after",
    }
}

/// *Record: Before FX · After FX* for track `tid`: a segmented control, dimmed (never
/// hidden) on a track with no input.
pub fn record_tap_row(sh: &mut Sheet<'_>, app: &CantusApp, tid: TrackId) {
    let Some(track) = app.project().track(tid) else {
        return;
    };
    let dim = track.input.is_none();
    let now = track.record_tap;
    let s = sh.s.function(Function::Source);
    let row = sh.row();
    sh.label(row.label, "pads.rec_tap", dim);
    let resp = sh
        .ui
        .interact(row.value, Id::new(("insp-rec-tap", tid.0)), Sense::click());
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(row.value, TAPS.len(), p.x));
    let selected = TAPS.iter().position(|t| *t == now).unwrap_or(0);
    let labels = TAPS.map(|t| Src::Key(tap_key(t)));
    let st = State {
        disabled: dim,
        ..State::of(&resp, false)
    };
    segmented::paint(
        sh.b,
        row.value,
        &s,
        st,
        &labels,
        selected,
        hovered.filter(|_| !dim),
    );
    if resp.hovered() {
        hint::set_text(
            sh.ui.ctx(),
            crate::views::clips_chrome::word("pads.rec_tap_tip"),
            "",
            Caps::NONE,
        );
    }
    if let (true, Some(i), false) = (resp.clicked(), hovered, dim) {
        if TAPS[i] != now {
            sh.acts.push(Act::Exec(Command::SetTrackRecordTap {
                id: tid,
                tap: TAPS[i],
            }));
        }
    }
}
