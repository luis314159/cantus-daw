//! (a) The mixer desk: FL's strip (ADR 0024 §6) — number cell, vertical name, meter,
//! mute/solo, pan, fader, phase/swap, FX/record, sends — with a legend column naming the
//! rows once for the whole desk, a master dock, a returns dock, one strip selected and
//! one colourful. Every meter goes into one batch.

use crate::common::{self, title_bar};
use cantus_ui::kit::color::{on_fill, safe_track, with_alpha, FillSize};
use cantus_ui::kit::fader::{self, FaderLook};
use cantus_ui::kit::glyph::{self, Glyph};
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::meter::{Levels, MeterBatch};
use cantus_ui::kit::text::{self, Elide, Policy, Role, Text};
use cantus_ui::kit::{chrome, led, Brush, Fill, State, Style, Tok};
use egui::{Align2, Color32, Painter, Pos2, Rect, Vec2};

pub const SIZE: Vec2 = Vec2::new(572.0, 540.0);

struct Strip {
    name: &'static str,
    color: u32,
    level: [f32; 2],
    fader: f32,
    pan: f32,
    fx: bool,
    rec: bool,
    muted: bool,
    solo: bool,
}

const STRIPS: [Strip; 10] = [
    Strip {
        name: "Kick",
        color: 0xbb7fc4,
        level: [-4.5, -4.8],
        fader: 0.80,
        pan: 0.5,
        fx: true,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Snare Top",
        color: 0x96b176,
        level: [-9.0, -8.2],
        fader: 0.74,
        pan: 0.46,
        fx: true,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Ghost Kick Layer 2",
        color: 0x7b9cba,
        level: [-16.0, -15.0],
        fader: 0.62,
        pan: 0.5,
        fx: false,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Hi-hats",
        color: 0x75b188,
        level: [-19.0, -21.0],
        fader: 0.66,
        pan: 0.62,
        fx: true,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Percussion",
        color: 0xb58b77,
        level: [-24.0, -22.0],
        fader: 0.58,
        pan: 0.36,
        fx: false,
        rec: false,
        muted: true,
        solo: false,
    },
    Strip {
        name: "Bass",
        color: 0x827fc4,
        level: [-6.5, -6.5],
        fader: 0.78,
        pan: 0.5,
        fx: true,
        rec: true,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Keys",
        color: 0xa5c47f,
        level: [-11.0, -12.5],
        fader: 0.70,
        pan: 0.5,
        fx: true,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "Pad Wide",
        color: 0x83afbb,
        level: [-15.0, -13.0],
        fader: 0.64,
        pan: 0.5,
        fx: true,
        rec: false,
        muted: false,
        solo: true,
    },
    Strip {
        name: "Lead Vocal",
        color: 0xc580a6,
        level: [-2.0, 0.8],
        fader: 0.84,
        pan: 0.5,
        fx: true,
        rec: false,
        muted: false,
        solo: false,
    },
    Strip {
        name: "FX Riser",
        color: 0x80c7bd,
        level: [-30.0, -28.0],
        fader: 0.52,
        pan: 0.58,
        fx: false,
        rec: false,
        muted: false,
        solo: false,
    },
];

/// The selected strip, and the colourful one.
const SELECTED: usize = 6;
const COLOURFUL: usize = 8;
/// Fader ticks every 6 dB, from +6 down, and the unity position.
const TICKS: [f32; 9] = [1.0, 0.8, 0.68, 0.56, 0.46, 0.37, 0.22, 0.10, 0.0];
const TICK_DB: [&str; 9] = ["+6", "0", "−6", "−12", "−18", "−24", "−36", "−48", "−∞"];

/// The rows of a strip, top to bottom: (legend key, height; 0 = flexible).
const ROWS: [(&str, f32); 9] = [
    ("kit.mixer.legend.number", 16.0),
    ("kit.mixer.legend.name", 64.0),
    ("kit.mixer.legend.meter", 0.0),
    ("kit.mixer.legend.mute", 14.0),
    ("kit.mixer.legend.pan", 22.0),
    ("kit.mixer.legend.fader", 0.0),
    ("kit.mixer.legend.phase", 16.0),
    ("kit.mixer.legend.fx", 14.0),
    ("kit.mixer.legend.sends", 18.0),
];

