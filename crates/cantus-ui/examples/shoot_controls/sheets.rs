//! The sheets: each family's parts in their states, in rows and columns with captions.

use cantus_ui::kit::automate::{self, Drivers};
use cantus_ui::kit::brush::Fill;
use cantus_ui::kit::cell::{self, CellLook, Control};
use cantus_ui::kit::dialog::{self, Kind};
use cantus_ui::kit::drive_sign::{self, Sign};
use cantus_ui::kit::fader::{self, FaderLook};
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::menu::{self, Check, Item};
use cantus_ui::kit::meter::{self, Levels};
use cantus_ui::kit::text::{self, Role, Src, Text};
use cantus_ui::kit::{chrome, led, segmented, well, with_brush, Brush, State, Style};
use egui::{Align2, Pos2, Rect, Vec2};

pub struct Family {
    pub name: &'static str,
    pub size: Vec2,
}

pub const FAMILIES: [Family; 7] = [
    Family {
        name: "knobs",
        size: Vec2::new(1280.0, 470.0),
    },
    Family {
        name: "faders",
        size: Vec2::new(900.0, 470.0),
    },
    Family {
        name: "fields",
        size: Vec2::new(760.0, 300.0),
    },
    Family {
        name: "menus",
        size: Vec2::new(620.0, 360.0),
    },
    Family {
        name: "switches",
        size: Vec2::new(860.0, 330.0),
    },
    Family {
        name: "dialogs",
        size: Vec2::new(980.0, 470.0),
    },
    Family {
        name: "tips",
        size: Vec2::new(760.0, 200.0),
    },
];

const REST: State = State::REST;
fn hov() -> State {
    State {
        hovered: true,
        ..REST
    }
}
fn act() -> State {
    State {
        active: true,
        hovered: true,
        ..REST
    }
}
fn foc() -> State {
    State {
        focused: true,
        ..REST
    }
}
fn dis() -> State {
    State {
        disabled: true,
        ..REST
    }
}
fn lit() -> State {
    State { lit: true, ..REST }
}

fn caption(b: &mut Brush, s: &Style, at: Pos2, words: &str, w: f32) {
    let r = Rect::from_min_size(at, Vec2::new(w, 14.0));
    let t = Text::new(words, Role::Label, s.t.text_dim).align(Align2::CENTER_CENTER);
    let _ = text::paint(b, r, &t, 1.0);
}

fn title(b: &mut Brush, s: &Style, at: Pos2, words: &str) {
    let r = Rect::from_min_size(at, Vec2::new(400.0, 18.0));
    let t = Text::new(words, Role::Title, s.t.text).align(Align2::LEFT_CENTER);
    let _ = text::paint(b, r, &t, 1.0);
}

pub fn paint(ui: &mut egui::Ui, s: &Style, family: &str) {
    let full = ui.max_rect();
    let painter = ui.painter().clone();
    with_brush(&painter, |b| {
        b.rect(full, Fill::Solid(s.t.toolbar));
        match family {
            "knobs" => knobs(b, s),
            "faders" => faders(b, s),
            "fields" => fields(b, s),
            "menus" => menus(b, s),
            "switches" => switches(b, s),
            "tips" => tips(b, s),
            _ => dialogs(b, s),
        }
    });
}

/// One knob column: its state and what drives it.
struct Col {
    name: &'static str,
    st: State,
    d: Drivers,
    v: f32,
    def: f32,
    rings: &'static [(f32, egui::Color32)],
    live: Option<f32>,
    value: &'static str,
}

/// What a column's knob is told before it is painted: its detents, where the clip had it.
fn prepare(name: &str) {
    match name {
        "Stepped" => knob::stepping(Some(5)),
        "Automated, held" => automate::held_clip(Some(0.5)),
        _ => {}
    }
}

