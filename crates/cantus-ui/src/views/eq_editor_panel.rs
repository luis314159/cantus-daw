//! The cards under the display — the selected band's, its dynamics (with its last
//! seconds against its threshold), the analyser's, the output's, and EQ match while it is
//! open — and the selectors inside the display's top-left corner.

use super::display::{Menu, MenuKind};
use super::*;
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
use crate::kit::{with_brush, State};
use crate::views::fx::controls::{self, Cell, Size};
use crate::views::fx::host::KeyTarget;
use egui::{Align2, Ui};

/// The band card's column of switches, at 100 %.
const COLUMN: f32 = 120.0;
/// The key switch over a dynamics history, at 100 %.
const LED_W: f32 = 64.0;
/// The ranges the display offers, as its field lists them.
const RANGE_WORDS: [&str; 5] = [
    "\u{b1}3 dB",
    "\u{b1}6 dB",
    "\u{b1}12 dB",
    "\u{b1}18 dB",
    "\u{b1}30 dB",
];

/// What was clicked on a card's title bar.
#[derive(Clone, Debug, Default)]
struct CardHits {
    led: bool,
    /// The lamp's and the selector name's responses: a band's On and its shape are
    /// parameters (their menus, the learn mode — round 29, review 2 of ADR 0062, #44).
    led_resp: Option<egui::Response>,
    name_resp: Option<egui::Response>,
    prev: bool,
    next: bool,
    /// The selector's name: where its list opens.
    name: Option<Rect>,
    /// The ⋯: where its menu opens.
    menu: Option<Rect>,
}

/// A card (ADR 0024 §4): its title bar — the section's LED when it can be switched, the
/// title, a ‹ type › selector, ⋯ — and `body` laid out left to right inside it.
#[allow(clippy::too_many_arguments)]
fn card<R>(
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    salt: &str,
    title: Src<'_>,
    on: Option<bool>,
    selector: Option<(&str, &str)>,
    menu: bool,
    body: impl FnOnce(&mut Ui, &CardRects) -> R,
) -> (R, CardHits) {
    let look = CardLook {
        title,
        switchable: on.is_some(),
        selector: selector.map(|x| x.0),
        menu,
        display: None,
    };
    let st = State {
        lit: on.unwrap_or(true),
        ..State::REST
    };
    let rects = with_brush(&ui.painter_at(rect.expand(4.0)), |b| {
        band::card(b, rect, s, st, &look)
    });
    let id = ui.id().with(("eq-card", salt));
    let mut hits = CardHits::default();
    let sense_resp = |ui: &mut Ui, r: Rect, part: &str, name: &str, caps: Caps| {
        let resp = ui.interact(r, id.with(part), Sense::click());
        if resp.hovered() {
            hint::set_text(ui.ctx(), name, "", caps);
        }
        resp
    };
    let sense = |ui: &mut Ui, r: Rect, part: &str, name: &str, caps: Caps| -> bool {
        sense_resp(ui, r, part, name, caps).clicked()
    };
    if let Some(led) = rects.led {
        let r = sense_resp(
            ui,
            led.expand(4.0 * s.k),
            "led",
            frame::tr("fx.frame.section_on"),
            Caps::NONE,
        );
        hits.led = r.clicked();
        hits.led_resp = Some(r);
    }
    if let (Some(sel), Some((_, name))) = (rects.selector, selector) {
        let w = 14.0 * s.k;
        let prev = Rect::from_min_size(sel.min, vec2(w, sel.height()));
        let next = Rect::from_min_max(pos2(sel.right() - w, sel.top()), sel.max);
        let mid = Rect::from_min_max(
            pos2(prev.right(), sel.top()),
            pos2(next.left(), sel.bottom()),
        );
        hits.prev = sense(ui, prev, "prev", name, Caps::NONE);
        hits.next = sense(ui, next, "next", name, Caps::NONE);
        let menu_caps = Caps {
            menu: true,
            ..Caps::NONE
        };
        let r = sense_resp(ui, mid, "name", name, menu_caps);
        if r.clicked() {
            hits.name = Some(mid);
        }
        hits.name_resp = Some(r);
    }
    if let Some(m) = rects.menu {
        if sense(
            ui,
            m.expand(2.0 * s.k),
            "more",
            frame::tr("shell.more"),
            Caps::NONE,
        ) {
            hits.menu = Some(m);
        }
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rects.body)
            .id_salt(("eq-card-body", salt))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing = vec2(s.d(4.0), s.d(5.0));
    if on == Some(false) {
        child.disable();
    }
    (body(&mut child, &rects), hits)
}

/// A line of words in a box inside a card: a message, an empty state.
fn say(ui: &mut Ui, s: &Style, text: &str, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width * s.k, s.d(18.0)), Sense::hover());
    with_brush(ui.painter(), |b| {
        let t = Text::new(text, Role::Body, color)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = ktext::paint(b, rect, &t, s.tk());
    });
}

