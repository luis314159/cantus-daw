//! Unda's oscillators on the kit: the table (a stack of frames you scrub, or the current
//! cycle flat with its warps drawn live) leading the view, the selected oscillator's warps
//! and unison beside it, a card per oscillator with its cycle, and the sub and noise.

use super::parts::{self, c, Pick, C};
use super::{count_text, db_text, import, names, pan_text, State};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::CardRects;
use crate::kit::color::with_alpha;
use crate::kit::knob::KnobSize;
use crate::kit::menu::{Check, Item};
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::inst::wavetable::{self as model, NAME_KEYS, TABLE_KEYS};
use cantus_core::{Command, NativeSpec, TrackId};
use cantus_dsp::unda::osc::{display_cycle, Warp, WarpSlot};
use cantus_dsp::unda::{factory, Table};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A table as the editor draws it: a factory one, or an imported one decoded here.
pub enum Shown {
    Factory(&'static Table),
    Imported(Arc<Table>),
}

impl std::ops::Deref for Shown {
    type Target = Table;
    fn deref(&self) -> &Table {
        match self {
            Shown::Factory(t) => t,
            Shown::Imported(t) => t,
        }
    }
}

fn key(o: usize, field: &str) -> String {
    format!("osc{}.{field}", o + 1)
}

/// The table oscillator `o` plays, for drawing. An imported file is decoded (and its mip
/// levels built) here, on the UI thread, once per path and frame size.
pub fn table_for(state: &mut State, spec: &NativeSpec, o: usize) -> Option<Shown> {
    match spec.text(TABLE_KEYS[o]) {
        None => {
            let i = spec.get(&key(o, "wt")).round().max(0.0) as usize;
            factory::tables().get(i).map(Shown::Factory)
        }
        Some(path) => {
            let frame = spec.get(&key(o, "frame")).round().max(1.0) as usize;
            let k = (PathBuf::from(path), frame);
            state
                .tables
                .entry(k)
                .or_insert_with(|| decode(Path::new(path), frame).map(Arc::new))
                .clone()
                .map(Shown::Imported)
        }
    }
}

fn decode(path: &Path, frame: usize) -> Option<Table> {
    let mut r = hound::WavReader::open(path).ok()?;
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let s = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .filter_map(Result::ok)
                .map(|v| v as f32 * s)
                .collect()
        }
    };
    (!samples.is_empty()).then(|| Table::from_audio(&samples, spec.channels as usize, frame))
}

/// Import a file into oscillator `o` (0-based): one undo step that sets its path, name and
/// frame size and switches the oscillator on. A Surge `.wt` is converted to a WAV in the
/// app's data folder first.
pub fn import_table(
    app: &mut CantusApp,
    track: TrackId,
    o: usize,
    path: &Path,
) -> Result<import::Found, String> {
    let found = import::inspect(path, &crate::app::data_dir().join("wavetables"))
        .map_err(|e| t(e.key()))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    app.exec(Command::Batch {
        label_key: "unda.cmd.import".into(),
        commands: vec![
            Command::SetInstrumentText {
                track,
                key: TABLE_KEYS[o].into(),
                text: Some(found.path.to_string_lossy().into_owned()),
            },
            Command::SetInstrumentText {
                track,
                key: NAME_KEYS[o].into(),
                text: Some(name),
            },
            Command::SetInstrumentValue {
                track,
                key: key(o, "frame"),
                value: found.frame as f32,
            },
            Command::SetInstrumentValue {
                track,
                key: key(o, "on"),
                value: 1.0,
            },
        ],
    });
    Ok(found)
}

/// Ask for a file and import it into oscillator `o`; the outcome goes to the status line.
fn import_dialog(app: &mut CantusApp, track: TrackId, o: usize) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(t("unda.import_filter"), &["wav", "wt"])
        .pick_file()
    else {
        return;
    };
    let note = match import_table(app, track, o, &path) {
        Ok(f) => tf(
            "unda.import.ok",
            &[("n", &f.frames.to_string()), ("size", &f.frame.to_string())],
        ),
        Err(e) => e,
    };
    app.set_status(note.clone());
    app.inst.wavetable.import_note = Some((o, note));
}

/// Back to a factory table: one undo step that forgets the imported file.
fn pick_factory(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, o: usize, i: usize) {
    let mut commands = vec![Command::SetInstrumentValue {
        track,
        key: key(o, "wt"),
        value: i as f32,
    }];
    for k in [TABLE_KEYS[o], NAME_KEYS[o]] {
        if spec.text(k).is_some() {
            commands.push(Command::SetInstrumentText {
                track,
                key: k.into(),
                text: None,
            });
        }
    }
    app.exec(Command::Batch {
        label_key: "unda.cmd.table".into(),
        commands,
    });
}

