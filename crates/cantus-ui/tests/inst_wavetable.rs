//! Unda's editor: it opens in every view and paints real numbers on the kit (ADR 0024);
//! no text is clipped, elided or overlapping at 75 %, 100 % and 200 % and in Spanish; a
//! drag on a cell is one undo step and a dropped source connects; its presets set what
//! they name, name their macros, and keep their headroom; table import reads what each
//! format says; every string has its Spanish twin.

mod inst_look;
mod support;
use cantus_core::inst::modulation::connections;
use cantus_core::*;
use cantus_ui::views::inst::wavetable::{self as unda, import, FACTORY_PRESETS};
use cantus_ui::views::inst::View;
use support::*;

fn with_unda() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
    );
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("Unda")
}

fn paint_inst(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 900.0), 3, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    })
}

/// Whether the painted text holds `want`, whatever its case (the kit sets titles and
/// labels in capitals).
fn has(text: &str, want: &str) -> bool {
    text.to_uppercase().contains(&want.to_uppercase())
}

#[test]
fn it_opens_in_every_view_and_paints_real_numbers() {
    let (mut a, track) = with_unda();
    a.open_inst_editor(track);
    a.inst.view = View::Edit;
    let text = paint_inst(&mut a);
    for want in [
        "Unda",
        "OSC 1",
        "OSC 3",
        "SUB",
        "NOISE",
        "Basic shapes",
        "Frame 1 / 4",
        "DETUNE",
        "+20",
        "FILTER 1",
        "FILTER 2",
        "2.00 kHz",
        "Serial",
        "off: a wire",
        "ENVELOPE 1",
        "LFO 1",
        "VOICE",
        "OSC 1 · ENGINE",
        "Wavetable",
    ] {
        assert!(has(&text, want), "Edit: missing {want:?} in {text}");
    }
    a.inst.wavetable.lfo = 2;
    a.inst.wavetable.env = 3;
    let text = paint_inst(&mut a);
    assert!(has(&text, "LFO 3") && has(&text, "ENVELOPE 4"), "{text}");
    a.inst.view = View::Play;
    let text = paint_inst(&mut a);
    for want in ["OSC 1", "FILTER 1", "AMP ENVELOPE", "1/4", "3D"] {
        assert!(has(&text, want), "Play: missing {want:?} in {text}");
    }
    a.inst.wavetable.osc = 2;
    assert!(has(&paint_inst(&mut a), "OSC 3"));
    a.inst.view = View::Matrix;
    assert!(has(&paint_inst(&mut a), "Unda"));
}

#[test]
fn the_partes_and_grana_panels_show_their_own_picture_and_knobs() {
    let (mut a, track) = with_unda();
    a.open_inst_editor(track);
    a.inst.view = View::Edit;
    a.set_inst_value(track, "osc1.engine", 1.0, false);
    let text = paint_inst(&mut a);
    for want in ["Partes", "64 partials", "TILT", "+0dB/oct", "All"] {
        assert!(has(&text, want), "Partes: missing {want:?} in {text}");
    }
    a.set_inst_value(track, "osc1.engine", 2.0, false);
    let text = paint_inst(&mut a);
    for want in ["Grana", "Grains of the table", "SPRAY", "Hann", "30Hz"] {
        assert!(has(&text, want), "Grana: missing {want:?} in {text}");
    }
}

#[test]
fn the_demo_is_alive() {
    let mut a = app(cantus_ui::starter_project());
    a.demo_editor("wavetable");
    assert!(a.inst.injected.is_some(), "the demo feeds telemetry");
    let track = a.inst.open.expect("the editor is open");
    let s = spec(&a, track);
    assert_eq!(s.text("macro1.name"), Some("Vowel"));
    a.inst.view = View::Edit;
    let text = paint_inst(&mut a);
    assert!(has(&text, "Vowels"), "{text}");
}