// ---- inside the display, top-left --------------------------------------------------------

/// The display's selectors: the key strip, EQ match, the piano, the dB range — then the
/// processing mode, and the linear phase's resolution while it is the mode.
pub(super) fn head(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    words: &mut Words,
) {
    let k = s.k;
    let inner = rect.shrink2(vec2(8.0 * k, 4.0 * k));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .id_salt("eq-head")
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = s.d(6.0);
    key_picker(host, st, ui, s);
    let matching = st.matching.is_some();
    if controls::key(
        ui,
        s,
        Src::Key("eq.match"),
        frame::tr("eq.match_tip"),
        64.0,
        matching,
    )
    .clicked()
    {
        st.matching = if matching {
            None
        } else {
            Some(MatchState::default())
        };
    }
    if controls::led(
        ui,
        s,
        st.piano,
        Src::Key("eq.piano"),
        frame::tr("eq.piano_tip"),
        72.0,
    )
    .clicked()
    {
        st.piano = !st.piano;
    }
    let range = st.range_db;
    let text = words.range.get(range.round() as u32, |w| {
        let _ = write!(w, "\u{b1}{range:.0} dB");
    });
    let id = ui.id().with("eq-range");
    if let Some(at) = controls::field(ui, s, id, text, frame::tr("eq.range_tip"), 80.0) {
        let items: [Item<'_>; 5] = std::array::from_fn(|i| {
            frame::radio(Src::Str(RANGE_WORDS[i]), (RANGES[i] - range).abs() < 0.01)
        });
        if let Some(i) = controls::list(ui, s, id, at, 96.0, &items) {
            st.range_db = RANGES[i];
        }
    }
    ui.add_space(s.d(10.0));
    let spec = host.spec();
    let mode = Mode::from_index(spec.get("mode").round() as usize);
    let res = Resolution::from_index(spec.get("lp_res").round() as usize);
    let labels = MODE_KEYS.map(Src::Key);
    controls::segmented_param(ui, s, host, "mode", &labels, 288.0);
    if mode == Mode::LinearPhase {
        let id = ui.id().with("eq-res");
        let value = frame::tr(RES_KEYS[res.index().min(3)]);
        if let Some(at) = controls::field_key(
            ui,
            s,
            host,
            "lp_res",
            id,
            value,
            frame::tr("eq.res_tip"),
            100.0,
        ) {
            // The rows name each resolution's latency: built only while the list is open.
            let sr = host.sample_rate();
            let rows: Vec<String> = Resolution::ALL
                .iter()
                .map(|r| {
                    let lat = latency_of(Mode::LinearPhase, *r, sr);
                    format!(
                        "{} \u{b7} {:.0} ms",
                        frame::tr(RES_KEYS[r.index().min(3)]),
                        lat as f32 / sr * 1_000.0
                    )
                })
                .collect();
            let items: Vec<Item<'_>> = rows
                .iter()
                .zip(Resolution::ALL)
                .map(|(w, r)| frame::radio(Src::Str(w), r == res))
                .collect();
            if let Some(i) = controls::list(ui, s, id, at, 160.0, &items) {
                host.set("lp_res", i as f32, None);
            }
        }
    }
}

/// The key strip: whose output the key-listening bands, the key's spectrum, the collisions
/// and EQ match hear — any strip of the desk (Cantus), or the plug-in's sidechain input.
fn key_picker(host: &mut dyn FxHost, st: &mut EqEditor, ui: &mut Ui, s: &Style) {
    let rev = host.revision();
    if st.key_source.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key_source = Some((rev, host.key_source()));
    }
    match st.key_source.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current,
            label,
            options,
        }) => {
            let current = *current;
            let id = ui.id().with("eq-key");
            let value = match current {
                Some(_) => label.as_str(),
                None => frame::tr("eq.key_none"),
            };
            if let Some(at) = controls::field(ui, s, id, value, frame::tr("eq.key"), 156.0) {
                let mut items = vec![frame::radio(Src::Key("eq.key_none"), current.is_none())];
                items.extend(
                    options
                        .iter()
                        .map(|(sid, text)| frame::radio(Src::Str(text), current == Some(*sid))),
                );
                if let Some(i) = controls::list(ui, s, id, at, 200.0, &items) {
                    let target = if i == 0 {
                        None
                    } else {
                        options.get(i - 1).map(|o| o.0)
                    };
                    drop(items);
                    host.set_key_source(KeyTarget::Strip(target));
                }
            }
        }
        Some(KeySource::Switch(on)) => {
            let on = *on;
            if controls::led(
                ui,
                s,
                on,
                Src::Key("eq.key"),
                frame::tr("eq.key_tip"),
                110.0,
            )
            .clicked()
            {
                host.set_key_source(KeyTarget::Switch(!on));
            }
        }
        None => {}
    }
}

