//! The channel sheet's recording rows (ADR 0025), under the input row: the track's device
//! **input** (a list of the device's inputs, with its level along the well's foot while
//! armed), its **monitor** mode, the input **trim**, and **arm**. What does not apply —
//! monitor and trim on a track with no input — is dimmed, never hidden.

use super::sheet::{self, Range, Sheet};
use super::{Act, Subject};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{cache, segmented, well, Fill, Function, State};
use crate::record_session::{input_choices, input_label, monitor_key};
use cantus_core::audio::{MonitorMode, TrackInput};
use cantus_core::{Command, MergeKey, StripId, TrackId};
use egui::{pos2, Id, Rect, Sense};

/// The track the sheet records for: the subject's, or the strip's recording track.
fn track_of(app: &CantusApp, subject: Subject, strip: Option<StripId>) -> Option<TrackId> {
    let p = app.project();
    match subject {
        Subject::Track(t) | Subject::Clip(t, _) | Subject::Notes(t, _) => Some(t),
        Subject::Strip(_) => strip.and_then(|id| {
            p.tracks_on_strip(id)
                .find(|t| t.input.is_some())
                .or_else(|| p.tracks_on_strip(id).next())
                .map(|t| t.id)
        }),
        Subject::Master => None,
    }
}

const MODES: [MonitorMode; 3] = [MonitorMode::Off, MonitorMode::Auto, MonitorMode::On];

pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    let Some(track) = track_of(app, subject, strip).and_then(|t| app.project().track(t)) else {
        return;
    };
    let tid = track.id;
    let s = sh.s.function(Function::Source);
    sh.band("rec.recording", Function::Source, None, false, None);
    let input = track.input.clone();
    let dim = input.is_none();

    // The input: a list, and the level along the well's foot while armed.
    let row = sh.row();
    sh.label(row.label, "rec.audio_in", false);
    let id = Id::new(("insp-rec-input", tid.0));
    let resp = sh.ui.interact(row.value, id, Sense::click());
    let open = egui::Popup::is_id_open(sh.ui.ctx(), egui::Popup::default_response_id(&resp));
    let text = sh.fmt.get(
        cache::key(("insp-rec-input", tid.0)),
        app.bus.revision(),
        |b| match &input {
            Some(i) => input_label(i, b),
            None => b.push_str(&t("rec.no_input_choice")),
        },
    );
    well::menu_field(sh.b, row.value, &s, State::of(&resp, open), text, false);
    if resp.hovered() {
        hint::set_text(
            sh.ui.ctx(),
            crate::views::clips_chrome::word("rec.input_tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if let (Some(i), true) = (&input, track.armed) {
        let level = app.input_level(i);
        let pos = ((cantus_core::linear_to_db(level) + 60.0) / 60.0).clamp(0.0, 1.0);
        let v = row.value;
        let lane = Rect::from_min_max(
            pos2(v.left() + 3.0, v.bottom() - 4.0),
            pos2(v.right() - 3.0, v.bottom() - 2.0),
        );
        if pos > 0.0 {
            let c = if pos > 0.97 {
                s.t.meter_clip
            } else if pos > 0.85 {
                s.t.meter_high
            } else {
                s.t.meter_low
            };
            sh.b.rect(
                Rect::from_min_max(
                    lane.min,
                    pos2(lane.left() + lane.width() * pos, lane.bottom()),
                ),
                Fill::Solid(c),
            );
        }
    }
    let acts = &mut *sh.acts;
    egui::Popup::menu(&resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            if let Some(cmd) = input_list(ui, app, tid, input.as_ref()) {
                acts.push(Act::Exec(cmd));
                ui.close();
            }
        });

    // Monitor: Off | Auto | On.
    let row = sh.row();
    sh.label(row.label, "rec.monitor", dim);
    let resp = sh.ui.interact(
        row.value,
        Id::new(("insp-rec-monitor", tid.0)),
        Sense::click(),
    );
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(row.value, MODES.len(), p.x));
    let selected = input
        .as_ref()
        .and_then(|i| MODES.iter().position(|m| *m == i.monitor))
        .unwrap_or(1);
    let labels = MODES.map(|m| Src::Key(monitor_key(m)));
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
            crate::views::clips_chrome::word("rec.monitor_tip"),
            "",
            Caps::NONE,
        );
    }
    if let (true, Some(i), Some(mut inp)) = (resp.clicked(), hovered, input.clone()) {
        if inp.monitor != MODES[i] {
            inp.monitor = MODES[i];
            sh.acts.push(Act::Exec(Command::SetTrackInput {
                id: tid,
                input: Some(inp),
            }));
        }
    }

    // Trim, before everything (and before the recorder: what is recorded is what is heard).
    let trim = input.as_ref().map(|i| i.trim_db).unwrap_or(0.0);
    let base = input.clone();
    sh.param_row(
        "rec.trim",
        Id::new(("insp-rec-trim", tid.0)),
        trim,
        &Range::lin(-24.0, 24.0, 0.0).bipolar(),
        false,
        sheet::db,
        Function::Source,
        dim,
        move |v| {
            let mut inp = base.unwrap_or_else(|| TrackInput::mono(0));
            inp.trim_db = (v * 10.0).round() / 10.0;
            Act::Merge(
                Command::SetTrackInput {
                    id: tid,
                    input: Some(inp),
                },
                MergeKey::of(&("trim", tid.0)),
            )
        },
    );

    // Arm.
    if sh.led_row(
        "rec.arm_row",
        "rec.armed",
        track.armed,
        Function::Dynamics,
        false,
    ) {
        sh.acts.push(Act::Exec(Command::SetTrackArmed {
            id: tid,
            armed: !track.armed,
        }));
    }
    // Low-latency monitoring (round 29, ADR 0064 §4): where the input is monitored.
    if sh.led_row(
        "freeze.ll.row",
        "freeze.ll.on",
        app.settings.low_latency,
        Function::Dynamics,
        dim,
    ) {
        sh.acts.push(Act::Freeze(
            crate::views::freeze_view::InspAct::LowLatency(!app.settings.low_latency),
        ));
    }
    if app.settings.low_latency && !dim {
        let off = crate::views::freeze_monitor::bypassed(app);
        if !off.is_empty() {
            let text = crate::i18n::tf("freeze.ll.without", &[("names", &off)]);
            sh.text_row("freeze.ll.now", &text, false, false);
        }
    }
}

