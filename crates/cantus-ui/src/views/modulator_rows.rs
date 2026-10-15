//! A modulator's own rows in the Modulators section (ADR 0057): its movement drawn, then
//! its settings in label | value rows — values dragged (one undo step per drag), choices
//! stepped with a click (forward) or a right-click (back), switches as LEDs, lists for
//! whose audio or notes it hears — and its amount.

use super::{Act, ModAct};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::{cache, well, State};
use crate::views::inspector::sheet::{self, Range, Sheet};
use crate::views::modulator::{function_of, paint, ListKind};
use cantus_core::modulator::{
    EnvTrigger, LfoShape, Listen, ModRate, ModSource, Modulator, Retrigger, MAX_STEPS,
};
use cantus_core::MergeKey;
use cantus_dsp::modfx_clock::DIVISIONS;
use egui::{pos2, Id, Rect, Sense};
use std::fmt::Write as _;

/// A preview row's height at 100 %.
const PREVIEW: f32 = 40.0;
/// The step editor's height at 100 %.
const STEPS: f32 = 56.0;

/// A division's name: 1/4, 1/8·, 1/8T (the modulation family's words).
fn division(i: u8, b: &mut String) {
    b.push_str(&crate::i18n::t(&format!("modfx.div.{i}")));
}

fn ms(v: f32, b: &mut String) {
    if v >= 1000.0 {
        let _ = write!(b, "{:.2} s", v / 1000.0);
    } else {
        let _ = write!(b, "{v:.0} ms");
    }
}

fn hz(v: f32, b: &mut String) {
    if v < 1.0 {
        let _ = write!(b, "{v:.2} Hz");
    } else {
        let _ = write!(b, "{v:.1} Hz");
    }
}

/// A dragged value of the modulator: its row, and the edit it makes.
#[allow(clippy::too_many_arguments)]
fn num(
    sh: &mut Sheet<'_>,
    m: &Modulator,
    key: &'static str,
    field: &'static str,
    v: f32,
    range: Range,
    write: impl FnOnce(f32, &mut String),
    set: impl FnOnce(&mut Modulator, f32),
) {
    let f = function_of(m.source.kind());
    let row = sh.row();
    sh.label(row.label, key, m.muted);
    let name = sheet::name(sh.fmt.tr(key));
    let d = sh.value(
        row.value,
        Id::new(("mod-num", m.id, field)),
        v,
        &range,
        false,
        &name,
        write,
        f,
        m.muted,
    );
    if let Some(nv) = d.value {
        let mut e = m.clone();
        set(&mut e, nv);
        if e.is_valid() {
            let k = MergeKey::of(&("mod-field", m.id.0, field));
            sh.acts.push(Act::Mod(ModAct::Set(Box::new(e), Some(k))));
        }
    }
    if d.ended {
        sh.acts.push(Act::EndGesture);
    }
}

/// A choice stepped by clicks: `value` names it; a click goes forward, a right-click back.
fn cycle(
    sh: &mut Sheet<'_>,
    m: &Modulator,
    key: &'static str,
    value: &str,
    step: impl FnOnce(&mut Modulator, i32),
) {
    let row = sh.row();
    sh.label(row.label, key, m.muted);
    let r = sh
        .ui
        .interact(row.value, Id::new(("mod-cycle", m.id, key)), Sense::click());
    let st = State {
        disabled: m.muted,
        ..State::of(&r, false)
    };
    well::menu_field(sh.b, row.value, &sh.s, st, value, true);
    if r.hovered() {
        let name = sheet::name(sh.fmt.tr(key));
        let tip = sheet::name(sh.fmt.tr("mods.cycle_tip"));
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    }
    let dir = if r.clicked() {
        1
    } else if r.secondary_clicked() {
        -1
    } else {
        0
    };
    if dir != 0 {
        let mut e = m.clone();
        step(&mut e, dir);
        if e.is_valid() {
            sh.acts.push(Act::Mod(ModAct::Set(Box::new(e), None)));
        }
    }
}

/// A switch: an LED button with its words across the value column.
fn switch(
    sh: &mut Sheet<'_>,
    m: &Modulator,
    key: &'static str,
    words: &'static str,
    on: bool,
    flip: impl FnOnce(&mut Modulator),
) {
    if sh.led_row(key, words, on, function_of(m.source.kind()), m.muted) {
        let mut e = m.clone();
        flip(&mut e);
        sh.acts.push(Act::Mod(ModAct::Set(Box::new(e), None)));
    }
}

