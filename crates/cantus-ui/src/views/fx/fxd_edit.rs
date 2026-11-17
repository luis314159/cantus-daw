//! fxd — what the controllers' windows do (ADRs 0147, 0148; step 3): each gesture one
//! command through the app, one undo step.
//!
//! - **The pad**: a drag sets X and Y as one `Batch` of two `SetModulatorInput` (label
//!   `cmd.fxd_set_input`) merged under the drag's key — so a pass writing automation writes
//!   two lanes, X and Y. *Draw path*: the drag's points over one loop (each point the
//!   drag's progress) become the path on release. *Record path*: while the song plays the
//!   drag is heard as the hand and sampled at the song's beats; on release it becomes the
//!   path over the loop it began in (`XyPath::from_gesture`) — the moves and the path one
//!   step.
//! - **Cards**: the inputs by `SetModulatorInput`, the amount by its own command, the rest
//!   by `SetModulator` — a knob's drag merged into one step.
//! - **Typing** on the formula's display: a draft at a caret; each edit that compiles is
//!   committed under the session's key (one step a session), one that does not stays a
//!   draft (nothing reaches the project or the audio) with its error shown. Enter, Esc or a
//!   click elsewhere ends the session; Esc drops a draft that does not compile (the last
//!   good text, playing all along, is shown again).
//! - **Dials** a, b, c: a vertical drag, one step; a double-click puts one back at 0.5.
//! - **Presets**: the header's well steps through the kind's presets or opens their list;
//!   choosing one is one `SetModulator`.

use super::fxd_window::State;
use super::{fxd_formula, fxd_xy};
use crate::app::CantusApp;
use cantus_core::modulator::xy::{MAX_INERTIA_MS, MAX_PATH};
use cantus_core::modulator::{presets, ModSource, Modulator, XyPath};
use cantus_core::{Command, MergeKey, ModulatorId};
use fxd_xy::PadMode;

/// Points a drawn path keeps at most.
pub const DRAW_POINTS: usize = 64;
/// Points a recorded path takes per beat of its loop.
pub const RECORD_PER_BEAT: f32 = 16.0;

/// A typing session on the formula's display.
#[derive(Debug)]
pub struct Typing {
    /// What is typed, and the caret (a byte offset into it).
    pub draft: String,
    pub caret: usize,
    /// The session's merge key: every text it commits is one undo step.
    pub key: MergeKey,
}

/// The id the formula's display holds the keyboard with.
pub fn display_id() -> egui::Id {
    egui::Id::new("fxd-display")
}

fn modulator(app: &CantusApp, id: ModulatorId) -> Option<Modulator> {
    app.project().modulator(id).map(|(_, _, m)| m.clone())
}

/// Set input `input` of `id` (merged under `key`).
fn input(app: &mut CantusApp, id: ModulatorId, input: u8, value: f32, key: MergeKey) {
    let value = value.clamp(0.0, 1.0);
    app.exec_merging(Command::SetModulatorInput { id, input, value }, key);
}

/// Edit the controller whole (merged under `key` when given).
fn edit(
    app: &mut CantusApp,
    id: ModulatorId,
    key: Option<MergeKey>,
    f: impl FnOnce(&mut Modulator),
) {
    let Some(mut m) = modulator(app, id) else {
        return;
    };
    let before = m.clone();
    f(&mut m);
    if m != before && m.is_valid() {
        app.edit_modulator(m, key);
    }
}

/// The card gesture's key (a new one when none goes on).
fn card_key(app: &mut CantusApp, id: ModulatorId) -> MergeKey {
    let st = &mut app.fx.fxd;
    match st.card {
        Some(k) => k,
        None => {
            let k = st.key("card", id);
            st.card = Some(k);
            k
        }
    }
}

/// A card's gesture ended.
fn card_done(app: &mut CantusApp) {
    app.fx.fxd.card = None;
    app.end_gesture();
}

// ---- the pad --------------------------------------------------------------------------

