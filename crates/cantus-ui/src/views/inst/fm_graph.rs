//! The operator graph on the kit: operators as nodes laid out by depth (the heard ones on
//! the bottom row over OUT, their modulators above), each edge carrying its index as a
//! value pill you drag. Drag from an operator onto another to connect it, onto itself for
//! feedback, onto OUT to hear it; right-click an operator for its menu. A back edge (a
//! later operator in the evaluation order) is dashed and its number carries a return mark:
//! it reads the previous sample, as its hint says (round 26: its old `z⁻¹` had no glyph). The card's title bar holds the algorithm picker (the 32 DX7 topologies, drawn),
//! the DX7 SysEx menu and Graph | Grid.

use super::ops::pill_num;
use super::parts;
use super::{algorithm_changes, algorithm_of, apply_algorithm, connect, disconnect, set_carrier};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, with_alpha};
use crate::kit::menu::Item;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Fill, Style};
use crate::views::inst::{source_color, write_readout};
use cantus_core::inst::fm::{matrix_key, DxAlgorithm, DX_ALGORITHMS, NAMES, OPS, OP_KEYS, OUT_OFF};
use cantus_core::inst::modulation::connections;
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::modulus::tel;
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};

/// A connection being drawn from an operator.
#[derive(Copy, Clone, Debug, Default)]
pub struct Drag {
    from: Option<usize>,
}

/// The topology as the graph draws it.
#[derive(Clone, Debug, Default)]
pub(super) struct Topo {
    /// `m[s][t]`, the diagonal being feedback.
    pub m: [[f32; OPS]; OPS],
    pub out: [f32; OPS],
    pub on: [bool; OPS],
}

impl Topo {
    pub fn of(spec: &NativeSpec) -> Topo {
        let mut tp = Topo::default();
        for (s, k) in OP_KEYS.iter().enumerate() {
            for t in 0..OPS {
                tp.m[s][t] = spec.get(matrix_key(s, t));
            }
            tp.out[s] = spec.get(k.out);
            tp.on[s] = spec.get(k.on) >= 0.5;
        }
        tp
    }

    pub fn of_dx(a: &DxAlgorithm) -> Topo {
        let mut tp = Topo {
            out: [OUT_OFF; OPS],
            on: [true; OPS],
            ..Default::default()
        };
        for &(s, t) in a.edges {
            tp.m[s as usize - 1][t as usize - 1] = 1.0;
        }
        let (f, i) = a.feedback;
        tp.m[f as usize - 1][i as usize - 1] = 1.0;
        for &c in a.carriers {
            tp.out[c as usize - 1] = 0.0;
        }
        tp
    }

    pub fn carrier(&self, o: usize) -> bool {
        self.out[o] > OUT_OFF
    }

    pub fn used(&self, o: usize) -> bool {
        self.carrier(o) || (0..OPS).any(|x| self.m[o][x] != 0.0 || self.m[x][o] != 0.0)
    }
}