/// Whether a key strip is chosen (Cantus) or the sidechain switched on (a plug-in).
fn key_chosen(st: &EqEditor) -> bool {
    match st.key_source.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips { current, .. }) => current.is_some(),
        Some(KeySource::Switch(on)) => *on,
        None => false,
    }
}

// ---- the band ----------------------------------------------------------------------------

/// A cell's own pill words.
type Pill = dyn Fn(f32, &mut String);

fn slope_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:.0} dB/oct", (v / 6.0).round() * 6.0);
}

fn ratio_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.1}:1");
}

/// Whole dB in the small cells, where a decimal would not fit the pill.
fn db0_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.0} dB");
}

fn thr_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0} dB");
}

fn ms_pill(v: f32, b: &mut String) {
    if v < 10.0 {
        let _ = write!(b, "{v:.1} ms");
    } else if v < 1_000.0 {
        let _ = write!(b, "{v:.0} ms");
    } else {
        let _ = write!(b, "{:.2} s", v / 1_000.0);
    }
}

/// A cell on one field of one band: a drag is one step of the host's undo; the display's
/// range widens when a gain leaves it.
fn band_cell(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    band: usize,
    field: &str,
    size: Size,
) {
    let info = binfo(band, field);
    let v = host.spec().get(info.key);
    let format: Option<&Pill> = match (field, size) {
        ("slope", _) => Some(&slope_pill),
        ("ratio", _) => Some(&ratio_pill),
        ("thr", Size::S) => Some(&thr_pill),
        ("range", Size::S) => Some(&db0_pill),
        ("att" | "rel", _) => Some(&ms_pill),
        _ => None,
    };
    let mut c = Cell::param(info, v).size(size);
    if let Some(f) = format {
        c = c.format(f);
    }
    if field == "slope" {
        c = c.stepped(false);
    }
    let out = c.show_in(ui, s, host, info.key);
    if let Some(mut v) = out.changed {
        if field == "slope" {
            v = ((v / 6.0).round() * 6.0).clamp(6.0, 96.0);
        }
        host.set(info.key, v, Some(info.key));
        if field == "gain" {
            st.range_db = st.range_for(v);
        }
    }
    if out.done {
        host.end_gesture();
    }
}