/// The y range of each row within `top..bottom`: fixed rows first, the meter and the
/// fader sharing the rest 38 : 62.
fn rows(top: f32, bottom: f32) -> [(f32, f32); 9] {
    let gap = 3.0;
    let fixed: f32 = ROWS.iter().map(|r| r.1).sum::<f32>() + gap * 8.0;
    let flex = (bottom - top - fixed).max(128.0);
    let mut y = top;
    let mut out = [(0.0, 0.0); 9];
    for (i, (_, h)) in ROWS.iter().enumerate() {
        let h = if *h > 0.0 {
            *h
        } else if i == 2 {
            (flex * 0.38).round()
        } else {
            (flex * 0.62).round()
        };
        out[i] = (y, y + h);
        y += h + gap;
    }
    out
}

/// Where the selected strip's fader cap is (the inspect demo clicks it).
pub fn selected_cap_centre() -> Pos2 {
    let rr = rows(32.0, SIZE.y - 18.0);
    let sw = 34.0;
    let x_strips = 74.0 + 2.0 + 58.0 + 6.0;
    let cx = x_strips + sw * SELECTED as f32 + (sw - 1.0) * 0.5;
    let f = Rect::from_min_max(Pos2::new(cx - 14.0, rr[5].0), Pos2::new(cx + 14.0, rr[5].1));
    let s = Style::new(cantus_ui::kit::Tokens::slate());
    fader::cap_rect(f, &s, STRIPS[SELECTED].fader).center()
}

