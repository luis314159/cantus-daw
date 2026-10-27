//! Modulus's editor: it opens in Play, Edit and Matrix and paints its name; no text is
//! clipped, elided or overlapping at 75 %, 100 % and 200 % and in Spanish (ADR 0024);
//! every gesture (a cell dragged, a source dropped, wiring the graph, an algorithm
//! template, a DX7 import, Bake, a preset) is one undo step that comes back exactly; the
//! SysEx codec reads what it writes.

mod inst_look;
mod support;
use cantus_core::inst::fm::{carrier_db, OP_KEYS, OUT_OFF};
use cantus_core::*;
use cantus_ui::views::inst::fm::{self, sysex};
use cantus_ui::views::inst::View;
use support::*;

fn with_fm() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Fm)),
    );
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("native")
}

fn paint_inst(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 1000.0), 3, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    })
}

#[test]
fn it_opens_in_every_view_and_paints_its_name_and_numbers() {
    let (mut a, track) = with_fm();
    fm::demo_setup(&mut a, track);
    a.open_inst_editor(track);
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        let text = paint_inst(&mut a);
        assert!(text.contains("MODULUS"), "{view:?}: {text}");
        if view != View::Matrix {
            assert!(text.contains("OUT"), "{view:?}: the graph's output bar");
            assert!(text.contains("Carson"), "{view:?}: the spectrum's edge");
        }
    }
    a.inst.view = View::Edit;
    a.inst.fm.matrix_view = true;
    let text = paint_inst(&mut a);
    assert!(
        text.contains("1.40"),
        "the matrix prints the tine's index: {text}"
    );
}

#[test]
fn wiring_the_graph_is_one_undo_step_each() {
    let (mut a, track) = with_fm();
    let s0 = spec(&a, track);
    fm::connect(&mut a, track, &s0, 1, 0);
    let s1 = spec(&a, track);
    assert_eq!(s1.get("m.b.a"), 1.5);
    fm::connect(&mut a, track, &s1, 1, 1);
    assert_eq!(spec(&a, track).get("fb.b"), 1.0, "onto itself: feedback");
    let s2 = spec(&a, track);
    fm::set_carrier(&mut a, track, &s2, 2, true);
    assert_eq!(spec(&a, track).get("out.c"), 0.0);
    a.undo();
    assert_eq!(spec(&a, track), s2);
    a.undo();
    assert_eq!(spec(&a, track), s1);
    a.undo();
    assert_eq!(spec(&a, track), s0);
    let s = spec(&a, track);
    fm::connect(&mut a, track, &s, 3, 0);
    let s = spec(&a, track);
    fm::disconnect(&mut a, track, &s, 3, 0);
    assert!(
        !spec(&a, track).is_stored("m.d.a"),
        "disconnecting leaves nothing stored"
    );
}

#[test]
fn a_two_way_drag_is_one_undo_step() {
    let (mut a, track) = with_fm();
    let before = spec(&a, track);
    let k = &OP_KEYS[0];
    for i in 1..=20 {
        let v = i as f32;
        fm::drag_values(
            &mut a,
            track,
            &[(k.env_l[1], 0.5 + v / 100.0), (k.env_t[1], 300.0 + v)],
        );
    }
    a.end_gesture();
    let s = spec(&a, track);
    assert!((s.get(k.env_l[1]) - 0.7).abs() < 1e-6 && s.get(k.env_t[1]) == 320.0);
    a.undo();
    assert_eq!(
        spec(&a, track),
        before,
        "twenty frames of a drag, one undo step"
    );
}

#[test]
fn an_algorithm_is_one_undo_step_and_names_itself() {
    let (mut a, track) = with_fm();
    let before = spec(&a, track);
    assert!(fm::algorithm_changes(&before, 5) > 0);
    fm::apply_algorithm(&mut a, track, &before, 5);
    let s = spec(&a, track);
    assert_eq!(fm::algorithm_of(&s), Some(5));
    assert_eq!(s.get("m.b.a"), 2.0);
    assert_eq!(s.get("out.e"), carrier_db(3));
    assert_eq!(
        fm::algorithm_changes(&s, 5),
        0,
        "applying it again changes nothing"
    );
    a.undo();
    assert_eq!(spec(&a, track), before);
}