/// Every text of Play and Edit fits its box: nothing clipped, nothing elided, nothing
/// overlapping — at 75 %, 100 % and 200 %, in English and in Spanish, for each engine.
#[test]
fn no_text_is_clipped_elided_or_overlapping_at_any_scale_or_language() {
    let mut found = Vec::new();
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for engine in [0.0, 1.0, 2.0] {
            let mut a = app(cantus_ui::starter_project());
            cantus_ui::i18n::set_thread_locale(locale);
            let track = a.project().tracks[0].id;
            a.set_instrument(
                track,
                InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
            );
            unda::demo_setup(&mut a, track);
            a.set_inst_value(track, "osc1.engine", engine, false);
            a.open_inst_editor(track);
            for k in [0.75, 1.0, 2.0] {
                for view in [View::Play, View::Edit] {
                    let ctx = ctx();
                    cantus_ui::i18n::set_thread_locale(locale);
                    let texts = inst_look::paint_at(&ctx, &mut a, track, view, k);
                    for p in inst_look::problems(&texts, inst_look::area(k, view)) {
                        found.push(format!("{locale:?} engine {engine} {k}× {view:?}: {p}"));
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

/// A real pointer drag on a cell (OSC 1's fine tune) moves it, and one undo takes the
/// whole drag back; a modulation chip dropped on a cell connects it.
#[test]
fn a_cell_drag_is_one_undo_step_and_a_dropped_source_connects() {
    let (mut a, track) = with_unda();
    a.open_inst_editor(track);
    let ctx = ctx();
    let texts = inst_look::paint_at(&ctx, &mut a, track, View::Edit, 1.0);
    let label = texts
        .iter()
        .find(|p| p.text == "FINE")
        .expect("OSC 1's FINE cell");
    // The knob sits above its label: 4 pt of gap, then the M knob (36 pt).
    let knob = egui::pos2(label.rect.center().x, label.rect.top() - 4.0 - 18.0);
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
    let moved = spec(&a, track).get("osc1.fine");
    assert!(moved > 10.0, "the drag moved the fine tune: {moved}");
    a.undo();
    assert!(
        !spec(&a, track).is_stored("osc1.fine"),
        "one undo takes the whole drag back"
    );
    // Drag the LFO1 chip from the modulation card onto the cell.
    let texts = inst_look::texts(&inst_look::frame(&ctx, &mut a, size, Vec::new(), t + 0.1));
    let chip = texts
        .iter()
        .find(|p| p.text == "LFO1")
        .expect("the LFO1 chip");
    inst_look::drag(&ctx, &mut a, size, chip.rect.center(), knob, 8, t + 0.2);
    let s = spec(&a, track);
    assert!(
        connections(&s).any(|c| c.src == "lfo1" && c.dst == "osc1.fine"),
        "the dropped chip connects LFO 1 to the fine tune"
    );
}

/// With `CANTUS_INST_SHOTS=<dir>`: Unda in every view and theme, and Edit at 200 % and in
/// Spanish — the design review.
#[test]
fn shots() {
    let Some(dir) = std::env::var_os("CANTUS_INST_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        for (view, tag) in [
            (View::Play, "play"),
            (View::Edit, "edit"),
            (View::Matrix, "matrix"),
        ] {
            let mut a = app(cantus_ui::starter_project());
            let track = a.project().tracks[0].id;
            a.set_instrument(
                track,
                InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
            );
            unda::demo_setup(&mut a, track);
            a.open_inst_editor(track);
            a.inst.view = view;
            let c = ctx();
            cantus_ui::theme::apply(&theme, &c);
            let path = dir.join(format!("unda-{tag}-{theme_name}.png"));
            cantus_ui::raster::shoot(&c, egui::vec2(1040.0, 680.0), 1.0, 6, Some(&path), |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui);
            });
        }
    }
    for (name, k, locale) in [
        ("unda-edit-slate-200", 2.0, cantus_ui::i18n::Locale::En),
        ("unda-play-slate-es", 1.0, cantus_ui::i18n::Locale::Es),
        ("unda-edit-slate-es", 1.0, cantus_ui::i18n::Locale::Es),
    ] {
        let mut a = app(cantus_ui::starter_project());
        cantus_ui::i18n::set_thread_locale(locale);
        let track = a.project().tracks[0].id;
        a.set_instrument(
            track,
            InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        );
        unda::demo_setup(&mut a, track);
        a.open_inst_editor(track);
        a.inst.view = if name.contains("play") {
            View::Play
        } else {
            View::Edit
        };
        let c = ctx();
        cantus_ui::i18n::set_thread_locale(locale);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &c);
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
    }
    // Edit scrolled to its foot, in both themes: the rows below the fold.
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for (theme_name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        let mut a = app(cantus_ui::starter_project());
        let track = a.project().tracks[0].id;
        a.set_instrument(
            track,
            InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable)),
        );
        unda::demo_setup(&mut a, track);
        a.open_inst_editor(track);
        a.inst.view = View::Edit;
        let c = ctx();
        cantus_ui::theme::apply(&theme, &c);
        let spec = spec(&a, track);
        let path = dir.join(format!("unda-edit-{theme_name}-full.png"));
        cantus_ui::raster::shoot(&c, egui::vec2(1040.0, 1120.0), 1.0, 4, Some(&path), |ui| {
            inst_look::full_body(ui, |ui, s| unda::body(&mut a, ui, track, &spec, s));
        });
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx());
}