/// The selected band's card: its on/off as the card's LED, its shape as the ‹ type ›
/// selector, FREQ / GAIN / Q (SLOPE for the cuts), and a column with its placement, the
/// audition, dynamics and the gain–Q interaction. ⋯ splits or removes it.
pub(super) fn band_card(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    words: &mut Words,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
) {
    let spec = host.spec();
    let Some(b) = st.primary.filter(|b| used(spec, *b)) else {
        // No band chosen: the card says how to make one.
        card(
            ui,
            s,
            rect,
            "band",
            Src::Key("eq.bands_card"),
            None,
            None,
            false,
            |ui, cr| {
                controls::column(ui, s, cr.body.width() / s.k, |ui| {
                    say(
                        ui,
                        s,
                        frame::tr("eq.empty"),
                        s.t.text_dim,
                        cr.body.width() / s.k,
                    );
                    say(
                        ui,
                        s,
                        frame::tr("eq.select_hint"),
                        s.t.text_dim,
                        cr.body.width() / s.k,
                    );
                });
            },
        );
        return;
    };
    let band = super::bands_of(st, host)[b];
    let spec = host.spec();
    let on = spec.get(bkey(b, "on")) >= 0.5;
    let others = st.selection.len().saturating_sub(1);
    let title = words.title.get((b, others), |w| {
        if others == 0 {
            w.push_str(&tf("eq.band_n", &[("n", &(b + 1))]));
        } else {
            w.push_str(&tf("eq.band_more", &[("n", &(b + 1)), ("more", &others)]));
        }
    });
    let shape = band.shape.index().min(8);
    let bs = s.with_hue(band_color(b));
    let (_, hits) = card(
        ui,
        &bs,
        rect,
        "band",
        Src::Str(title),
        Some(on),
        Some((frame::tr(SHAPE_KEYS[shape]), frame::tr("fx.eq.shape"))),
        true,
        |ui, _| {
            let cut = band.shape.has_slope();
            band_cell(ui, &bs, host, st, b, "freq", Size::M);
            if cut {
                band_cell(ui, &bs, host, st, b, "q", Size::M);
                band_cell(ui, &bs, host, st, b, "slope", Size::M);
            } else {
                if band.shape.has_gain() {
                    band_cell(ui, &bs, host, st, b, "gain", Size::M);
                }
                band_cell(ui, &bs, host, st, b, "q", Size::M);
            }
            controls::column(ui, &bs, COLUMN, |ui| {
                let labels = PLACE_KEYS.map(Src::Key);
                let place = band.placement.index().min(4);
                if let Some(i) = controls::segmented_key(
                    ui,
                    &bs,
                    host,
                    bkey(b, "place"),
                    place,
                    &labels,
                    frame::tr("fx.eq.place"),
                    COLUMN,
                ) {
                    set_band(host, b, &[("place", i as f32)], None);
                }
                let soloed = host.spec().get("solo").round() as usize == b + 1;
                let r = controls::led(
                    ui,
                    &bs,
                    soloed,
                    Src::Key("eq.solo_led"),
                    frame::tr("eq.solo_tip"),
                    COLUMN,
                );
                // The solo is a parameter (which band, or none): its menu, its mark, the
                // learn mode (round 29, review 3 of ADR 0062, #35).
                let learnt =
                    controls::stepped_offer(ui, &bs, host, "solo", frame::tr("eq.solo_tip"), &r);
                if r.clicked() && !learnt {
                    set_solo(host, (!soloed).then_some(b));
                }
                if band.shape.has_gain() {
                    let r = controls::led(
                        ui,
                        &bs,
                        band.dynamic,
                        Src::Key("fx.eq.dyn"),
                        frame::tr("eq.dyn_tip"),
                        COLUMN,
                    );
                    let name = frame::tr("eq.dyn_tip");
                    let learnt = controls::stepped_offer(ui, &bs, host, bkey(b, "dyn"), name, &r);
                    if r.clicked() && !learnt {
                        set_band(
                            host,
                            b,
                            &[("dyn", if band.dynamic { 0.0 } else { 1.0 })],
                            None,
                        );
                    }
                }
                if band.shape == EqShape::Bell {
                    let gq = host.spec().get(bkey(b, "gq")) >= 0.5;
                    let r = controls::led(
                        ui,
                        &bs,
                        gq,
                        Src::Key("fx.eq.gq"),
                        frame::tr("eq.gq_tip"),
                        COLUMN,
                    );
                    let name = frame::tr("eq.gq_tip");
                    let learnt = controls::stepped_offer(ui, &bs, host, bkey(b, "gq"), name, &r);
                    if r.clicked() && !learnt {
                        set_band(host, b, &[("gq", if gq { 0.0 } else { 1.0 })], None);
                    }
                }
            });
        },
    );
    // The band's On and its shape are parameters (round 29, review 2 of ADR 0062, #44).
    let s_on = frame::tr("fx.frame.section_on");
    let led_learnt = hits
        .led_resp
        .as_ref()
        .is_some_and(|r| controls::stepped_offer(ui, &bs, host, bkey(b, "on"), s_on, r));
    let shape_learnt = hits.name_resp.as_ref().is_some_and(|r| {
        controls::stepped_offer(ui, &bs, host, bkey(b, "shape"), frame::tr("fx.eq.shape"), r)
    });
    if hits.led && !led_learnt {
        set_band(host, b, &[("on", if on { 0.0 } else { 1.0 })], None);
    }
    if (hits.prev || hits.next) && !shape_learnt {
        let n = SHAPE_KEYS.len();
        let to = if hits.next {
            (shape + 1) % n
        } else {
            (shape + n - 1) % n
        };
        set_band(host, b, &[("shape", to as f32)], None);
    }
    if let Some(r) = hits.name {
        st.menu = Some(Menu::new(MenuKind::Shape(b), r.left_bottom()));
    }
    if let Some(r) = hits.menu {
        st.menu = Some(Menu::new(MenuKind::Band(b), r.left_bottom()));
    }
}

/// A dynamic band's card: which way it moves (the ‹ Above › selector), its five numbers,
/// and its last seconds, where the threshold is dragged.
#[allow(clippy::too_many_arguments)]
pub(super) fn dynamics_card(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    b: usize,
) {
    let ds = s.function(Function::Dynamics);
    let band = super::bands_of(st, host)[b];
    let dir = if band.below {
        "eq.dir.below_long"
    } else {
        "eq.dir.above_long"
    };
    let (_, hits) = card(
        ui,
        &ds,
        rect,
        "dyn",
        Src::Key("eq.dynamics"),
        Some(true),
        Some((frame::tr(dir), frame::tr("eq.dir_tip"))),
        false,
        |ui, cr| {
            for field in ["thr", "range", "ratio", "att", "rel"] {
                band_cell(ui, &ds, host, st, b, field, Size::S);
            }
            let used_w = controls::cells_width(&[Size::S; 5]) * s.k + s.d(4.0);
            let w = (cr.body.width() - used_w).max(60.0 * s.k);
            let (hr, _) = ui.allocate_exact_size(vec2(w, cr.body.height()), Sense::hover());
            history(host, st, ui, &ds, hr, b, &band);
        },
    );
    if hits.led {
        set_band(host, b, &[("dyn", 0.0)], None);
    }
    if hits.prev || hits.next || hits.name.is_some() {
        set_band(
            host,
            b,
            &[("dir", if band.below { 0.0 } else { 1.0 })],
            None,
        );
    }
}