/// The name of the table oscillator `o` plays.
pub fn table_name(spec: &NativeSpec, o: usize) -> String {
    match spec.text(NAME_KEYS[o]) {
        Some(n) => n.to_string(),
        None if spec.text(TABLE_KEYS[o]).is_some() => t("unda.imported"),
        None => {
            let i = spec.get(&key(o, "wt")).round().max(0.0) as usize;
            model::TABLES
                .get(i)
                .map(|n| t(&format!("unda.wt.{n}")))
                .unwrap_or_default()
        }
    }
}

/// The table list: the factory tables, then Import…; `Some(i)` when one is chosen (the
/// last index is Import).
fn tables_menu(
    ui: &egui::Ui,
    id: egui::Id,
    anchor: Rect,
    s: &Style,
    spec: &NativeSpec,
    o: usize,
) -> Option<usize> {
    let labels: Vec<String> = model::TABLES
        .iter()
        .map(|n| t(&format!("unda.wt.{n}")))
        .collect();
    let now = spec.get(&key(o, "wt")).round().max(0.0) as usize;
    let imported = spec.text(TABLE_KEYS[o]).is_some();
    let mut items: Vec<Item<'_>> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| Item::Action {
            label: Src::Str(l),
            check: Check::Radio(!imported && i == now),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    items.push(Item::Separator);
    items.push(Item::action(Src::Key("unda.import")));
    parts::popup(ui, id, anchor, s, &items).map(|i| if i > labels.len() { labels.len() } else { i })
}

/// Act on a table pick: a factory table, or Import….
fn apply_table_pick(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, o: usize, i: usize) {
    if i >= model::TABLES.len() {
        import_dialog(app, track, o);
    } else {
        pick_factory(app, track, spec, o, i);
    }
}

/// The warp slots as the audio sees them (for drawing the flat view).
fn warps(spec: &NativeSpec, o: usize) -> [WarpSlot; 2] {
    let slot = |k: usize| WarpSlot {
        mode: Warp::from_index(spec.get(&key(o, &format!("warp{k}")))),
        amt: spec.get(&key(o, &format!("warp{k}.amt"))),
        src: spec.get(&key(o, &format!("warp{k}.src"))).round().max(0.0) as usize,
    };
    [slot(1), slot(2)]
}

/// Frame `k` of `n`, as the displays print it.
pub fn frame_text(pos: f32, frames: usize) -> String {
    let n = frames.max(1);
    let k = (pos.clamp(0.0, 1.0) * (n - 1) as f32).round() as usize + 1;
    tf(
        "unda.frame_of",
        &[("k", &k.to_string()), ("n", &n.to_string())],
    )
}

/// The position as a cell prints it: `8/25`.
fn pos_fmt(frames: usize) -> impl Fn(f32, &mut String) {
    move |v: f32, b: &mut String| {
        let n = frames.max(1);
        let _ = write!(
            b,
            "{}/{n}",
            (v.clamp(0.0, 1.0) * (n - 1) as f32).round() as usize + 1
        );
    }
}

// ---- drawing -------------------------------------------------------------------------------

const POINTS: usize = 128;

