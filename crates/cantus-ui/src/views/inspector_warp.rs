//! The audio clip's warp in the inspector (round 29, ADR 0061): *Follow the project
//! tempo*, the mode, the transposition (semitones, cents), the formants, Rhythm's slice
//! ends, the source's tempo and what the analysis detected — and the keys: *Detect
//! tempo*, *Quantise audio…*, *Warp from here*, *Set the project tempo from this clip*,
//! *Edit warp*, *Bounce warped audio*, *Reset warp*. Every edit one `SetClipWarp`
//! (merged over a drag).

use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::kit::cap;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, led, Function, State};
use cantus_core::warp::{WarpMode, WarpOp, WarpTail};
use cantus_core::{ClipId, Command, TrackId};
use egui::{Align2, Id, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// What the section asks the application to do.
#[derive(Clone, Debug)]
pub enum WarpAct {
    Mode(ClipRef, WarpMode),
    Follow(ClipRef, bool),
    Formants(ClipRef, bool),
    Tail(ClipRef, WarpTail),
    Detect(ClipRef),
    Quantise(ClipRef),
    FromHere(ClipRef),
    ProjectTempo(ClipRef),
    Edit(ClipRef),
    Bounce(ClipRef),
    Reset(ClipRef),
}

pub fn apply(app: &mut CantusApp, a: WarpAct) {
    match a {
        WarpAct::Mode(r, m) => {
            app.edit_warp(r, WarpOp::Mode, |w| {
                w.mode = m;
                w.follow = true;
            });
        }
        WarpAct::Follow(r, on) => {
            app.set_warp_follow(r, on);
        }
        WarpAct::Formants(r, on) => {
            app.set_warp_formants(r, on);
        }
        WarpAct::Tail(r, t) => {
            app.set_warp_tail(r, t);
        }
        WarpAct::Detect(r) => {
            app.warp_by_analysis(r);
        }
        WarpAct::Quantise(r) => app.open_quantise(vec![r]),
        WarpAct::FromHere(r) => {
            let at = app.engine.position();
            app.warp_from_here(r, at);
        }
        WarpAct::ProjectTempo(r) => {
            app.project_tempo_from_clip(r);
        }
        WarpAct::Edit(r) => app.open_warp_editor(r),
        WarpAct::Bounce(r) => {
            app.bounce_warped(r);
        }
        WarpAct::Reset(r) => {
            app.reset_warp(r);
        }
    }
}

/// A key across the value column (the audio section's look); whether it was clicked.
fn key(sh: &mut Sheet<'_>, id: impl std::hash::Hash + std::fmt::Debug, word: &str, enabled: bool) -> bool {
    let s = sh.s;
    let row = sh.row();
    let w = row.value.width().min(s.s(170.0));
    let br = Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
    let r = sh.ui.interact(br, Id::new(("insp-warp-key", id)), Sense::click());
    let st = State {
        disabled: !enabled,
        ..State::of(&r, false)
    };
    let look = cap::CapLook {
        lit: Some(s.t.selection),
        ..Default::default()
    };
    cap::paint(sh.b, br, &s, st, &look);
    let ink = crate::kit::color::on_fill(cap::face(&s, st, &look));
    let tx = Text::key(word, Role::Control, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(sh.b, br.shrink2(Vec2::new(s.s(6.0), 0.0)), &tx, s.tk());
    if r.hovered() {
        let name = sh.fmt.tr(word).to_string();
        hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
    }
    r.clicked() && enabled
}

/// A row of LED choices across the value column (`labels`, the lit one `on`); the one
/// clicked.
fn choices(sh: &mut Sheet<'_>, label: &str, id: &str, labels: &[&'static str], on: Option<usize>, dim: bool) -> Option<usize> {
    let row = sh.row();
    sh.label(row.label, label, dim);
    let n = labels.len().max(1) as f32;
    let gap = sh.s.s(3.0);
    let w = (row.value.width() - gap * (n - 1.0)) / n;
    let mut hit = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            row.value.min + Vec2::new(i as f32 * (w + gap), 0.0),
            Vec2::new(w, row.value.height()),
        );
        let resp = sh.ui.interact(r, Id::new(("insp-warp-choice", id, i)), Sense::click());
        let st = State {
            disabled: dim,
            ..State::of(&resp, on == Some(i))
        };
        if resp.hovered() {
            let name = sh.fmt.tr(l).to_string();
            hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
        }
        led::button(sh.b, r, &sh.s.function(Function::Source), st, crate::kit::text::Src::Key(l));
        if resp.clicked() && !dim {
            hit = Some(i);
        }
    }
    hit
}

