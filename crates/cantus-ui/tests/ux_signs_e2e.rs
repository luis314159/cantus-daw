//! An automated control says so (round 39, ADR 0109), end to end: the app
//! `CantusApp::assemble` puts together on the demo it opens with, a Chorda added the way the
//! instrument picker adds one, an automation clip on its *Hardness* — the case of round 38,
//! when the owner took knobs set by clips for broken. Its editor open in the window: the
//! knob wears the automated sign, shows the clip's value, and its hint says what drives it
//! and how to take it back, in the top bar's hint panel; the clip deleted, the sign goes,
//! the hint says nothing of a clip and the knob shows its own value again.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::automate::{probe, Drivers};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Pos2, Rect};

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup over the window.
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    /// What a shot paints: every texture uploaded, the last frame's shapes.
    textures: cantus_ui::raster::Textures,
    last: Vec<egui::epaint::ClippedShape>,
}

impl Rig {
    /// One frame with the pointer at `pointer`; every painted text with where it lies.
    fn frame(&mut self, pointer: Option<Pos2>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: pointer.map(egui::Event::PointerMoved).into_iter().collect(),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.textures.update(&out.textures_delta);
        out.textures_delta.clear();
        self.last = out.shapes.clone();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
        texts
    }

    /// `n` frames; the signs painted in the last.
    fn signs(&mut self, n: usize, pointer: Option<Pos2>) -> Vec<(Rect, Drivers)> {
        for _ in 1..n {
            self.frame(pointer);
        }
        probe::start_signs();
        self.frame(pointer);
        probe::take_signs()
    }
}

impl Rig {
    /// The last frame, rendered on the CPU into `path` with the pointer at `at`.
    fn shoot(&mut self, path: &std::path::Path, at: Pos2) {
        self.frame(Some(at));
        let shapes = std::mem::take(&mut self.last);
        let prims = self.ctx.tessellate(shapes, 1.0);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut canvas = cantus_ui::raster::Canvas::new(1600, 900, bg);
        canvas.paint(&prims, &self.textures, 1.0);
        let _ = canvas.write_png(path);
    }
}

/// The words the top bar's hint panel painted this frame.
fn panel_words(ctx: &egui::Context, texts: &[(String, Rect)]) -> String {
    let panel = cantus_ui::kit::inspect::items(ctx)
        .into_iter()
        .find(|i| i.part == "hint_panel")
        .expect("the hint panel")
        .rect;
    texts
        .iter()
        .filter(|(_, at)| panel.expand(1.0).contains_rect(*at))
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_knob_a_clip_drives_wears_the_sign_and_says_how_to_take_it_back() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let song = cantus_ui::demos::startup_project();
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(song),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    let mut r = Rig {
        app,
        ctx,
        t: 0.0,
        textures: Default::default(),
        last: Vec::new(),
    };
    r.frame(None);

    // A Chorda, the way the picker adds one, its editor open.
    r.app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = r
        .app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the Chorda track");
    if r.app.inst.open != Some(track) {
        r.app.open_inst_editor(track);
    }
    let target = AutoTarget::Instrument {
        track,
        key: "hardness".into(),
    };
    // Nothing drives it yet: no sign anywhere in the window.
    assert!(
        r.signs(8, None).is_empty(),
        "no control is driven before the clip"
    );

    // An automation clip on Hardness, as its menu's *Create automation clip* makes one,
    // then held at 90 % of its travel — far from the knob's own value.
    assert!(r.app.create_automation_clip(target.clone()));
    let (ctrack, clip, len) = r
        .app
        .project()
        .tracks
        .iter()
        .find_map(|t| {
            t.clips
                .iter()
                .find(|c| c.automation.as_ref().is_some_and(|a| a.target == target))
                .map(|c| (t.id, c.id, c.length))
        })
        .expect("the clip");
    let held = vec![AutoPoint::new(Ticks::ZERO, 0.9), AutoPoint::new(len, 0.9)];
    assert!(r.app.exec(Command::SetClipAutomation {
        track: ctrack,
        clip,
        automation: Some(Automation::new(target.clone(), held)),
    }));
    let (clip_name, track_name) = {
        let p = r.app.project();
        let t = p.track(ctrack).unwrap();
        let c = t.clips.iter().find(|c| c.id == clip).unwrap();
        (c.name.clone(), t.name.clone())
    };

    // The knob wears the automated sign: one control, the clip's sign.
    let signs = r.signs(8, None);
    let driven: Vec<_> = signs.iter().filter(|(_, d)| d.clip).collect();
    assert_eq!(driven.len(), 1, "one knob wears the clip's sign: {signs:?}");
    let knob = driven[0].0;

    // Hovered: its hint names it, shows the clip's value, says what drives it and how to
    // take it back — the menu's own words.
    let at = knob.center();
    let texts = {
        r.frame(Some(at));
        r.frame(Some(at));
        r.frame(Some(at));
        r.frame(Some(at))
    };
    let h = cantus_ui::kit::hint::current(&r.ctx).expect("the knob's hint");
    assert_eq!(h.name, cantus_ui::i18n::t("chorda.hardness"), "{h:?}");
    let driven_value = h.value.clone();
    for words in [
        format!("‘{clip_name}’"),
        track_name.clone(),
        cantus_ui::i18n::t("auto.param.open"),
    ] {
        assert!(
            h.driven.contains(&words),
            "the hint says {words:?}: {:?}",
            h.driven
        );
    }
    cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
    let texts = {
        let _ = texts;
        r.frame(Some(at));
        r.frame(Some(at))
    };
    let panel = panel_words(&r.ctx, &texts);
    // `CANTUS_UX_SHOTS=<dir>`: the window as it is now, for the round's pictures.
    if let Ok(dir) = std::env::var("CANTUS_UX_SHOTS") {
        r.shoot(
            &std::path::Path::new(&dir).join("driven-knob-hovered.png"),
            at,
        );
    }
    assert!(
        panel.contains("Automated by the clip") && panel.contains("Edit automation"),
        "the top bar's hint panel says it: {panel:?}"
    );

    // The clip deleted: the sign goes, the hint says nothing of a clip, the knob shows its
    // own value again.
    assert!(r.app.exec(Command::RemoveClip {
        track: ctrack,
        clip
    }));
    let signs = r.signs(6, Some(at));
    assert!(
        signs.iter().all(|(_, d)| !d.clip),
        "no clip's sign once the clip is gone: {signs:?}"
    );
    r.frame(Some(at));
    let h = cantus_ui::kit::hint::current(&r.ctx).expect("the knob's hint");
    assert_eq!(h.name, cantus_ui::i18n::t("chorda.hardness"));
    assert!(h.driven.is_empty(), "nothing drives it: {:?}", h.driven);
    assert_ne!(
        h.value, driven_value,
        "the knob shows its own value, not the clip's"
    );
    let _ = pos2(0.0, 0.0);
}