/// A recognisable voice: algorithm 5, operator 2 at ratio 14, operator 6 fixed at 100 Hz.
fn test_voice() -> sysex::DxVoice {
    let mut v = sysex::init_voice();
    v.algorithm = 4;
    v.feedback = 6;
    v.ops[1].coarse = 14;
    v.ops[1].output = 80;
    v.ops[1].velocity = 7;
    v.ops[1].rates = [99, 60, 40, 70];
    v.ops[1].levels = [99, 70, 0, 0];
    v.ops[5].fixed = true;
    v.ops[5].coarse = 2;
    v.ops[5].fine = 0;
    v.ops[2].output = 99;
    v.ops[2].left_depth = 99;
    v.ops[2].left_curve = 1;
    v.transpose = 36;
    v.name = *b"TEST TINE ";
    v
}

#[test]
fn sysex_reads_what_it_writes_byte_for_byte() {
    let voices: Vec<_> = (0..32)
        .map(|i| {
            let mut v = test_voice();
            v.lfo_speed = i as u8;
            v.ops[0].fine = (i * 3) as u8;
            v
        })
        .collect();
    let bank = sysex::bank_sysex(&voices);
    assert_eq!(bank.len(), 4104);
    let read = sysex::parse(&bank).unwrap();
    assert!(read.checksum_ok && read.clamped == 0);
    let sysex::Parsed::Bank(back) = read.parsed else {
        panic!("a bank");
    };
    assert_eq!(back, voices);
    assert_eq!(sysex::bank_sysex(&back), bank, "byte for byte");
    let one = sysex::voice_sysex(&test_voice());
    assert_eq!(one.len(), 163);
    let read = sysex::parse(&one).unwrap();
    assert_eq!(read.parsed, sysex::Parsed::Voice(test_voice()));
    // Out of range is clamped and counted; a wrong checksum still loads.
    let mut bad = one.clone();
    bad[6] = 120;
    let n = bad.len();
    bad[n - 2] ^= 1;
    let read = sysex::parse(&bad).unwrap();
    assert_eq!(read.clamped, 1);
    assert!(!read.checksum_ok);
    assert_eq!(
        sysex::parse(&[0xF0, 0x41]),
        Err(sysex::SysexError::NotYamaha)
    );
}

#[test]
fn importing_a_dx_voice_is_one_undo_step_with_the_published_scalings() {
    let (mut a, track) = with_fm();
    let before = spec(&a, track);
    fm::import_bytes(&mut a, track, &sysex::voice_sysex(&test_voice()));
    let s = spec(&a, track);
    assert_eq!(fm::algorithm_of(&s), Some(5));
    assert_eq!(s.get(OP_KEYS[1].ratio), 14.0);
    assert!((s.get(OP_KEYS[1].level) - (80.0 + 28.0 - 127.0) * 0.7526).abs() < 0.01);
    assert_eq!(s.get(OP_KEYS[1].vel), 1.0);
    assert_eq!(s.get(OP_KEYS[1].env_mode), 1.0);
    assert_eq!(s.get(OP_KEYS[1].dx_r[2]), 40.0);
    assert_eq!(s.get(OP_KEYS[5].freq_mode), 1.0);
    assert!((s.get(OP_KEYS[5].fixed) - 100.0).abs() < 0.01);
    assert_eq!(s.get(OP_KEYS[2].ks_ldepth), 48.0);
    assert_eq!(s.get(OP_KEYS[2].ks_lcurve), 1.0);
    assert_eq!(
        s.get("fb.f"),
        std::f32::consts::PI / 2.0,
        "feedback 6 = π/2"
    );
    assert_eq!(s.get("transpose"), 12.0);
    assert_eq!(s.get(OP_KEYS[6].out), OUT_OFF);
    assert_eq!(s.text("name"), Some("TEST TINE"));
    a.undo();
    assert_eq!(spec(&a, track), before, "one undo step");
    // A cartridge waits for a pick.
    fm::import_bytes(&mut a, track, &sysex::bank_sysex(&[test_voice()]));
    assert_eq!(a.inst.fm.bank.as_ref().map(|b| b.len()), Some(32));
    assert_eq!(spec(&a, track), before);
}