/// A row whose value opens a list (whose audio, whose notes).
fn list(sh: &mut Sheet<'_>, m: &Modulator, key: &'static str, value: &str, kind: ListKind) {
    let row = sh.row();
    sh.label(row.label, key, m.muted);
    let r = sh
        .ui
        .interact(row.value, Id::new(("mod-list", m.id, key)), Sense::click());
    let st = State {
        disabled: m.muted,
        ..State::of(&r, false)
    };
    well::menu_field(sh.b, row.value, &sh.s, st, value, false);
    // Named on the status line like every other row (review 1).
    if r.hovered() {
        let name = sheet::name(sh.fmt.tr(key));
        crate::kit::hint::set_text(
            sh.ui.ctx(),
            &name,
            value,
            crate::kit::hint::Caps {
                menu: true,
                ..crate::kit::hint::Caps::NONE
            },
        );
    }
    if r.clicked() {
        sh.acts.push(Act::Mod(ModAct::List(m.id, kind, row.value)));
    }
}

/// The picture row: the movement over one cycle (or the sequence, or the shape), drawn
/// from the engine's own functions, with where it is now.
fn preview(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    let s = sh.s.function(function_of(m.source.kind()));
    let w = sh.width();
    let (full, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(PREVIEW)), Sense::hover());
    let mg = s.s(sheet::MARGIN);
    let r = Rect::from_min_max(
        pos2(full.left() + mg, full.top() + s.s(2.0)),
        pos2(full.right() - mg, full.bottom() - s.s(2.0)),
    );
    let live = app.engine.modulation().value(m.id);
    let now = live.map(|v| if m.amount > 0.0 { v / m.amount } else { 0.0 });
    let st = State {
        disabled: m.muted,
        ..State::REST
    };
    paint::preview(sh.b, r, &s, st, &m.source, now, s.hue);
}

