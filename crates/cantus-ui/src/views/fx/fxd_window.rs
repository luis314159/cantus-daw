//! fxd — the controllers' windows in the app (ADRs 0147, 0148; step 3): Tactus (the X-Y
//! pad) and Calculus (the formula controller) opened from their modulator's row in the
//! inspector (its *Open* key, or a double-click on its picture) as a floating device
//! window like an effect's editor — dragged by its header, closed by its key or Esc,
//! minimised to the tray and maximised through the workspace's device registry, scaled
//! from the shell's grip. One controller window at a time (`State::open`); it closes when
//! its modulator is gone (an undo, a removal).
//!
//! Every edit is one command through the app (`exec`, `exec_merging`, `edit_modulator`):
//! a drag, a typing session, a dial's turn is one undo step (`fxd_edit`). The puck comes
//! from the engine's telemetry (`ModTelemetry::pad`), or from `State::injected` in tests
//! and pictures; its trail is a fixed ring, so a frame at rest allocates nothing more.

use super::fxd_edit;
use super::{frame, fxd, fxd_formula, fxd_xy};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::shell::{self, HeaderRects};
use crate::kit::tokens::Function;
use crate::views::workspace_device as device;
use cantus_core::modulator::{presets, ModHost, ModSource, Modulator};
use cantus_core::{MergeKey, ModulatorId};
use egui::{Pos2, Rect, Vec2};

/// Points the inertia trail keeps at most.
pub const TRAIL: usize = 64;

/// The controllers' window: which is open, where, and what a gesture on it holds.
#[derive(Debug, Default)]
pub struct State {
    /// The modulator whose window is open.
    pub open: Option<ModulatorId>,
    /// Bring the window to the front on its next frame (opened again from its row).
    pub raise: bool,
    /// Where it floats (every controller's window opens where the last was).
    pub pos: Option<Pos2>,
    /// The puck (where it is, where it heads) injected by tests and pictures, used instead
    /// of the engine's telemetry while set.
    pub injected: Option<([f32; 2], [f32; 2])>,
    /// What a drag on the pad does now.
    pub mode: fxd_xy::PadMode,
    /// The loop a path is drawn over while the pad has none yet (the Loop card's value).
    pub loop_beats: f32,
    /// The trail: a ring of the puck's last places, `trail_len` of them ending at
    /// `trail_head`.
    trail: Vec<[f32; 2]>,
    trail_len: usize,
    trail_head: usize,
    /// What the faces show (kept from frame to frame: their vectors are reused).
    pub live_xy: fxd_xy::Live,
    pub live_f: fxd_formula::Live,
    /// Gestures begun, for their merge keys: each drag, typing session, dial turn its own.
    pub(crate) gestures: u64,
    /// The pad's drag going on: its merge key, and its samples (song beat or progress,
    /// x, y) while drawing or recording a path.
    pub(crate) drag: Option<MergeKey>,
    pub(crate) samples: Vec<(f64, f32, f32)>,
    /// A recording drag switched the playing path off so the hand is heard over it (it is
    /// switched back on with the new path, or as it was when nothing was laid).
    pub(crate) paused_path: bool,
    /// A card's control being dragged: its merge key.
    pub(crate) card: Option<MergeKey>,
    /// The typing session on the formula's display.
    pub(crate) typing: Option<fxd_edit::Typing>,
    /// A dial being turned: which, where it is, and its merge key.
    pub(crate) dial: Option<(usize, f32, MergeKey)>,
    /// The preset list, open under the header's well: where, and the well that opened it.
    pub(crate) menu: Option<(Pos2, Rect)>,
    /// Where the window's parts were drawn last frame (tests drive them).
    pub rects_xy: fxd_xy::Rects,
    pub rects_f: fxd_formula::Rects,
    pub header: Option<HeaderRects>,
    /// The preset list's rows, while it is open.
    pub menu_rows: Vec<Rect>,
    /// The window's words, kept until the project or the language changes: the context,
    /// the links, the chip, the preset's name.
    words: Option<Words>,
    /// The designed size at 100 % for (formula, language).
    designed: Option<(bool, crate::i18n::Locale, u32, Vec2)>,
}

#[derive(Debug)]
struct Words {
    rev: u64,
    lang: crate::i18n::Locale,
    id: ModulatorId,
    context: String,
    links: Vec<fxd_xy::Link>,
    chip: String,
    preset: String,
}