/// The band's last seconds: its detector's level (a fill), its threshold (a dashed line
/// you drag), and the gain it moved by (a line in its colour, on the range's scale). Its
/// top-left switch makes the detector listen to the key strip.
#[allow(clippy::too_many_arguments)]
fn history(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    b: usize,
    band: &BandSettings,
) {
    const LO: f32 = -72.0;
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("eq-history"), Sense::click_and_drag());
    let head = 26.0 * k;
    let plot = Rect::from_min_max(pos2(rect.left(), rect.top() + head), rect.max);
    let y = |d: f32| plot.bottom() - ((d - LO) / -LO).clamp(0.0, 1.0) * plot.height();
    let d_of = |py: f32| LO + (plot.bottom() - py) / plot.height() * -LO;
    let range = band.range_db.abs().max(1.0);
    let gy = |g: f32| plot.center().y - (g / range).clamp(-1.0, 1.0) * plot.height() * 0.45;
    let ty = y(band.threshold_db);
    let near = resp
        .hover_pos()
        .is_some_and(|pt| plot.contains(pt) && (pt.y - ty).abs() < 6.0 * k);
    let hot = near || st.hist_drag;
    let color = band_color(b);
    let hist = &st.history;
    let n = hist.len().min(HISTORY);
    let dx = plot.width() / HISTORY as f32;
    let x0 = plot.right() - dx * n as f32;
    let mut fill = [Pos2::ZERO; HISTORY];
    let mut moved = [Pos2::ZERO; HISTORY];
    for (i, (d, l)) in hist.iter().enumerate().take(n) {
        let x = x0 + dx * i as f32;
        fill[i] = pos2(x, y(l[b]));
        moved[i] = pos2(x, gy(d[b]));
    }
    let now = hist.back().map_or(0.0, |h| h.0[b]);
    let no_key = band.external && !key_chosen(st);
    with_brush(&ui.painter_at(rect), |bb| {
        canvas::ground_b(bb, rect, &ink, s.hue_tok);
        let px = bb.px();
        bb.hline(
            plot.left(),
            plot.right(),
            bb.snap_mid(gy(0.0)),
            px,
            ink.grid_strong,
        );
        if n > 1 {
            bb.area(&fill[..n], plot.bottom(), with_alpha(ink.dim, 0.30));
            bb.polyline(&moved[..n], 2.0 * k, color);
        }
        let w = if hot { 2.0 } else { 1.25 } * k;
        super::display::dashed(
            bb,
            &[pos2(plot.left(), ty), pos2(plot.right(), ty)],
            w,
            ink.hue,
            6.0 * k,
            4.0 * k,
        );
        // Words last: how far it has moved now, right of the key switch — only where it
        // fits whole, never under the switch — and the threshold on its line.
        let words = Rect::from_min_max(
            pos2(rect.left() + (LED_W + 10.0) * k, rect.top()),
            pos2(rect.right() - 6.0 * k, rect.top() + head),
        );
        let mut buf = [0u8; 16];
        let mut cur = std::io::Cursor::new(&mut buf[..]);
        let _ = std::io::Write::write_fmt(&mut cur, format_args!("{now:+.1} dB"));
        let n = cur.position() as usize;
        let t = Text::new(
            std::str::from_utf8(&buf[..n]).unwrap_or(""),
            Role::Micro,
            color,
        )
        .caps(false)
        .align(Align2::RIGHT_CENTER);
        if ktext::measure(bb.ctx(), &t, tk).x <= words.width() {
            let _ = ktext::paint(bb, words, &t, tk);
        }
        let above = ty - 3.0 * k > plot.top() + 14.0 * k;
        canvas::putf(
            bb,
            pos2(
                plot.right() - 6.0 * k,
                if above { ty - 3.0 * k } else { ty + 3.0 * k },
            ),
            if above {
                Align2::RIGHT_BOTTOM
            } else {
                Align2::RIGHT_TOP
            },
            Role::Body,
            ink.hue,
            tk,
            Some(ink.plate),
            format_args!("{} {:.1} dB", frame::tr("fx.eq.thr"), band.threshold_db),
        );
        if no_key {
            canvas::put(
                bb,
                pos2(plot.left() + 6.0 * k, plot.bottom() - 4.0 * k),
                Align2::LEFT_BOTTOM,
                &Text::key("eq.src.no_key", Role::Micro, ink.danger),
                tk,
                Some(ink.plate),
            );
        }
        canvas::edge_b(bb, rect, &ink);
    });
    // The detector's source: this band, or the key strip.
    let led_rect =
        Rect::from_min_size(rect.min + vec2(4.0 * k, 3.0 * k), vec2(LED_W * k, 20.0 * k));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(led_rect).id_salt("eq-src"));
    if controls::led(
        &mut child,
        s,
        band.external,
        Src::Key("eq.src.key"),
        frame::tr("eq.src_tip"),
        LED_W,
    )
    .clicked()
    {
        set_band(
            host,
            b,
            &[("key", if band.external { 0.0 } else { 1.0 })],
            None,
        );
    }
    if hot && !led_rect.contains(resp.hover_pos().unwrap_or(Pos2::ZERO)) || st.hist_drag {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.drag_started() {
        let at = ui.input(|i| i.pointer.press_origin());
        st.hist_drag = at.is_some_and(|pt| (pt.y - ty).abs() < 8.0 * k);
    }
    if resp.dragged() && st.hist_drag {
        if let Some(pt) = resp.interact_pointer_pos() {
            let key = bkey(b, "thr");
            host.set(key, d_of(pt.y).clamp(-80.0, 0.0), Some(key));
        }
    }
    if resp.drag_stopped() && std::mem::take(&mut st.hist_drag) {
        host.end_gesture();
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("eq.history"),
            frame::tr("eq.history_tip"),
            Caps::NONE,
        );
    }
}