pub fn paint(b: &mut Brush, _p: &Painter, s: &Style, size: Vec2) {
    let t = &s.t;
    b.rect(Rect::from_min_size(Pos2::ZERO, size), Fill::Solid(t.panel));
    title_bar(
        b,
        Rect::from_min_size(Pos2::ZERO, Vec2::new(size.x, 28.0)),
        s,
        "kit.mixer.title",
        "Keys",
        true,
    );
    common::mixer_tools(b, Pos2::new(30.0, 14.0), s);
    let desk_top = 32.0;
    let rr = rows(desk_top, size.y - 18.0);
    let legend_w = 74.0;
    let x_master = legend_w + 2.0;
    let master_w = 58.0;
    let sw = 34.0;
    let x_strips = x_master + master_w + 6.0;
    let x_returns = x_strips + sw * 10.0 + 6.0;

    // The legend column: each row named once, and the fader's dB scale.
    for (i, (key, _)) in ROWS.iter().enumerate() {
        let (y0, y1) = rr[i];
        let r = Rect::from_min_max(
            Pos2::new(8.0, y0),
            Pos2::new(legend_w - 4.0, y1.min(y0 + 16.0).max(y0 + 12.0)),
        );
        let r = if ROWS[i].1 == 0.0 || ROWS[i].1 > 30.0 {
            Rect::from_min_max(r.min, Pos2::new(r.max.x, y0 + 14.0))
        } else {
            Rect::from_center_size(
                Pos2::new(r.center().x, (y0 + y1) * 0.5),
                Vec2::new(r.width(), 14.0),
            )
        };
        let tt = Text::key(key, Role::Micro, t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, r, &tt, 1.0);
    }
    let fader_rect = |x: f32| Rect::from_min_max(Pos2::new(x, rr[5].0), Pos2::new(x + sw, rr[5].1));
    let fr = fader_rect(0.0);
    for (i, p) in TICKS.iter().enumerate() {
        let cap = fader::CAP.y * s.k;
        let y = fr.bottom() - cap * 0.5 - (fr.height() - cap) * p;
        let r = Rect::from_min_max(
            Pos2::new(legend_w - 30.0, y - 7.0),
            Pos2::new(legend_w - 6.0, y + 7.0),
        );
        let _ = text::paint(
            b,
            r,
            &Text::new(TICK_DB[i], Role::Micro, t.text_dim)
                .caps(false)
                .align(Align2::RIGHT_CENTER),
            1.0,
        );
    }

    // Docks: the master at the left, the returns at the right, divided FL's way.
    for x in [x_master + master_w + 1.0, x_returns - 5.0] {
        common::dock_divider(b, x, desk_top, size.y - 4.0, s);
    }

    let mut meters = MeterBatch::new(s);
    // The master strip.
    let m = Rect::from_min_max(
        Pos2::new(x_master, desk_top),
        Pos2::new(x_master + master_w, size.y - 18.0),
    );
    b.rect(m, Fill::Solid(t.strip));
    let cell = Rect::from_min_max(Pos2::new(m.left(), rr[0].0), Pos2::new(m.right(), rr[0].1));
    b.rect(cell, Fill::Solid(t.header_cell));
    let _ = text::paint(
        b,
        cell,
        &Text::new("M", Role::Label, on_fill(t.header_cell)),
        1.0,
    );
    let name = Rect::from_min_max(
        Pos2::new(m.left() + 3.0, rr[1].0),
        Pos2::new(m.right() - 3.0, rr[1].1),
    );
    let _ = text::paint(
        b,
        name,
        &Text::key("kit.mixer.master", Role::Label, t.label)
            .caps(true)
            .align(Align2::CENTER_BOTTOM),
        1.0,
    );
    let mw = Rect::from_center_size(
        Pos2::new(m.center().x, (rr[2].0 + rr[2].1) * 0.5),
        Vec2::new(28.0, rr[2].1 - rr[2].0),
    );
    meters.paint(
        b,
        mw,
        &Levels {
            level: [-3.1, -2.6],
            hold: [-1.2, -0.9],
        },
    );
    master_rows(b, m, &rr, s);

    for (i, st) in STRIPS.iter().enumerate() {
        let x = x_strips + sw * i as f32;
        strip(b, x, sw, &rr, s, st, i, &mut meters);
    }
    // Returns.
    for (i, (name, hue)) in [("Reverb", t.hue_space), ("Delay", t.hue_mod)]
        .iter()
        .enumerate()
    {
        let x = x_returns + sw * i as f32;
        let st = Strip {
            name,
            color: 0,
            level: [-18.0 - 4.0 * i as f32, -19.0],
            fader: 0.72,
            pan: 0.5,
            fx: true,
            rec: false,
            muted: false,
            solo: false,
        };
        let tint = safe_track(*hue, t.dark, FillSize::Small);
        return_strip(b, x, sw, &rr, s, &st, ["A", "B"][i], tint, &mut meters);
    }
    // The strips' scrollbar.
    let sb = Rect::from_min_max(
        Pos2::new(x_strips + 4.0, size.y - 12.0),
        Pos2::new(x_returns - 10.0, size.y - 6.0),
    );
    chrome::scrollbar(b, sb, s, State::REST, 0.18, 0.42);
}

fn master_rows(b: &mut Brush, m: Rect, rr: &[(f32, f32); 9], s: &Style) {
    let cx = m.center().x;
    led::paint(
        b,
        Pos2::new(cx, (rr[3].0 + rr[3].1) * 0.5),
        9.0,
        true,
        s,
        Tok::Led,
        Tok::Strip,
        1.0,
    );
    knob::paint(
        b,
        Pos2::new(cx, (rr[4].0 + rr[4].1) * 0.5),
        s,
        State::REST,
        &KnobLook::new(KnobSize::Mini, 0.5, 0.5),
    );
    let f = Rect::from_min_max(Pos2::new(cx - 17.0, rr[5].0), Pos2::new(cx + 17.0, rr[5].1));
    fader::paint(
        b,
        f,
        s,
        State::REST,
        &FaderLook {
            pos: 0.8,
            ticks: &TICKS,
            unity: Some(0.8),
        },
    );
    icons_rows(b, cx, rr, s, true, false, false);
}