impl State {
    /// The trail, oldest first, into `out` (cleared; its room reused).
    fn trail_into(&self, out: &mut Vec<[f32; 2]>) {
        out.clear();
        for i in 0..self.trail_len {
            let at = (self.trail_head + TRAIL - self.trail_len + 1 + i) % TRAIL;
            out.push(self.trail[at]);
        }
    }

    /// The puck now: a new place joins the trail; still, the trail's oldest fades away.
    fn note_puck(&mut self, p: [f32; 2]) {
        let last = (self.trail_len > 0).then(|| self.trail[self.trail_head]);
        let moved = last.is_none_or(|l| (l[0] - p[0]).abs() + (l[1] - p[1]).abs() > 1e-4);
        if self.trail.len() != TRAIL {
            // Its room, once: a fixed ring from then on.
            self.trail.resize(TRAIL, [0.0; 2]);
        }
        if moved {
            self.trail_head = (self.trail_head + 1) % TRAIL;
            self.trail[self.trail_head] = p;
            self.trail_len = (self.trail_len + 1).min(TRAIL);
        } else if self.trail_len > 1 {
            self.trail_len -= 1;
        } else {
            self.trail_len = 0;
        }
    }

    /// A gesture begins: its own merge key.
    pub(crate) fn key(&mut self, what: &str, id: ModulatorId) -> MergeKey {
        self.gestures += 1;
        MergeKey::of(&("fxd", what, id.0, self.gestures))
    }

    /// Forget what the window held for another modulator (a new one opened).
    fn reset(&mut self) {
        self.trail_len = 0;
        self.drag = None;
        self.samples.clear();
        self.card = None;
        self.typing = None;
        self.dial = None;
        self.menu = None;
        self.words = None;
        self.mode = fxd_xy::PadMode::Hand;
        self.live_xy.links_first = 0;
        self.live_f.links_first = 0;
    }
}

/// The device key of the controllers' window for `id`.
pub fn device_id(id: ModulatorId) -> egui::Id {
    egui::Id::new(("fxd-window", id.0))
}

impl CantusApp {
    /// Open modulator `id`'s window (a pad's or a formula's), or bring it to the front.
    pub fn open_controller_window(&mut self, id: ModulatorId) {
        let is_controller = self
            .project()
            .modulator(id)
            .is_some_and(|(_, _, m)| is_controller(m));
        if !is_controller {
            return;
        }
        if self.fx.fxd.open != Some(id) {
            self.close_controller_window();
            self.fx.fxd.open = Some(id);
        }
        self.fx.fxd.raise = true;
    }

    /// Close the controllers' window (a typing session's last good text stays).
    pub fn close_controller_window(&mut self) {
        if self.fx.fxd.open.take().is_some() {
            fxd_edit::end_typing(self, None);
            self.end_gesture();
        }
        self.fx.fxd.reset();
    }
}

/// A modulator with a window of its own.
pub fn is_controller(m: &Modulator) -> bool {
    matches!(m.source, ModSource::XyPad(_) | ModSource::Formula(_))
}

/// How the host a modulator lives on is named: its track, its strip, the master.
fn host_words(app: &CantusApp, host: ModHost) -> String {
    let p = app.project();
    match host {
        ModHost::Track(tr) => p.track(tr).map(|x| x.name.clone()).unwrap_or_default(),
        ModHost::Strip(s) => crate::routing_actions::strip_name(p, s),
        ModHost::Master => t("mixer.master"),
    }
}

/// The words of the window for `id`, again when the project or the language changed.
fn refresh_words(app: &mut CantusApp, id: ModulatorId) {
    let rev = app.bus.revision();
    let lang = crate::i18n::locale();
    if app
        .fx
        .fxd
        .words
        .as_ref()
        .is_some_and(|w| w.rev == rev && w.lang == lang && w.id == id)
    {
        return;
    }
    let Some((host, _, m)) = app.project().modulator(id) else {
        return;
    };
    let links_of: &[cantus_core::modulator::RouteLink] = match &m.source {
        ModSource::XyPad(x) => &x.links,
        ModSource::Formula(f) => &f.links,
        _ => &[],
    };
    let links: Vec<fxd_xy::Link> = m
        .routes
        .iter()
        .map(|r| {
            let link = links_of.iter().find(|l| l.target == r.target);
            fxd_xy::Link {
                name: app.target_name(&r.target),
                axis: link.map_or(0, |l| l.output),
                range: link.and_then(|l| l.range),
                depth: r.depth,
            }
        })
        .collect();
    let name = crate::views::modulator::mod_name(app.project(), id).unwrap_or_default();
    let context = tf(
        "fxd.window.context",
        &[("modulator", &name), ("host", &host_words(app, host))],
    );
    let n = links.len();
    let chip = match n {
        0 => t("fxd.window.chip.none"),
        1 => t("fxd.window.chip.one"),
        _ => tf("fxd.window.chip", &[("n", &n.to_string())]),
    };
    let preset = presets::matching(m).map_or_else(
        || t("fxd.preset.custom"),
        |p| t(&format!("fxd.preset.{}", p.id)),
    );
    app.fx.fxd.words = Some(Words {
        rev,
        lang,
        id,
        context,
        links,
        chip,
        preset,
    });
}