/// Draw a table in `rect`: a stack of frames receding up and right, the frame at `pos`
/// bold in the "now" colour with its fill; or (`flat`) the cycle at `pos` with its warps.
pub fn draw_table(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    table: &Table,
    pos: f32,
    flat: bool,
    w: &[WarpSlot; 2],
) {
    let now = s.t.highlight;
    let mut buf = [0.0f32; POINTS];
    if flat {
        draw_cycle(b, rect, s, table, pos, w, now);
        return;
    }
    let pad = s.s(10.0);
    let r = rect.shrink2(Vec2::new(pad, pad * 0.8));
    // At least 24 lines, so a table of a few frames still reads as a morph: the frames
    // themselves drawn brighter, the interpolated ones between them dimmer.
    let frames = table.frames();
    let shown = frames.clamp(24, 40);
    let (dx, dy) = (r.width() * 0.26, r.height() * 0.42);
    let (w0, h0) = (r.width() - dx, r.height() - dy);
    let amp = h0 * 0.42;
    let place = |depth: f32, i: usize, y: f32| {
        Pos2::new(
            r.left() + depth * dx + w0 * i as f32 / (POINTS - 1) as f32,
            r.bottom() - depth * dy - h0 * 0.5 - y * amp,
        )
    };
    let mut line = [Pos2::ZERO; POINTS];
    let lw = s.s(1.0);
    for k in (0..shown).rev() {
        let depth = if shown > 1 {
            k as f32 / (shown - 1) as f32
        } else {
            0.0
        };
        display_cycle(table, depth, &[WarpSlot::default(); 2], &mut buf);
        for (i, y) in buf.iter().enumerate() {
            line[i] = place(depth, i, *y);
        }
        let at_frame = frames <= 1 || {
            let f = depth * (frames - 1) as f32;
            (f - f.round()).abs() * (shown as f32 / frames as f32) < 0.5
        };
        let strength = if at_frame {
            0.40 + 0.45 * (1.0 - depth)
        } else {
            0.14 + 0.18 * (1.0 - depth)
        };
        b.polyline(&line, lw, with_alpha(s.hue, strength));
    }
    // The current frame: filled and bold.
    let depth = pos.clamp(0.0, 1.0);
    display_cycle(table, depth, &[WarpSlot::default(); 2], &mut buf);
    for (i, y) in buf.iter().enumerate() {
        line[i] = place(depth, i, *y);
    }
    let base = place(depth, 0, 0.0).y;
    b.area(
        &line,
        base,
        with_alpha(now, if s.t.dark { 0.22 } else { 0.18 }),
    );
    b.polyline(&line, s.s(2.0), now);
}

/// One cycle, flat, centred on the display's axis, filled toward it.
fn draw_cycle(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    table: &Table,
    pos: f32,
    w: &[WarpSlot; 2],
    color: egui::Color32,
) {
    let mut buf = [0.0f32; POINTS];
    let r = rect.shrink2(Vec2::new(s.s(8.0), s.s(4.0)));
    let mid = r.center().y;
    b.hline(r.left(), r.right(), b.snap_mid(mid), b.px(), s.t.grid);
    display_cycle(table, pos, w, &mut buf);
    let amp = r.height() * 0.44;
    let mut line = [Pos2::ZERO; POINTS];
    for (i, y) in buf.iter().enumerate() {
        line[i] = Pos2::new(
            r.left() + r.width() * i as f32 / (POINTS - 1) as f32,
            mid - y * amp,
        );
    }
    b.area(
        &line,
        mid,
        with_alpha(color, if s.t.dark { 0.20 } else { 0.14 }),
    );
    b.polyline(&line, s.s(1.8), color);
}

/// The position of oscillator `o`: the newest voice's when one plays, else the knob's.
fn now_pos(app: &CantusApp, spec: &NativeSpec, o: usize) -> f32 {
    if super::live(app) {
        app.inst_telemetry()[cantus_engine::inst::wavetable::tel::POS + o]
    } else {
        spec.get(&key(o, "pos"))
    }
}

/// Paint an oscillator's picture in `d`: its engine's (the cycle with its warps, the
/// partials, the grain source).
fn picture(app: &mut CantusApp, b: &mut Brush, d: Rect, s: &Style, spec: &NativeSpec, o: usize) {
    let pos = now_pos(app, spec, o);
    let engine = super::engines::engine(spec, o);
    let shown = table_for(&mut app.inst.wavetable, spec, o);
    let on = spec.get(&key(o, "on")) >= 0.5;
    let color = if on { s.hue } else { s.t.text_dim };
    match &shown {
        Some(table) if engine == 1 => super::engines::draw_partes(b, d, s, table, spec, o, color),
        Some(table) if engine == 2 => super::engines::draw_grana(b, d, s, table, spec, o, color),
        Some(table) => draw_cycle(b, d, s, table, pos, &warps(spec, o), color),
        None => parts::say(
            b,
            d.shrink(s.s(6.0)),
            s,
            &t("unda.table_missing"),
            Role::Label,
            s.t.danger,
            Align2::CENTER_CENTER,
        ),
    }
}

/// Drag up and down on `rect` to scrub oscillator `o`'s position (one undo step).
fn scrub(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    o: usize,
    frames: usize,
) -> egui::Response {
    let pos_key = key(o, "pos");
    let r = ui.interact(
        rect,
        ui.id().with(("unda-scrub", o, rect.width() as i32)),
        Sense::click_and_drag(),
    );
    let pos = spec.get(&pos_key);
    if r.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let v = (pos - fine * r.drag_delta().y / rect.height().max(1.0)).clamp(0.0, 1.0);
        app.set_inst_value(track, &pos_key, v, true);
    }
    if r.drag_stopped() {
        app.end_gesture();
    }
    if r.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    parts::display_hint(ui, &r, "unda.table_tip", &frame_text(pos, frames));
    r
}