fn knobs(b: &mut Brush, s: &Style) {
    let clip = Drivers {
        clip: true,
        ..Drivers::default()
    };
    let modu = Drivers {
        modulated: true,
        ..Drivers::default()
    };
    let midi = Drivers {
        mapped: true,
        ..Drivers::default()
    };
    const RING: &[(f32, egui::Color32)] = &[(0.18, egui::Color32::WHITE)];
    let none = Drivers::default();
    let cols = [
        Col {
            name: "Rest",
            st: REST,
            d: none,
            v: 0.62,
            def: 0.0,
            rings: &[],
            live: None,
            value: "2.40 kHz",
        },
        Col {
            name: "Hover",
            st: hov(),
            d: none,
            v: 0.62,
            def: 0.0,
            rings: &[],
            live: None,
            value: "2.40 kHz",
        },
        Col {
            name: "Dragged",
            st: act(),
            d: none,
            v: 0.74,
            def: 0.0,
            rings: &[],
            live: None,
            value: "3.10 kHz",
        },
        Col {
            name: "Focused",
            st: foc(),
            d: none,
            v: 0.62,
            def: 0.0,
            rings: &[],
            live: None,
            value: "2.40 kHz",
        },
        Col {
            name: "Disabled",
            st: dis(),
            d: none,
            v: 0.62,
            def: 0.0,
            rings: &[],
            live: None,
            value: "2.40 kHz",
        },
        Col {
            name: "Automated",
            st: REST,
            d: clip,
            v: 0.5,
            def: 0.0,
            rings: &[],
            live: None,
            value: "1.85 kHz",
        },
        Col {
            name: "Automated, held",
            st: act(),
            d: clip,
            v: 0.74,
            def: 0.0,
            rings: &[],
            live: None,
            value: "3.10 kHz",
        },
        Col {
            name: "Modulated",
            st: REST,
            d: modu,
            v: 0.62,
            def: 0.0,
            rings: RING,
            live: Some(0.72),
            value: "2.40 kHz",
        },
        Col {
            name: "MIDI learned",
            st: REST,
            d: midi,
            v: 0.62,
            def: 0.0,
            rings: &[],
            live: None,
            value: "2.40 kHz",
        },
        Col {
            name: "Bipolar centre",
            st: REST,
            d: none,
            v: 0.5,
            def: 0.5,
            rings: &[],
            live: None,
            value: "C",
        },
        Col {
            name: "Bipolar right",
            st: REST,
            d: none,
            v: 0.67,
            def: 0.5,
            rings: &[],
            live: None,
            value: "R 45",
        },
        Col {
            name: "Stepped",
            st: REST,
            d: none,
            v: 0.75,
            def: 0.0,
            rings: &[],
            live: None,
            value: "4x",
        },
    ];
    title(
        b,
        s,
        Pos2::new(16.0, 10.0),
        "Knobs — Small (Mini), Medium (M), Large (L)",
    );
    let x0 = 120.0;
    let cw = 96.0;
    for (i, c) in cols.iter().enumerate() {
        let x = x0 + cw * i as f32;
        caption(b, s, Pos2::new(x, 36.0), c.name, cw);
        // Small: the knob alone.
        let look = KnobLook {
            size: KnobSize::Mini,
            value: c.v,
            default: c.def,
            rings: c.rings,
            live: c.live,
        };
        let center = Pos2::new(x + cw * 0.5, 80.0);
        automate::driving(c.d);
        prepare(c.name);
        knob::paint(b, center, s, c.st, &look);
        automate::signs(b, Rect::from_center_size(center, Vec2::splat(26.0)), s, c.d);
        for (row, size, y) in [(1, KnobSize::M, 110.0), (2, KnobSize::L, 240.0)] {
            let _ = row;
            let look = KnobLook {
                size,
                value: c.v,
                default: c.def,
                rings: c.rings,
                live: c.live,
            };
            let sz = cell::size(size);
            let rect = Rect::from_min_size(Pos2::new(x + (cw - sz.x) * 0.5, y), sz);
            automate::driving(c.d);
            prepare(c.name);
            let label = if size == KnobSize::L {
                "Drive"
            } else {
                "Cutoff"
            };
            let l = CellLook {
                control: Control::Knob(look),
                value: c.value,
                label: Src::Str(label),
            };
            cell::paint(b, rect, s, c.st, &l);
            automate::signs(b, rect, s, c.d);
        }
    }
    for (y, n) in [(80.0, "Small"), (150.0, "Medium"), (300.0, "Large")] {
        let r = Rect::from_min_size(Pos2::new(16.0, y - 8.0), Vec2::new(100.0, 16.0));
        let _ = text::paint(
            b,
            r,
            &Text::new(n, Role::Control, s.t.text).align(Align2::LEFT_CENTER),
            1.0,
        );
    }
    // The signs at every height.
    title(b, s, Pos2::new(16.0, 390.0), "The driven signs");
    for (j, g) in Sign::ALL.iter().enumerate() {
        for (i, h) in [5.0f32, 7.0, 9.0, 12.0].iter().enumerate() {
            let c = Pos2::new(140.0 + 34.0 * i as f32 + 170.0 * j as f32, 432.0);
            drive_sign::paint_sign(b, c, *h, *g, g.color(s), None);
        }
    }
}