/// Where each operator sits in `rect` (an unused one sits in the spare column), and the
/// OUT bar's top. Sizes are points at scale `k`.
pub(super) fn layout(
    tp: &Topo,
    rect: Rect,
    spare_w: f32,
    k: f32,
) -> ([Pos2; OPS], [bool; OPS], f32) {
    let out_h = 18.0 * k;
    let main = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.right() - spare_w, rect.bottom() - out_h),
    );
    let mut depth = [0usize; OPS];
    for _ in 0..OPS {
        for s in 0..OPS {
            for t in 0..s {
                if tp.m[s][t] != 0.0 && !tp.carrier(s) {
                    depth[s] = depth[s].max(depth[t] + 1);
                }
            }
            if !tp.carrier(s) && depth[s] == 0 && tp.used(s) {
                depth[s] = 1;
            }
        }
    }
    let used: Vec<usize> = (0..OPS).filter(|o| tp.used(*o)).collect();
    let rows = used.iter().map(|o| depth[*o]).max().unwrap_or(0) + 1;
    let row_h = main.height() / rows as f32;
    let y = |d: usize| main.bottom() - row_h * (d as f32 + 0.5);
    let mut pos = [Pos2::ZERO; OPS];
    let mut spare = [false; OPS];
    // Carriers evenly spaced, in letter order; everything above at the mean of its targets.
    let carriers: Vec<usize> = used.iter().copied().filter(|o| depth[*o] == 0).collect();
    for (i, o) in carriers.iter().enumerate() {
        let x = main.left() + main.width() * (i as f32 + 0.5) / carriers.len() as f32;
        pos[*o] = Pos2::new(x, y(0));
    }
    let min_gap = 44.0 * k;
    for d in 1..rows {
        let mut row: Vec<(f32, usize)> = used
            .iter()
            .copied()
            .filter(|o| depth[*o] == d)
            .map(|s| {
                let targets: Vec<f32> = (0..s)
                    .filter(|t| tp.m[s][*t] != 0.0 && depth[*t] < d)
                    .map(|t| pos[t].x)
                    .collect();
                let x = if targets.is_empty() {
                    main.center().x
                } else {
                    targets.iter().sum::<f32>() / targets.len() as f32
                };
                (x, s)
            })
            .collect();
        row.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        // Push apart, then centre the group on where it wanted to be.
        let want: f32 = row.iter().map(|r| r.0).sum::<f32>() / row.len().max(1) as f32;
        for i in 1..row.len() {
            row[i].0 = row[i].0.max(row[i - 1].0 + min_gap);
        }
        let have: f32 = row.iter().map(|r| r.0).sum::<f32>() / row.len().max(1) as f32;
        for (x, s) in row {
            let x = (x + want - have).clamp(
                main.left() + 16.0 * k,
                (main.right() - 16.0 * k).max(main.left() + 16.0 * k),
            );
            pos[s] = Pos2::new(x, y(d));
        }
    }
    let mut n = 0;
    for o in 0..OPS {
        if !tp.used(o) {
            spare[o] = true;
            let x = rect.right() - spare_w * 0.5;
            pos[o] = Pos2::new(x, rect.top() + 16.0 * k + n as f32 * 32.0 * k);
            n += 1;
        }
    }
    (pos, spare, rect.bottom() - out_h)
}

/// The value printed on an edge.
fn index_text(v: f32) -> String {
    format!("{v:.2}")
}

/// A frequency as an operator's subtitle: `×3.50` or `440 Hz`.
pub(super) fn freq_text(spec: &NativeSpec, o: usize) -> String {
    let k = &OP_KEYS[o];
    let mut b = String::new();
    if spec.get(k.freq_mode) >= 0.5 {
        write_readout(spec.get(k.fixed), "Hz", &mut b);
    } else {
        b = format!("\u{00d7}{:.2}", spec.get(k.ratio));
    }
    b
}

/// A number on the graph: its key, where, its text, the colour of a source reaching it,
/// and the edge it belongs to (none for an output level).
type Pill = (
    &'static str,
    Pos2,
    String,
    Option<Color32>,
    Option<(usize, usize)>,
);

/// A node's radius at 100 %.
const R: f32 = 15.0;

/// A cubic Bézier, sampled.
fn bezier(p: [Pos2; 4], n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            Pos2::new(
                w[0] * p[0].x + w[1] * p[1].x + w[2] * p[2].x + w[3] * p[3].x,
                w[0] * p[0].y + w[1] * p[1].y + w[2] * p[2].y + w[3] * p[3].y,
            )
        })
        .collect()
}

/// A dashed polyline: every other piece of `dash` points (the kit's, round 25).
fn dashed(b: &mut Brush, pts: &[Pos2], w: f32, color: Color32, dash: f32) {
    b.dashed(pts, w, color, dash, dash);
}

// ---- the card ------------------------------------------------------------------------------