/// What the pad's face did.
pub fn xy_actions(app: &mut CantusApp, id: ModulatorId, actions: Vec<fxd_xy::Action>) {
    for a in actions {
        match a {
            fxd_xy::Action::Pad(x, y) => pad(app, id, x, y),
            fxd_xy::Action::PadDone => pad_done(app, id),
            fxd_xy::Action::Mode(m) => app.fx.fxd.mode = m,
            fxd_xy::Action::LinksAt(n) => app.fx.fxd.live_xy.links_first = n,
            fxd_xy::Action::Link(e) => link_edit(app, id, e),
            fxd_xy::Action::Card(fxd_art_moved) => xy_card(app, id, fxd_art_moved),
        }
    }
}

/// The hand at (x, y) on the pad.
fn pad(app: &mut CantusApp, id: ModulatorId, x: f32, y: f32) {
    let st = &mut app.fx.fxd;
    let key = match st.drag {
        Some(k) => k,
        None => {
            let k = st.key("pad", id);
            st.drag = Some(k);
            st.samples.clear();
            st.live_xy.stroke.clear();
            st.paused_path = false;
            // Recording over a path that plays: the hand is heard, not the old path (the
            // engine plays a path over the inputs while the song plays) — in the same step.
            if st.mode == PadMode::Record && playing_path(app, id) {
                app.fx.fxd.paused_path = true;
                edit(app, id, Some(k), |m| {
                    if let ModSource::XyPad(x) = &mut m.source {
                        x.play_path = false;
                    }
                });
            }
            k
        }
    };
    let st = &mut app.fx.fxd;
    match st.mode {
        PadMode::Hand => set_xy(app, id, x, y, key),
        PadMode::Draw => {
            let n = st.samples.len();
            st.samples.push((n as f64, x, y));
            st.live_xy.stroke.push([x, y]);
        }
        PadMode::Record => {
            // Heard as the hand while it records; sampled at the song's beats.
            if app.engine.is_playing() {
                let beats = app.engine.position().0 as f64 / cantus_core::PPQ as f64;
                let st = &mut app.fx.fxd;
                st.samples.push((beats, x, y));
                st.live_xy.stroke.push([x, y]);
            }
            set_xy(app, id, x, y, key);
        }
    }
}

/// X and Y as one command (two lanes when a pass writes automation).
fn set_xy(app: &mut CantusApp, id: ModulatorId, x: f32, y: f32, key: MergeKey) {
    let Some(ModSource::XyPad(src)) = app.project().modulator(id).map(|(_, _, m)| &m.source) else {
        return;
    };
    if src.x == x && src.y == y {
        return;
    }
    let commands = vec![
        Command::SetModulatorInput {
            id,
            input: 0,
            value: x,
        },
        Command::SetModulatorInput {
            id,
            input: 1,
            value: y,
        },
    ];
    app.exec_merging(
        Command::Batch {
            label_key: "cmd.fxd_set_input".into(),
            commands,
        },
        key,
    );
}

/// The hand left the pad: a drawn or recorded path is laid; the gesture ends.
fn pad_done(app: &mut CantusApp, id: ModulatorId) {
    let st = &mut app.fx.fxd;
    let key = st.drag.take();
    let samples = std::mem::take(&mut st.samples);
    st.live_xy.stroke.clear();
    let mode = st.mode;
    let paused = std::mem::take(&mut st.paused_path);
    let loop_beats = if st.loop_beats > 0.0 {
        st.loop_beats
    } else {
        4.0
    };
    if let (Some(key), true) = (key, samples.len() >= 2 && mode != PadMode::Hand) {
        let beats = match app.project().modulator(id).map(|(_, _, m)| &m.source) {
            Some(ModSource::XyPad(x)) => x.path.as_ref().map_or(loop_beats, |p| p.beats),
            _ => loop_beats,
        };
        let path = if mode == PadMode::Draw {
            drawn_path(&samples, beats)
        } else {
            recorded_path(&samples, beats)
        };
        let laid = path.is_some();
        if let Some(mut path) = path {
            edit(app, id, Some(key), |m| {
                if let ModSource::XyPad(x) = &mut m.source {
                    path.smooth = x.path.as_ref().is_none_or(|p| p.smooth);
                    x.path = Some(path);
                    x.play_path = true;
                }
            });
            app.fx.fxd.mode = PadMode::Hand;
        }
        if !laid && paused {
            resume_path(app, id, key);
        }
    } else if let (Some(key), true) = (key, paused) {
        resume_path(app, id, key);
    }
    app.fx.fxd.samples = samples;
    app.fx.fxd.samples.clear();
    app.end_gesture();
}

