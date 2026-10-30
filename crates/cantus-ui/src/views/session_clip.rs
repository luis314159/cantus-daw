//! A session clip's panel (ADR 0077) — Live's clip view, on the kit: four cards in a row
//! at the foot of the Session pane, each a column of label | value rows.
//!
//! - **Launch**: mode (trigger, gate, toggle, repeat), quantisation, legato, the
//!   velocity amount.
//! - **Loop**: loop on or off, the start marker, where the loop starts and how long it is
//!   (or, without one, where the clip ends) — positions as bar.beat.sixteenth from 1,
//!   lengths from 0, as Live writes them; a drag moves a sixteenth per few points (Shift:
//!   a tick), a double-click resets.
//! - **Follow**: on, action A, the chance of A (B has the rest), action B, after (linked
//!   to the clip, or a time), how many times; a *jump* names its scene.
//! - **Envelopes** (`env`): the clip's own automation over its loop.
//!
//! Every value is a session command; a drag merges into one undo step.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Fill, State, Style};
use cantus_core::session::{
    FollowAction, FollowActions, FollowAfter, LaunchMode, LaunchSettings, Quantize,
    SessionCommand as S, SlotRef, SlotRegion,
};
use cantus_core::{MergeKey, Ticks, TimeSignature, PPQ};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// A card's rows: label | value.
pub(crate) struct Rows {
    pub rect: Rect,
    pub y: f32,
    pub k: f32,
    pub label_w: f32,
}

impl Rows {
    pub fn new(card: Rect, k: f32, label_w: f32) -> Rows {
        Rows {
            rect: card,
            y: card.top() + 24.0 * k,
            k,
            label_w,
        }
    }

    /// The next row: its label's rect and its value's.
    pub fn next(&mut self) -> (Rect, Rect) {
        let k = self.k;
        let h = 22.0 * k;
        let label = Rect::from_min_size(
            pos2(self.rect.left() + 6.0 * k, self.y),
            vec2(self.label_w, h),
        );
        let value = Rect::from_min_max(
            pos2(label.right() + 6.0 * k, self.y + 1.0 * k),
            pos2(self.rect.right() - 6.0 * k, self.y + h - 1.0 * k),
        );
        self.y += h + 3.0 * k;
        (label, value)
    }
}

/// A card's ground and its title band.
pub(crate) fn card(ui: &Ui, s: &Style, r: Rect, key: &str) {
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        b.rrect(r, s.r(3.0), Fill::Solid(s.t.panel_raised));
        b.rrect_stroke(r.shrink(b.px() * 0.5), s.r(3.0), b.px(), s.t.edge);
        let tx = Text {
            caps: true,
            ..Text::key(key, Role::Section, s.t.label)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End))
        };
        let _ = text::paint(
            b,
            Rect::from_min_size(
                r.min + vec2(8.0 * s.k, 2.0 * s.k),
                vec2(r.width() - 16.0 * s.k, 18.0 * s.k),
            ),
            &tx,
            s.tk(),
        );
    });
}

/// A row's label, right-aligned, shortened when it does not fit.
pub(crate) fn label(ui: &Ui, s: &Style, r: Rect, key: &str, dim: bool) {
    with_brush(ui.painter(), |b| {
        let c = if dim {
            s.t.text_dim.gamma_multiply(0.5)
        } else {
            s.t.text_dim
        };
        let tx = Text::key(key, Role::Body, c)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Abbreviate);
        let _ = text::paint(b, r, &tx, s.tk());
    });
}