// ---- the table card ------------------------------------------------------------------------

/// The selected oscillator's table, leading the view: the stack of frames (or the cycle
/// with its warps, 2D), the frame the newest voice plays lit; drag to scrub. The title
/// bar picks the oscillator, the table and 3D | 2D; in Play the position is the one XL
/// cell below, with the oscillator's key cells.
#[allow(clippy::too_many_arguments)]
pub fn table_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
    play: bool,
) {
    let k = s.k;
    let cells_h = if play {
        super::parts::cell_size(KnobSize::XL, s).y + 12.0 * k
    } else {
        0.0
    };
    let disp_h = (rect.height() / k - 24.0 - 1.0 - cells_h / k).max(60.0);
    let title = tf("unda.osc_n", &[("n", &(o + 1).to_string())]);
    let on = spec.get(&key(o, "on")) >= 0.5;
    let cr = parts::card(ui, rect, s, Src::Str(&title), Some(on), None, Some(disp_h));
    parts::power(app, ui, track, spec, &key(o, "on"), &cr);
    title_controls(app, ui, track, spec, s, &cr, o);
    let flat = app.inst.wavetable.flat[o];
    let pos = spec.get(&key(o, "pos"));
    let now = now_pos(app, spec, o);
    let Some(d) = cr.display else {
        return;
    };
    let shown = table_for(&mut app.inst.wavetable, spec, o);
    let frames = shown.as_ref().map_or(1, |t| t.frames());
    let w = warps(spec, o);
    with_brush(&ui.painter_at(d), |b| {
        match &shown {
            Some(table) => draw_table(b, d, s, table, now, flat, &w),
            None => parts::say(
                b,
                d,
                s,
                &t("unda.table_missing"),
                Role::Label,
                s.t.danger,
                Align2::CENTER_CENTER,
            ),
        }
        let pad = s.s(8.0);
        let line = s.s(18.0);
        parts::chip(
            b,
            Rect::from_min_size(
                Pos2::new(d.left() + pad, d.bottom() - pad - line),
                Vec2::new(d.width() * 0.5, line),
            ),
            s,
            &frame_text(pos, frames),
            s.t.text,
            Align2::LEFT_CENTER,
        );
    });
    scrub(app, ui, track, spec, d, o, frames);
    if !play {
        return;
    }
    let body = parts::below_display(&cr, s);
    let fmt = pos_fmt(frames);
    let xl = super::parts::cell_size(KnobSize::XL, s);
    parts::cell_at(
        app,
        ui,
        track,
        spec,
        c(&key(o, "pos")).fmt(&fmt),
        s,
        KnobSize::XL,
        Pos2::new(body.left(), body.center().y - xl.y * 0.5),
    );
    let rest = Rect::from_min_max(
        Pos2::new(body.left() + xl.x + s.d(8.0), body.top()),
        body.max,
    );
    let keys = play_keys(spec, o);
    let list: Vec<C<'_>> = keys
        .iter()
        .map(|(k, l)| {
            let mut x = c(k);
            if let Some(l) = l {
                x = x.label(Src::Key(l));
            }
            x
        })
        .collect();
    parts::cells_spread(app, ui, track, spec, s, KnobSize::L, rest, &list);
}

/// The cells beside the XL position in Play: what shapes the oscillator's sound live.
fn play_keys(spec: &NativeSpec, o: usize) -> Vec<(String, Option<&'static str>)> {
    match super::engines::engine(spec, o) {
        1 => vec![
            (key(o, "tilt"), None),
            (key(o, "oddeven"), None),
            (key(o, "stretch"), None),
            (key(o, "level"), None),
        ],
        2 => vec![
            (key(o, "density"), None),
            (key(o, "glen"), None),
            (key(o, "spray"), None),
            (key(o, "level"), None),
        ],
        _ => vec![
            (key(o, "warp1.amt"), Some("unda.warp_1")),
            (key(o, "warp2.amt"), Some("unda.warp_2")),
            (key(o, "detune"), None),
            (key(o, "level"), None),
        ],
    }
}