/// The section, under the audio clip's.
pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, c: ClipId) {
    let r: ClipRef = (t, c);
    let Some(w) = app.warp_to_edit(r) else {
        return;
    };
    let warp = app.clip_warp(r).cloned();
    let warped = warp.as_ref().is_some_and(|w| w.follow);
    sh.band("warp.inspector.band", Function::Source, None, false, None);
    if sh.led_row(
        "warp.inspector.follow",
        "warp.inspector.follow_short",
        warped,
        Function::Source,
        false,
    ) {
        sh.acts.push(Act::Warp(WarpAct::Follow(r, !warped)));
    }
    let mode = warped.then_some(super::super::warp::mode_index(w.mode));
    let half = |i: usize| super::super::warp::MODES[i];
    if let Some(i) = choices(sh, "warp.inspector.mode", "mode-a", &[half(0), half(1)], mode.filter(|m| *m < 2), false) {
        sh.acts.push(Act::Warp(WarpAct::Mode(r, WarpMode::ALL[i])));
    }
    if let Some(i) = choices(sh, "warp.inspector.mode_more", "mode-b", &[half(2), half(3)], mode.filter(|m| *m >= 2).map(|m| m - 2), false) {
        sh.acts.push(Act::Warp(WarpAct::Mode(r, WarpMode::ALL[i + 2])));
    }
    // The transposition, in semitones (cents as the fraction).
    let wt = w.clone();
    sh.param_row(
        "warp.inspector.transpose",
        Id::new(("insp-warp-transpose", c)),
        w.cents as f32 / 100.0,
        &Range::lin(-24.0, 24.0, 0.0).bipolar(),
        false,
        |v, b| crate::warp::write_cents(b, (v * 100.0).round() as i32),
        Function::Source,
        !w.mode.transposes(),
        |v| {
            let mut n = wt.clone();
            n.cents = (v * 100.0).round() as i32;
            Act::Merge(
                Command::SetClipWarp {
                    track: t,
                    clip: c,
                    warp: Some(Box::new(n)),
                    op: WarpOp::Transpose,
                },
                crate::warp::merge_key(r, WarpOp::Transpose),
            )
        },
    );
    if sh.led_row(
        "warp.inspector.formants",
        "warp.inspector.formants_on",
        w.formants,
        Function::Source,
        !w.mode.keeps_formants() || w.cents == 0,
    ) {
        sh.acts.push(Act::Warp(WarpAct::Formants(r, !w.formants)));
    }
    if w.mode == WarpMode::Rhythm {
        let tails = ["warp.tail.fade", "warp.tail.loop", "warp.tail.pingpong"];
        let on = WarpTail::ALL.iter().position(|x| *x == w.tail);
        if let Some(i) = choices(sh, "warp.inspector.tail", "tail", &tails, on, false) {
            sh.acts.push(Act::Warp(WarpAct::Tail(r, WarpTail::ALL[i])));
        }
    }
    // The source's tempo.
    let wt = w.clone();
    sh.param_row(
        "warp.inspector.tempo",
        Id::new(("insp-warp-tempo", c)),
        w.tempo() as f32,
        &Range::lin(40.0, 240.0, w.tempo() as f32),
        false,
        |v, b| {
            let _ = write!(b, "{v:.2} BPM");
        },
        Function::Source,
        false,
        |v| {
            Act::Merge(
                Command::SetClipWarp {
                    track: t,
                    clip: c,
                    warp: Some(Box::new(wt.with_tempo(v as f64))),
                    op: WarpOp::Tempo,
                },
                crate::warp::merge_key(r, WarpOp::Tempo),
            )
        },
    );
    // What the analysis found.
    let rev = app.bus.revision();
    let analysis = app.warp_analysis(r);
    let busy = analysis.is_none() && app.warp_analysing(r);
    let found = sheet::name(sh.fmt.get(
        cache::key(("insp-warp-detected", c, busy, analysis.is_some())),
        rev,
        |b| match (&analysis, busy) {
            (Some(a), _) if a.bpm() > 0.0 => {
                let _ = write!(b, "{:.2} BPM", a.bpm());
                if let Some(bars) = a.grid.bars {
                    let _ = write!(b, " · {}", crate::i18n::tf("warp.inspector.bars", &[("n", &bars)]));
                }
                let sure = if a.acid_bpm.is_some() { 1.0 } else { a.grid.confidence };
                let _ = write!(b, " · {:.0} %", sure * 100.0);
            }
            (_, true) => b.push_str(&crate::i18n::t("warp.status.analysing_short")),
            _ => b.push_str(&crate::i18n::t("warp.inspector.none")),
        },
    ));
    sh.text_row("warp.inspector.detected", &found, true, busy);
    let ready = analysis.is_some();
    let keys: [(&str, WarpAct, bool); 7] = [
        ("warp.menu.detect", WarpAct::Detect(r), ready),
        ("warp.menu.quantise", WarpAct::Quantise(r), true),
        ("warp.menu.from_here", WarpAct::FromHere(r), ready),
        ("warp.menu.project_tempo", WarpAct::ProjectTempo(r), ready || warp.is_some()),
        ("warp.menu.edit", WarpAct::Edit(r), true),
        ("warp.menu.bounce", WarpAct::Bounce(r), warp.is_some()),
        ("warp.menu.reset", WarpAct::Reset(r), warp.is_some()),
    ];
    for (word, act, enabled) in keys {
        if key(sh, (c, word), word, enabled) {
            sh.acts.push(Act::Warp(act));
        }
    }
}