/// Whether the pad's path plays now.
fn playing_path(app: &CantusApp, id: ModulatorId) -> bool {
    matches!(
        app.project().modulator(id).map(|(_, _, m)| &m.source),
        Some(ModSource::XyPad(x)) if x.play_path && x.path.is_some()
    )
}

/// The path a recording paused plays again (nothing was laid over it).
fn resume_path(app: &mut CantusApp, id: ModulatorId, key: MergeKey) {
    edit(app, id, Some(key), |m| {
        if let ModSource::XyPad(x) = &mut m.source {
            x.play_path = true;
        }
    });
}

/// A drawn path: each sample at its share of the drag (`i / n` of the loop), resampled to
/// at most [`DRAW_POINTS`].
pub fn drawn_path(samples: &[(f64, f32, f32)], beats: f32) -> Option<XyPath> {
    let n = samples.len();
    let timed: Vec<(f64, f32, f32)> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| (f64::from(beats) * i as f64 / n as f64, s.1, s.2))
        .collect();
    XyPath::from_gesture(&timed, 0.0, beats, n.clamp(2, DRAW_POINTS))
}

/// A recorded path: the samples at their song beats over the loop the gesture began in
/// (so the path plays back where it was played), [`RECORD_PER_BEAT`] points a beat.
pub fn recorded_path(samples: &[(f64, f32, f32)], beats: f32) -> Option<XyPath> {
    let first = samples.first()?.0;
    let len = f64::from(beats.max(cantus_core::modulator::xy::MIN_PATH_BEATS));
    let start = (first / len).floor() * len;
    let n = ((beats * RECORD_PER_BEAT).round() as usize).clamp(8, MAX_PATH);
    XyPath::from_gesture(samples, start, beats, n)
}

/// A card of the pad's face.
fn xy_card(app: &mut CantusApp, id: ModulatorId, mv: super::fxd_art::Moved) {
    use super::fxd_art::Moved;
    let (card, item, v) = match mv {
        Moved::Done { .. } => return card_done(app),
        Moved::To { card, item, value } => (card, item, value),
    };
    let key = card_key(app, id);
    let on = v >= 0.5;
    match (card, item) {
        (0, 0) => input(app, id, 0, v, key),
        (0, 1) => input(app, id, 1, v, key),
        (0, 2) => edit(app, id, None, |m| {
            if let ModSource::XyPad(x) = &mut m.source {
                x.unipolar = on;
            }
        }),
        (0, _) => edit(app, id, Some(key), |m| m.amount = v.clamp(0.0, 1.0)),
        (1, 0) => edit(app, id, Some(key), |m| {
            if let ModSource::XyPad(x) = &mut m.source {
                x.inertia_ms = v.clamp(0.0, MAX_INERTIA_MS).round();
            }
        }),
        (1, 1) => edit(app, id, None, |m| {
            if let ModSource::XyPad(x) = &mut m.source {
                x.play_path = on;
            }
        }),
        (1, 2) => {
            // Quarter beats; a pad with no path yet keeps it for the next one drawn.
            let q = ((v * 4.0).round() / 4.0).clamp(0.25, 64.0);
            app.fx.fxd.loop_beats = q;
            edit(app, id, Some(key), |m| {
                if let ModSource::XyPad(x) = &mut m.source {
                    if let Some(p) = &mut x.path {
                        p.beats = q;
                    }
                }
            });
        }
        _ => edit(app, id, None, |m| {
            if let ModSource::XyPad(x) = &mut m.source {
                if let Some(p) = &mut x.path {
                    p.smooth = on;
                }
            }
        }),
    }
}

// ---- the links ------------------------------------------------------------------------