/// The table card's title bar: the table (‹ name ▾ ›), which oscillator, 3D | 2D.
fn title_controls(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    cr: &CardRects,
    o: usize,
) {
    let bar = cr.title;
    let h = s.s(18.0);
    let y = bar.center().y - h * 0.5;
    let gap = s.s(6.0);
    let at = |right: f32, w: f32| Rect::from_min_size(Pos2::new(right - w, y), Vec2::new(w, h));
    let views = [Src::Key("unda.view_3d"), Src::Key("unda.view_2d")];
    let flat = app.inst.wavetable.flat[o];
    let vr = at(bar.right() - gap, s.s(58.0));
    if let Some(i) = parts::seg(
        ui,
        ui.id().with(("unda-flat", o)),
        vr,
        s,
        &views,
        flat as usize,
        &t("unda.view_tip"),
    ) {
        app.inst.wavetable.flat[o] = i == 1;
    }
    let oscs = [Src::Str("1"), Src::Str("2"), Src::Str("3")];
    let or = at(vr.left() - gap, s.s(72.0));
    if let Some(i) = parts::seg(
        ui,
        ui.id().with("unda-osc-pick"),
        or,
        s,
        &oscs,
        o,
        &t("unda.osc_pick"),
    ) {
        app.inst.wavetable.osc = i;
    }
    let left_edge = bar.left() + s.s(96.0);
    let tw = (or.left() - gap - left_edge).clamp(s.s(60.0), s.s(190.0));
    let tr = at(or.left() - gap, tw);
    let id = ui.id().with(("unda-tables-big", o));
    let (field, fresp) = well_field(ui, id, tr, s, &table_name(spec, o), &t("unda.table"));
    // The table is a parameter: its menu, its mark, the learn mode (round 29, review 3 of
    // ADR 0062, #35).
    let wt = key(o, "wt");
    let field = if crate::controllers::stepped_inst(app, ui, track, spec, &wt, &fresp) {
        Pick::default()
    } else {
        field
    };
    let now = spec.get(&key(o, "wt")).round().max(0.0) as usize;
    let n = model::TABLES.len();
    if let Some(i) = parts::step(now, n, field) {
        pick_factory(app, track, spec, o, i);
    }
    if field.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    if let Some(i) = tables_menu(ui, id.with("menu"), tr, s, spec, o) {
        apply_table_pick(app, track, spec, o, i);
    }
}

/// A menu field with ‹ › that reports its clicks (the caller owns the list).
fn well_field(
    ui: &egui::Ui,
    id: egui::Id,
    rect: Rect,
    s: &Style,
    value: &str,
    name: &str,
) -> (Pick, egui::Response) {
    let open: bool = ui.data(|d| d.get_temp(id.with("menu"))).unwrap_or(false);
    let (resp, c) = crate::kit::widgets::MenuField::new(value)
        .steppers(true)
        .open(open)
        .id(id)
        .name(name)
        .style(*s)
        .show_at(ui, rect);
    (
        Pick {
            prev: c.prev,
            next: c.next,
            open: c.open,
        },
        resp,
    )
}

// ---- the detail card -----------------------------------------------------------------------

/// A label | value row's label column width.
fn label_w(s: &Style) -> f32 {
    s.s(46.0)
}

/// A label | field pair in `rect`: the label right-aligned in its column, the field after.
fn labelled(ui: &egui::Ui, rect: Rect, s: &Style, label_key: &str) -> Rect {
    let lw = label_w(s);
    with_brush(ui.painter(), |b| {
        parts::row_label(
            b,
            Rect::from_min_size(rect.min, Vec2::new(lw - s.s(6.0), rect.height())),
            s,
            label_key,
        );
    });
    Rect::from_min_max(Pos2::new(rect.left() + lw, rect.top()), rect.max)
}