/// The graph's card: the algorithm (‹ DX 5 ▾ ›, the 32 topologies drawn), the DX7 SysEx
/// menu and Graph | Grid in the title bar; the graph — or the matrix grid — bleeding to the
/// card's edges.
pub(super) fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let disp = rect.height() / s.k - 24.0 - 1.0;
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key("fm.graph_title"),
        None,
        None,
        Some(disp),
    );
    let bar = cr.title;
    let h = s.s(18.0);
    let y = bar.center().y - h * 0.5;
    let mut right = bar.right() - s.s(6.0);
    let mut take = |w: f32| {
        let r = Rect::from_min_size(Pos2::new(right - w, y), Vec2::new(w, h));
        right -= w + s.s(6.0);
        r
    };
    let views = [Src::Key("fm.view.graph"), Src::Key("fm.view.grid")];
    let vr = take(s.s(96.0));
    if let Some(i) = parts::seg(
        ui,
        ui.id().with("fm-view"),
        vr,
        s,
        &views,
        app.inst.fm.matrix_view as usize,
        &t("fm.view.matrix_tip"),
    ) {
        app.inst.fm.matrix_view = i == 1;
    }
    let dx = take(s.s(70.0));
    dx7_field(app, ui, track, s, dx);
    let ar = take(s.s(96.0));
    algorithm_field(app, ui, track, spec, s, ar);
    let Some(d) = cr.display else {
        return;
    };
    let s = &crate::views::inst::face::on_display(s);
    if app.inst.fm.matrix_view {
        super::matrix::grid(app, ui, track, spec, s, d, "graph");
    } else {
        editor(app, ui, track, spec, s, d);
    }
}

/// The algorithm as a menu field: its name ("DX 5", or "Free"); ‹ › walk the 32 templates,
/// the field opens them as pictures, each saying how many cells it would change.
fn algorithm_field(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let id = ui.id().with("fm-alg");
    let now = algorithm_of(spec);
    let value = now
        .map(|n| tf("fm.alg.short", &[("n", &n.to_string())]))
        .unwrap_or_else(|| t("fm.alg.free_short"));
    let open: bool = ui.data(|d| d.get_temp(id.with("menu"))).unwrap_or(false);
    let tip = t("fm.alg.menu_tip");
    let (_, c) = crate::kit::widgets::MenuField::new(&value)
        .steppers(true)
        .open(open)
        .id(id)
        .name(&tip)
        .style(*s)
        .show_at(ui, rect);
    let n = ALGS as i32;
    let at = now.map(|n| n as i32).unwrap_or(0);
    if c.prev {
        apply_algorithm(app, track, spec, ((at - 2).rem_euclid(n) + 1) as usize);
    } else if c.next {
        apply_algorithm(app, track, spec, (at.rem_euclid(n) + 1) as usize);
    } else if c.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    if open {
        algorithm_popup(app, ui, track, spec, s, id.with("menu"), rect);
    }
}

const ALGS: usize = DX_ALGORITHMS.len();