#[test]
fn bake_writes_the_easy_macros_into_the_patch_in_one_step() {
    let (mut a, track) = with_fm();
    let s = spec(&a, track);
    fm::connect(&mut a, track, &s, 1, 0);
    a.set_inst_value(track, "macro.bright", 0.5, false);
    a.set_inst_value(track, "macro.a", 1.0 / 3.0, false);
    let before = spec(&a, track);
    fm::bake(&mut a, track, &before);
    let s = spec(&a, track);
    assert!(
        (s.get("m.b.a") - 3.0).abs() < 1e-4,
        "index ×2 at brightness +50 %"
    );
    assert!(
        (s.get(OP_KEYS[0].env_t[0]) - 10.0).abs() < 1e-3,
        "attack ×2"
    );
    assert!(!s.is_stored("macro.bright") && !s.is_stored("macro.a"));
    // What sounds is the same before and after baking.
    let (p0, p1) = (
        cantus_engine::inst::fm::patch_of(&before),
        cantus_engine::inst::fm::patch_of(&s),
    );
    let mut m0 = cantus_dsp::modulus::Modulus::new(48_000.0);
    let mut m1 = cantus_dsp::modulus::Modulus::new(48_000.0);
    m0.set_patch(&p0);
    m1.set_patch(&p1);
    let render = |m: &mut cantus_dsp::modulus::Modulus| {
        m.note_on(60, 1, 1.0);
        let (mut l, mut r) = (vec![0f32; 9_600], vec![0f32; 9_600]);
        m.process(&mut l, &mut r);
        l
    };
    let (x0, x1) = (render(&mut m0), render(&mut m1));
    let err = x0
        .iter()
        .zip(&x1)
        .fold(0f32, |e, (a, b)| e.max((a - b).abs()));
    assert!(err < 1e-4, "baking changed the sound by {err}");
    a.undo();
    assert_eq!(spec(&a, track), before);
}

#[test]
fn every_factory_preset_sets_what_it_names_in_one_step() {
    assert!(fm::FACTORY_PRESETS.len() >= 8);
    for p in fm::FACTORY_PRESETS {
        let (mut a, track) = with_fm();
        let target = cantus_ui::views::inst::preset_spec(InstrumentKind::Fm, p);
        for (k, v) in p.values {
            assert_eq!(target.get(k), *v, "{}: {k} out of range", p.label_key);
        }
        a.apply_inst_spec(track, &target, "cmd.fx_preset");
        assert_eq!(spec(&a, track), target, "{}", p.label_key);
        assert!(
            !cantus_ui::i18n::t(p.label_key).starts_with("fm."),
            "{} has no name",
            p.label_key
        );
        assert!(
            target.text("macro1.name").is_some(),
            "{} names its macro",
            p.label_key
        );
        a.undo();
        assert!(spec(&a, track).params.is_empty());
    }
}

