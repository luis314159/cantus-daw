//! (b) The inspector for a channel (ADR 0024 §7): what was clicked last in its title,
//! then the signal flow top to bottom — input, inserts as tiles, the EQ as a display,
//! sends, routing — in one row grammar, with the channel's own meter, fader and pan
//! pinned at the bottom.

use crate::common::{row, title_bar};
use crate::eq;
use cantus_ui::kit::fader::{self, FaderLook};
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::meter::{self, Levels};
use cantus_ui::kit::text::{self, Role, Src, Text};
use cantus_ui::kit::tile::{self, TileLook};
use cantus_ui::kit::{band, led, segmented, well, Brush, Fill, Function, State, Style};
use egui::{Painter, Pos2, Rect, Vec2};

pub const SIZE: Vec2 = Vec2::new(288.0, 732.0);

pub fn paint(b: &mut Brush, _p: &Painter, s: &Style, size: Vec2) {
    let t = &s.t;
    b.rect(
        Rect::from_min_size(Pos2::ZERO, size),
        Fill::Solid(t.panel_light),
    );
    title_bar(
        b,
        Rect::from_min_size(Pos2::ZERO, Vec2::new(size.x, 28.0)),
        s,
        "kit.mixer.title",
        "Keys",
        false,
    );
    let x0 = 6.0;
    let w = size.x - 12.0;
    let mut y = 34.0;
    let full = |y: f32, h: f32| Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, h));

    // Input.
    let panel = |b: &mut Brush, r: Rect| b.rect(b.snap_rect(r), Fill::Solid(t.panel));
    panel(
        b,
        Rect::from_min_max(Pos2::new(0.0, 28.0), Pos2::new(size.x, y + 30.0)),
    );
    row(
        b,
        full(y, 24.0),
        s,
        "kit.inspector.input",
        "Audio In 1 + 2",
        false,
        false,
    );
    y += 30.0;

    // Inserts: every slot drawn, empty ones as placeholders, so nothing jumps.
    band::section(
        b,
        Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, band::BAND)),
        s,
        State::REST,
        Src::Key("kit.inspector.inserts"),
        None,
        false,
    );
    y += band::BAND + 4.0;
    let slots: [(Option<&str>, f32, bool); 8] = [
        (Some("Aequalis"), 1.0, true),
        (Some("Comprimo"), 0.8, true),
        (Some("Spatium"), 0.35, false),
        (Some("ValhallaVintageVerb (hosted)"), 0.5, true),
        (None, 1.0, false),
        (None, 1.0, false),
        (None, 1.0, false),
        (None, 1.0, false),
    ];
    let names = ["Slot 5", "Slot 6", "Slot 7", "Slot 8"];
    for (i, (name, mix, on)) in slots.iter().enumerate() {
        let look = TileLook {
            name: name.map(Src::Str),
            placeholder: Src::Str(names[i.saturating_sub(4).min(3)]),
            mix: Some(*mix),
            on: *on,
        };
        let st = State {
            lit: i == 0,
            ..State::REST
        };
        tile::paint(b, full(y, tile::HEIGHT), s, st, &look);
        y += tile::HEIGHT + 1.0;
    }
    y += 5.0;

    // The channel EQ: a display that is a control.
    band::section(
        b,
        Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, band::BAND)),
        &s.function(Function::Tone),
        State::REST,
        Src::Key("kit.inspector.eq"),
        Some(true),
        false,
    );
    y += band::BAND;
    let d = Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, 112.0));
    eq::display(
        b,
        d,
        &s.function(Function::Tone),
        &eq::CHANNEL,
        Some(1),
        false,
    );
    y += 112.0 + 6.0;

    // Sends: one row each — destination, pre/post, level.
    band::section(
        b,
        Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, band::BAND)),
        &s.function(Function::Space),
        State::REST,
        Src::Key("kit.inspector.sends"),
        None,
        false,
    );
    y += band::BAND + 4.0;
    let prepost = [
        Src::Key("kit.inspector.pre"),
        Src::Key("kit.inspector.post"),
    ];
    for (dest, post, level) in [("Reverb", 1, "−6.0 dB"), ("Delay", 0, "−12.5 dB")] {
        let r = full(y, 22.0);
        well::menu_field(
            b,
            Rect::from_min_size(r.min, Vec2::new(104.0, 22.0)),
            s,
            State::REST,
            dest,
            false,
        );
        let sp = &s.function(Function::Space);
        segmented::paint(
            b,
            Rect::from_min_size(r.min + Vec2::new(110.0, 0.0), Vec2::new(70.0, 22.0)),
            sp,
            State::REST,
            &prepost,
            post,
            None,
        );
        well::readout(
            b,
            Rect::from_min_max(Pos2::new(r.left() + 186.0, r.top()), r.max),
            s,
            State::REST,
            level,
            true,
        );
        y += 26.0;
    }
    y += 2.0;

    // Routing: label | value rows; what does not apply is dimmed, not hidden.
    band::section(
        b,
        Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, band::BAND)),
        &s.function(Function::Output),
        State::REST,
        Src::Key("kit.inspector.routing"),
        None,
        false,
    );
    y += band::BAND + 4.0;
    row(
        b,
        full(y, 24.0),
        s,
        "kit.inspector.delay",
        "2.0 ms",
        true,
        false,
    );
    y += 26.0;
    row(
        b,
        full(y, 24.0),
        s,
        "kit.inspector.latency",
        "88 smp",
        true,
        true,
    );
    y += 26.0;
    row(
        b,
        full(y, 24.0),
        s,
        "kit.inspector.output",
        "Master",
        false,
        false,
    );

    // The channel's own strip, pinned at the bottom.
    let pin = Rect::from_min_max(Pos2::new(0.0, size.y - 150.0), Pos2::new(size.x, size.y));
    b.rect(b.snap_rect(pin), Fill::Solid(t.strip));
    b.hline(
        pin.left(),
        pin.right(),
        pin.top() + b.px() * 0.5,
        b.px(),
        cantus_ui::kit::color::shade(t.strip, -0.12),
    );
    let m = Rect::from_min_size(pin.min + Vec2::new(14.0, 12.0), Vec2::new(18.0, 126.0));
    meter::paint(
        b,
        m,
        s,
        &Levels {
            level: [-11.0, -12.5],
            hold: [-8.5, -9.8],
        },
    );
    let f = Rect::from_min_size(pin.min + Vec2::new(38.0, 8.0), Vec2::new(30.0, 134.0));
    let ticks = [1.0, 0.8, 0.68, 0.56, 0.46, 0.37, 0.22, 0.1, 0.0];
    fader::paint(
        b,
        f,
        s,
        State {
            lit: true,
            ..State::REST
        },
        &FaderLook {
            pos: 0.7,
            ticks: &ticks,
            unity: Some(0.8),
        },
    );
    let cx = pin.left() + 110.0;
    knob::paint(
        b,
        Pos2::new(cx, pin.top() + 34.0),
        s,
        State::REST,
        &KnobLook::new(KnobSize::S, 0.5, 0.5),
    );
    let cap = |b: &mut Brush, r: Rect, k: &str| {
        let _ = text::paint(b, r, &Text::key(k, Role::Micro, t.text_dim), 1.0);
    };
    cap(
        b,
        Rect::from_center_size(Pos2::new(cx, pin.top() + 58.0), Vec2::new(60.0, 12.0)),
        "kit.param.pan",
    );
    let rr = Rect::from_min_size(
        Pos2::new(pin.left() + 146.0, pin.top() + 14.0),
        Vec2::new(126.0, 22.0),
    );
    well::readout(b, rr, s, State::REST, "−2.4 dB", true);
    let rr2 = rr.translate(Vec2::new(0.0, 28.0));
    well::readout(b, rr2, s, State::REST, "C", true);
    let lb = Rect::from_min_size(
        Pos2::new(pin.left() + 90.0, pin.top() + 78.0),
        Vec2::new(88.0, 20.0),
    );
    led::button(
        b,
        lb,
        &s.function(Function::Dynamics),
        State::REST,
        Src::Key("kit.mixer.mute"),
    );
    led::button(
        b,
        lb.translate(Vec2::new(94.0, 0.0)),
        &s.function(Function::Dynamics),
        State::LIT,
        Src::Key("kit.mixer.solo"),
    );
    let lb2 = lb.translate(Vec2::new(0.0, 26.0));
    led::button(
        b,
        lb2,
        &s.function(Function::Output),
        State::REST,
        Src::Key("kit.mixer.mono"),
    );
    led::button(
        b,
        lb2.translate(Vec2::new(94.0, 0.0)),
        &s.function(Function::Output),
        State::REST,
        Src::Key("kit.mixer.phase"),
    );
}