// ---- the analyser, the output ------------------------------------------------------------

/// The analyser: what it draws (the input, the output, the key strip), whether it holds,
/// grabs peaks or hatches collisions — each an LED — and its resolution, speed, range and
/// tilt in one menu field.
pub(super) fn analyser_card(
    st: &mut EqEditor,
    words: &mut Words,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
) {
    let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5 || key_chosen(st);
    card(
        ui,
        s,
        rect,
        "analyser",
        Src::Key("eq.analyser"),
        None,
        None,
        false,
        |ui, cr| {
            let w = cr.body.width() / s.k;
            controls::column(ui, s, w, |ui| {
                // Three switches a row when the card is wide, else two.
                let per = if w >= 3.0 * 84.0 + 8.0 { 3 } else { 2 };
                let cw = (w - 4.0 * (per - 1) as f32) / per as f32;
                let show = &mut st.show;
                let leds: [(&str, &str, bool, bool); 6] = [
                    ("eq.an.pre_led", "eq.an.pre_tip", show.pre, true),
                    ("eq.an.post_led", "eq.an.post_tip", show.post, true),
                    ("eq.an.key", "eq.an.key_tip", show.key, key_on),
                    ("eq.an.freeze", "eq.an.freeze_tip", show.freeze, true),
                    ("eq.an.grab", "eq.an.grab_tip", show.grab, true),
                    (
                        "eq.an.collisions",
                        "eq.an.collisions_tip",
                        show.collisions,
                        key_on,
                    ),
                ];
                let mut flip = None;
                for (r, row) in leds.chunks(per).enumerate() {
                    // Exactly a switch high: `horizontal` would pad the row to egui's
                    // interact height, and at 75 % the last row would leave the card.
                    let size = vec2(w * s.k, s.d(20.0));
                    let lay = egui::Layout::left_to_right(egui::Align::Center);
                    ui.allocate_ui_with_layout(size, lay, |ui| {
                        ui.spacing_mut().item_spacing.x = s.d(4.0);
                        for (c, (label, tip, on, enabled)) in row.iter().enumerate() {
                            let led = |ui: &mut Ui| {
                                controls::led(ui, s, *on, Src::Key(label), frame::tr(tip), cw)
                            };
                            // A disabled scope only where one is needed: each is a child Ui.
                            let resp = if *enabled {
                                led(ui)
                            } else {
                                ui.add_enabled_ui(false, led).inner
                            };
                            if resp.clicked() {
                                flip = Some(r * per + c);
                            }
                        }
                    });
                }
                match flip {
                    Some(0) => show.pre = !show.pre,
                    Some(1) => show.post = !show.post,
                    Some(2) => show.key = !show.key,
                    Some(3) => show.freeze = !show.freeze,
                    Some(4) => show.grab = !show.grab,
                    Some(5) => show.collisions = !show.collisions,
                    _ => {}
                }
                let v = *show;
                let text = words.analyser.get(
                    (
                        v.resolution,
                        v.speed,
                        v.range_db as u32,
                        (v.tilt * 10.0).round() as u32,
                    ),
                    |t| {
                        let _ = write!(
                            t,
                            "{} \u{b7} {} \u{b7} {:.0} dB \u{b7} {:.1} dB/oct",
                            frame::tr(AN_RES_KEYS[v.resolution.min(3)]),
                            frame::tr(SPEED_KEYS[v.speed.min(4)]),
                            v.range_db,
                            v.tilt
                        );
                    },
                );
                let id = ui.id().with("eq-an-settings");
                if let Some(at) =
                    controls::field(ui, s, id, text, frame::tr("eq.an.settings_tip"), w)
                {
                    analyser_list(st, ui, s, id, at);
                }
            });
        },
    );
}