/// The window's size at 100 % for its words (measured once per language and text size).
fn designed(app: &mut CantusApp, ctx: &egui::Context, formula: bool) -> Vec2 {
    let lang = crate::i18n::locale();
    let ts = crate::kit::Style::plain().t.text_scale.to_bits();
    if let Some((f, l, s, v)) = app.fx.fxd.designed {
        if f == formula && l == lang && s == ts {
            return v;
        }
    }
    let ui = egui::Ui::new(
        ctx.clone(),
        egui::Id::new("fxd-measure"),
        egui::UiBuilder::new(),
    );
    let v = fxd::designed_size(&ui, formula);
    app.fx.fxd.designed = Some((formula, lang, ts, v));
    v
}

/// The puck from the engine (or injected), the trail, where the song is on the path —
/// read from the project in place (nothing copied a frame).
fn feed_xy(app: &mut CantusApp, id: ModulatorId) {
    let playing = app.engine.is_playing();
    let beats = app.engine.position().0 as f64 / cantus_core::PPQ as f64;
    let Some(ModSource::XyPad(src)) = app.project().modulator(id).map(|(_, _, m)| &m.source) else {
        return;
    };
    let (puck, heads) = app
        .fx
        .fxd
        .injected
        .or_else(|| app.engine.modulation().pad(id))
        .unwrap_or_else(|| {
            let h = src.target(beats, playing, [src.x, src.y]);
            (h, h)
        });
    let path_at = src
        .path
        .as_ref()
        .filter(|_| src.play_path && playing)
        .map(|p| (beats / f64::from(p.beats)).rem_euclid(1.0) as f32);
    let st = &mut app.fx.fxd;
    st.note_puck(puck);
    let mut trail = std::mem::take(&mut st.live_xy.trail);
    st.trail_into(&mut trail);
    st.live_xy.trail = trail;
    st.live_xy.puck = puck;
    st.live_xy.heads = heads;
    st.live_xy.path_at = path_at;
    st.live_xy.mode = st.mode;
    st.live_xy.loop_beats = if st.loop_beats > 0.0 {
        st.loop_beats
    } else {
        4.0
    };
}

/// The song's place and tempo for the formula's plot; the draft and the caret.
fn feed_formula(app: &mut CantusApp) {
    let tick = app.engine.position().0;
    let p = app.project();
    let tempo = if p.tempo_points.is_empty() {
        p.tempo_bpm
    } else {
        p.tempo_map().bpm_at(tick as f64)
    };
    let st = &mut app.fx.fxd;
    st.live_f.beats = tick as f64 / cantus_core::PPQ as f64;
    st.live_f.tempo = tempo;
    fxd_edit::typing_live(st);
}

