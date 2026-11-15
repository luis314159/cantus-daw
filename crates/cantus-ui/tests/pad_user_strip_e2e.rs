//! Review mid-round 40, B-2 and B-3 (ADR 0117 §5): a strip the user chose for a pad — from
//! the pad's well, a free strip given an insert — is the user's, end to end through
//! `CantusApp::assemble` and the engine's compile: the desk's own strip menu (a right click
//! on its head, the menu as drawn — R2-B10) offers no *Split / Merge* on it (it belongs to
//! no kit), the strip menu's *Bounce* takes it (the pad in it, the song to the bit
//! afterwards), and the kit's *Merge back* leaves the pad on it, the strip with its insert —
//! Merge back only undoes *Split to strips* (R2-B3). Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use egui::{Event, Modifiers, Pos2, Rect};
use std::time::{Duration, Instant};

/// The whole window, frame by frame, with the words each frame painted and where.
struct Window {
    ctx: egui::Context,
    frame: u64,
}

type Words = Vec<(String, Rect)>;

impl Window {
    const SIZE: egui::Vec2 = egui::vec2(1600.0, 900.0);

    fn frame(&mut self, app: &mut CantusApp, at: Option<Pos2>, ev: Vec<Event>) -> Words {
        self.frame += 1;
        let mut events = Vec::new();
        if let Some(p) = at {
            events.push(Event::PointerMoved(p));
        }
        events.extend(ev);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Self::SIZE)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Words) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut words = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut words);
        }
        words
    }

    fn idle(&mut self, app: &mut CantusApp, n: usize) -> Words {
        let mut w = Vec::new();
        for _ in 0..n {
            w = self.frame(app, None, Vec::new());
        }
        w
    }

    fn button(&mut self, app: &mut CantusApp, at: Pos2, b: egui::PointerButton) -> Words {
        self.frame(app, Some(at), Vec::new());
        for pressed in [true, false] {
            let ev = Event::PointerButton {
                pos: at,
                button: b,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frame(app, Some(at), vec![ev]);
        }
        self.idle(app, 2)
    }

    /// The desk's own strip menu on `strip`, opened by a right click on its head (R2-B10).
    fn strip_menu(&mut self, app: &mut CantusApp, strip: StripId) -> Words {
        use cantus_ui::views::mixer::{control_rect, Control};
        let head = control_rect(&self.ctx, Some(strip), Control::Head)
            .unwrap_or_else(|| panic!("{strip:?} is not on the desk"));
        let open = self.button(app, head.center(), egui::PointerButton::Secondary);
        assert!(
            has(&open, "Where it goes…"),
            "the strip menu is open: {:?}",
            words(&open)
        );
        open
    }

    /// A click on the menu's row that reads `label`.
    fn choose(&mut self, app: &mut CantusApp, menu: &Words, label: &str) {
        let at = menu
            .iter()
            .find(|(t, _)| t == label)
            .unwrap_or_else(|| panic!("no {label:?} in {:?}", words(menu)))
            .1
            .center();
        self.button(app, at, egui::PointerButton::Primary);
    }

    fn escape(&mut self, app: &mut CantusApp) {
        for pressed in [true, false] {
            let ev = Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            };
            self.frame(app, None, vec![ev]);
        }
        self.idle(app, 2);
    }
}

fn has(w: &Words, word: &str) -> bool {
    w.iter().any(|(t, _)| t == word)
}

fn words(w: &Words) -> Vec<&str> {
    w.iter().map(|(t, _)| t.as_str()).collect()
}

fn song() -> Project {
    let mut p = Project::new("Pads");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    let pitches: Vec<u8> = t
        .instrument
        .pulsus()
        .unwrap()
        .pads
        .iter()
        .map(|x| x.pitch)
        .collect();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        for k in 0..1 + (step % 3) {
            let pitch = pitches[((step * 5 + k * 3) as usize) % pitches.len()];
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 3), pitch, 50 + (step * 13 % 70) as u8));
        }
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn boot(dir: &std::path::Path) -> (CantusApp, egui::Context) {
    std::fs::write(
        dir.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
    )
    .unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir);
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, Some(song()), engine, None, None, None, false);
    app.engine.frozen.set_app_dir(dir.join("renders"));
    app.path = Some(dir.join("song.cantus"));
    (app, ctx)
}

fn settle(app: &mut CantusApp, ctx: &egui::Context) {
    let t0 = Instant::now();
    loop {
        app.sync_engine();
        cantus_ui::freeze::tick(app, ctx);
        if !app.freeze.busy() || t0.elapsed() > Duration::from_secs(120) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cantus_ui::freeze::tick(app, ctx);
}

fn mix(app: &mut CantusApp) -> Vec<f32> {
    app.sync_engine();
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert!(
        a.len() >= b.len(),
        "{what}: lengths {} {}",
        a.len(),
        b.len()
    );
    if let Some(i) = a[..b.len()]
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {} vs {}", a[i], b[i]);
    }
    assert!(b.iter().any(|x| *x != 0.0), "{what}: silent");
}