/// The analyser's settings, built when the field opens.
fn analyser_list(st: &mut EqEditor, ui: &mut Ui, s: &Style, id: egui::Id, at: controls::Anchor) {
    let v = st.show;
    let res: Vec<String> = (0..4)
        .map(|r| format!("{} \u{b7} {}", frame::tr(AN_RES_KEYS[r]), 1024 << r))
        .collect();
    let ranges: Vec<String> = AnalyserView::RANGES
        .iter()
        .map(|r| format!("{r:.0} dB"))
        .collect();
    let tilts: Vec<String> = AnalyserView::TILTS
        .iter()
        .map(|t| format!("{t:.1} dB/oct"))
        .collect();
    let mut items: Vec<Item<'_>> = Vec::with_capacity(20);
    let mut acts: Vec<(u8, usize)> = Vec::with_capacity(20);
    items.push(Item::Band(Src::Key("eq.an.resolution")));
    acts.push((9, 0));
    for (i, w) in res.iter().enumerate() {
        items.push(frame::radio(Src::Str(w), v.resolution == i));
        acts.push((0, i));
    }
    items.push(Item::Band(Src::Key("eq.an.speed")));
    acts.push((9, 0));
    for (i, key) in SPEED_KEYS.iter().enumerate() {
        items.push(frame::radio(Src::Key(key), v.speed == i));
        acts.push((1, i));
    }
    items.push(Item::Band(Src::Key("eq.an.range")));
    acts.push((9, 0));
    for (i, w) in ranges.iter().enumerate() {
        items.push(frame::radio(
            Src::Str(w),
            (AnalyserView::RANGES[i] - v.range_db).abs() < 0.01,
        ));
        acts.push((2, i));
    }
    items.push(Item::Band(Src::Key("eq.an.tilt")));
    acts.push((9, 0));
    for (i, w) in tilts.iter().enumerate() {
        items.push(frame::radio(
            Src::Str(w),
            (AnalyserView::TILTS[i] - v.tilt).abs() < 0.01,
        ));
        acts.push((3, i));
    }
    if let Some(i) = controls::list(ui, s, id, at, 200.0, &items) {
        match acts.get(i).copied() {
            Some((0, r)) => st.show.resolution = r,
            Some((1, sp)) => st.show.speed = sp,
            Some((2, r)) => st.show.range_db = AnalyserView::RANGES[r],
            Some((3, t)) => st.show.tilt = AnalyserView::TILTS[t],
            _ => {}
        }
        // Read again at the new settings.
        st.buf.fresh = true;
    }
}

/// The output (in the output's hue): its gain and the gain scale, and auto gain — with
/// the gain it is applying now.
pub(super) fn output_card(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    words: &mut Words,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
) {
    let size = Size::M;
    let _ = st;
    let so = s.function(Function::Output);
    let auto = host.spec().get("auto") >= 0.5;
    let gain = tel[aequalis::telemetry::AUTO_GAIN];
    card(
        ui,
        &so,
        rect,
        "output",
        Src::Key("kit.param.output"),
        None,
        None,
        false,
        |ui, cr| {
            // In the OUTPUT card, the output's level is its GAIN.
            if let Some(info) = host.spec().kind.param("out") {
                let out = Cell::param(info, host.spec().get("out"))
                    .size(size)
                    .label(Src::Key("eq.out_gain"))
                    .show_in(ui, &so, host, "out");
                if let Some(v) = out.changed {
                    host.set("out", v, Some("out"));
                }
                if out.done {
                    host.end_gesture();
                }
            }
            controls::param(ui, &so, host, "scale", size);
            let w = (cr.body.width() / s.k - controls::cells_width(&[size; 2]) - 4.0).max(56.0);
            controls::column(ui, &so, w, |ui| {
                controls::led_param(ui, &so, host, "auto", Src::Key("fx.eq.auto"), w);
                if auto {
                    let text = words.auto.get((gain * 10.0).round() as i32, |t| {
                        let _ = write!(t, "{gain:+.1} dB");
                    });
                    controls::readout(ui, &so, text, true, frame::tr("eq.auto_tip"), w);
                }
            });
        },
    );
}

// ---- EQ match ------------------------------------------------------------------------------