/// The 32 DX topologies as pictures in a floating kit menu.
fn algorithm_popup(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    id: egui::Id,
    anchor: Rect,
) {
    let tile = Vec2::new(s.s(64.0), s.s(56.0));
    let gap = s.s(4.0);
    let cols = 8;
    let rows = ALGS.div_ceil(cols);
    let size = Vec2::new(
        cols as f32 * (tile.x + gap) + gap,
        rows as f32 * (tile.y + gap) + gap,
    );
    let now = algorithm_of(spec);
    let area = egui::Area::new(id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + Vec2::new(0.0, s.s(2.0)))
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let at = |i: usize| {
                let (c, r) = (i % cols, i / cols);
                Rect::from_min_size(
                    rect.min
                        + Vec2::new(
                            gap + c as f32 * (tile.x + gap),
                            gap + r as f32 * (tile.y + gap),
                        ),
                    tile,
                )
            };
            let hovered = resp
                .hover_pos()
                .and_then(|p| (0..ALGS).find(|i| at(*i).contains(p)));
            with_brush(ui.painter(), |b| {
                crate::kit::menu::paint(b, rect, s, &[], None);
                for (i, a) in DX_ALGORITHMS.iter().enumerate() {
                    let r = at(i);
                    let lit = now == Some(i + 1);
                    let face = if hovered == Some(i) || lit {
                        mix(s.t.menu, s.hue, 0.22)
                    } else {
                        crate::kit::color::shade(s.t.menu, -0.04)
                    };
                    b.rrect(r, s.r(3.0), Fill::Solid(face));
                    if lit {
                        b.rrect_stroke(r, s.r(3.0), s.s(1.5), s.hue);
                    }
                    parts::say(
                        b,
                        Rect::from_min_size(
                            r.min + Vec2::new(s.s(3.0), s.s(1.0)),
                            Vec2::new(s.s(20.0), s.s(12.0)),
                        ),
                        s,
                        &(i + 1).to_string(),
                        Role::Micro,
                        s.t.menu_text,
                        Align2::LEFT_CENTER,
                    );
                    paint_small(
                        b,
                        r.shrink2(Vec2::new(s.s(8.0), s.s(8.0)))
                            .translate(Vec2::new(0.0, s.s(4.0))),
                        &Topo::of_dx(a),
                        s.t.menu_text,
                        s.k,
                    );
                }
            });
            if let Some(i) = hovered {
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &tf(
                        "fm.alg.changes",
                        &[
                            ("n", &(i + 1).to_string()),
                            ("k", &algorithm_changes(spec, i + 1).to_string()),
                        ],
                    ),
                    "",
                    crate::kit::hint::Caps::NONE,
                );
            }
            (resp, hovered)
        });
    let (resp, hovered) = area.inner;
    if resp.clicked() {
        if let Some(i) = hovered {
            apply_algorithm(app, track, spec, i + 1);
        }
        ui.data_mut(|d| d.insert_temp(id, false));
    } else if resp.clicked_elsewhere() {
        ui.data_mut(|d| d.insert_temp(id, false));
    }
}

/// DX7 SysEx as a menu field: Import… (a voice arrives at once, one undo step; a cartridge
/// lists its 32 voices here to pick one).
fn dx7_field(app: &mut CantusApp, ui: &egui::Ui, track: TrackId, s: &Style, rect: Rect) {
    let id = ui.id().with("fm-dx7");
    let open: bool = ui.data(|d| d.get_temp(id.with("menu"))).unwrap_or(false);
    let label = t("fm.dx7");
    let tip = t("fm.import.tip");
    let (_, c) = crate::kit::widgets::MenuField::new(&label)
        .open(open)
        .id(id)
        .name(&tip)
        .style(*s)
        .show_at(ui, rect);
    if c.open {
        parts::toggle_popup(ui, id.with("menu"));
    }
    let bank = app.inst.fm.bank.clone();
    let names: Vec<String> = bank
        .as_ref()
        .map(|v| v.iter().map(|x| x.name()).collect())
        .unwrap_or_default();
    let mut items = vec![Item::action(Src::Key("fm.import"))];
    if !names.is_empty() {
        items.push(Item::Band(Src::Key("fm.import.pick")));
        items.extend(names.iter().map(|n| Item::action(Src::Str(n))));
        items.push(Item::Separator);
        items.push(Item::action(Src::Key("fm.import.forget")));
    }
    let anchor = Rect::from_min_size(rect.min, Vec2::new(s.s(200.0), rect.height()));
    match parts::popup(ui, id.with("menu"), anchor, s, &items) {
        Some(0) => super::import_dialog(app, track),
        Some(i) if !names.is_empty() && i >= 2 && i < 2 + names.len() => {
            if let Some(v) = bank.as_ref().and_then(|b| b.get(i - 2)) {
                super::import_voice(app, track, v);
            }
        }
        Some(i) if !names.is_empty() && i == items.len() - 1 => app.inst.fm.bank = None,
        _ => {}
    }
}

// ---- the graph -----------------------------------------------------------------------------