fn faders(b: &mut Brush, s: &Style) {
    title(b, s, Pos2::new(16.0, 10.0), "Faders and meters");
    let ticks = [0.0, 0.2, 0.36, 0.51, 0.70, 0.90];
    let none = Drivers::default();
    let clip = Drivers { clip: true, ..none };
    let vca = Drivers { vca: true, ..none };
    let cols: [(&str, State, Drivers, f32); 7] = [
        ("Rest", REST, none, 0.8),
        ("Hover", hov(), none, 0.8),
        ("Dragged", act(), none, 0.66),
        ("Automated", REST, clip, 0.55),
        ("VCA", REST, vca, 0.8),
        ("Selected", lit(), none, 0.8),
        ("Frozen", dis(), none, 0.8),
    ];
    for (i, (name, st, d, pos)) in cols.iter().enumerate() {
        let x = 30.0 + 88.0 * i as f32;
        caption(b, s, Pos2::new(x, 36.0), name, 80.0);
        let rect = Rect::from_min_size(Pos2::new(x + 32.0, 60.0), Vec2::new(24.0, 160.0));
        let look = FaderLook {
            pos: *pos,
            ticks: &ticks,
            unity: Some(0.8),
        };
        automate::driving(*d);
        if *name == "VCA" {
            automate::vca_ghost(Some(0.62));
        }
        fader::paint(b, rect, s, *st, &look);
        automate::signs(b, rect, s, *d);
    }
    let meters: [(&str, Levels); 4] = [
        ("Silent", Levels::SILENT),
        (
            "Playing",
            Levels {
                level: [-14.0, -11.0],
                hold: [-60.0, -60.0],
            },
        ),
        (
            "Peak hold",
            Levels {
                level: [-20.0, -16.0],
                hold: [-6.0, -4.5],
            },
        ),
        (
            "Clip",
            Levels {
                level: [2.0, -1.0],
                hold: [3.0, 0.5],
            },
        ),
    ];
    for (i, (name, l)) in meters.iter().enumerate() {
        let x = 660.0 + 56.0 * i as f32;
        caption(b, s, Pos2::new(x - 10.0, 36.0), name, 56.0);
        meter::paint(
            b,
            Rect::from_min_size(Pos2::new(x + 12.0, 60.0), Vec2::new(8.0, 160.0)),
            s,
            l,
        );
    }
    // Sliders and value bars.
    use cantus_ui::kit::slider::{self, BarLook, SliderLook};
    type Row = (&'static str, State, Drivers, bool, Option<(f32, f32)>);
    let rows: [Row; 6] = [
        ("Rest", REST, none, false, None),
        ("Hover", hov(), none, false, None),
        ("Dragged", act(), none, false, None),
        ("Automated", REST, clip, false, None),
        (
            "Modulated",
            REST,
            Drivers {
                modulated: true,
                ..none
            },
            false,
            Some((0.15, 0.7)),
        ),
        ("Bipolar", REST, none, true, None),
    ];
    for (i, (name, st, d, bip, swing)) in rows.iter().enumerate() {
        let x = 30.0 + 140.0 * i as f32;
        caption(b, s, Pos2::new(x, 330.0), name, 130.0);
        let look = SliderLook {
            value: if *bip { 0.35 } else { 0.6 },
            upper: None,
            bipolar: *bip,
        };
        slider::slider(
            b,
            Rect::from_min_size(Pos2::new(x + 10.0, 350.0), Vec2::new(110.0, 16.0)),
            s,
            *st,
            &look,
            *d,
        );
        let bar = BarLook {
            name: Src::Str(if *bip { "Pan" } else { "Cutoff" }),
            value: if *bip { "L 30" } else { "2.40 kHz" },
            pos: look.value,
            bipolar: *bip,
            swing: *swing,
        };
        slider::value_bar(
            b,
            Rect::from_min_size(Pos2::new(x, 390.0), Vec2::new(130.0, 20.0)),
            s,
            *st,
            &bar,
            *d,
        );
    }
    let r = Rect::from_min_size(Pos2::new(660.0, 240.0), Vec2::new(14.0, 70.0));
    meter::paint(
        b,
        r,
        s,
        &Levels {
            level: [-9.0, -8.0],
            hold: [-3.0, -2.0],
        },
    );
    caption(b, s, Pos2::new(640.0, 312.0), "Master 5 pt", 70.0);
}

fn fields(b: &mut Brush, s: &Style) {
    title(b, s, Pos2::new(16.0, 10.0), "Fields and drop-downs");
    let cols: [(&str, State); 5] = [
        ("Rest", REST),
        ("Hover", hov()),
        ("Open", lit()),
        ("Focused", foc()),
        ("Disabled", dis()),
    ];
    for (i, (name, st)) in cols.iter().enumerate() {
        let x = 20.0 + 146.0 * i as f32;
        caption(b, s, Pos2::new(x, 36.0), name, 130.0);
        well::menu_field(
            b,
            Rect::from_min_size(Pos2::new(x, 58.0), Vec2::new(130.0, 24.0)),
            s,
            *st,
            "Lowpass 24",
            false,
        );
        well::readout(
            b,
            Rect::from_min_size(Pos2::new(x, 100.0), Vec2::new(130.0, 24.0)),
            s,
            *st,
            "2.40 kHz",
            true,
        );
        well::paint(
            b,
            Rect::from_min_size(Pos2::new(x, 142.0), Vec2::new(130.0, 24.0)),
            s,
            *st,
            None,
        );
        well::menu_field(
            b,
            Rect::from_min_size(Pos2::new(x, 184.0), Vec2::new(130.0, 20.0)),
            s,
            *st,
            "1/16",
            true,
        );
    }
    caption(
        b,
        s,
        Pos2::new(20.0, 230.0),
        "drop-down · readout · field (Open = typing) · stepped drop-down",
        520.0,
    );
}

fn menus(b: &mut Brush, s: &Style) {
    let items = [
        Item::Action {
            label: Src::Str("Undo"),
            check: Check::None,
            shortcut: Some("Ctrl+Z"),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Str("Redo"),
            check: Check::None,
            shortcut: Some("Ctrl+Y"),
            submenu: false,
            enabled: false,
        },
        Item::Separator,
        Item::Action {
            label: Src::Str("Cut"),
            check: Check::None,
            shortcut: Some("Ctrl+X"),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Str("Copy"),
            check: Check::None,
            shortcut: Some("Ctrl+C"),
            submenu: false,
            enabled: true,
        },
        Item::Band(Src::Str("Grid")),
        Item::Action {
            label: Src::Str("Quantise"),
            check: Check::None,
            shortcut: None,
            submenu: true,
            enabled: true,
        },
        Item::Action {
            label: Src::Str("Snap to grid"),
            check: Check::On,
            shortcut: Some("Ctrl+G"),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Str("Show triplet lines"),
            check: Check::Off,
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    let w = menu::width(b.ctx(), &items, s).max(220.0);
    let r = Rect::from_min_size(
        Pos2::new(24.0, 24.0),
        Vec2::new(w, menu::height_in(&items, s)),
    );
    menu::paint(b, r, s, &items, Some(6));
    let sub = [
        Item::Band(Src::Str("Grid value")),
        Item::Action {
            label: Src::Str("1/8"),
            check: Check::Radio(false),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Str("1/16"),
            check: Check::Radio(true),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Separator,
        Item::Action {
            label: Src::Str("Swing…"),
            check: Check::None,
            shortcut: Some("Ctrl+Alt+Q"),
            submenu: false,
            enabled: true,
        },
    ];
    let w2 = menu::width(b.ctx(), &sub, s).max(160.0);
    let top = menu::row_in(r, &items, s, 6).top() - 5.0;
    let r2 = Rect::from_min_size(
        Pos2::new(r.right() - 4.0, top),
        Vec2::new(w2, menu::height_in(&sub, s)),
    );
    menu::paint(b, r2, s, &sub, Some(2));
}

fn switches(b: &mut Brush, s: &Style) {
    title(
        b,
        s,
        Pos2::new(16.0, 10.0),
        "Tabs, segmented, lamps, scrollbars",
    );
    let states: [(&str, State); 4] = [
        ("Rest", REST),
        ("Hover", hov()),
        ("Active", lit()),
        (
            "Active, focused",
            State {
                lit: true,
                focused: true,
                ..REST
            },
        ),
    ];
    b.rect(
        Rect::from_min_size(Pos2::new(16.0, 64.0), Vec2::new(520.0, 40.0)),
        Fill::Solid(s.t.panel),
    );
    for (i, (name, st)) in states.iter().enumerate() {
        let x = 16.0 + 120.0 * i as f32;
        caption(b, s, Pos2::new(x, 36.0), name, 110.0);
        chrome::tab(
            b,
            Rect::from_min_size(Pos2::new(x, 50.0), Vec2::new(110.0, 28.0)),
            s,
            *st,
            Src::Str("Mixer"),
        );
    }
    let labels = [Src::Str("Pattern"), Src::Str("Song"), Src::Str("Loop")];
    for (i, (h, hovered)) in [(24.0, None), (24.0, Some(2)), (20.0, None)]
        .iter()
        .enumerate()
    {
        let r = Rect::from_min_size(
            Pos2::new(16.0 + 200.0 * i as f32, 130.0),
            Vec2::new(180.0, *h),
        );
        segmented::paint(b, r, s, REST, &labels, 1, *hovered);
    }
    for (i, (name, st)) in [
        ("Off", REST),
        ("Off, hover", hov()),
        ("On", lit()),
        ("Disabled", dis()),
    ]
    .iter()
    .enumerate()
    {
        let x = 16.0 + 130.0 * i as f32;
        caption(b, s, Pos2::new(x, 172.0), name, 110.0);
        led::button(
            b,
            Rect::from_min_size(Pos2::new(x, 190.0), Vec2::new(110.0, 22.0)),
            s,
            *st,
            Src::Str("Oversample"),
        );
    }
    let lamp_on = Pos2::new(560.0, 200.0);
    led::slate_lamp(b, lamp_on, 8.0, true, false, s, s.t.led, 1.0);
    led::slate_lamp(
        b,
        lamp_on + Vec2::new(24.0, 0.0),
        8.0,
        false,
        false,
        s,
        s.t.led,
        1.0,
    );
    led::slate_lamp(
        b,
        lamp_on + Vec2::new(48.0, 0.0),
        8.0,
        false,
        true,
        s,
        s.t.led,
        1.0,
    );
    caption(b, s, Pos2::new(540.0, 172.0), "Enable lamp", 100.0);
    for (i, (name, st)) in [("Rest", REST), ("Hover", hov()), ("Dragged", act())]
        .iter()
        .enumerate()
    {
        let x = 16.0 + 160.0 * i as f32;
        caption(b, s, Pos2::new(x, 240.0), name, 140.0);
        b.rect(
            Rect::from_min_size(Pos2::new(x, 258.0), Vec2::new(140.0, 50.0)),
            Fill::Solid(s.t.panel),
        );
        chrome::scrollbar(
            b,
            Rect::from_min_size(Pos2::new(x, 296.0), Vec2::new(140.0, 8.0)),
            s,
            *st,
            0.2,
            0.4,
        );
    }
}

fn dialogs(b: &mut Brush, s: &Style) {
    title(b, s, Pos2::new(16.0, 10.0), "Buttons and dialogs");
    let states: [(&str, State, Option<f32>); 6] = [
        ("Rest", REST, None),
        ("Hover", hov(), None),
        ("Pressed", act(), None),
        ("Focused", foc(), None),
        ("Disabled", dis(), None),
        ("Busy", REST, Some(0.1)),
    ];
    for (j, kind) in Kind::ALL.iter().enumerate() {
        for (i, (name, st, busy)) in states.iter().enumerate() {
            let x = 120.0 + 140.0 * i as f32;
            if j == 0 {
                caption(b, s, Pos2::new(x, 36.0), name, 110.0);
            }
            let label = match kind {
                Kind::Primary => "Export",
                Kind::Secondary => "Cancel",
                Kind::Destructive => "Delete",
                Kind::Quiet => "Details",
            };
            let w = dialog::width(b.ctx(), s, Src::Str(label), false, busy.is_some());
            let r = Rect::from_min_size(
                Pos2::new(x, 58.0 + 40.0 * j as f32),
                Vec2::new(w, dialog::HEIGHT),
            );
            dialog::button(b, r, s, *st, *kind, Src::Str(label), *busy);
        }
        let n = Rect::from_min_size(
            Pos2::new(16.0, 58.0 + 40.0 * j as f32),
            Vec2::new(100.0, 28.0),
        );
        let _ = text::paint(
            b,
            n,
            &Text::new(&format!("{kind:?}"), Role::Control, s.t.text).align(Align2::LEFT_CENTER),
            1.0,
        );
    }
    let window = Rect::from_min_size(Pos2::new(16.0, 230.0), Vec2::new(948.0, 230.0));
    b.rect(window, Fill::Solid(s.t.panel));
    dialog::scrim(b, window, s);
    let f = Rect::from_min_size(Pos2::new(60.0, 246.0), Vec2::new(440.0, 200.0));
    let parts = dialog::frame(b, f, s, Src::Str("Delete 3 tracks?"), true, false);
    let body = Text::new(
        "Kick, Clap and Hats go, with their 14 clips.",
        Role::Body,
        s.t.text_dim,
    )
    .align(Align2::LEFT_TOP);
    let _ = text::paint(b, parts.body, &body, 1.0);
    let by = parts.footer.top() + 14.0;
    let w2 = dialog::width(b.ctx(), s, Src::Str("Delete tracks"), false, false);
    let w1 = dialog::width(b.ctx(), s, Src::Str("Cancel"), false, false);
    let x2 = parts.footer.right() - 20.0 - w2;
    let x1 = x2 - 10.0 - w1;
    dialog::button(
        b,
        Rect::from_min_size(Pos2::new(x1, by), Vec2::new(w1, 28.0)),
        s,
        foc(),
        Kind::Secondary,
        Src::Str("Cancel"),
        None,
    );
    dialog::button(
        b,
        Rect::from_min_size(Pos2::new(x2, by), Vec2::new(w2, 28.0)),
        s,
        REST,
        Kind::Destructive,
        Src::Str("Delete tracks"),
        None,
    );
    let kw = dialog::keycap(
        b,
        Pos2::new(parts.footer.left() + 20.0, by + 14.0),
        s,
        "Esc",
    );
    let r = Rect::from_min_size(
        Pos2::new(parts.footer.left() + 26.0 + kw, by + 6.0),
        Vec2::new(80.0, 16.0),
    );
    let _ = text::paint(
        b,
        r,
        &Text::new("cancels", Role::Label, s.t.placeholder).align(Align2::LEFT_CENTER),
        1.0,
    );
}

fn tips(b: &mut Brush, s: &Style) {
    use cantus_ui::kit::hint::tip::{self, Look};
    title(b, s, Pos2::new(16.0, 10.0), "Tooltips");
    let plain = Look {
        name: "Cutoff",
        value: "2.40 kHz",
        what: "Where the filter starts to cut.",
        how: "Drag · Shift for fine · double-click to type",
        sign: None,
    };
    tip::paint(b, Pos2::new(16.0, 40.0), s, &plain);
    let auto = Look {
        name: "Cutoff",
        value: "1.85 kHz",
        what: "Automated by the clip 'Cutoff' on Lead",
        how: "Right-click → Edit automation, or delete the clip",
        sign: Some(Sign::Automated),
    };
    tip::paint(b, Pos2::new(300.0, 40.0), s, &auto);
    let modu = Look {
        name: "Drive",
        value: "6.0 dB",
        what: "Modulated by LFO 1",
        how: "Alt+drag sets the depth",
        sign: Some(Sign::Modulated),
    };
    tip::paint(b, Pos2::new(300.0, 120.0), s, &modu);
    let line = "Cutoff 2.40 kHz \u{b7} Drag, Shift for fine \u{b7} Alt+drag sets the depth \u{b7} Ctrl+Z undoes";
    let r = Rect::from_min_size(Pos2::new(16.0, 168.0), Vec2::new(700.0, 22.0));
    b.rect(r, Fill::Solid(s.t.toolbar));
    cantus_ui::kit::hint::keys::paint(b, r, s, line, Role::Body, s.t.text_dim, 1.0);
}