fn icons_rows(
    b: &mut Brush,
    cx: f32,
    rr: &[(f32, f32); 9],
    s: &Style,
    fx: bool,
    rec: bool,
    send_knob: bool,
) {
    let t = &s.t;
    let dim = with_alpha(t.text_dim, 0.55);
    let ph = (rr[6].0 + rr[6].1) * 0.5;
    glyph::paint(
        b,
        Rect::from_center_size(Pos2::new(cx - 7.0, ph), Vec2::splat(11.0)),
        Glyph::Phase,
        dim,
    );
    glyph::paint(
        b,
        Rect::from_center_size(Pos2::new(cx + 7.0, ph), Vec2::splat(11.0)),
        Glyph::Swap,
        dim,
    );
    let fy = (rr[7].0 + rr[7].1) * 0.5;
    let bulb = if fx {
        t.fx_on
    } else {
        with_alpha(t.text_dim, 0.35)
    };
    glyph::paint(
        b,
        Rect::from_center_size(Pos2::new(cx - 7.0, fy), Vec2::splat(12.0)),
        Glyph::Bulb,
        bulb,
    );
    let rc = if rec {
        t.danger
    } else {
        with_alpha(t.text_dim, 0.6)
    };
    glyph::paint(
        b,
        Rect::from_center_size(Pos2::new(cx + 7.0, fy), Vec2::splat(10.0)),
        Glyph::Record,
        rc,
    );
    let sy = (rr[8].0 + rr[8].1) * 0.5;
    if send_knob {
        knob::paint(
            b,
            Pos2::new(cx, sy),
            &s.scaled(0.75),
            State::REST,
            &KnobLook::new(KnobSize::Mini, 0.62, 0.0),
        );
    } else {
        glyph::paint(
            b,
            Rect::from_center_size(Pos2::new(cx, sy), Vec2::splat(12.0)),
            Glyph::Send,
            t.text,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn strip(
    b: &mut Brush,
    x: f32,
    w: f32,
    rr: &[(f32, f32); 9],
    s: &Style,
    st: &Strip,
    i: usize,
    meters: &mut MeterBatch,
) {
    let t = &s.t;
    let selected = i == SELECTED;
    let body = Rect::from_min_max(Pos2::new(x, rr[0].0), Pos2::new(x + w - 1.0, rr[8].1 + 2.0));
    b.rect(
        body,
        Fill::Solid(if selected { t.strip_selected } else { t.strip }),
    );
    let color = safe_track(
        Color32::from_rgb(
            (st.color >> 16) as u8,
            (st.color >> 8) as u8,
            st.color as u8,
        ),
        t.dark,
        FillSize::Small,
    );
    if i == COLOURFUL {
        // Colourful mixer, "medium": the whole strip tinted in its track colour.
        b.rect(
            body,
            Fill::V(with_alpha(color, 0.30), with_alpha(color, 0.16)),
        );
    }
    let cell = Rect::from_min_max(Pos2::new(x, rr[0].0), Pos2::new(x + w - 1.0, rr[0].1));
    b.rect(
        cell,
        Fill::V(
            cantus_ui::kit::color::shade(color, 0.03),
            cantus_ui::kit::color::shade(color, -0.03),
        ),
    );
    if selected {
        b.rrect_stroke(
            b.snap_rect(cell).shrink(b.px() * 1.5),
            0.0,
            b.px() * 1.5,
            t.text,
        );
    }
    let num = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"][i];
    let _ = text::paint(b, cell, &Text::new(num, Role::Label, on_fill(color)), 1.0);
    let name = Rect::from_min_max(Pos2::new(x + 2.0, rr[1].0), Pos2::new(x + w - 3.0, rr[1].1));
    let ink = if selected { t.text } else { t.label };
    let nt = Text::new(st.name, Role::Label, ink)
        .align(Align2::CENTER_BOTTOM)
        .policy(Policy::Rotate90(Elide::Middle));
    let _ = text::paint(b, name, &nt, 1.0);
    let cx = x + (w - 1.0) * 0.5;
    let mw = Rect::from_center_size(
        Pos2::new(cx, (rr[2].0 + rr[2].1) * 0.5),
        Vec2::new(16.0, rr[2].1 - rr[2].0),
    );
    let hold = [st.level[0] + 2.5, st.level[1] + 2.0];
    meters.paint(
        b,
        mw,
        &Levels {
            level: st.level,
            hold,
        },
    );
    let my = (rr[3].0 + rr[3].1) * 0.5;
    led::paint(
        b,
        Pos2::new(cx - 6.0, my),
        9.0,
        !st.muted,
        s,
        Tok::Led,
        Tok::Strip,
        1.0,
    );
    led::paint(
        b,
        Pos2::new(cx + 6.0, my),
        9.0,
        st.solo,
        s,
        Tok::Highlight,
        Tok::Strip,
        1.0,
    );
    knob::paint(
        b,
        Pos2::new(cx, (rr[4].0 + rr[4].1) * 0.5),
        s,
        State::REST,
        &KnobLook::new(KnobSize::Mini, st.pan, 0.5),
    );
    let f = Rect::from_min_max(Pos2::new(cx - 14.0, rr[5].0), Pos2::new(cx + 14.0, rr[5].1));
    let fs = State {
        lit: selected,
        ..State::REST
    };
    fader::paint(
        b,
        f,
        s,
        fs,
        &FaderLook {
            pos: st.fader,
            ticks: &TICKS,
            unity: Some(0.8),
        },
    );
    icons_rows(b, cx, rr, s, st.fx, st.rec, i == 3);
}

#[allow(clippy::too_many_arguments)]
fn return_strip(
    b: &mut Brush,
    x: f32,
    w: f32,
    rr: &[(f32, f32); 9],
    s: &Style,
    st: &Strip,
    letter: &str,
    color: Color32,
    meters: &mut MeterBatch,
) {
    let t = &s.t;
    let body = Rect::from_min_max(Pos2::new(x, rr[0].0), Pos2::new(x + w - 1.0, rr[8].1 + 2.0));
    b.rect(body, Fill::Solid(t.strip));
    let cell = Rect::from_min_max(Pos2::new(x, rr[0].0), Pos2::new(x + w - 1.0, rr[0].1));
    b.rect(cell, Fill::Solid(color));
    let _ = text::paint(
        b,
        cell,
        &Text::new(letter, Role::Label, on_fill(color)),
        1.0,
    );
    let name = Rect::from_min_max(Pos2::new(x + 2.0, rr[1].0), Pos2::new(x + w - 3.0, rr[1].1));
    let _ = text::paint(
        b,
        name,
        &Text::new(st.name, Role::Label, t.label)
            .align(Align2::CENTER_BOTTOM)
            .policy(Policy::Rotate90(Elide::Middle)),
        1.0,
    );
    let cx = x + (w - 1.0) * 0.5;
    let mw = Rect::from_center_size(
        Pos2::new(cx, (rr[2].0 + rr[2].1) * 0.5),
        Vec2::new(16.0, rr[2].1 - rr[2].0),
    );
    meters.paint(
        b,
        mw,
        &Levels {
            level: st.level,
            hold: [st.level[0] + 3.0, st.level[1] + 3.0],
        },
    );
    led::paint(
        b,
        Pos2::new(cx - 6.0, (rr[3].0 + rr[3].1) * 0.5),
        9.0,
        true,
        s,
        Tok::Led,
        Tok::Strip,
        1.0,
    );
    led::paint(
        b,
        Pos2::new(cx + 6.0, (rr[3].0 + rr[3].1) * 0.5),
        9.0,
        false,
        s,
        Tok::Highlight,
        Tok::Strip,
        1.0,
    );
    knob::paint(
        b,
        Pos2::new(cx, (rr[4].0 + rr[4].1) * 0.5),
        s,
        State::REST,
        &KnobLook::new(KnobSize::Mini, 0.5, 0.5),
    );
    let f = Rect::from_min_max(Pos2::new(cx - 14.0, rr[5].0), Pos2::new(cx + 14.0, rr[5].1));
    fader::paint(
        b,
        f,
        s,
        State::REST,
        &FaderLook {
            pos: st.fader,
            ticks: &TICKS,
            unity: Some(0.8),
        },
    );
    icons_rows(b, cx, rr, s, true, false, false);
}