/// EQ match, in the band cards' place: listen to this strip and the key strip, then fit
/// the difference into bands — with an amount, a band budget and limits, and the residual
/// stated.
#[allow(clippy::too_many_arguments)]
pub(super) fn match_card(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    words: &mut Words,
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    sr: f32,
) {
    let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5;
    let rev = host.revision();
    let key_name = match st.key_source.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current: Some(_),
            label,
            ..
        }) => Some(label.clone()),
        Some(KeySource::Switch(true)) => Some(t("eq.key")),
        _ => None,
    };
    let title = words.match_title.get(rev, |w| match &key_name {
        Some(name) => w.push_str(&tf("eq.match_to_title", &[("strip", name)])),
        None => w.push_str(&t("eq.match")),
    });
    let Some(m) = st.matching.as_mut() else {
        return;
    };
    m.refit(sr);
    let mut apply = None;
    let mut close = false;
    // How to read the display while matching: in the help line, over the card (its
    // controls say what they do instead).
    if ui.rect_contains_pointer(rect) {
        hint::set_text(
            ui.ctx(),
            frame::tr("eq.match"),
            frame::tr("eq.match_hint"),
            Caps::NONE,
        );
    }
    card(
        ui,
        s,
        rect,
        "match",
        Src::Str(title),
        None,
        None,
        false,
        |ui, cr| {
            controls::column(ui, s, 128.0, |ui| {
                if controls::led(
                    ui,
                    s,
                    m.listening,
                    Src::Key("eq.match_listen"),
                    frame::tr("eq.match_listen_tip"),
                    128.0,
                )
                .clicked()
                {
                    m.listening = !m.listening;
                }
                let tenths = (m.frames as f32 / 6.0).round() as usize;
                let heard = words.heard.get(tenths, |w| {
                    w.push_str(&tf(
                        "eq.match_heard",
                        &[("s", &format!("{:.1}", tenths as f32 / 10.0))],
                    ))
                });
                controls::readout(ui, s, heard, false, "", 128.0);
                if controls::key(
                    ui,
                    s,
                    Src::Key("eq.match_reset"),
                    frame::tr("eq.match_reset"),
                    128.0,
                    false,
                )
                .clicked()
                {
                    *m = MatchState {
                        amount: m.amount,
                        budget: m.budget,
                        max_boost: m.max_boost,
                        max_cut: m.max_cut,
                        ..MatchState::default()
                    };
                }
            });
            let amount = Cell::new(m.amount, 0.0..=1.0, 1.0)
                .unit("%")
                .label(Src::Key("eq.match_amount"))
                .name(frame::tr("eq.match_amount"))
                .show(ui, s);
            if let Some(v) = amount.changed {
                m.amount = v;
            }
            let count = |v: f32, b: &mut String| {
                let _ = write!(b, "{v:.0}");
            };
            let budget = Cell::new(m.budget as f32, 1.0..=12.0, 6.0)
                .stepped(true)
                .format(&count)
                .label(Src::Key("eq.match_bands"))
                .name(frame::tr("eq.match_bands"))
                .show(ui, s);
            if let Some(v) = budget.changed {
                m.budget = v.round() as usize;
            }
            let boost = Cell::new(m.max_boost, 0.0..=18.0, 9.0)
                .unit("dB")
                .label(Src::Key("eq.match_boost"))
                .name(frame::tr("eq.match_boost"))
                .show(ui, s);
            if let Some(v) = boost.changed {
                m.max_boost = v;
            }
            let minus = |v: f32, b: &mut String| {
                let _ = write!(b, "\u{2212}{v:.1} dB");
            };
            let cut = Cell::new(m.max_cut, 0.0..=24.0, 12.0)
                .format(&minus)
                .label(Src::Key("eq.match_cut"))
                .name(frame::tr("eq.match_cut"))
                .show(ui, s);
            if let Some(v) = cut.changed {
                m.max_cut = v;
            }
            let used_w = 128.0 + controls::cells_width(&[Size::M; 4]) + 8.0;
            let w = (cr.body.width() / s.k - used_w).max(120.0);
            controls::column(ui, s, w, |ui| {
                match (&m.fit, key_name.is_some()) {
                    (_, false) => say(ui, s, frame::tr("eq.match_no_key"), s.t.danger, w),
                    (Some(fit), true) => {
                        let text = words.result.get(
                            (
                                fit.bands.len(),
                                (fit.rms_db * 10.0).round() as i32,
                                (fit.max_db * 10.0).round() as i32,
                            ),
                            |t| {
                                t.push_str(&tf(
                                    "eq.match_result",
                                    &[
                                        ("n", &fit.bands.len()),
                                        ("rms", &format!("{:.1}", fit.rms_db)),
                                        ("max", &format!("{:.1}", fit.max_db)),
                                    ],
                                ))
                            },
                        );
                        say(ui, s, text, s.t.text, w);
                    }
                    (None, true) => say(
                        ui,
                        s,
                        frame::tr("eq.match_wait"),
                        if key_on { s.t.text } else { s.t.text_dim },
                        w,
                    ),
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = s.d(6.0);
                    let kw = ((w - 6.0) * 0.5).min(140.0);
                    let ready = m.fit.as_ref().is_some_and(|f| !f.bands.is_empty());
                    let add = ui
                        .add_enabled_ui(ready, |ui| {
                            controls::key(
                                ui,
                                s,
                                Src::Key("eq.match_apply"),
                                frame::tr("eq.match_apply"),
                                kw,
                                ready,
                            )
                        })
                        .inner;
                    if add.clicked() {
                        apply = m.fit.clone();
                    }
                    if controls::key(
                        ui,
                        s,
                        Src::Key("layout.cancel"),
                        frame::tr("layout.cancel"),
                        kw,
                        false,
                    )
                    .clicked()
                    {
                        close = true;
                    }
                });
            });
        },
    );
    if let Some(fit) = apply {
        apply_fit(host, st, &fit);
        close = true;
    }
    if close {
        st.matching = None;
    }
}