#[test]
fn a_gesture_is_one_undo_step() {
    let (mut a, track) = with_unda();
    for v in [0.1, 0.2, 0.35] {
        a.set_inst_value(track, "osc1.pos", v, true);
    }
    a.end_gesture();
    assert_eq!(spec(&a, track).get("osc1.pos"), 0.35);
    a.undo();
    assert!(
        !spec(&a, track).is_stored("osc1.pos"),
        "one undo takes the whole drag back"
    );
    // A drag on the filter curve moves two values: still one step.
    for (c, r) in [(800.0, 0.2), (600.0, 0.3), (400.0, 0.5)] {
        unda::set_values(
            &mut a,
            track,
            "flt1.drag",
            &[("flt1.cutoff", c), ("flt1.res", r)],
        );
    }
    a.end_gesture();
    assert_eq!(spec(&a, track).get("flt1.res"), 0.5);
    a.undo();
    let s = spec(&a, track);
    assert!(!s.is_stored("flt1.cutoff") && !s.is_stored("flt1.res"));
}

#[test]
fn presets_set_what_they_name_and_name_their_macros() {
    assert!((8..=16).contains(&FACTORY_PRESETS.len()));
    let kind = InstrumentKind::Wavetable;
    for p in FACTORY_PRESETS {
        let name = cantus_ui::i18n::t(p.label_key);
        assert_ne!(name, p.label_key, "{} has no string", p.label_key);
        for (k, _) in p.values {
            assert!(
                kind.param(k).is_some(),
                "{}: {k} is not a parameter",
                p.label_key
            );
        }
        for (k, _) in p.text {
            assert!(
                kind.is_text_key(k),
                "{}: {k} is not a text key",
                p.label_key
            );
        }
        let s = cantus_ui::views::inst::preset_spec(kind, p);
        let routes: Vec<_> = connections(&s).collect();
        assert!(!routes.is_empty(), "{}: no modulation", p.label_key);
        for c in &routes {
            assert!(
                cantus_engine::inst::wavetable::target_of(c.dst).is_some(),
                "{}: {} is not modulatable",
                p.label_key,
                c.dst
            );
            if c.src.starts_with("macro") {
                let n = &c.src[5..];
                assert!(
                    s.text(&format!("macro{n}.name")).is_some(),
                    "{}: {} unnamed",
                    p.label_key,
                    c.src
                );
            }
        }
        assert!(!p.tags.is_empty());
        let (mut a, track) = with_unda();
        a.set_inst_value(track, "osc3.level", -20.0, false);
        a.apply_inst_spec(track, &s, "cmd.fx_preset");
        assert_eq!(spec(&a, track), s, "{}", p.label_key);
    }
}