/// The selected oscillator's engine in detail: for the wavetable engine its two warps (mode,
/// source, amount) and its unison (law, stack, spread, pitch, phase, random phase); for
/// Grana its window and the rest of its grains; for Partes its partials, large.
pub fn detail_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    let engine = super::engines::engine(spec, o);
    let n = (o + 1).to_string();
    let title = tf("unda.detail", &[("n", &n)]);
    let disp = match engine {
        1 => Some(rect.height() / s.k - 24.0 - 1.0),
        2 => Some(84.0),
        _ => None,
    };
    let cr = parts::card(ui, rect, s, Src::Str(&title), None, None, disp);
    // The engine, in the title bar: the wavetable, Partes (additive), Grana (granular).
    let engines = [
        Src::Key("unda.engine.wavetable"),
        Src::Key("unda.engine.partes"),
        Src::Key("unda.engine.grana"),
    ];
    let h = s.s(18.0);
    let w = (cr.title.width() * 0.56).min(s.s(240.0));
    let er = Rect::from_min_size(
        Pos2::new(
            cr.title.right() - s.s(6.0) - w,
            cr.title.center().y - h * 0.5,
        ),
        Vec2::new(w, h),
    );
    parts::seg_key(app, ui, track, spec, &key(o, "engine"), er, s, &engines);
    match engine {
        1 => {
            if let (Some(d), Some(table)) =
                (cr.display, table_for(&mut app.inst.wavetable, spec, o))
            {
                with_brush(&ui.painter_at(d), |b| {
                    super::engines::draw_partes(b, d, s, &table, spec, o, s.hue)
                });
            }
        }
        2 => {
            if let Some(d) = cr.display {
                with_brush(&ui.painter_at(d), |b| {
                    super::engines::draw_window(b, d, s, spec, o)
                });
            }
            let body = parts::below_display(&cr, s);
            let cols = parts::columns(body, s, &[0.45, 0.55]);
            parts::cells(
                app,
                ui,
                track,
                spec,
                s,
                KnobSize::M,
                cols[0],
                &[c(&key(o, "greverse")), c(&key(o, "width"))],
            );
            let fh = s.s(20.0);
            let fr = Rect::from_min_size(
                Pos2::new(cols[1].left(), cols[1].center().y - fh * 0.5),
                Vec2::new(cols[1].width(), fh),
            );
            let field = labelled(ui, fr, s, "unda.gwin");
            parts::choice_key(
                app,
                ui,
                track,
                spec,
                &key(o, "gwin"),
                field,
                s,
                &names("unda.gwin", model::GRAIN_WINDOWS),
            );
        }
        _ => {
            let body = cr.body;
            let size = if parts::row_width(6, KnobSize::M, s) <= body.width() {
                KnobSize::M
            } else {
                KnobSize::S
            };
            let ch = parts::cell_size(size, s).y;
            let fh = s.s(20.0);
            let block = ch + s.s(10.0) + fh * 2.0 + s.s(4.0);
            let top = body.top() + ((body.height() - block) * 0.5).max(0.0);
            let row = Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), ch));
            let deg = |v: f32, b: &mut String| {
                let _ = write!(b, "{v:.0}°");
            };
            parts::cells_spread(
                app,
                ui,
                track,
                spec,
                s,
                size,
                row,
                &[
                    c(&key(o, "warp1.amt")).label(Src::Key("unda.warp_1")),
                    c(&key(o, "warp2.amt")).label(Src::Key("unda.warp_2")),
                    c(&key(o, "spread")),
                    c(&key(o, "pitch")),
                    c(&key(o, "phase")).fmt(&deg),
                    c(&key(o, "rand")),
                ],
            );
            let grid = Rect::from_min_max(
                Pos2::new(body.left(), row.bottom() + s.s(10.0)),
                Pos2::new(body.right(), row.bottom() + s.s(10.0) + fh * 2.0 + s.s(4.0)),
            );
            // Under the warp amounts, each warp's mode over its source; under the unison's
            // cells, its law and stack as label | value rows.
            let cols = parts::columns(grid, s, &[0.29, 0.29, 0.42]);
            let warp_names = names("unda.warp", model::WARPS);
            let src_names = names("unda.src", model::WARP_SOURCES);
            for (i, col) in cols.iter().enumerate() {
                let r1 = Rect::from_min_size(col.min, Vec2::new(col.width(), fh));
                let r2 = Rect::from_min_size(
                    Pos2::new(col.left(), col.top() + fh + s.s(4.0)),
                    Vec2::new(col.width(), fh),
                );
                if i < 2 {
                    let w = i + 1;
                    let mode_key = key(o, &format!("warp{w}"));
                    parts::choice_key_plain(app, ui, track, spec, &mode_key, r1, s, &warp_names);
                    let src_key = key(o, &format!("warp{w}.src"));
                    if Warp::from_index(spec.get(&mode_key)).needs_source() {
                        parts::choice_key_plain(app, ui, track, spec, &src_key, r2, s, &src_names);
                    } else {
                        let v = spec.get(&src_key).round().max(0.0) as usize;
                        parts::field_off(
                            ui,
                            r2,
                            s,
                            src_names.get(v).map(String::as_str).unwrap_or(""),
                            false,
                        );
                    }
                } else {
                    let f1 = labelled(ui, r1, s, "unda.ucurve");
                    parts::choice_key_plain(
                        app,
                        ui,
                        track,
                        spec,
                        &key(o, "ucurve"),
                        f1,
                        s,
                        &names("unda.ucurve", model::UNISON_CURVES),
                    );
                    let f2 = labelled(ui, r2, s, "unda.stack");
                    parts::choice_key_plain(
                        app,
                        ui,
                        track,
                        spec,
                        &key(o, "stack"),
                        f2,
                        s,
                        &names("unda.stack", model::STACKS),
                    );
                }
            }
        }
    }
}