/// The interactive graph in `rect` (the card's display).
pub(super) fn editor(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let k = s.k;
    let tp = Topo::of(spec);
    let spare_w = 44.0 * k;
    let (pos, spare, out_y) = layout(&tp, rect.shrink(10.0 * k), spare_w, k);
    let h = s.hue;
    let r = R * k;
    let tel_ = app.inst_telemetry();
    let routed: Vec<(String, String)> = connections(spec)
        .filter(|c| c.on)
        .map(|c| (c.dst.to_string(), c.src.to_string()))
        .collect();
    let ring = |key: &str| {
        routed
            .iter()
            .find(|(d, _)| d == key)
            .map(|(_, s)| source_color(s))
    };
    let out_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 10.0 * k, out_y),
        Pos2::new(rect.right() - spare_w - 10.0 * k, rect.bottom() - 6.0 * k),
    );
    // Where the numbers go, so the pills are painted after the lines.
    let mut pills: Vec<Pill> = Vec::new();
    let pointer = ui.input(|i| i.pointer.interact_pos());
    with_brush(&ui.painter_at(rect), |b| {
        // OUT.
        b.rrect(
            out_rect,
            s.r(3.0),
            Fill::Solid(with_alpha(h, if s.t.dark { 0.22 } else { 0.16 })),
        );
        parts::say(
            b,
            out_rect.shrink2(Vec2::new(8.0 * k, 0.0)),
            s,
            &t("fm.out").to_uppercase(),
            Role::Section,
            s.t.text,
            Align2::LEFT_CENTER,
        );
        // Output lines, with their level to drag.
        for o in 0..OPS {
            if !tp.carrier(o) || spare[o] {
                continue;
            }
            let a = pos[o] + Vec2::new(0.0, r);
            let z = Pos2::new(pos[o].x, out_rect.top());
            b.segment(a, z, 2.0 * k, h, false);
            let mut label = String::new();
            write_readout(tp.out[o], "dB", &mut label);
            let key = OP_KEYS[o].out;
            pills.push((
                key,
                Pos2::new(a.x, (a.y + z.y) * 0.5),
                label,
                ring(key),
                None,
            ));
        }
        // Edges.
        for s_ in 0..OPS {
            for tt in 0..OPS {
                let v = tp.m[s_][tt];
                if v == 0.0 || spare[s_] || spare[tt] {
                    continue;
                }
                let key = matrix_key(s_, tt);
                let width = (1.0 + v.abs().min(8.0) * 0.35) * k;
                let label_at = if s_ == tt {
                    // Feedback: a loop on the operator's shoulder.
                    let c = pos[s_] + Vec2::new(r * 1.1, -r * 1.1);
                    b.circle_stroke(c, r * 0.6, width, h);
                    c + Vec2::new(r * 1.9, -r * 0.4)
                } else {
                    let a = pos[s_];
                    let z = pos[tt];
                    let back = s_ < tt;
                    let ctrl = if back {
                        let side = if a.x <= z.x { -1.0 } else { 1.0 };
                        let dx = side * 56.0 * k;
                        [a + Vec2::new(dx, 0.0), z + Vec2::new(dx, 0.0)]
                    } else {
                        [
                            a + Vec2::new(0.0, (z.y - a.y) * 0.5),
                            z - Vec2::new(0.0, (z.y - a.y) * 0.5),
                        ]
                    };
                    let pts = bezier([a, ctrl[0], ctrl[1], z], 24);
                    if back {
                        dashed(b, &pts, width, h, 5.0 * k);
                    } else {
                        b.polyline(&pts, width, h);
                    }
                    // The arrow head at the target.
                    let dir = (z - ctrl[1]).normalized();
                    let tip = z - dir * r;
                    let n = Vec2::new(-dir.y, dir.x);
                    b.polygon(
                        &[
                            tip,
                            tip - dir * 7.0 * k + n * 4.0 * k,
                            tip - dir * 7.0 * k - n * 4.0 * k,
                        ],
                        h,
                    );
                    pts[12]
                };
                let text = index_text(v);
                pills.push((key, label_at, text, ring(key), Some((s_, tt))));
            }
        }
        // Nodes.
        for o in 0..OPS {
            let c = pos[o];
            let rr = if spare[o] { r * 0.8 } else { r };
            let selected = app.inst.fm.selected == o;
            let muted = !tp.on[o];
            let fill = if tp.carrier(o) && !spare[o] {
                mix(s.t.deck, h, 0.35)
            } else {
                s.t.deck
            };
            b.circle(c, rr, Fill::Solid(fill));
            let stroke_c = if spare[o] || muted { s.t.text_dim } else { h };
            b.circle_stroke(c, rr, if selected { 2.5 * k } else { 1.5 * k }, stroke_c);
            if selected {
                b.circle_stroke(c, rr + 4.0 * k, 1.0 * k, s.t.text_dim);
            }
            // Live: how loud the operator is in the newest voice.
            let amp = tel_[tel::AMP + o].clamp(0.0, 1.0);
            if amp > 0.0 && !spare[o] {
                let a0 = -std::f32::consts::FRAC_PI_2;
                b.arc(
                    c,
                    rr + 2.5 * k,
                    a0,
                    a0 + std::f32::consts::TAU * amp,
                    2.5 * k,
                    s.t.highlight,
                    false,
                );
            }
            parts::say(
                b,
                Rect::from_center_size(c, Vec2::splat(rr * 2.0)),
                s,
                NAMES[o],
                Role::Title,
                if muted { s.t.text_dim } else { s.t.text },
                Align2::CENTER_CENTER,
            );
            if !spare[o] {
                parts::say(
                    b,
                    Rect::from_min_size(
                        c + Vec2::new(rr + 3.0 * k, rr * 0.3),
                        Vec2::new(60.0 * k, 14.0 * k),
                    ),
                    s,
                    &freq_text(spec, o),
                    Role::Micro,
                    s.t.text,
                    Align2::LEFT_CENTER,
                );
            }
        }
        if let (Some(from), Some(p)) = (app.inst.fm.graph.from, pointer) {
            b.segment(pos[from], p, 1.5 * k, s.t.text_dim, true);
        }
        if tp.m.iter().flatten().all(|v| *v == 0.0)
            && (0..OPS).filter(|o| tp.carrier(*o)).count() <= 1
        {
            parts::chip(
                b,
                Rect::from_min_size(
                    rect.min + Vec2::new(8.0 * k, 6.0 * k),
                    Vec2::new(rect.width() - 16.0 * k, 16.0 * k),
                ),
                s,
                &t("fm.graph.hint"),
                s.t.text_dim,
                Align2::CENTER_CENTER,
            );
        }
    });
    // The numbers: pills you drag; double-click an edge's to remove it.
    for (key, at, text, rng, edge) in pills {
        let range = if edge.is_some_and(|(a, b)| a == b) {
            Some((0.0, 4.0))
        } else if edge.is_some() {
            Some((-16.0, 16.0))
        } else {
            Some((-60.0, 6.0))
        };
        let per_px = if edge.is_some() { 0.02 } else { 0.1 };
        let ctx = ui.ctx().clone();
        let w = crate::kit::text::measure(
            &ctx,
            &crate::kit::text::Text::new(&text, Role::Value, s.t.text),
            s.tk(),
        )
        .x + 10.0 * k;
        let pr = Rect::from_center_size(at, Vec2::new(w, 16.0 * k));
        let resp = pill_num(
            app,
            ui,
            track,
            spec,
            key,
            pr,
            s,
            &text,
            rng,
            range,
            Some(per_px),
        );
        if let Some((a, b)) = edge {
            if resp.double_clicked() {
                disconnect(app, track, spec, a, b);
            }
            // A back edge (from a later operator, heard one sample late): a return mark
            // beside its number, and the hint says so (round 26: its old z⁻¹ had no glyph).
            let back = a < b;
            if back {
                let mark = Rect::from_center_size(
                    pr.left_center() - Vec2::new(8.0 * k, 0.0),
                    Vec2::splat(11.0 * k),
                );
                crate::kit::with_brush(ui.painter(), |b| {
                    crate::kit::glyph::paint(b, mark, crate::kit::glyph::Glyph::Undo, s.hue)
                });
            }
            if resp.hovered() {
                crate::kit::hint::set_text(
                    ui.ctx(),
                    &tf(
                        if back {
                            "fm.edge.back_tip"
                        } else {
                            "fm.edge.tip"
                        },
                        &[
                            ("from", &NAMES[a]),
                            ("to", &NAMES[b]),
                            ("v", &index_text(tp.m[a][b])),
                        ],
                    ),
                    "",
                    crate::kit::hint::Caps::PARAM,
                );
            }
        }
    }
    // The nodes: click selects, drag wires, right-click opens the operator's menu.
    for o in 0..OPS {
        let c = pos[o];
        let rr = if spare[o] { r * 0.8 } else { r };
        let node = Rect::from_center_size(c, Vec2::splat(rr * 2.0));
        let resp = ui.interact(node, ui.id().with(("fm-node", o)), Sense::click_and_drag());
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &tf("fm.node.tip", &[("op", &NAMES[o])]),
                &freq_text(spec, o),
                crate::kit::hint::Caps {
                    menu: true,
                    ..crate::kit::hint::Caps::NONE
                },
            );
        }
        if resp.clicked() {
            app.inst.fm.selected = o;
        }
        if resp.drag_started() {
            app.inst.fm.graph.from = Some(o);
        }
        if resp.drag_stopped() {
            if let (Some(from), Some(p)) = (app.inst.fm.graph.from.take(), pointer) {
                if let Some(to) = (0..OPS).find(|x| pos[*x].distance(p) <= r * 1.2) {
                    connect(app, track, spec, from, to);
                } else if out_rect.contains(p) {
                    set_carrier(app, track, spec, from, true);
                }
            }
        }
        let menu_id = ui.id().with(("fm-node-menu", o));
        if resp.secondary_clicked() {
            parts::toggle_popup(ui, menu_id);
        }
        let items = [
            Item::action(Src::Key(if tp.carrier(o) {
                "fm.node.unhear"
            } else {
                "fm.node.hear"
            })),
            Item::action(Src::Key(if tp.on[o] {
                "fm.node.mute"
            } else {
                "fm.node.unmute"
            })),
        ];
        match parts::popup(ui, menu_id, node, s, &items) {
            Some(0) => set_carrier(app, track, spec, o, !tp.carrier(o)),
            Some(1) => app.set_inst_value(
                track,
                OP_KEYS[o].on,
                if tp.on[o] { 0.0 } else { 1.0 },
                false,
            ),
            _ => {}
        }
    }
    if app.inst.fm.graph.from.is_some() && !ui.input(|i| i.pointer.primary_down()) {
        app.inst.fm.graph.from = None;
    }
}