/// The channel sheet's *Freeze* row (round 29, ADR 0064): under the recording rows, for a
/// track or a strip.
pub fn freeze_row(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    let track = match subject {
        Subject::Track(t) | Subject::Clip(t, _) | Subject::Notes(t, _) => Some(t),
        _ => None,
    };
    let p = app.project();
    let target = match (track, strip) {
        (Some(t), _) => p.freeze_target_of_track(t),
        (None, Some(s)) => Some(cantus_core::FreezeTarget::Strip(s)),
        _ => None,
    };
    let Some(target) = target else {
        return;
    };
    // What holds it: its own freeze, or a frozen strip it feeds — the row says so and a
    // click gives that one back.
    let holder = match (track, target) {
        (Some(t), _) => app.freeze_holding_track(t),
        (None, cantus_core::FreezeTarget::Strip(s)) => app.freeze_holding_strip(s),
        _ => None,
    };
    let key = if holder.is_some() {
        "freeze.frozen"
    } else {
        "freeze.freeze"
    };
    if sh.led_row("freeze.row", key, holder.is_some(), Function::Source, false) {
        sh.acts.push(Act::Freeze(match holder {
            Some(h) => crate::views::freeze_view::InspAct::Unfreeze(h),
            None => crate::views::freeze_view::InspAct::Freeze(target),
        }));
    }
}

/// The input list: *No input*, then each input of the open device; with none open, it
/// says so. Returns the command for the row clicked.
fn input_list(
    ui: &mut egui::Ui,
    app: &CantusApp,
    tid: TrackId,
    current: Option<&TrackInput>,
) -> Option<Command> {
    let channels = app.input_channels();
    let choices = input_choices(channels);
    let words: Vec<String> = choices
        .iter()
        .map(|c| {
            let mut s = String::new();
            input_label(c, &mut s);
            s
        })
        .collect();
    let same = |c: &TrackInput| current.is_some_and(|i| i.first == c.first && i.stereo == c.stereo);
    let mut items = vec![
        Item::Band(Src::Key("rec.input")),
        Item::Action {
            label: Src::Key("rec.no_input_choice"),
            check: Check::Radio(current.is_none()),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    if channels == 0 {
        items.push(Item::Action {
            label: Src::Key("rec.no_device"),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: false,
        });
    }
    for (c, w) in choices.iter().zip(&words) {
        items.push(Item::Action {
            label: Src::Str(w),
            check: Check::Radio(same(c)),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let i = crate::kit::widgets::menu_list(ui, &items, 200.0)
        .1
        .clicked?;
    let first_choice = if channels == 0 { 3 } else { 2 };
    let input = match i {
        1 => None,
        i if i >= first_choice => {
            let mut c = choices.get(i - first_choice)?.clone();
            if let Some(cur) = current {
                c.monitor = cur.monitor;
                c.trim_db = cur.trim_db;
            }
            Some(c)
        }
        _ => return None,
    };
    (input.as_ref() != current).then_some(Command::SetTrackInput { id: tid, input })
}