/// A row of *What it drives* edited: route `i`'s axis or range, written into the source's
/// `links` (one per target; a link that reads X whole is no link). A range's drag is one
/// undo step (merged as a card's), an axis or a cleared range one each.
fn link_edit(app: &mut CantusApp, id: ModulatorId, e: fxd_xy::LinkEdit) {
    use cantus_core::modulator::RouteLink;
    use fxd_xy::LinkEdit;
    let (i, key) = match e {
        LinkEdit::Done => return card_done(app),
        LinkEdit::Range(i, _) => (i, Some(card_key(app, id))),
        LinkEdit::Axis(i) | LinkEdit::Clear(i) => (i, None),
    };
    edit(app, id, key, |m| {
        let Some(target) = m.routes.get(i).map(|r| r.target.clone()) else {
            return;
        };
        let (links, axes) = match &mut m.source {
            ModSource::XyPad(x) => (&mut x.links, 2u8),
            ModSource::Formula(f) => (&mut f.links, 1u8),
            _ => return,
        };
        let at = match links.iter().position(|l| l.target == target) {
            Some(at) => at,
            None => {
                links.push(RouteLink {
                    target,
                    output: 0,
                    range: None,
                });
                links.len() - 1
            }
        };
        let l = &mut links[at];
        match e {
            LinkEdit::Axis(_) if axes > 1 => l.output = 1 - l.output.min(1),
            LinkEdit::Range(_, g) => l.range = Some(g),
            LinkEdit::Clear(_) => l.range = None,
            _ => {}
        }
        if l.output == 0 && l.range.is_none() {
            links.remove(at);
        }
    });
}

// ---- the formula ----------------------------------------------------------------------

/// What the formula's face did.
pub fn formula_actions(
    app: &mut CantusApp,
    ctx: &egui::Context,
    id: ModulatorId,
    actions: Vec<fxd_formula::Action>,
) {
    for a in actions {
        match a {
            fxd_formula::Action::Focus(n) => {
                begin_typing(app, id);
                if let Some(t) = &mut app.fx.fxd.typing {
                    t.caret = t.draft.char_indices().nth(n).map_or(t.draft.len(), |c| c.0);
                }
            }
            fxd_formula::Action::Type(word) => {
                begin_typing(app, id);
                if let Some(t) = &mut app.fx.fxd.typing {
                    t.draft.insert_str(t.caret, word);
                    t.caret += word.len();
                }
                commit_draft(app, id);
            }
            fxd_formula::Action::Dial(i, d) => dial(app, id, i, d),
            fxd_formula::Action::DialDone(_) => {
                app.fx.fxd.dial = None;
                app.end_gesture();
            }
            fxd_formula::Action::DialReset(i) => {
                app.fx.fxd.dial = None;
                app.exec(Command::SetModulatorInput {
                    id,
                    input: i as u8,
                    value: 0.5,
                });
            }
            fxd_formula::Action::LinksAt(n) => app.fx.fxd.live_f.links_first = n,
            fxd_formula::Action::Link(e) => link_edit(app, id, e),
            fxd_formula::Action::Card(mv) => formula_card(app, id, mv),
        }
    }
    if app.fx.fxd.typing.is_some() {
        hold_keyboard(ctx);
    }
    typing_live(&mut app.fx.fxd);
}

/// A dial turned by `d` of its travel.
fn dial(app: &mut CantusApp, id: ModulatorId, i: usize, d: f32) {
    let Some(ModSource::Formula(f)) = app.project().modulator(id).map(|(_, _, m)| &m.source) else {
        return;
    };
    let now = f.inputs[i.min(2)];
    let st = &mut app.fx.fxd;
    let (v, key) = match st.dial {
        Some((j, v, k)) if j == i => (v + d, k),
        _ => (now + d, st.key("dial", id)),
    };
    let v = v.clamp(0.0, 1.0);
    st.dial = Some((i, v, key));
    input(app, id, i as u8, v, key);
}

/// A card of the formula's face.
fn formula_card(app: &mut CantusApp, id: ModulatorId, mv: super::fxd_art::Moved) {
    use super::fxd_art::Moved;
    let (card, item, v) = match mv {
        Moved::Done { .. } => return card_done(app),
        Moved::To { card, item, value } => (card, item, value),
    };
    let key = card_key(app, id);
    match (card, item) {
        (0, i) => input(app, id, i.min(2) as u8, v, key),
        (_, 0) => edit(app, id, None, |m| {
            if let ModSource::Formula(f) = &mut m.source {
                f.bipolar = v >= 0.5;
            }
        }),
        _ => edit(app, id, Some(key), |m| m.amount = v.clamp(0.0, 1.0)),
    }
}