#[test]
fn every_preset_keeps_its_headroom_on_a_loud_chord() {
    use std::sync::Arc;
    let mut report = Vec::new();
    for p in FACTORY_PRESETS {
        let s = cantus_ui::views::inst::preset_spec(InstrumentKind::Wavetable, p);
        let mut proj = Project::new("unda");
        proj.tempo_bpm = 120.0;
        let mut t = proj.make_track("Unda");
        t.instrument = InstrumentSpec::Native(s);
        let mut clip = proj.make_clip("Chord", Ticks::ZERO, Ticks::beats(4));
        for key in [36u8, 48, 55, 60, 64, 67, 72, 79] {
            clip.notes
                .push(proj.make_note(Ticks::ZERO, Ticks::beats(3), key, 127));
        }
        t.clips.push(clip);
        proj.tracks.push(t);
        let opts = cantus_engine::RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        };
        let out = cantus_engine::render(Arc::new(Snapshot::compile(&proj, 1)), &opts)
            .unwrap()
            .samples;
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        report.push(format!("{}: {:.1} dBFS", p.label_key, 20.0 * peak.log10()));
        assert!(peak < 1.0 && peak > 0.01, "{}: peak {peak}", p.label_key);
    }
    println!("{}", report.join("\n"));
}

#[test]
fn a_table_file_is_imported_in_one_step_with_its_frame_size() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("four.wav");
    let samples: Vec<f32> = (0..4 * 256)
        .map(|i| ((i % 256) as f32 / 128.0) - 1.0)
        .collect();
    std::fs::write(&path, import::table_wav(&samples, 256)).unwrap();
    let (mut a, track) = with_unda();
    let found = unda::import_table(&mut a, track, 1, &path).unwrap();
    assert_eq!((found.frame, found.frames, found.source), (256, 4, "clm"));
    let s = spec(&a, track);
    assert_eq!(s.text("osc2.table"), Some(path.to_string_lossy().as_ref()));
    assert_eq!(s.text("osc2.name"), Some("four"));
    assert_eq!((s.get("osc2.frame"), s.get("osc2.on")), (256.0, 1.0));
    // The editor decodes it for drawing.
    let shown = unda::table_for(&mut a.inst.wavetable, &s, 1).expect("decoded");
    assert_eq!(shown.frames(), 4);
    a.undo();
    let s = spec(&a, track);
    assert!(s.text("osc2.table").is_none() && !s.is_stored("osc2.frame"));
}

/// A minimal WAV with the given extra chunks.
fn wav_with(chunks: &[(&[u8; 4], Vec<u8>)], samples: usize) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(b"WAVE");
    let mut fmt = Vec::new();
    for v in [1u16, 1] {
        fmt.extend_from_slice(&v.to_le_bytes());
    }
    fmt.extend_from_slice(&48_000u32.to_le_bytes());
    fmt.extend_from_slice(&96_000u32.to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let mut all: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"fmt ", fmt)];
    all.extend(chunks.iter().cloned());
    all.push((b"data", vec![0u8; samples * 2]));
    for (id, c) in all {
        body.extend_from_slice(id);
        body.extend_from_slice(&(c.len() as u32).to_le_bytes());
        body.extend_from_slice(&c);
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

#[test]
fn every_format_says_its_frame_size() {
    let dir = tempfile::tempdir().unwrap();
    let check = |name: &str, bytes: Vec<u8>, want: (usize, &str)| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let f = import::inspect(&path, dir.path()).unwrap();
        assert_eq!((f.frame, f.source), want, "{name}");
    };
    check(
        "clm.wav",
        wav_with(&[(b"clm ", b"<!>1024 01000000 wavetable".to_vec())], 4096),
        (1024, "clm"),
    );
    check(
        "uhwt.wav",
        wav_with(&[(b"uhWT", vec![0; 4])], 4096),
        (2048, "uhwt"),
    );
    let mut srge = 1i32.to_le_bytes().to_vec();
    srge.extend_from_slice(&512i32.to_le_bytes());
    check(
        "srge.wav",
        wav_with(&[(b"srge", srge)], 4096),
        (512, "srge"),
    );
    let mut smpl = vec![0u8; 36];
    smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
    let mut lp = vec![0u8; 24];
    lp[8..12].copy_from_slice(&0u32.to_le_bytes());
    lp[12..16].copy_from_slice(&511u32.to_le_bytes());
    smpl.extend(lp);
    check(
        "smpl.wav",
        wav_with(&[(b"smpl", smpl)], 4096),
        (512, "smpl"),
    );
    let mut cue = 2u32.to_le_bytes().to_vec();
    for off in [0u32, 256] {
        let mut p = vec![0u8; 24];
        p[20..24].copy_from_slice(&off.to_le_bytes());
        cue.extend(p);
    }
    check("cue.wav", wav_with(&[(b"cue ", cue)], 4096), (256, "cue"));
    check("plain.wav", wav_with(&[], 8192), (2048, "multiple"));
    check("single.wav", wav_with(&[], 600), (600, "single"));
    assert_eq!(
        import::inspect(&dir.path().join("none.wav"), dir.path()).unwrap_err(),
        import::ImportError::Unreadable
    );
    std::fs::write(dir.path().join("junk.wav"), b"not a wav at all").unwrap();
    assert_eq!(
        import::inspect(&dir.path().join("junk.wav"), dir.path()).unwrap_err(),
        import::ImportError::NotWav
    );
}

