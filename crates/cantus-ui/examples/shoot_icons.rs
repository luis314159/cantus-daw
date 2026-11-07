//! The icon family and the kit's controls on one sheet per theme and scale (round 37, the
//! finish builder, ADR 0103), for the design review:
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_icons -- <out-dir>
//! ```
//!
//! Writes `icons-<theme>-<scale>.png` (every glyph at 14, 18 and 24 pt, at rest, as a lit
//! toolbar key and as a plain key) and `controls-<theme>-<scale>.png` (the knobs, the mini
//! knob, the fader at rest and selected, LEDs lit and off, keys, a readout, a field, a
//! meter) for Cantus Night and Cantus Paper at 100 % and 150 %.

use cantus_ui::kit::cap::{self, KeyFace};
use cantus_ui::kit::fader::{self, FaderLook};
use cantus_ui::kit::glyph::{self, Glyph};
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::meter::{self, Levels};
use cantus_ui::kit::{gallery, led, well, Brush, Fill, Function, State, Style, Tok, Tokens};
use egui::{pos2, vec2, Pos2, Rect};
use std::path::PathBuf;

fn icons(b: &mut Brush, s: &Style, size: egui::Vec2) {
    b.rect(
        Rect::from_min_size(Pos2::ZERO, size),
        Fill::Solid(s.t.toolbar),
    );
    let cols = 16usize;
    for (i, g) in Glyph::ALL.iter().enumerate() {
        let x = 10.0 + (i % cols) as f32 * 40.0;
        let y = 10.0 + (i / cols) as f32 * 130.0;
        for (j, side) in [14.0f32, 18.0, 24.0].iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(x + (24.0 - side) * 0.5, y + j as f32 * 28.0),
                vec2(*side, *side),
            );
            glyph::paint(b, r, *g, s.t.text);
        }
        let key = Rect::from_min_size(pos2(x - 1.0, y + 86.0), vec2(28.0, 28.0));
        cap::toolbar_key(b, key, s, State::REST, KeyFace::Glyph(*g));
    }
    for (i, g) in Glyph::ALL.iter().enumerate().take(cols) {
        let x = 10.0 + i as f32 * 40.0;
        let key = Rect::from_min_size(pos2(x - 1.0, size.y - 40.0), vec2(28.0, 28.0));
        cap::toolbar_key(b, key, s, State::LIT, KeyFace::Glyph(*g));
    }
}

fn controls(b: &mut Brush, s: &Style, size: egui::Vec2) {
    b.rect(Rect::from_min_size(Pos2::ZERO, size), Fill::Solid(s.t.deck));
    let strip = Rect::from_min_max(pos2(0.0, 130.0), pos2(size.x, size.y));
    b.rect(strip, Fill::Solid(s.t.strip));
    let st = s.function(Function::Tone);
    let mut x = 40.0;
    for (k, v) in [
        (KnobSize::S, 0.7),
        (KnobSize::M, 0.3),
        (KnobSize::L, 0.85),
        (KnobSize::XL, 0.6),
    ] {
        let r = k.diameter() * 0.5;
        knob::paint(
            b,
            pos2(x + r, 60.0),
            &st,
            State::REST,
            &KnobLook::new(k, v, 0.0),
        );
        x += k.diameter() + 26.0;
    }
    for (i, (v, stt)) in [
        (0.5, State::REST),
        (0.2, State::HOVER),
        (0.85, State::FOCUS),
    ]
    .iter()
    .enumerate()
    {
        knob::paint(
            b,
            pos2(40.0 + i as f32 * 36.0, 160.0),
            s,
            *stt,
            &KnobLook::new(KnobSize::Mini, *v, 0.5),
        );
    }
    for (i, lit) in [true, false].iter().enumerate() {
        led::paint(
            b,
            pos2(160.0 + i as f32 * 20.0, 160.0),
            9.0,
            *lit,
            s,
            Tok::Led,
            Tok::Strip,
            1.0,
        );
        led::paint(
            b,
            pos2(210.0 + i as f32 * 20.0, 160.0),
            11.0,
            *lit,
            s,
            Tok::Danger,
            Tok::Strip,
            1.0,
        );
    }
    let ticks = [0.0, 0.14, 0.28, 0.42, 0.56, 0.7, 0.8, 0.9];
    for (i, stt) in [State::REST, State::LIT, State::HOVER].iter().enumerate() {
        let r = Rect::from_min_size(pos2(30.0 + i as f32 * 40.0, 185.0), vec2(28.0, 150.0));
        fader::paint(
            b,
            r,
            s,
            *stt,
            &FaderLook {
                pos: 0.72,
                ticks: &ticks,
                unity: Some(0.8),
            },
        );
    }
    let levels = Levels {
        level: [-9.0, -3.0],
        hold: [-6.0, -1.0],
    };
    meter::paint(
        b,
        Rect::from_min_size(pos2(160.0, 190.0), vec2(22.0, 140.0)),
        s,
        &levels,
    );
    well::readout(
        b,
        Rect::from_min_size(pos2(200.0, 190.0), vec2(80.0, 22.0)),
        s,
        State::REST,
        "+1.0",
        true,
    );
    well::menu_field(
        b,
        Rect::from_min_size(pos2(200.0, 222.0), vec2(120.0, 22.0)),
        s,
        State::REST,
        "Comp · EQ",
        false,
    );
    for (i, stt) in [State::REST, State::HOVER, State::ACTIVE, State::LIT]
        .iter()
        .enumerate()
    {
        let r = Rect::from_min_size(pos2(200.0 + i as f32 * 34.0, 256.0), vec2(28.0, 28.0));
        cap::toolbar_key(b, r, s, *stt, KeyFace::Glyph(Glyph::Mixer));
    }
    for (i, lit) in [false, true].iter().enumerate() {
        let r = Rect::from_min_size(pos2(200.0 + i as f32 * 92.0, 296.0), vec2(84.0, 22.0));
        led::button(
            b,
            r,
            &st,
            State::REST.lit(*lit),
            cantus_ui::kit::text::Src::Str("Mute"),
        );
    }
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-icons"));
    std::fs::create_dir_all(&dir).expect("out dir");
    let themes: [(&str, Tokens); 2] = [
        ("night", cantus_ui::theme::cantus_night().kit_tokens()),
        ("paper", cantus_ui::theme::cantus_paper().kit_tokens()),
    ];
    for (name, t) in themes {
        let s = Style::new(t);
        for (label, ppp) in [("100", 1.0), ("150", 1.5)] {
            let rows = Glyph::ALL.len().div_ceil(16) as f32;
            let isize = vec2(16.0 * 40.0 + 10.0, rows * 130.0 + 60.0);
            let c = gallery::render(isize, ppp, |b, _| icons(b, &s, isize));
            let f = dir.join(format!("icons-{name}-{label}.png"));
            std::fs::write(&f, gallery::encode_png(&c).expect("png")).expect("write");
            let csize = vec2(420.0, 345.0);
            let c = gallery::render(csize, ppp, |b, _| controls(b, &s, csize));
            let f2 = dir.join(format!("controls-{name}-{label}.png"));
            std::fs::write(&f2, gallery::encode_png(&c).expect("png")).expect("write");
            println!("wrote {} and {}", f.display(), f2.display());
        }
    }
}