/// A menu field showing `value`; the index chosen among `options` when one is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn menu(
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    id: Id,
    value: &str,
    options: &[String],
    current: Option<usize>,
    tip: &str,
    dim: bool,
) -> Option<usize> {
    let resp = ui.interact(r, id, Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let st = State {
        disabled: dim,
        ..State::of(&resp, open)
    };
    with_brush(ui.painter(), |b| {
        well::menu_field(b, r, s, st, value, false)
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &t(tip),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let mut picked = None;
    if dim {
        return None;
    }
    egui::Popup::menu(&resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            let items: Vec<Item> = options
                .iter()
                .enumerate()
                .map(|(i, o)| Item::Action {
                    label: Src::Str(o),
                    check: Check::Radio(current == Some(i)),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = crate::kit::widgets::menu_list(ui, &items, r.width().max(140.0 * s.k))
                .1
                .clicked
            {
                picked = Some(i);
                ui.close();
            }
        });
    picked
}

/// An LED switch; `true` when clicked.
pub(crate) fn led(ui: &mut Ui, s: &Style, r: Rect, id: Id, on: bool, key: &str, tip: &str) -> bool {
    let resp = ui.interact(r, id, Sense::click());
    let st = State {
        lit: on,
        ..State::of(&resp, false)
    };
    with_brush(ui.painter(), |b| {
        crate::kit::led::button(b, r, s, st, Src::Key(key))
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), &t(tip), "", Caps::NONE);
    }
    resp.clicked()
}

/// What a dragged value did this frame.
pub(crate) struct Dragged {
    /// How far it moved, in steps (Shift: fine).
    pub steps: i64,
    pub fine: bool,
    pub ended: bool,
    pub reset: bool,
}

/// A value in a well that a drag moves (up and right: more), a double-click resets.
pub(crate) fn drag_value(
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    id: Id,
    value: &str,
    tip: &str,
    dim: bool,
) -> Dragged {
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let st = State {
        disabled: dim,
        ..State::of(&resp, false)
    };
    with_brush(ui.painter(), |b| well::readout(b, r, s, st, value, true));
    if resp.hovered() || resp.dragged() {
        hint::set_text(ui.ctx(), &t(tip), value, Caps::PARAM);
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let fine = ui.input(|i| i.modifiers.shift);
    let mut steps = 0;
    if resp.dragged() && !dim {
        let acc = Id::new(("session-drag-acc", id));
        let d = resp.drag_delta();
        let mut a: f32 = ui.ctx().data(|m| m.get_temp(acc)).unwrap_or(0.0);
        a += d.x - d.y;
        let per = 5.0 * s.k;
        steps = (a / per).trunc() as i64;
        a -= steps as f32 * per;
        ui.ctx().data_mut(|m| m.insert_temp(acc, a));
    }
    Dragged {
        steps,
        fine,
        ended: resp.drag_stopped(),
        reset: resp.double_clicked() && !dim,
    }
}

/// A sixteenth's ticks.
const SIXTEENTH: i64 = PPQ / 4;

/// A position in content ticks as Live writes it: bar.beat.sixteenth, each from 1.
pub fn position_text(t: Ticks, m: TimeSignature) -> String {
    let beat = PPQ * 4 / i64::from(m.denominator.max(1));
    let bar = beat * i64::from(m.numerator.max(1));
    let v = t.0.max(0);
    format!(
        "{}.{}.{}",
        v / bar + 1,
        (v % bar) / beat + 1,
        (v % beat) / SIXTEENTH + 1
    )
}

/// A length as Live writes it: bars.beats.sixteenths, from 0.
pub fn length_text(t: Ticks, m: TimeSignature) -> String {
    let beat = PPQ * 4 / i64::from(m.denominator.max(1));
    let bar = beat * i64::from(m.numerator.max(1));
    let v = t.0.max(0);
    format!(
        "{}.{}.{}",
        v / bar,
        (v % bar) / beat,
        (v % beat) / SIXTEENTH
    )
}

/// The panel of `slot`'s clip in `rect`.
pub fn panel(app: &mut CantusApp, ui: &mut Ui, rect: Rect, slot: SlotRef) {
    let s = Style::current();
    let k = s.k;
    let Some(c) = app.project().slot_clip(slot).cloned() else {
        return;
    };
    with_brush(ui.painter(), |b| {
        b.rect(rect, Fill::Solid(s.t.panel));
        b.hline(
            rect.left(),
            rect.right(),
            rect.top() + b.px(),
            b.px(),
            s.t.edge,
        );
    });
    // A band with the clip's colour and name.
    let band = Rect::from_min_size(rect.min + vec2(0.0, 2.0 * k), vec2(rect.width(), 22.0 * k));
    let colour = app
        .project()
        .track(slot.track)
        .map(|t| crate::theme::track_color(c.clip.color.unwrap_or(t.color)))
        .unwrap_or(s.t.selection);
    with_brush(ui.painter(), |b| {
        b.rrect(
            Rect::from_min_size(band.min + vec2(8.0 * k, 5.0 * k), vec2(12.0 * k, 12.0 * k)),
            s.r(2.0),
            Fill::Solid(colour),
        );
        let name = if c.clip.name.is_empty() {
            t("session.clip.unnamed")
        } else {
            c.clip.name.clone()
        };
        let tx = Text::new(&name, Role::Body, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(
            b,
            Rect::from_min_max(
                pos2(band.left() + 26.0 * k, band.top()),
                pos2(band.left() + 320.0 * k, band.bottom()),
            ),
            &tx,
            s.tk(),
        );
    });
    let body = Rect::from_min_max(
        pos2(rect.left() + 6.0 * k, band.bottom() + 2.0 * k),
        pos2(rect.right() - 6.0 * k, rect.bottom() - 6.0 * k),
    );
    // Each card keeps a width its rows read at; a pane narrower than all four scrolls
    // them sideways rather than squeezing a menu to its arrow or a word to an ellipsis.
    let gap = 6.0 * k;
    let w = 236.0 * k;
    let env_min = 300.0 * k;
    let total = (3.0 * (w + gap) + env_min).max(body.width());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(body)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    child.set_clip_rect(body.intersect(ui.clip_rect()));
    egui::ScrollArea::horizontal()
        .id_salt(("session-clip-panel", slot.track.0, slot.scene.0))
        .auto_shrink([false, false])
        // Seen whenever the cards overflow: the envelopes beyond the edge are not hidden.
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show_viewport(&mut child, |ui, _| {
            let (area, _) = ui.allocate_exact_size(vec2(total, body.height()), Sense::hover());
            let at = |i: usize| {
                Rect::from_min_size(
                    pos2(area.left() + i as f32 * (w + gap), area.top()),
                    vec2(w, area.height() - 8.0 * k),
                )
            };
            let env = Rect::from_min_max(
                pos2(at(3).left(), area.top()),
                pos2(area.right(), area.bottom() - 8.0 * k),
            );
            launch_card(app, ui, &s, at(0), slot, &c.launch);
            loop_card(app, ui, &s, at(1), slot, &c);
            follow_card(app, ui, &s, at(2), slot, &c.follow);
            super::env::card(app, ui, &s, env, slot);
        });
}

fn quant_options() -> Vec<String> {
    Quantize::ALL.iter().map(|q| t(q.label_key())).collect()
}

fn launch_card(
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    slot: SlotRef,
    l: &LaunchSettings,
) {
    card(ui, s, r, "session.clip.launch");
    let k = s.k;
    let mut rows = Rows::new(r, k, 64.0 * k);
    let set = |app: &mut CantusApp, launch: LaunchSettings, merge: bool| {
        let cmd = cantus_core::Command::Session(S::SetSlotLaunch { slot, launch });
        if merge {
            app.exec_merging(
                cmd,
                MergeKey::of(&("session-launch", slot.track.0, slot.scene.0)),
            );
        } else {
            app.exec(cmd);
        }
    };
    let id = |n: &str| Id::new(("session-clip", n, slot.track.0, slot.scene.0));
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.mode", false);
    let modes: Vec<String> = LaunchMode::ALL.iter().map(|m| t(m.label_key())).collect();
    let sel = LaunchMode::ALL.iter().position(|m| *m == l.mode);
    if let Some(i) = menu(
        ui,
        s,
        vr,
        id("mode"),
        &t(l.mode.label_key()),
        &modes,
        sel,
        "session.clip.mode_tip",
        false,
    ) {
        set(
            app,
            LaunchSettings {
                mode: LaunchMode::ALL[i],
                ..*l
            },
            false,
        );
    }
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.quantize", false);
    let q = Quantize::ALL.iter().position(|x| *x == l.quantize);
    if let Some(i) = menu(
        ui,
        s,
        vr,
        id("quant"),
        &t(l.quantize.label_key()),
        &quant_options(),
        q,
        "session.clip.quantize_tip",
        false,
    ) {
        set(
            app,
            LaunchSettings {
                quantize: Quantize::ALL[i],
                ..*l
            },
            false,
        );
    }
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.legato", false);
    if led(
        ui,
        s,
        vr,
        id("legato"),
        l.legato,
        if l.legato {
            "session.on"
        } else {
            "session.off"
        },
        "session.clip.legato_tip",
    ) {
        set(
            app,
            LaunchSettings {
                legato: !l.legato,
                ..*l
            },
            false,
        );
    }
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.velocity", false);
    let text = format!("{:.0} %", l.velocity * 100.0);
    let d = drag_value(
        ui,
        s,
        vr,
        id("vel"),
        &text,
        "session.clip.velocity_tip",
        false,
    );
    if d.steps != 0 || d.reset {
        let v = if d.reset {
            0.0
        } else {
            (l.velocity + d.steps as f32 * 0.01).clamp(0.0, 1.0)
        };
        set(app, LaunchSettings { velocity: v, ..*l }, true);
    }
    if d.ended {
        app.end_gesture();
    }
}

fn loop_card(
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    slot: SlotRef,
    c: &cantus_core::session::SessionClip,
) {
    card(ui, s, r, "session.clip.loop");
    let k = s.k;
    let m = crate::views::bars::map(app.project()).signature_at(Ticks::ZERO);
    let mut rows = Rows::new(r, k, 64.0 * k);
    let region = SlotRegion::of(c);
    let id = |n: &str| Id::new(("session-clip", n, slot.track.0, slot.scene.0));
    let set = |app: &mut CantusApp, region: SlotRegion, merge: bool| {
        let cmd = cantus_core::Command::Session(S::SetSlotRegion { slot, region });
        if merge {
            app.exec_merging(
                cmd,
                MergeKey::of(&("session-region", slot.track.0, slot.scene.0)),
            );
        } else {
            app.exec(cmd);
        }
    };
    let looping = region.loop_length.is_some();
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.loop_on", false);
    if led(
        ui,
        s,
        vr,
        id("loop"),
        looping,
        if looping { "session.on" } else { "session.off" },
        "session.clip.loop_tip",
    ) {
        super::keys::toggle_loop(app);
    }
    let step = |d: &Dragged| if d.fine { d.steps } else { d.steps * SIXTEENTH };
    // The start marker.
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.clip.start", false);
    let start = region.start.unwrap_or(region.offset);
    let d = drag_value(
        ui,
        s,
        vr,
        id("start"),
        &position_text(start, m),
        "session.clip.start_tip",
        false,
    );
    if d.steps != 0 || d.reset {
        let end = region.offset + region.loop_length.unwrap_or(region.length);
        let v = if d.reset {
            region.offset
        } else {
            Ticks((start.0 + step(&d)).clamp(0, end.0 - 1))
        };
        set(
            app,
            SlotRegion {
                start: (v != region.offset).then_some(v),
                ..region
            },
            true,
        );
    }
    if d.ended {
        app.end_gesture();
    }
    // Where the loop (or the clip) starts.
    let (lr, vr) = rows.next();
    label(
        ui,
        s,
        lr,
        if looping {
            "session.clip.loop_start"
        } else {
            "session.clip.clip_start"
        },
        false,
    );
    let d = drag_value(
        ui,
        s,
        vr,
        id("offset"),
        &position_text(region.offset, m),
        "session.clip.loop_start_tip",
        false,
    );
    if d.steps != 0 {
        let v = Ticks((region.offset.0 + step(&d)).max(0));
        let mut next = SlotRegion {
            offset: v,
            ..region
        };
        if next
            .start
            .is_some_and(|x| x >= v + next.loop_length.unwrap_or(next.length))
        {
            next.start = None;
        }
        set(app, next, true);
    }
    if d.ended {
        app.end_gesture();
    }
    // Its length (or, without a loop, where it ends).
    let (lr, vr) = rows.next();
    label(
        ui,
        s,
        lr,
        if looping {
            "session.clip.loop_length"
        } else {
            "session.clip.length"
        },
        false,
    );
    let len = region.loop_length.unwrap_or(region.length);
    let d = drag_value(
        ui,
        s,
        vr,
        id("length"),
        &length_text(len, m),
        "session.clip.length_tip",
        false,
    );
    if d.steps != 0 {
        let v = Ticks((len.0 + step(&d)).max(SIXTEENTH.min(len.0.max(1))));
        let mut next = region;
        if looping {
            next.loop_length = Some(v);
        } else {
            next.length = v;
        }
        if next.start.is_some_and(|x| x >= next.offset + v) {
            next.start = None;
        }
        set(app, next, true);
    }
    if d.ended {
        app.end_gesture();
    }
}

/// The follow actions' kinds as the menu lists them (a jump per scene at the end).
fn action_options(app: &CantusApp) -> (Vec<FollowAction>, Vec<String>) {
    let mut acts: Vec<FollowAction> = FollowAction::KINDS
        .iter()
        .copied()
        .filter(|a| !matches!(a, FollowAction::Jump(_)))
        .collect();
    let mut words: Vec<String> = acts.iter().map(|a| t(a.label_key())).collect();
    for (j, sc) in app.project().session.scenes.iter().enumerate() {
        acts.push(FollowAction::Jump(sc.id));
        let name = if sc.name.is_empty() {
            (j + 1).to_string()
        } else {
            sc.name.clone()
        };
        words.push(crate::i18n::tf(
            "session.follow.jump_to",
            &[("scene", &name)],
        ));
    }
    (acts, words)
}

/// The words of one action (a jump names its scene).
pub(crate) fn action_text(app: &CantusApp, a: FollowAction) -> String {
    match a {
        FollowAction::Jump(id) => {
            let j = app.project().session.scene_index(id);
            let name = app
                .project()
                .session
                .scene(id)
                .map(|s| s.name.clone())
                .filter(|n| !n.is_empty());
            let name = name
                .or_else(|| j.map(|j| (j + 1).to_string()))
                .unwrap_or_else(|| t("auto.target.missing"));
            crate::i18n::tf("session.follow.jump_to", &[("scene", &name)])
        }
        a => t(a.label_key()),
    }
}

/// The follow card; also the scenes' (`follow_rows`).
fn follow_card(
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    slot: SlotRef,
    f: &FollowActions,
) {
    card(ui, s, r, "session.clip.follow");
    let set = move |app: &mut CantusApp, follow: FollowActions, merge: bool| {
        let cmd = cantus_core::Command::Session(S::SetSlotFollow { slot, follow });
        if merge {
            app.exec_merging(
                cmd,
                MergeKey::of(&("session-follow", slot.track.0, slot.scene.0)),
            );
        } else {
            app.exec(cmd);
        }
    };
    follow_rows(app, ui, s, r, ("slot", slot.track.0, slot.scene.0), f, &set);
}

/// The rows of a set of follow actions, a clip's or a scene's.
pub(crate) fn follow_rows(
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    r: Rect,
    key: (&str, u64, u64),
    f: &FollowActions,
    set: &dyn Fn(&mut CantusApp, FollowActions, bool),
) {
    let k = s.k;
    let mut rows = Rows::new(r, k, 64.0 * k);
    let id = |n: &str| Id::new(("session-follow", n, key));
    let dim = !f.on;
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.follow.on", false);
    if led(
        ui,
        s,
        vr,
        id("on"),
        f.on,
        if f.on { "session.on" } else { "session.off" },
        "session.follow.on_tip",
    ) {
        set(app, FollowActions { on: !f.on, ..*f }, false);
    }
    let (acts, words) = action_options(app);
    for (which, key_l) in [(0, "session.follow.a"), (1, "session.follow.b")] {
        let (lr, vr) = rows.next();
        label(ui, s, lr, key_l, dim);
        let cur = if which == 0 { f.a } else { f.b };
        let at = acts.iter().position(|x| *x == cur);
        if let Some(i) = menu(
            ui,
            s,
            vr,
            id(key_l),
            &action_text(app, cur),
            &words,
            at,
            "session.follow.action_tip",
            dim,
        ) {
            let mut n = *f;
            if which == 0 {
                n.a = acts[i];
            } else {
                n.b = acts[i];
            }
            set(app, n, false);
        }
        if which == 0 {
            let (lr, vr) = rows.next();
            label(ui, s, lr, "session.follow.chance", dim);
            let text = format!("{} : {}", f.chance_a, 100 - f.chance_a);
            let d = drag_value(
                ui,
                s,
                vr,
                id("chance"),
                &text,
                "session.follow.chance_tip",
                dim,
            );
            if d.steps != 0 || d.reset {
                let v = if d.reset {
                    100
                } else {
                    (i64::from(f.chance_a) + d.steps).clamp(0, 100) as u8
                };
                set(app, FollowActions { chance_a: v, ..*f }, true);
            }
            if d.ended {
                app.end_gesture();
            }
        }
    }
    // After: linked to the clip, or a time; then how many times.
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.follow.after", dim);
    let linked = f.after == FollowAfter::Linked;
    let afters = vec![t("session.follow.linked"), t("session.follow.time")];
    let now_after = if linked { &afters[0] } else { &afters[1] };
    if let Some(i) = menu(
        ui,
        s,
        vr,
        id("after"),
        now_after,
        &afters,
        Some(usize::from(!linked)),
        "session.follow.after_tip",
        dim,
    ) {
        let after = if i == 0 {
            FollowAfter::Linked
        } else {
            FollowAfter::Time(Ticks::beats(4))
        };
        set(app, FollowActions { after, ..*f }, false);
    }
    let (lr, rest) = rows.next();
    label(ui, s, lr, "session.follow.time", dim || linked);
    if let FollowAfter::Time(tk) = f.after {
        let m = crate::views::bars::map(app.project()).signature_at(Ticks::ZERO);
        let d = drag_value(
            ui,
            s,
            rest,
            id("time"),
            &length_text(tk, m),
            "session.follow.time_tip",
            dim,
        );
        if d.steps != 0 {
            let step = if d.fine { SIXTEENTH } else { PPQ };
            let v = Ticks((tk.0 + d.steps * step).max(SIXTEENTH));
            set(
                app,
                FollowActions {
                    after: FollowAfter::Time(v),
                    ..*f
                },
                true,
            );
        }
        if d.ended {
            app.end_gesture();
        }
    } else {
        with_brush(ui.painter(), |b| {
            well::readout(
                b,
                rest,
                s,
                State {
                    disabled: true,
                    ..State::REST
                },
                &t("session.follow.clip_length"),
                false,
            )
        });
    }
    let (lr, vr) = rows.next();
    label(ui, s, lr, "session.follow.count", dim);
    let d = drag_value(
        ui,
        s,
        vr,
        id("count"),
        &format!("\u{d7} {}", f.count),
        "session.follow.count_tip",
        dim,
    );
    if d.steps != 0 || d.reset {
        let v = if d.reset {
            1
        } else {
            (i64::from(f.count) + d.steps)
                .clamp(1, i64::from(cantus_core::session::MAX_FOLLOW_COUNT)) as u16
        };
        set(app, FollowActions { count: v, ..*f }, true);
    }
    if d.ended {
        app.end_gesture();
    }
}

/// A scene's panel: its name and colour band, the tempo and metre launching it sets (each
/// on or off, a drag sets the value), and its follow actions.
pub fn scene_panel(
    app: &mut CantusApp,
    ui: &mut Ui,
    rect: Rect,
    scene: cantus_core::session::SceneId,
) {
    let s = Style::current();
    let k = s.k;
    let Some(sc) = app.project().session.scene(scene).cloned() else {
        return;
    };
    let j = app.project().session.scene_index(scene).unwrap_or(0);
    with_brush(ui.painter(), |b| {
        b.rect(rect, Fill::Solid(s.t.panel));
        b.hline(
            rect.left(),
            rect.right(),
            rect.top() + b.px(),
            b.px(),
            s.t.edge,
        );
    });
    let band = Rect::from_min_size(rect.min + vec2(0.0, 2.0 * k), vec2(rect.width(), 22.0 * k));
    with_brush(ui.painter(), |b| {
        let c = sc.color.map_or(s.t.text_dim, crate::theme::track_color);
        b.rrect(
            Rect::from_min_size(band.min + vec2(8.0 * k, 5.0 * k), vec2(12.0 * k, 12.0 * k)),
            s.r(2.0),
            Fill::Solid(c),
        );
        let name = if sc.name.is_empty() {
            crate::i18n::tf("session.scene.numbered", &[("n", &(j + 1))])
        } else {
            sc.name.clone()
        };
        let tx = Text::new(&name, Role::Body, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(
            b,
            Rect::from_min_max(
                pos2(band.left() + 26.0 * k, band.top()),
                pos2(band.left() + 320.0 * k, band.bottom()),
            ),
            &tx,
            s.tk(),
        );
    });
    let body = Rect::from_min_max(
        pos2(rect.left() + 6.0 * k, band.bottom() + 2.0 * k),
        pos2(rect.right() - 6.0 * k, rect.bottom() - 6.0 * k),
    );
    let gap = 6.0 * k;
    let w = (236.0 * k).min((body.width() - gap) / 2.0);
    let a = Rect::from_min_size(body.min, vec2(w, body.height()));
    let f = Rect::from_min_size(pos2(a.right() + gap, body.top()), vec2(w, body.height()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let ui = &mut child;
    card(ui, &s, a, "session.scene.title");
    let mut rows = Rows::new(a, k, 64.0 * k);
    let id = |n: &str| Id::new(("session-scene", n, scene.0));
    // Tempo: on or off, then its value.
    let (lr, vr) = rows.next();
    label(ui, &s, lr, "session.scene.tempo_on", false);
    let on = sc.tempo.is_some();
    if led(
        ui,
        &s,
        vr,
        id("tempo-on"),
        on,
        if on { "session.on" } else { "session.off" },
        "session.scene.tempo_tip",
    ) {
        let tempo = if on {
            None
        } else {
            Some(
                crate::views::bars::map(app.project())
                    .bpm_at(super::now(app).0 as f64)
                    .clamp(cantus_core::time::MIN_BPM, cantus_core::time::MAX_BPM),
            )
        };
        super::exec(app, S::SetSceneTempo { id: scene, tempo });
    }
    let (lr, vr) = rows.next();
    label(ui, &s, lr, "session.scene.tempo", !on);
    let bpm = sc.tempo.unwrap_or(app.project().tempo_bpm);
    let d = drag_value(
        ui,
        &s,
        vr,
        id("tempo"),
        &format!("{bpm:.2} BPM"),
        "session.scene.tempo_value_tip",
        !on,
    );
    if d.steps != 0 && on {
        let step = if d.fine { 0.01 } else { 1.0 };
        let v = (bpm + d.steps as f64 * step)
            .clamp(cantus_core::time::MIN_BPM, cantus_core::time::MAX_BPM);
        app.exec_merging(
            cantus_core::Command::Session(S::SetSceneTempo {
                id: scene,
                tempo: Some(v),
            }),
            MergeKey::of(&("session-scene-tempo", scene.0)),
        );
    }
    if d.ended {
        app.end_gesture();
    }
    // Metre: on or off, then its numerator (a drag) over its denominator (a menu).
    let (lr, vr) = rows.next();
    label(ui, &s, lr, "session.scene.metre_on", false);
    let m_on = sc.metre.is_some();
    if led(
        ui,
        &s,
        vr,
        id("metre-on"),
        m_on,
        if m_on { "session.on" } else { "session.off" },
        "session.scene.metre_tip",
    ) {
        let metre = if m_on {
            None
        } else {
            Some(crate::views::bars::map(app.project()).signature_at(super::now(app)))
        };
        super::exec(app, S::SetSceneMetre { id: scene, metre });
    }
    let (lr, vr) = rows.next();
    label(ui, &s, lr, "session.scene.metre", !m_on);
    let m = sc.metre.unwrap_or(app.project().time_signature);
    let half = Rect::from_min_max(vr.min, pos2(vr.center().x - 2.0 * k, vr.bottom()));
    let rest = Rect::from_min_max(pos2(vr.center().x + 2.0 * k, vr.top()), vr.max);
    let d = drag_value(
        ui,
        &s,
        half,
        id("num"),
        &m.numerator.to_string(),
        "session.scene.numerator_tip",
        !m_on,
    );
    if d.steps != 0 && m_on {
        let n = (i64::from(m.numerator) + d.steps).clamp(1, 32) as u8;
        let metre = TimeSignature {
            numerator: n as _,
            ..m
        };
        app.exec_merging(
            cantus_core::Command::Session(S::SetSceneMetre {
                id: scene,
                metre: Some(metre),
            }),
            MergeKey::of(&("session-scene-metre", scene.0)),
        );
    }
    if d.ended {
        app.end_gesture();
    }
    let dens = ["1", "2", "4", "8", "16", "32"];
    let words: Vec<String> = dens.iter().map(|d| format!("/ {d}")).collect();
    let at = dens
        .iter()
        .position(|d| d.parse::<u32>().ok() == Some(u32::from(m.denominator)));
    if let Some(i) = menu(
        ui,
        &s,
        rest,
        id("den"),
        &format!("/ {}", m.denominator),
        &words,
        at,
        "session.scene.denominator_tip",
        !m_on,
    ) {
        let metre = TimeSignature {
            denominator: dens[i].parse().unwrap_or(4),
            ..m
        };
        super::exec(
            app,
            S::SetSceneMetre {
                id: scene,
                metre: Some(metre),
            },
        );
    }
    // Its follow actions.
    card(ui, &s, f, "session.clip.follow");
    let set = move |app: &mut CantusApp, follow: FollowActions, merge: bool| {
        let cmd = cantus_core::Command::Session(S::SetSceneFollow { id: scene, follow });
        if merge {
            app.exec_merging(cmd, MergeKey::of(&("session-scene-follow", scene.0)));
        } else {
            app.exec(cmd);
        }
    };
    follow_rows(app, ui, &s, f, ("scene", scene.0, 0), &sc.follow, &set);
}