#[test]
fn a_strip_the_user_chose_for_a_pad_bounces_merges_back_and_stays() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, ctx) = boot(dir.path());
    let t = app.project().tracks[0].id;
    let kick = app.project().tracks[0].instrument.pulsus().unwrap().pads[0].id;
    let free = app.project().first_free_strip().expect("a free strip");
    // The kick to the free strip, as its well does (one step), and an insert on it.
    let cmd = cantus_core::padout::set_pad_track(app.project(), t, kick, Some(free)).unwrap();
    assert!(app.exec(cmd));
    let fx = app.bus.new_effect(EffectKind::Saturator);
    assert!(app.exec(Command::InsertEffect {
        strip: free,
        index: 0,
        slot: Box::new(fx.clone()),
    }));
    // The kick's well lists each strip once (review mid-round 40, C-8): the kit's own as
    // "The kit's strip (n)" only, the free one as "First free strip (n)" only.
    let snare = app.project().tracks[0].instrument.pulsus().unwrap().pads[1].id;
    let list = cantus_ui::views::channel_rack::pad_track::entries(&app, t, snare);
    let mut to: Vec<Option<StripId>> = list.iter().map(|e| e.to).collect();
    let n = to.len();
    to.sort();
    to.dedup();
    assert_eq!(to.len(), n, "a strip listed twice: {list:?}");
    let kit_strip = app.project().tracks[0].strip;
    assert!(list.iter().all(|e| e.to != kit_strip), "{list:?}");
    assert!(
        list.iter()
            .any(|e| e.to == Some(free) && e.label.contains(" · ")),
        "the kick's own strip, by number and name: {list:?}"
    );
    let live = mix(&mut app);
    // The desk's own strip menu on it (R2-B10): no kit's Split / Merge — it is the user's.
    app.show_central(cantus_ui::app::CentralView::Mixer);
    let mut w = Window {
        ctx: ctx.clone(),
        frame: 0,
    };
    w.idle(&mut app, 4);
    let menu = w.strip_menu(&mut app, free);
    for word in ["Pad outputs", "Split to strips", "Merge back"] {
        assert!(
            !has(&menu, word),
            "{word} on the user's strip: {:?}",
            words(&menu)
        );
    }
    w.escape(&mut app);
    assert_eq!(app.kit_on_strip(free), None);
    // The strip menu's Bounce takes it, the kick in it: the song sounds as before.
    let plan = cantus_ui::bounce::strip_plan(&app, free);
    assert!(plan.is_ok(), "refused: {plan:?}");
    let tracks = app.project().tracks.len();
    app.bounce_strip(free);
    settle(&mut app, &ctx);
    assert!(!app.status.error, "{}", app.status.text);
    assert_eq!(app.project().tracks.len(), tracks + 1, "a new track");
    assert!(app.project().strip(free).unwrap().muted, "the strip muted");
    let after = mix(&mut app);
    same_bits(&after, &live, "the bounced strip");
    app.undo();
    // R2-B3: the kit's strip menu shows Merge back, but nothing Split to strips made is
    // there — chosen, it leaves the kick where the user put it; asked, it says why.
    let kit_strip = kit_strip.expect("the kit has a strip");
    let before = app.project().clone();
    assert!(!cantus_core::padout::can_merge(app.project(), t));
    let menu = w.strip_menu(&mut app, kit_strip);
    assert!(has(&menu, "Pad outputs") && has(&menu, "Merge back"));
    w.choose(&mut app, &menu, "Merge back");
    w.escape(&mut app);
    assert!(
        app.project() == &before,
        "a disabled Merge back changed the song"
    );
    assert!(!app.merge_kit(t), "nothing to merge back");
    assert!(
        app.status.error && app.status.text.contains("strips you chose"),
        "the refusal says why: {}",
        app.status.text
    );
    // Split to strips from the same menu, then Merge back from a strip it made: those
    // strips go, the kick stays on the user's strip, the strip with its insert.
    let menu = w.strip_menu(&mut app, kit_strip);
    w.choose(&mut app, &menu, "Split to strips");
    let made: Vec<StripId> = app.project().tracks[0]
        .instrument
        .output_strips()
        .into_iter()
        .filter(|s| *s != free)
        .collect();
    assert!(!made.is_empty(), "Split to strips made strips");
    assert!(cantus_core::padout::can_merge(app.project(), t));
    w.idle(&mut app, 3);
    let menu = w.strip_menu(&mut app, made[0]);
    w.choose(&mut app, &menu, "Merge back");
    let p = app.project();
    assert!(
        made.iter().all(|s| p.strip(*s).is_none()),
        "Split's strips gone"
    );
    assert_eq!(
        p.tracks[0].instrument.pulsus().unwrap().pads[0].output,
        Some(free),
        "the kick stays on the user's strip"
    );
    let kept = p.strip(free).expect("the user's strip kept");
    assert_eq!(
        kept.effects.iter().map(|e| e.id).collect::<Vec<_>>(),
        [fx.id]
    );
    let sound = mix(&mut app);
    same_bits(&sound, &live, "merged back, the user's routing kept");
}