/// A typing session begins (when none goes on): the formula's text, the caret at its end.
fn begin_typing(app: &mut CantusApp, id: ModulatorId) {
    if app.fx.fxd.typing.is_some() {
        return;
    }
    let Some(ModSource::Formula(f)) = app.project().modulator(id).map(|(_, _, m)| &m.source) else {
        return;
    };
    let draft = f.text.clone();
    let st = &mut app.fx.fxd;
    let key = st.key("type", id);
    st.typing = Some(Typing {
        caret: draft.len(),
        draft,
        key,
    });
}

/// The draft after an edit: committed when it compiles and differs from what plays (merged
/// into the session's step); kept as a draft, its error shown, when it does not compile.
fn commit_draft(app: &mut CantusApp, id: ModulatorId) {
    let Some(t) = &app.fx.fxd.typing else {
        return;
    };
    if cantus_core::modulator::formula::Program::compile(&t.draft).is_err() {
        return;
    }
    let (text, key) = (t.draft.clone(), t.key);
    edit(app, id, Some(key), |m| {
        if let ModSource::Formula(f) = &mut m.source {
            f.text = text;
        }
    });
}

/// End the typing session: what compiled is kept, a draft that does not is dropped (the
/// last good text shows again). `ctx` lets the display give the keyboard back.
pub fn end_typing(app: &mut CantusApp, ctx: Option<&egui::Context>) {
    if app.fx.fxd.typing.take().is_some() {
        app.end_gesture();
        if let Some(ctx) = ctx {
            ctx.memory_mut(|m| m.surrender_focus(display_id()));
        }
    }
    typing_live(&mut app.fx.fxd);
}

/// What the face shows of the session.
pub fn typing_live(st: &mut State) {
    match &st.typing {
        Some(t) => {
            st.live_f
                .draft
                .get_or_insert_with(String::new)
                .clone_from(&t.draft);
            st.live_f.caret = Some(t.caret);
            st.live_f.focused = true;
        }
        None => {
            st.live_f.draft = None;
            st.live_f.caret = None;
            st.live_f.focused = false;
        }
    }
}

/// The display holds the keyboard: the app's shortcuts wait, and Esc, Tab and the arrows
/// stay the session's (egui would let the focus go on Esc before the session reads it).
fn hold_keyboard(ctx: &egui::Context) {
    let id = display_id();
    ctx.memory_mut(|m| {
        m.request_focus(id);
        m.set_focus_lock_filter(
            id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            },
        );
    });
}

/// The char before byte `at` of `s`, its length in bytes (0 at the start).
fn before(s: &str, at: usize) -> usize {
    s[..at].chars().next_back().map_or(0, char::len_utf8)
}

/// The char at byte `at` of `s`, its length in bytes (0 at the end).
fn after(s: &str, at: usize) -> usize {
    s[at..].chars().next().map_or(0, char::len_utf8)
}

