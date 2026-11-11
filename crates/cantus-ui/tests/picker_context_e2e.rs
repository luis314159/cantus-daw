//! The device picker says where an effect goes (round 39, fixes; left by ux-b): its subtitle
//! names the strip as the mixer does — its own name, else the track feeding it, else
//! "Strip n" — never "on —" for a strip without a name of its own. Through
//! `CantusApp::assemble`, the picker opened as the mixer's slot opens it, the window drawn.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::picker::PickPurpose;
use cantus_ui::CantusApp;

fn painted(app: &mut CantusApp, ctx: &egui::Context) -> Vec<String> {
    let mut texts = Vec::new();
    for i in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(i as f64 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        texts.clear();
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
    }
    texts
}

#[test]
fn the_pickers_subtitle_names_the_strip_an_effect_goes_on() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    std::fs::write(
        data.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    // A track feeding a strip with no name of its own, and a strip nothing feeds.
    let mut p = cantus_core::Project::new("strips");
    let mut t = p.make_track("Bass");
    let fed = t.strip.expect("a track's own strip");
    t.name = "Bass".into();
    p.tracks.push(t);
    for s in p.strips.iter_mut() {
        s.name.clear();
    }
    let lone = p.make_strip("");
    let lone_id = lone.id;
    p.strips.push(lone);
    let n = p.strip_number(lone_id).expect("numbered");
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    app.open_picker(PickPurpose::AddEffect { strip: fed });
    let texts = painted(&mut app, &ctx);
    assert!(
        texts.iter().any(|t| t == "on Bass"),
        "the subtitle names the track feeding the strip: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "on —" || t == "on "),
        "{texts:?}"
    );
    app.picker = None;
    app.open_picker(PickPurpose::AddEffect { strip: lone_id });
    let texts = painted(&mut app, &ctx);
    let want = format!("on Strip {n}");
    assert!(texts.contains(&want), "{want}: {texts:?}");
    // In Spanish, the same.
    let (title, context) = {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
        let r = cantus_ui::views::picker::title(&app, &PickPurpose::AddEffect { strip: fed });
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        r
    };
    assert!(!title.is_empty());
    assert_eq!(context, "en Bass");
}