/// The controllers' window, drawn every frame while one is open.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(id) = app.fx.fxd.open else {
        return;
    };
    let (formula, look) = match app.project().modulator(id).map(|(_, _, m)| &m.source) {
        Some(ModSource::XyPad(x)) => (false, x.look),
        Some(ModSource::Formula(f)) => (true, f.look),
        // Gone (an undo, a removal) or no controller any more: the window closes.
        _ => {
            app.close_controller_window();
            return;
        }
    };
    let wid = device_id(id);
    let key = wid.value();
    let st = device::register_area(ctx, key, Function::Mod, || {
        t(if formula {
            "fxd.formula.name"
        } else {
            "fxd.xy.name"
        })
    });
    if st.closed {
        app.close_controller_window();
        return;
    }
    if st.minimised {
        return;
    }
    if std::mem::take(&mut app.fx.fxd.raise) || st.raise {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, wid));
    }
    refresh_words(app, id);
    // The keyboard first: a typing session's keys are its own.
    if formula {
        fxd_edit::keys(app, ctx, id);
    }
    let designed = designed(app, ctx, formula);
    let shell_k = fxd::shell_of(formula, look).scale(ctx);
    let size = designed * shell_k;
    let (at, size) = match st.maximised {
        Some(area) => (area.min, area.size()),
        None => (
            frame::position(ctx, st.place.or(app.fx.fxd.pos), size),
            size,
        ),
    };
    // What the face shows, read before the face borrows the project.
    if formula {
        feed_formula(app);
    } else {
        feed_xy(app, id);
    }
    let maximised = device::is_maximised(ctx, key);
    let ctl = crate::kit::window_ctl::WindowCtl {
        swap: None,
        minimise: true,
        maximised: Some(maximised),
        close: true,
    };
    let mut st_ui = std::mem::take(&mut app.fx.fxd);
    let words = st_ui.words.take();
    let mut xy_out = None;
    let mut f_out = None;
    if let (Some(w), Some((_, _, m))) = (&words, app.project().modulator(id)) {
        let chips = [w.chip.as_str()];
        let chrome = fxd::Chrome {
            context: &w.context,
            on: !m.muted,
            preset: Some(&w.preset),
            chips: &chips,
            window: Some(ctl),
        };
        frame::place(ctx, wid, at, size, |ui| match &m.source {
            ModSource::XyPad(x) => {
                xy_out = Some(fxd::xy(ui, &chrome, x, m.amount, &w.links, &st_ui.live_xy));
            }
            ModSource::Formula(f) => {
                f_out = Some(fxd::formula(
                    ui,
                    &chrome,
                    f,
                    m.amount,
                    &w.links,
                    &st_ui.live_f,
                ));
            }
            _ => {}
        });
    }
    st_ui.words = words;
    app.fx.fxd = st_ui;
    let (shell_out, header) = match (xy_out, f_out) {
        (Some(o), _) => {
            app.fx.fxd.rects_xy = o.rects;
            fxd_edit::xy_actions(app, id, o.actions);
            (o.shell, o.header)
        }
        (_, Some(o)) => {
            app.fx.fxd.rects_f = o.rects;
            fxd_edit::formula_actions(app, ctx, id, o.actions);
            (o.shell, o.header)
        }
        _ => return,
    };
    app.fx.fxd.header = Some(header);
    let whole = Rect::from_min_size(at, size);
    let head = Rect::from_min_max(at, egui::pos2(whole.right(), shell_out.body.top()));
    device::note_shown(ctx, key, whole, head, shell_out.window);
    let mut moved = Vec2::ZERO;
    for ev in &shell_out.events {
        match *ev {
            shell::Event::Move(d) => moved += d,
            shell::Event::Close => {
                app.close_controller_window();
                return;
            }
            shell::Event::Minimise => device::minimise(ctx, key),
            shell::Event::Maximise => device::toggle_maximise(ctx, key),
            shell::Event::Bypass => fxd_edit::toggle_on(app, id),
            shell::Event::PresetPrev => fxd_edit::step_preset(app, id, -1),
            shell::Event::PresetNext => fxd_edit::step_preset(app, id, 1),
            shell::Event::PresetBrowse => {
                let well = header.preset.unwrap_or(header.name);
                app.fx.fxd.menu = match app.fx.fxd.menu {
                    Some(_) => None,
                    None => Some((well.left_bottom(), well)),
                };
            }
            _ => {}
        }
    }
    fxd_edit::preset_menu(app, ctx, id, formula);
    if st.maximised.is_none() {
        let to = at + moved;
        app.fx.fxd.pos = Some(to);
        device::remember_place(ctx, key, Rect::from_min_size(to, size));
    }
    // Moving things ask for frames through the ticker; a window at rest asks for none.
    let fxd = &app.fx.fxd;
    let at_rest = fxd.drag.is_none()
        && fxd.dial.is_none()
        && fxd.card.is_none()
        && !app.engine.is_playing()
        && (formula || (fxd.trail_len == 0 && fxd.live_xy.puck == fxd.live_xy.heads));
    if !at_rest {
        crate::views::workspace_ticker::moving(ctx, crate::views::workspace_ticker::Reason::Live);
    }
}