/// The keyboard while a session types: the typed text, Backspace, Delete, the arrows,
/// Home and End at the caret; Enter and Esc end it — and a press outside the display, the
/// keypad and the dials ends it too.
pub fn keys(app: &mut CantusApp, ctx: &egui::Context, id: ModulatorId) {
    if app.fx.fxd.typing.is_none() {
        return;
    }
    let r = &app.fx.fxd.rects_f;
    let inside = |p: egui::Pos2| {
        r.display.contains(p)
            || r.keys.iter().any(|k| k.contains(p))
            || r.dials.iter().any(|d| d.contains(p))
    };
    let away = ctx
        .input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !inside(p)));
    if away {
        return end_typing(app, Some(ctx));
    }
    let events = ctx.input(|i| i.events.clone());
    // Esc is the session's: nothing after reads it (the window's own close among them).
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut edited = false;
    let mut end = false;
    if let Some(t) = &mut app.fx.fxd.typing {
        for e in &events {
            match e {
                egui::Event::Text(s) | egui::Event::Paste(s) => {
                    let s: String = s.chars().filter(|c| !c.is_control()).collect();
                    if !s.is_empty() {
                        t.draft.insert_str(t.caret, &s);
                        t.caret += s.len();
                        edited = true;
                    }
                }
                egui::Event::Key {
                    key, pressed: true, ..
                } => match key {
                    egui::Key::Backspace => {
                        let n = before(&t.draft, t.caret);
                        if n > 0 {
                            t.caret -= n;
                            t.draft.replace_range(t.caret..t.caret + n, "");
                            edited = true;
                        }
                    }
                    egui::Key::Delete => {
                        let n = after(&t.draft, t.caret);
                        if n > 0 {
                            t.draft.replace_range(t.caret..t.caret + n, "");
                            edited = true;
                        }
                    }
                    egui::Key::ArrowLeft => t.caret -= before(&t.draft, t.caret),
                    egui::Key::ArrowRight => t.caret += after(&t.draft, t.caret),
                    egui::Key::Home => t.caret = 0,
                    egui::Key::End => t.caret = t.draft.len(),
                    egui::Key::Enter | egui::Key::Escape => end = true,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    if edited {
        commit_draft(app, id);
    }
    if end {
        end_typing(app, Some(ctx));
    } else {
        hold_keyboard(ctx);
        typing_live(&mut app.fx.fxd);
    }
}

// ---- the frame: on, presets ---------------------------------------------------------------

/// The header's lamp: the modulator on or off (`muted`), one step.
pub fn toggle_on(app: &mut CantusApp, id: ModulatorId) {
    edit(app, id, None, |m| m.muted = !m.muted);
}

/// Apply preset `p` to the controller: one `SetModulator`, one step.
/// The colourways a face offers, in `look`'s order.
pub fn look_keys(formula: bool) -> [&'static str; 3] {
    if formula {
        [
            "fxd.formula.look.bakelite",
            "fxd.formula.look.ivory",
            "fxd.formula.look.ebony",
        ]
    } else {
        [
            "fxd.xy.look.titanium",
            "fxd.xy.look.copper",
            "fxd.xy.look.frost",
        ]
    }
}

/// The ⋯ list: the face's colourways (the face's own `look`, one undo step a choice).
pub fn looks_menu(app: &mut CantusApp, ctx: &egui::Context, id: ModulatorId, formula: bool) {
    use crate::kit::menu;
    use crate::kit::text::Src;
    let Some((at, opener)) = app.fx.fxd.looks_menu else {
        app.fx.fxd.looks_rows.clear();
        return;
    };
    let Some(m) = app.project().modulator(id).map(|(_, _, m)| m) else {
        return;
    };
    let look = match &m.source {
        ModSource::XyPad(x) => x.look,
        ModSource::Formula(f) => f.look,
        _ => 0,
    };
    let keys = look_keys(formula);
    let items: Vec<menu::Item<'_>> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| super::frame::radio(Src::Key(k), usize::from(look) == i))
        .collect();
    let shell = super::fxd::shell_of(formula, look);
    let s = crate::kit::Style::plain()
        .scaled(shell.scale(ctx))
        .function(crate::kit::tokens::Function::Mod)
        .skinned(shell.skin);
    let width = menu::width(ctx, &items, &s).max(180.0 * s.k);
    let h = menu::height(&items, s.d(1.0));
    let mut hovered = None;
    let mut clicked = None;
    let mut rows = std::mem::take(&mut app.fx.fxd.looks_rows);
    rows.clear();
    let area = egui::Area::new(egui::Id::new(("fxd-looks", id.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, h), egui::Sense::click());
            for i in 0..items.len() {
                rows.push(menu::row(rect, &items, s.d(1.0), i));
            }
            hovered = resp
                .hover_pos()
                .and_then(|p| (0..items.len()).find(|&i| rows[i].contains(p)));
            if resp.clicked() {
                clicked = hovered;
            }
            crate::kit::with_brush(ui.painter(), |b| menu::paint(b, rect, &s, &items, hovered));
        });
    app.fx.fxd.looks_rows = rows;
    let inside = area.response.rect;
    let away = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !inside.contains(p) && !opener.contains(p))
    }) || ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if let Some(i) = clicked {
        app.fx.fxd.looks_menu = None;
        let to = i as u8;
        edit(app, id, None, |m| match &mut m.source {
            ModSource::XyPad(x) => x.look = to,
            ModSource::Formula(f) => f.look = to,
            _ => {}
        });
    } else if away {
        app.fx.fxd.looks_menu = None;
    }
}