/// Draw a topology small (no numbers): the algorithm picker and the inspector.
pub(super) fn paint_small(b: &mut Brush, rect: Rect, tp: &Topo, color: Color32, k: f32) {
    let (pos, spare, out_y) = layout(tp, rect, 0.0, k * 0.35);
    let sz = (rect.width().min(rect.height()) * 0.09).clamp(3.0 * k, 8.0 * k);
    let w = k.max(1.0);
    for a in 0..OPS {
        for c in 0..OPS {
            if tp.m[a][c] != 0.0 && !spare[a] && a != c {
                b.segment(pos[a], pos[c], w, color, false);
            }
        }
        if tp.m[a][a] != 0.0 {
            b.circle_stroke(pos[a] + Vec2::new(sz, -sz), sz * 0.7, w, color);
        }
    }
    for o in 0..OPS {
        if spare[o] {
            continue;
        }
        let r = Rect::from_center_size(pos[o], Vec2::splat(sz * 2.0));
        if tp.carrier(o) {
            b.rect(r, Fill::Solid(color));
            b.segment(
                Pos2::new(pos[o].x, r.bottom()),
                Pos2::new(pos[o].x, out_y),
                w,
                color,
                false,
            );
        } else {
            b.rrect_stroke(r, 1.0, 1.2 * w, color);
        }
    }
    b.segment(
        Pos2::new(rect.left(), out_y),
        Pos2::new(rect.right(), out_y),
        w,
        color,
        false,
    );
}

/// The inspector's read-only picture.
pub(super) fn picture(ui: &mut egui::Ui, spec: &NativeSpec, size: Vec2) -> egui::Response {
    let s = Style::current().function(crate::kit::Function::Source);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.ink));
        paint_small(b, rect.shrink(8.0), &Topo::of(spec), s.hue, 1.0);
    });
    resp
}