/// The step editor: click or drag across the bars to draw the steps.
fn steps(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    let ModSource::Step(st) = &m.source else {
        return;
    };
    let s = sh.s.function(function_of(m.source.kind()));
    let w = sh.width();
    let (full, _) = sh
        .ui
        .allocate_exact_size(egui::vec2(w, s.s(STEPS)), Sense::hover());
    let mg = s.s(sheet::MARGIN);
    let r = Rect::from_min_max(
        pos2(full.left() + mg, full.top() + s.s(2.0)),
        pos2(full.right() - mg, full.bottom() - s.s(2.0)),
    );
    let resp = sh
        .ui
        .interact(r, Id::new(("mod-steps", m.id)), Sense::click_and_drag());
    // The step playing now: where the song is, in steps.
    let beats = app.engine.position().0 as f64 / cantus_core::PPQ as f64;
    let len = cantus_dsp::modfx_clock::division_beats(f32::from(st.division));
    let playing = app
        .engine
        .is_playing()
        .then(|| ((beats / len).floor() as i64).rem_euclid(st.steps.len() as i64) as usize);
    paint::steps(sh.b, r, &s, &st.steps, st.unipolar, playing, s.hue);
    if resp.hovered() {
        let name = sheet::name(sh.fmt.tr("mods.steps"));
        let tip = sheet::name(sh.fmt.tr("mods.steps_tip"));
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    }
    if let (true, Some(p)) = (
        resp.clicked() || resp.dragged(),
        resp.interact_pointer_pos(),
    ) {
        let (i, v) = paint::step_at(r, &s, st.steps.len(), p);
        if st.steps.get(i).is_some_and(|x| *x != v) {
            let mut e = m.clone();
            if let ModSource::Step(x) = &mut e.source {
                x.steps[i] = v;
            }
            let k = MergeKey::of(&("mod-steps", m.id.0));
            sh.acts.push(Act::Mod(ModAct::Set(Box::new(e), Some(k))));
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        sh.acts.push(Act::EndGesture);
    }
}

/// A rate: synced (a division, dragged through the table) or free (Hz).
fn rate(sh: &mut Sheet<'_>, m: &Modulator, rate: ModRate, set: fn(&mut Modulator, ModRate)) {
    let synced = matches!(rate, ModRate::Sync { .. });
    switch(sh, m, "mods.sync", "mods.sync_on", synced, move |e| {
        set(
            e,
            if synced {
                ModRate::Hz { hz: 1.0 }
            } else {
                ModRate::Sync {
                    division: cantus_dsp::modfx_clock::DIV_QUARTER as u8,
                }
            },
        )
    });
    match rate {
        ModRate::Sync { division: d } => num(
            sh,
            m,
            "mods.rate",
            "div",
            f32::from(d),
            Range::lin(0.0, (DIVISIONS.len() - 1) as f32, 8.0).stepped(true),
            |v, b| division(v.round() as u8, b),
            move |e, v| {
                set(
                    e,
                    ModRate::Sync {
                        division: v.round() as u8,
                    },
                )
            },
        ),
        ModRate::Hz { hz: h } => num(
            sh,
            m,
            "mods.rate",
            "hz",
            h,
            Range::log(ModRate::MIN_HZ, ModRate::MAX_HZ, 1.0),
            hz,
            move |e, v| set(e, ModRate::Hz { hz: v }),
        ),
    }
}

/// The rows of the modulator's source.
pub fn source(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator) {
    match &m.source {
        ModSource::Lfo(l) => {
            preview(sh, app, m);
            let i = LfoShape::ALL
                .iter()
                .position(|x| *x == l.shape)
                .unwrap_or(0);
            let word = sheet::name(sh.fmt.tr(&format!("modfx.shape.{i}")));
            cycle(sh, m, "mods.shape", &word, |e, d| {
                if let ModSource::Lfo(x) = &mut e.source {
                    let n = LfoShape::ALL.len() as i32;
                    x.shape = LfoShape::ALL[(i as i32 + d).rem_euclid(n) as usize];
                }
            });
            rate(sh, m, l.rate, |e, r| {
                if let ModSource::Lfo(x) = &mut e.source {
                    x.rate = r;
                }
            });
            num(
                sh,
                m,
                "mods.phase",
                "phase",
                l.phase,
                Range::lin(0.0, 1.0, 0.0),
                |v, b| {
                    let _ = write!(b, "{:.0}\u{b0}", v * 360.0);
                },
                |e, v| {
                    if let ModSource::Lfo(x) = &mut e.source {
                        x.phase = v;
                    }
                },
            );
            let note = l.retrigger == Retrigger::Note;
            switch(sh, m, "mods.retrigger", "mods.retrigger_on", note, |e| {
                if let ModSource::Lfo(x) = &mut e.source {
                    x.retrigger = if note {
                        Retrigger::Song
                    } else {
                        Retrigger::Note
                    };
                }
            });
            polarity(sh, m, l.unipolar);
        }
        ModSource::Envelope(env) => {
            preview(sh, app, m);
            let time = |sh: &mut Sheet<'_>, key: &'static str, field: &'static str, v: f32| {
                num(
                    sh,
                    m,
                    key,
                    field,
                    v.max(0.5),
                    Range::log(0.5, 10_000.0, 10.0),
                    ms,
                    move |e, v| {
                        if let ModSource::Envelope(x) = &mut e.source {
                            match field {
                                "a" => x.attack_ms = v,
                                "d" => x.decay_ms = v,
                                _ => x.release_ms = v,
                            }
                        }
                    },
                )
            };
            // Attack, decay, sustain, release: the order every synth reads them in.
            time(sh, "mods.attack", "a", env.attack_ms);
            time(sh, "mods.decay", "d", env.decay_ms);
            num(
                sh,
                m,
                "mods.sustain",
                "s",
                env.sustain,
                Range::lin(0.0, 1.0, 0.5),
                sheet::percent,
                |e, v| {
                    if let ModSource::Envelope(x) = &mut e.source {
                        x.sustain = v;
                    }
                },
            );
            time(sh, "mods.release", "r", env.release_ms);
            let audio = matches!(env.trigger, EnvTrigger::Audio { .. });
            let word = sheet::name(sh.fmt.tr(if audio {
                "mods.trigger.audio"
            } else {
                "mods.trigger.notes"
            }));
            cycle(sh, m, "mods.trigger", &word, |e, _| {
                if let ModSource::Envelope(x) = &mut e.source {
                    x.trigger = if audio {
                        EnvTrigger::Notes { track: None }
                    } else {
                        EnvTrigger::Audio {
                            listen: Listen::Host,
                            threshold_db: -24.0,
                        }
                    };
                }
            });
            match env.trigger {
                EnvTrigger::Notes { track } => {
                    let word = notes_word(sh, app, m, track);
                    list(sh, m, "mods.notes", &word, ListKind::Notes);
                }
                EnvTrigger::Audio {
                    listen,
                    threshold_db,
                } => {
                    let word = listen_word(sh, app, m, listen);
                    list(sh, m, "mods.listen", &word, ListKind::Listen);
                    num(
                        sh,
                        m,
                        "mods.threshold",
                        "thr",
                        threshold_db,
                        Range::lin(-90.0, 0.0, -24.0),
                        sheet::db,
                        |e, v| {
                            if let ModSource::Envelope(x) = &mut e.source {
                                if let EnvTrigger::Audio { threshold_db, .. } = &mut x.trigger {
                                    *threshold_db = v;
                                }
                            }
                        },
                    );
                }
            }
        }
        ModSource::Follower(f) => {
            preview(sh, app, m);
            let word = listen_word(sh, app, m, f.listen);
            list(sh, m, "mods.listen", &word, ListKind::Listen);
            for (key, field, v) in [
                ("mods.attack", "a", f.attack_ms),
                ("mods.release", "r", f.release_ms),
            ] {
                num(
                    sh,
                    m,
                    key,
                    field,
                    v.max(0.5),
                    Range::log(0.5, 5_000.0, 10.0),
                    ms,
                    move |e, v| {
                        if let ModSource::Follower(x) = &mut e.source {
                            if field == "a" {
                                x.attack_ms = v;
                            } else {
                                x.release_ms = v;
                            }
                        }
                    },
                );
            }
            num(
                sh,
                m,
                "mods.gain",
                "gain",
                f.gain_db,
                Range::lin(-24.0, 24.0, 0.0).bipolar(),
                sheet::db,
                |e, v| {
                    if let ModSource::Follower(x) = &mut e.source {
                        x.gain_db = v;
                    }
                },
            );
        }
        ModSource::Step(st) => {
            steps(sh, app, m);
            num(
                sh,
                m,
                "mods.step_count",
                "n",
                st.steps.len() as f32,
                Range::lin(1.0, MAX_STEPS as f32, 8.0).stepped(true),
                |v, b| {
                    let _ = write!(b, "{:.0}", v);
                },
                |e, v| {
                    if let ModSource::Step(x) = &mut e.source {
                        x.steps
                            .resize(v.round().clamp(1.0, MAX_STEPS as f32) as usize, 0.0);
                    }
                },
            );
            num(
                sh,
                m,
                "mods.step_length",
                "div",
                f32::from(st.division),
                Range::lin(0.0, (DIVISIONS.len() - 1) as f32, 14.0).stepped(true),
                |v, b| division(v.round() as u8, b),
                |e, v| {
                    if let ModSource::Step(x) = &mut e.source {
                        x.division = v.round() as u8;
                    }
                },
            );
            num(
                sh,
                m,
                "mods.glide",
                "glide",
                st.glide,
                Range::lin(0.0, 1.0, 0.0),
                sheet::percent,
                |e, v| {
                    if let ModSource::Step(x) = &mut e.source {
                        x.glide = v;
                    }
                },
            );
            polarity(sh, m, st.unipolar);
        }
        ModSource::Random(r) => {
            preview(sh, app, m);
            rate(sh, m, r.rate, |e, rt| {
                if let ModSource::Random(x) = &mut e.source {
                    x.rate = rt;
                }
            });
            let smooth = r.smooth;
            switch(sh, m, "mods.smooth", "mods.smooth_on", smooth, |e| {
                if let ModSource::Random(x) = &mut e.source {
                    x.smooth = !smooth;
                }
            });
            num(
                sh,
                m,
                "mods.seed",
                "seed",
                r.seed as f32,
                Range::lin(0.0, 999.0, 1.0).stepped(true),
                |v, b| {
                    let _ = write!(b, "{:.0}", v);
                },
                |e, v| {
                    if let ModSource::Random(x) = &mut e.source {
                        x.seed = v.round() as u32;
                    }
                },
            );
            polarity(sh, m, r.unipolar);
        }
    }
}