/// Every text of Play and Edit fits its box: nothing clipped, nothing elided, nothing
/// overlapping — at 75 %, 100 % and 200 %, in English and in Spanish, on each detail page
/// and with the matrix grid in the graph's place.
#[test]
fn no_text_is_clipped_elided_or_overlapping_at_any_scale_or_language() {
    let mut found = Vec::new();
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for (page, grid) in [
            (fm::Detail::Envelope, false),
            (fm::Detail::Scaling, true),
            (fm::Detail::Oscillator, false),
        ] {
            let (mut a, track) = with_fm();
            cantus_ui::i18n::set_thread_locale(locale);
            fm::demo_setup(&mut a, track);
            a.inst.fm.detail = page;
            a.inst.fm.matrix_view = grid;
            a.open_inst_editor(track);
            for k in [0.75, 1.0, 2.0] {
                for view in [View::Play, View::Edit] {
                    let ctx = ctx();
                    cantus_ui::i18n::set_thread_locale(locale);
                    let texts = inst_look::paint_at(&ctx, &mut a, track, view, k);
                    for p in inst_look::problems(&texts, inst_look::area(k, view)) {
                        found.push(format!(
                            "{locale:?} {page:?} grid {grid} {k}× {view:?}: {p}"
                        ));
                    }
                }
            }
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(
        found.is_empty(),
        "{} problems:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// A real pointer drag on a cell (the easy brightness, in Play) moves it, and one undo
/// takes the whole drag back; a modulation chip dropped on it connects it.
#[test]
fn a_cell_drag_is_one_undo_step_and_a_dropped_source_connects() {
    let (mut a, track) = with_fm();
    a.open_inst_editor(track);
    let ctx = ctx();
    let texts = inst_look::paint_at(&ctx, &mut a, track, View::Play, 1.0);
    let label = texts
        .iter()
        .find(|p| p.text == "BRIGHTNESS" || p.text == "BRIGHT")
        .expect("the easy brightness cell");
    // The knob sits above its label: 4 pt of gap, then the L knob (56 pt).
    let knob = egui::pos2(label.rect.center().x, label.rect.top() - 4.0 - 28.0);
    let size = egui::vec2(1080.0, 720.0);
    let t = inst_look::drag(
        &ctx,
        &mut a,
        size,
        knob,
        knob - egui::vec2(0.0, 60.0),
        6,
        1.0,
    );
    let moved = spec(&a, track).get("macro.bright");
    assert!(moved > 0.1, "the drag moved the brightness: {moved}");
    a.undo();
    assert!(
        !spec(&a, track).is_stored("macro.bright"),
        "one undo takes the whole drag back"
    );
    let texts = inst_look::texts(&inst_look::frame(&ctx, &mut a, size, Vec::new(), t + 0.1));
    let chip = texts
        .iter()
        .find(|p| p.text == "LFO1")
        .expect("the LFO1 chip");
    inst_look::drag(&ctx, &mut a, size, chip.rect.center(), knob, 8, t + 0.2);
    let s = spec(&a, track);
    assert!(
        cantus_core::inst::modulation::connections(&s)
            .any(|c| c.src == "lfo1" && c.dst == "macro.bright"),
        "the dropped chip connects LFO 1 to the brightness"
    );
}

/// With `CANTUS_INST_SHOTS=<dir>`: Modulus in every view and theme, the other detail pages
/// and the grid, Edit at 200 % and in Spanish — the design review.
#[test]
fn shots() {
    let Some(dir) = std::env::var_os("CANTUS_INST_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let shoot = |name: &str,
                 view: View,
                 theme: &cantus_ui::theme::Theme,
                 k: f32,
                 locale: cantus_ui::i18n::Locale,
                 setup: &dyn Fn(&mut cantus_ui::CantusApp)| {
        let (mut a, track) = with_fm();
        cantus_ui::i18n::set_thread_locale(locale);
        fm::demo_setup(&mut a, track);
        setup(&mut a);
        a.open_inst_editor(track);
        a.inst.view = view;
        let c = ctx();
        cantus_ui::i18n::set_thread_locale(locale);
        cantus_ui::theme::apply(theme, &c);
        cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(&c, k);
        let path = dir.join(format!("{name}.png"));
        cantus_ui::raster::shoot(
            &c,
            egui::vec2(1040.0 * k, 680.0 * k),
            1.0,
            6,
            Some(&path),
            |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui);
            },
        );
    };
    let en = cantus_ui::i18n::Locale::En;
    let none = |_: &mut cantus_ui::CantusApp| {};
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for (view, tag) in [
            (View::Play, "play"),
            (View::Edit, "edit"),
            (View::Matrix, "matrix"),
        ] {
            shoot(
                &format!("modulus-{tag}-{theme_name}"),
                view,
                &theme,
                1.0,
                en,
                &none,
            );
        }
    }
    let slate = cantus_ui::theme::slate();
    shoot(
        "modulus-edit-slate-scaling-grid",
        View::Edit,
        &slate,
        1.0,
        en,
        &|a| {
            a.inst.fm.detail = fm::Detail::Scaling;
            a.inst.fm.matrix_view = true;
        },
    );
    shoot(
        "modulus-edit-slate-osc",
        View::Edit,
        &slate,
        1.0,
        en,
        &|a| a.inst.fm.detail = fm::Detail::Oscillator,
    );
    shoot("modulus-edit-slate-200", View::Edit, &slate, 2.0, en, &none);
    shoot(
        "modulus-edit-slate-es",
        View::Edit,
        &slate,
        1.0,
        cantus_ui::i18n::Locale::Es,
        &none,
    );
    shoot(
        "modulus-play-slate-es",
        View::Play,
        &slate,
        1.0,
        cantus_ui::i18n::Locale::Es,
        &none,
    );
    // Edit scrolled to its foot, in both themes: the rows below the fold.
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        let (mut a, track) = with_fm();
        cantus_ui::i18n::set_thread_locale(en);
        fm::demo_setup(&mut a, track);
        a.open_inst_editor(track);
        a.inst.view = View::Edit;
        let c = ctx();
        cantus_ui::theme::apply(&theme, &c);
        let spec = spec(&a, track);
        let path = dir.join(format!("modulus-edit-{theme_name}-full.png"));
        cantus_ui::raster::shoot(&c, egui::vec2(1040.0, 1020.0), 1.0, 4, Some(&path), |ui| {
            inst_look::full_body(ui, |ui, s| fm::body(&mut a, ui, track, &spec, s));
        });
    }
    cantus_ui::i18n::set_thread_locale(en);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}