#[test]
fn a_surge_wt_becomes_a_wav_the_engine_reads() {
    let dir = tempfile::tempdir().unwrap();
    for (flags, name) in [(0u16, "float"), (0x04 | 0x08, "int16"), (0x04, "int15")] {
        let mut b = b"vawt".to_vec();
        b.extend_from_slice(&256u32.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&flags.to_le_bytes());
        for i in 0..3 * 256 {
            let v = ((i % 256) as f32 / 256.0 * std::f32::consts::TAU).sin() * 0.5;
            if flags & 0x04 != 0 {
                let scale = if flags & 0x08 != 0 {
                    32_767.0
                } else {
                    16_384.0
                };
                b.extend_from_slice(&((v * scale) as i16).to_le_bytes());
            } else {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        let src = dir.path().join(format!("{name}.wt"));
        std::fs::write(&src, b).unwrap();
        let out = dir.path().join("converted");
        let f = import::inspect(&src, &out).unwrap();
        assert_eq!((f.frame, f.frames, f.source), (256, 3, "wt"));
        assert!(f.path.starts_with(&out) && f.path.extension().unwrap() == "wav");
        // The converted file declares its frame size and holds the samples at their level.
        let again = import::inspect(&f.path, &out).unwrap();
        assert_eq!((again.frame, again.source), (256, "clm"));
        let mut r = hound::WavReader::open(&f.path).unwrap();
        let peak = r
            .samples::<f32>()
            .map(Result::unwrap)
            .fold(0f32, |m, x| m.max(x.abs()));
        assert!((peak - 0.5).abs() < 1e-3, "{name}: {peak}");
    }
    // A sample, not a table, is refused.
    let mut b = b"vawt".to_vec();
    b.extend_from_slice(&256u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    let src = dir.path().join("sample.wt");
    std::fs::write(&src, b).unwrap();
    assert_eq!(
        import::inspect(&src, dir.path()).unwrap_err(),
        import::ImportError::BadWt
    );
}

#[test]
fn every_english_string_has_its_spanish_twin() {
    let keys = |s: &str| -> std::collections::BTreeSet<String> {
        s.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_string()))
            .collect()
    };
    let en = keys(include_str!("../locales/inst/wavetable.en.txt"));
    let es = keys(include_str!("../locales/inst/wavetable.es.txt"));
    assert_eq!(en, es);
    for info in InstrumentKind::Wavetable.params() {
        assert!(
            en.contains(info.label_key) || info.label_key.starts_with("inst."),
            "{}",
            info.label_key
        );
    }
}