/// The unipolar switch of an LFO, a step sequence, a random source.
fn polarity(sh: &mut Sheet<'_>, m: &Modulator, unipolar: bool) {
    switch(
        sh,
        m,
        "mods.polarity",
        "mods.unipolar",
        unipolar,
        |e| match &mut e.source {
            ModSource::Lfo(x) => x.unipolar = !unipolar,
            ModSource::Step(x) => x.unipolar = !unipolar,
            ModSource::Random(x) => x.unipolar = !unipolar,
            _ => {}
        },
    );
}

/// Whose audio it hears, in words.
fn listen_word(sh: &mut Sheet<'_>, app: &CantusApp, m: &Modulator, l: Listen) -> sheet::NameBuf {
    let text = sh.fmt.get(
        cache::key(("mod-listen", m.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match l {
            Listen::Host => b.push_str(&crate::i18n::t("mods.listen.host")),
            Listen::Master => b.push_str(&crate::i18n::t("kit.mixer.master")),
            // A removed strip: it hears silence (never the whole mix), and says so.
            Listen::Strip(s) if app.project().strip(s).is_none() => {
                b.push_str(&crate::i18n::t("mods.listen.missing"))
            }
            Listen::Strip(s) => b.push_str(
                &crate::automation::target_parts(
                    app.project(),
                    &cantus_core::AutoTarget::StripGain(s),
                )
                .0,
            ),
        },
    );
    sheet::name(text)
}

/// Whose notes it hears, in words.
fn notes_word(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    m: &Modulator,
    t: Option<cantus_core::TrackId>,
) -> sheet::NameBuf {
    let text = sh.fmt.get(
        cache::key(("mod-notes", m.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match t.and_then(|t| app.project().track(t)) {
            Some(tr) => b.push_str(&tr.name),
            None => b.push_str(&crate::i18n::t("mods.notes.host")),
        },
    );
    sheet::name(text)
}