pub fn apply_preset(app: &mut CantusApp, id: ModulatorId, p: &presets::ControllerPreset) {
    let Some(m) = modulator(app, id) else {
        return;
    };
    let next = presets::apply(&m, p);
    if next != m && next.is_valid() {
        app.edit_modulator(next, None);
    }
}

/// The well's arrows: the next or the previous preset of the kind (from the one the
/// settings match; from the ends when they match none).
pub fn step_preset(app: &mut CantusApp, id: ModulatorId, dir: i32) {
    let Some(m) = modulator(app, id) else {
        return;
    };
    let list: Vec<&'static presets::ControllerPreset> = presets::of_kind(m.source.kind()).collect();
    if list.is_empty() {
        return;
    }
    let n = list.len() as i32;
    let at = presets::matching(&m)
        .and_then(|p| list.iter().position(|q| q.id == p.id))
        .map_or(if dir > 0 { 0 } else { n - 1 }, |i| {
            (i as i32 + dir).rem_euclid(n)
        });
    apply_preset(app, id, list[at as usize]);
}

/// The preset list under the header's well while it is open: each preset by name (its
/// description on the help line when pointed at), the matching one marked; a click
/// applies one, a press elsewhere or Esc closes it.
pub fn preset_menu(app: &mut CantusApp, ctx: &egui::Context, id: ModulatorId, formula: bool) {
    use crate::kit::menu;
    use crate::kit::text::Src;
    let Some((at, opener)) = app.fx.fxd.menu else {
        app.fx.fxd.menu_rows.clear();
        return;
    };
    let Some(m) = app.project().modulator(id).map(|(_, _, m)| m) else {
        return;
    };
    let list: Vec<&'static presets::ControllerPreset> = presets::of_kind(m.source.kind()).collect();
    let now = presets::matching(m).map(|p| p.id);
    let keys: Vec<String> = list
        .iter()
        .map(|p| format!("fxd.preset.{}", p.id))
        .collect();
    let items: Vec<menu::Item<'_>> = keys
        .iter()
        .zip(&list)
        .map(|(k, p)| super::frame::radio(Src::Key(k), now == Some(p.id)))
        .collect();
    let look = match &m.source {
        ModSource::XyPad(x) => x.look,
        ModSource::Formula(f) => f.look,
        _ => 0,
    };
    let shell = super::fxd::shell_of(formula, look);
    let s = crate::kit::Style::plain()
        .scaled(shell.scale(ctx))
        .function(crate::kit::tokens::Function::Mod)
        .skinned(shell.skin);
    let width = menu::width(ctx, &items, &s).max(opener.width());
    let h = menu::height(&items, s.d(1.0));
    let mut hovered = None;
    let mut clicked = None;
    let mut rows = std::mem::take(&mut app.fx.fxd.menu_rows);
    rows.clear();
    let area = egui::Area::new(egui::Id::new(("fxd-presets", id.0)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, h), egui::Sense::click());
            for i in 0..items.len() {
                rows.push(menu::row(rect, &items, s.d(1.0), i));
            }
            hovered = resp
                .hover_pos()
                .and_then(|p| (0..items.len()).find(|&i| rows[i].contains(p)));
            if resp.clicked() {
                clicked = hovered;
            }
            crate::kit::with_brush(ui.painter(), |b| menu::paint(b, rect, &s, &items, hovered));
        });
    app.fx.fxd.menu_rows = rows;
    if let Some(i) = hovered {
        let name = crate::i18n::t(&keys[i]);
        let what = crate::i18n::t(&format!("{}.what", keys[i]));
        crate::kit::hint::set_text(ctx, &name, &what, crate::kit::hint::Caps::NONE);
    }
    let inside = area.response.rect;
    let away = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !inside.contains(p) && !opener.contains(p))
    }) || ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if let Some(i) = clicked {
        app.fx.fxd.menu = None;
        apply_preset(app, id, list[i]);
    } else if away {
        app.fx.fxd.menu = None;
    }
}