// ---- oscillator, sub and noise cards -------------------------------------------------------

/// One oscillator: its table (‹ name ›, Import… at the end of the list), its cycle — drag
/// to scrub, click to show it in the big table —, engine, octave, semitones and route,
/// and two rows of cells.
pub fn osc_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    o: usize,
) {
    let title = tf("unda.osc_n", &[("n", &(o + 1).to_string())]);
    let on = spec.get(&key(o, "on")) >= 0.5;
    let name = table_name(spec, o);
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Str(&title),
        Some(on),
        Some(&name),
        Some(60.0),
    );
    parts::power(app, ui, track, spec, &key(o, "on"), &cr);
    let id = ui.id().with(("unda-tables", o));
    let pick = parts::selector_key(
        app,
        ui,
        id,
        &cr,
        &t("unda.table"),
        track,
        spec,
        &key(o, "wt"),
    );
    let now = spec.get(&key(o, "wt")).round().max(0.0) as usize;
    if let Some(i) = parts::step(now, model::TABLES.len(), pick) {
        pick_factory(app, track, spec, o, i);
    }
    if pick.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    if let Some(sr) = cr.selector {
        if let Some(i) = tables_menu(ui, id.with("menu"), sr, s, spec, o) {
            apply_table_pick(app, track, spec, o, i);
        }
    }
    if app.inst.wavetable.osc == o {
        parts::outline(ui, rect, s);
    }
    let Some(d) = cr.display else {
        return;
    };
    let frames = table_for(&mut app.inst.wavetable, spec, o).map_or(1, |t| t.frames());
    let pos = spec.get(&key(o, "pos"));
    with_brush(&ui.painter_at(d), |b| {
        picture(app, b, d, s, spec, o);
        let line = s.s(16.0);
        parts::chip(
            b,
            Rect::from_min_size(
                Pos2::new(d.left() + s.s(6.0), d.bottom() - line - s.s(4.0)),
                Vec2::new(d.width() * 0.6, line),
            ),
            s,
            &frame_text(pos, frames),
            s.t.text,
            Align2::LEFT_CENTER,
        );
    });
    let r = scrub(app, ui, track, spec, d, o, frames);
    if r.clicked() || r.drag_started() {
        app.inst.wavetable.osc = o;
    }
    // Octave, semitones, route (the engine is the detail card's).
    let body = parts::below_display(&cr, s);
    let fh = s.s(20.0);
    let fields = Rect::from_min_size(body.min, Vec2::new(body.width(), fh));
    let f = parts::columns(fields, s, &[0.33, 0.33, 0.34]);
    let oct = |v: f32, b: &mut String| {
        let _ = write!(b, "{v:+.0} oct");
    };
    let semi = |v: f32, b: &mut String| {
        let _ = write!(b, "{v:+.0} st");
    };
    parts::stepper_key(app, ui, track, spec, &key(o, "oct"), f[0], s, &oct);
    parts::stepper_key(app, ui, track, spec, &key(o, "semi"), f[1], s, &semi);
    parts::choice_key_plain(
        app,
        ui,
        track,
        spec,
        &key(o, "route"),
        f[2],
        s,
        &names("unda.route", model::ROUTES),
    );
    // Two rows of cells.
    let rows_r = Rect::from_min_max(Pos2::new(body.left(), fields.bottom() + s.s(4.0)), body.max);
    let half = (rows_r.height() - s.s(2.0)) * 0.5;
    let r1 = Rect::from_min_size(rows_r.min, Vec2::new(rows_r.width(), half));
    let r2 = Rect::from_min_size(
        Pos2::new(rows_r.left(), r1.bottom() + s.s(2.0)),
        Vec2::new(rows_r.width(), half),
    );
    let fmt = pos_fmt(frames);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::M,
        r1,
        &[
            c(&key(o, "pos")).fmt(&fmt),
            c(&key(o, "fine")),
            c(&key(o, "level")).fmt(&db_text),
            c(&key(o, "pan")).fmt(&pan_text),
        ],
    );
    let voices = |v: f32, b: &mut String| {
        let _ = write!(
            b,
            "{}",
            tf("unda.voices_n", &[("n", &format!("{}", v.round() as u32))])
        );
    };
    let engine = super::engines::engine(spec, o);
    let fields: [&str; 4] = match engine {
        1 => ["partials", "tilt", "oddeven", "stretch"],
        2 => ["density", "glen", "spray", "gpitch"],
        _ => ["uni", "detune", "blend", "width"],
    };
    let k2 = fields.map(|f| key(o, f));
    let fmts: [Option<parts::Fmt<'_>>; 4] = match engine {
        1 => [
            Some(&count_text),
            Some(&super::engines::tilt_text),
            Some(&super::engines::oddeven_text),
            None,
        ],
        2 => [None; 4],
        _ => [Some(&voices), None, None, None],
    };
    let second: Vec<C<'_>> = k2
        .iter()
        .zip(fmts)
        .map(|(k, f)| match f {
            Some(f) => c(k).fmt(f),
            None => c(k),
        })
        .collect();
    parts::cells_spread(app, ui, track, spec, s, KnobSize::M, r2, &second);
}

/// The sub oscillator: its wave, octave and route, level and pan.
pub fn sub_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let on = spec.get("sub.on") >= 0.5;
    let cr = parts::card(ui, rect, s, Src::Key("unda.sub"), Some(on), None, None);
    parts::power(app, ui, track, spec, "sub.on", &cr);
    let body = cr.body;
    let fh = s.s(20.0);
    let mut y = body.top();
    let mut field = || {
        let r = Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), fh));
        y += fh + s.s(4.0);
        r
    };
    let (a, b2, c3) = (field(), field(), field());
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        "sub.wave",
        a,
        s,
        &names("unda.sub", model::SUB_WAVES),
    );
    let oct = |v: f32, b: &mut String| {
        let _ = write!(b, "{v:+.0} oct");
    };
    parts::stepper_key(app, ui, track, spec, "sub.oct", b2, s, &oct);
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        "sub.route",
        c3,
        s,
        &names("unda.route", model::ROUTES),
    );
    let rest = Rect::from_min_max(Pos2::new(body.left(), c3.bottom() + s.s(2.0)), body.max);
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        rest,
        &[
            c("sub.level").fmt(&super::db_s),
            c("sub.pan").fmt(&pan_text),
        ],
    );
}

/// The noise: its route, level, colour, pan and width.
pub fn noise_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let on = spec.get("noise.on") >= 0.5;
    let cr = parts::card(ui, rect, s, Src::Key("unda.noise"), Some(on), None, None);
    parts::power(app, ui, track, spec, "noise.on", &cr);
    let body = cr.body;
    let fh = s.s(20.0);
    let f = Rect::from_min_size(body.min, Vec2::new(body.width(), fh));
    parts::choice_key(
        app,
        ui,
        track,
        spec,
        "noise.route",
        f,
        s,
        &names("unda.route", model::ROUTES),
    );
    let rest = Rect::from_min_max(Pos2::new(body.left(), f.bottom() + s.s(2.0)), body.max);
    let half = rest.height() * 0.5;
    let r1 = Rect::from_min_size(rest.min, Vec2::new(rest.width(), half));
    let r2 = Rect::from_min_size(
        Pos2::new(rest.left(), r1.bottom()),
        Vec2::new(rest.width(), half),
    );
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        r1,
        &[
            c("noise.level").fmt(&super::db_s),
            c("noise.color").fmt(&color_text),
        ],
    );
    parts::cells_spread(
        app,
        ui,
        track,
        spec,
        s,
        KnobSize::S,
        r2,
        &[c("noise.pan").fmt(&pan_text), c("noise.width")],
    );
}

/// The noise's colour as a small cell prints it: 0 % is white, below darker, above
/// brighter (the help line says it in words).
fn color_text(v: f32, b: &mut String) {
    let p = (v * 100.0).round();
    let _ = if p == 0.0 {
        write!(b, "0 %")
    } else {
        write!(b, "{p:+.0} %")
    };
}

/// The inspector's picture: oscillator 1's current cycle, flat, with the table's name.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(crate::kit::Function::Source);
    let (rect, r) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(220.0), 56.0),
        Sense::hover(),
    );
    let i = spec.get("osc1.wt").round().max(0.0) as usize;
    with_brush(&ui.painter_at(rect), |b| {
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.ink));
        if spec.text(TABLE_KEYS[0]).is_none() {
            if let Some(table) = factory::tables().get(i) {
                draw_cycle(
                    b,
                    rect,
                    &s,
                    table,
                    spec.get("osc1.pos"),
                    &warps(spec, 0),
                    s.hue,
                );
            }
        }
        let label = format!("{} \u{b7} {}", t(spec.kind.name_key()), table_name(spec, 0));
        parts::chip(
            b,
            Rect::from_min_size(
                rect.min + Vec2::splat(3.0),
                Vec2::new(rect.width() - 6.0, 16.0),
            ),
            &s,
            &label,
            s.t.text,
            Align2::LEFT_CENTER,
        );
    });
    r
}
