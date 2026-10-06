//! The device picker (round 20, ADR 0029), driven as a person drives it: real keys, real
//! clicks at the places the view published, through egui's input.

mod support;

use cantus_core::*;
use cantus_host::{Format, PluginInfo, PluginKind};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::picker_catalog::{self as catalog, Category, Role, Source};
use cantus_ui::picker_prefs::Prefs;
use cantus_ui::picker_query::{self as query, Query, Shelf};
use cantus_ui::views::{picker, picker_keys};
use cantus_ui::{starter_project, CantusApp};
use support::{app, ctx};

fn plugin(format: Format, id: &str, name: &str, vendor: &str, kind: PluginKind, features: &[&str]) -> PluginInfo {
    PluginInfo {
        format,
        id: id.into(),
        name: name.into(),
        vendor: vendor.into(),
        version: "1.2.0".into(),
        path: std::path::PathBuf::from(format!("C:/plugins/{name}.clap")),
        kind,
        main_inputs: if kind == PluginKind::Instrument { 0 } else { 2 },
        main_outputs: 2,
        sidechain: false,
        editor: true,
        features: features.iter().map(|s| s.to_string()).collect(),
    }
}

fn plugins() -> Vec<PluginInfo> {
    vec![
        plugin(Format::Clap, "org.surge-synth-team.surge-xt", "Surge XT", "Surge Synth Team", PluginKind::Instrument, &["instrument", "synthesizer", "stereo"]),
        plugin(Format::Vst3, "ABCDEF0123456789ABCDEF0123456789", "Pro-Q 4", "FabFilter", PluginKind::Effect, &["Fx|EQ"]),
        plugin(Format::Clap, "com.example.verb", "Big Verb", "Example Audio", PluginKind::Effect, &["audio-effect", "reverb"]),
        plugin(Format::Clap, "com.example.arp", "Arp", "Example Audio", PluginKind::NoteEffect, &["note-effect"]),
        plugin(Format::Clap, "com.example.drums", "Kit Machine", "Example Audio", PluginKind::Instrument, &["instrument", "drum-machine"]),
        // The same plug-in found in a second folder.
        plugin(Format::Clap, "com.example.verb", "Big Verb", "Example Audio", PluginKind::Effect, &["audio-effect", "reverb"]),
    ]
}

fn db_with(list: &[PluginInfo]) -> cantus_host::PluginDb {
    let mut db = cantus_host::PluginDb::default();
    for (i, p) in list.iter().enumerate() {
        db.put(cantus_host::DbEntry {
            path: std::path::PathBuf::from(format!("C:/plugins/{i}.clap")),
            format: p.format,
            modified: 1,
            size: 1,
            status: cantus_host::ScanStatus::Ok,
            plugins: vec![p.clone()],
        });
    }
    db
}

fn app_with_plugins() -> CantusApp {
    let mut a = app(starter_project());
    a.plugins.db = db_with(&plugins());
    a
}

#[test]
fn the_catalogue_lists_every_native_kind_and_every_scanned_plugin() {
    support::ctx();
    let list = plugins();
    let devices = catalog::build(&list);
    let has = |c: &PickChoice| devices.iter().any(|d| &d.choice == c);
    assert!(has(&PickChoice::Firmus));
    assert!(has(&PickChoice::Pulsus));
    for k in InstrumentKind::ready_kinds() {
        assert!(has(&PickChoice::Instrument(k)), "{k:?} missing");
    }
    for k in EffectKind::ALL {
        assert!(has(&PickChoice::Effect(k)), "{k:?} missing");
    }
    let hosted: Vec<_> = devices.iter().filter(|d| d.source != Source::Native).collect();
    assert_eq!(hosted.len(), 4, "every offered plug-in once, no note effect: {hosted:#?}");
    let surge = devices.iter().find(|d| d.name == "Surge XT").unwrap();
    assert_eq!((surge.role, surge.primary(), surge.source), (Role::Instrument, Category::Synths, Source::Clap));
    let proq = devices.iter().find(|d| d.name == "Pro-Q 4").unwrap();
    assert_eq!((proq.role, proq.primary(), proq.source), (Role::Effect, Category::Eq, Source::Vst3));
    let kit = devices.iter().find(|d| d.name == "Kit Machine").unwrap();
    assert_eq!(kit.primary(), Category::Drums);
    // Keys are unique and stable.
    let mut keys: Vec<_> = devices.iter().map(|d| d.key.clone()).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), devices.len());
    assert_eq!(catalog::key_of(&PickChoice::Instrument(InstrumentKind::Wavetable)).as_deref(), Some("cantus:unda"));
}

#[test]
fn search_finds_by_name_vendor_category_and_preset_and_folds_accents() {
    support::ctx();
    let list = plugins();
    let devices = catalog::build(&list);
    let prefs = Prefs::new();
    let names = |role: Role, text: &str| -> Vec<String> {
        let q = Query { role: Some(role), text: text.into(), ..Default::default() };
        query::run(&devices, &q, &prefs).into_iter().map(|i| devices[i].name.clone()).collect()
    };
    assert_eq!(names(Role::Instrument, "unda").first().map(String::as_str), Some("Unda"));
    assert_eq!(names(Role::Effect, "fabfilter"), vec!["Pro-Q 4"]);
    let verbs = names(Role::Effect, "reverb");
    for n in ["Spatium", "Reverb", "Big Verb"] {
        assert!(verbs.iter().any(|x| x == n), "{n} not in {verbs:?}");
    }
    // Spanish words and accents: "batería" and "bateria" find the drums.
    assert!(names(Role::Instrument, "batería").iter().any(|n| n == "Pulsus II"));
    assert!(names(Role::Instrument, "bateria").iter().any(|n| n == "Kit Machine"));
    // A word that is in nothing finds nothing; two words must both match.
    assert!(names(Role::Effect, "zzzz").is_empty());
    assert_eq!(names(Role::Effect, "pro fabfilter"), vec!["Pro-Q 4"]);
    // A factory preset's name finds its device.
    let lamina = devices.iter().find(|d| d.name == "Lamina").unwrap();
    let preset = catalog::presets(&lamina.choice).into_iter().next().expect("Lamina has presets");
    assert!(names(Role::Instrument, &preset).iter().any(|n| n == "Lamina"), "{preset}");
}

#[test]
fn shelves_group_and_count() {
    support::ctx();
    let list = plugins();
    let devices = catalog::build(&list);
    let mut prefs = Prefs::new();
    let q = Query { role: Some(Role::Instrument), ..Default::default() };
    let groups = query::groups(&devices, &q, &prefs);
    let titles: Vec<_> = groups.iter().map(|g| g.title).collect();
    assert_eq!(titles.first(), Some(&Some(Category::Synths)));
    assert!(titles.contains(&Some(Category::Drums)));
    // Ours come first in a group, in the designed order.
    assert_eq!(devices[groups[0].items[0]].name, "Unda");
    let shelves = query::shelves(&devices, Some(Role::Effect));
    assert!(shelves.contains(&Shelf::Category(Category::Mastering)));
    assert!(!shelves.contains(&Shelf::Category(Category::Synths)));
    assert_eq!(query::count(&devices, Some(Role::Effect), Shelf::Source(Source::Vst3), &prefs), 1);
    prefs.toggle_favourite("cantus:comprimo");
    prefs.used("cantus:spatium");
    prefs.used("cantus:comprimo");
    assert_eq!(query::count(&devices, Some(Role::Effect), Shelf::Favourites, &prefs), 1);
    let recent = Query { role: Some(Role::Effect), shelf: Shelf::Recent, ..Default::default() };
    let r: Vec<_> = query::run(&devices, &recent, &prefs).into_iter().map(|i| devices[i].key.clone()).collect();
    assert_eq!(r, vec!["cantus:comprimo", "cantus:spatium"]);
}

#[test]
fn the_grid_lays_rows_under_headings_and_the_arrows_follow_the_screen() {
    use cantus_ui::picker_query::Group;
    use cantus_ui::views::picker_grid::{lay_out, Item};
    let groups = vec![
        Group { title: Some(Category::Synths), items: vec![0, 1, 2, 3, 4] },
        Group { title: Some(Category::Keys), items: vec![5, 6] },
    ];
    let (items, h) = lay_out(&groups, 800.0, 212.0, 1.0);
    let tiles: Vec<egui::Rect> = items.iter().filter_map(|i| match i { Item::Tile(_, r) => Some(*r), _ => None }).collect();
    assert_eq!(tiles.len(), 7);
    // Three across in 800 points; the fourth starts a row.
    assert!((tiles[0].top() - tiles[2].top()).abs() < 0.1 && tiles[3].top() > tiles[2].bottom());
    assert!(tiles[5].top() > tiles[4].bottom(), "the next group starts below a heading");
    assert!(h > tiles[6].bottom());
    use picker_keys::{step, Dir};
    assert_eq!(step(&tiles, 0, Dir::Right, 400.0), 1);
    assert_eq!(step(&tiles, 1, Dir::Down, 400.0), 4, "down keeps the column");
    assert_eq!(step(&tiles, 2, Dir::Down, 400.0), 4, "or the nearest one");
    assert_eq!(step(&tiles, 4, Dir::Down, 400.0), 6, "into the next group");
    assert_eq!(step(&tiles, 6, Dir::Up, 400.0), 4);
    assert_eq!(step(&tiles, 6, Dir::Down, 400.0), 6, "the last row stays");
    assert_eq!(step(&tiles, 3, Dir::Home, 400.0), 0);
    assert_eq!(step(&tiles, 0, Dir::End, 400.0), 6);
}

/// One frame of the picker over an empty window, with `events`.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 960.0))),
        events,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        let c = ui.ctx().clone();
        picker::frame(a, &c);
    })
    .textures_delta
    .clear();
}

fn key(k: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let b = egui::PointerButton::Primary;
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton { pos: at, button: b, pressed: true, modifiers: Default::default() },
        egui::Event::PointerButton { pos: at, button: b, pressed: false, modifiers: Default::default() },
    ]
}

fn first_strip(a: &CantusApp) -> StripId {
    a.project().tracks[0].strip.expect("a strip")
}

#[test]
fn opening_for_a_purpose_offers_instruments_or_effects() {
    let mut a = app_with_plugins();
    let strip = first_strip(&a);
    a.open_picker(PickPurpose::AddEffect { strip });
    assert!(a.picker.is_some());
    let v = &a.picker_view;
    assert!(v.visible().iter().all(|&i| v.catalog[i].role == Role::Effect));
    assert!(v.visible().iter().any(|&i| v.catalog[i].name == "Pro-Q 4"));
    a.open_picker(PickPurpose::AddInstrument { index: None });
    let v = &a.picker_view;
    assert!(v.visible().iter().all(|&i| v.catalog[i].role == Role::Instrument));
    assert_eq!(v.visible().len(), 9 + 2, "nine of ours and two hosted instruments");
}

#[test]
fn replacing_selects_the_device_that_is_there() {
    let mut a = app_with_plugins();
    let strip = first_strip(&a);
    a.add_effect(strip, EffectKind::Space);
    let effect = a.project().effects_of(strip).unwrap().last().unwrap().id;
    a.open_picker(PickPurpose::ReplaceEffect { strip, effect });
    assert_eq!(a.picker_view.current.as_deref(), Some("cantus:spatium"));
    assert_eq!(a.picker_view.selected.as_deref(), Some("cantus:spatium"));
    let track = a.project().tracks[0].id;
    a.open_picker(PickPurpose::ReplaceInstrument { track });
    let cur = a.picker_view.current.clone().expect("the track's instrument");
    assert_eq!(a.picker_view.selected.as_ref(), Some(&cur));
}

#[test]
fn typing_searches_arrows_move_and_enter_applies_through_apply_pick() {
    let mut a = app_with_plugins();
    let ctx = ctx();
    let strip = first_strip(&a);
    let before = a.project().effects_of(strip).map_or(0, |e| e.len());
    a.open_picker(PickPurpose::AddEffect { strip });
    frame(&mut a, &ctx, vec![]);
    // Type to search: no field needs focus.
    frame(&mut a, &ctx, vec![egui::Event::Text("comp".into())]);
    assert_eq!(a.picker_view.query.text, "comp");
    let first = a.picker_view.visible()[0];
    assert_eq!(a.picker_view.catalog[first].name, "Comprimo");
    assert_eq!(a.picker_view.selected.as_deref(), Some("cantus:comprimo"));
    // Esc clears the search first; the selection stays.
    frame(&mut a, &ctx, vec![key(egui::Key::Escape, Default::default())]);
    assert!(a.picker_view.query.text.is_empty() && a.picker.is_some());
    // The right arrow moves to the next tile on screen; Enter chooses it.
    frame(&mut a, &ctx, vec![]);
    let tiles: Vec<String> = a.picker_view.geo.tiles.iter().map(|t| t.0.clone()).collect();
    let at = tiles.iter().position(|k| k == "cantus:comprimo").unwrap();
    frame(&mut a, &ctx, vec![key(egui::Key::ArrowRight, Default::default())]);
    let next = tiles[at + 1].clone();
    assert_eq!(a.picker_view.selected.as_ref(), Some(&next));
    frame(&mut a, &ctx, vec![key(egui::Key::Enter, Default::default())]);
    assert!(a.picker.is_none(), "choosing closes the picker");
    let chosen = a.picker_view.device(&next).unwrap().choice.clone();
    assert_eq!(a.picker_view.last_choice, Some((PickPurpose::AddEffect { strip }, chosen.clone())));
    let after = a.project().effects_of(strip).unwrap();
    assert_eq!(after.len(), before + 1);
    if let PickChoice::Effect(k) = chosen {
        assert_eq!(after.last().unwrap().spec.kind, k);
    }
    assert_eq!(a.picker_view.prefs.recent.first(), Some(&next), "it is the most recent now");
}

#[test]
fn clicking_a_tile_selects_it_the_button_applies_and_esc_closes() {
    let mut a = app_with_plugins();
    let ctx = ctx();
    a.open_picker(PickPurpose::AddInstrument { index: None });
    let tracks = a.project().tracks.len();
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let (k, r) = a.picker_view.geo.shown.iter().find(|t| t.0 == "cantus:organum").cloned().expect("Organum on screen");
    frame(&mut a, &ctx, click(r.center()));
    assert_eq!(a.picker_view.selected.as_deref(), Some(k.as_str()));
    // A click on the tile's star stars it, and keeps the selection.
    let star = cantus_ui::views::picker_grid::star_rect(r, &cantus_ui::kit::Style::current());
    frame(&mut a, &ctx, click(star.center()));
    assert!(a.picker_view.prefs.is_favourite(&k));
    let choose = a.picker_view.geo.choose;
    frame(&mut a, &ctx, click(choose.center()));
    assert!(a.picker.is_none());
    assert_eq!(a.project().tracks.len(), tracks + 1);
    let last = a.project().tracks.last().unwrap();
    assert!(matches!(&last.instrument, InstrumentSpec::Native(n) if n.kind == InstrumentKind::Organ));
    // Esc with nothing typed closes, and nothing changes.
    a.open_picker(PickPurpose::AddInstrument { index: None });
    let rev = a.bus.revision();
    frame(&mut a, &ctx, vec![key(egui::Key::Escape, Default::default())]);
    assert!(a.picker.is_none());
    assert_eq!(a.bus.revision(), rev);
}

#[test]
fn favourites_and_recents_are_kept_for_the_user_not_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("devices.json");
    let mut a = app_with_plugins();
    a.picker_view.load_prefs(path.clone());
    let ctx = ctx();
    let strip = first_strip(&a);
    a.open_picker(PickPurpose::AddEffect { strip });
    frame(&mut a, &ctx, vec![egui::Event::Text("culmen".into())]);
    frame(&mut a, &ctx, vec![key(egui::Key::D, egui::Modifiers::COMMAND)]);
    assert!(a.picker_view.prefs.is_favourite("cantus:culmen"));
    frame(&mut a, &ctx, vec![key(egui::Key::Enter, Default::default())]);
    let back = Prefs::load(&path);
    assert!(back.is_favourite("cantus:culmen"));
    assert_eq!(back.recent.first().map(String::as_str), Some("cantus:culmen"));
    // Favourites are not in the project.
    let json = serde_json::to_string(a.project()).unwrap();
    assert!(!json.contains("cantus:culmen"));
    // The favourites shelf holds it.
    a.open_picker(PickPurpose::AddEffect { strip });
    a.picker_view.set_shelf(Shelf::Favourites);
    let v = &a.picker_view;
    let vis: Vec<_> = v.visible().into_iter().map(|i| v.catalog[i].key.clone()).collect();
    assert_eq!(vis, vec!["cantus:culmen"]);
}

#[test]
fn the_shelves_filter_when_clicked() {
    let mut a = app_with_plugins();
    let ctx = ctx();
    let strip = first_strip(&a);
    a.open_picker(PickPurpose::AddEffect { strip });
    // The overlay's first frame sizes it (egui's area sizing pass): nothing is clickable yet.
    frame(&mut a, &ctx, vec![]);
    frame(&mut a, &ctx, vec![]);
    let (_, r) = *a.picker_view.geo.shelves.iter().find(|s| s.0 == Shelf::Category(Category::Space)).unwrap();
    frame(&mut a, &ctx, click(r.center()));
    assert_eq!(a.picker_view.query.shelf, Shelf::Category(Category::Space));
    let v = &a.picker_view;
    let names: Vec<_> = v.visible().into_iter().map(|i| v.catalog[i].name.clone()).collect();
    assert!(names.contains(&"Spatium".to_string()) && names.contains(&"Big Verb".to_string()));
    assert!(!names.contains(&"Comprimo".to_string()));
}

#[test]
fn sixty_plugins_draw_only_what_is_on_screen_in_a_few_meshes() {
    let mut list = Vec::new();
    let kinds = ["eq", "compressor", "reverb", "delay", "distortion"];
    for i in 0..64 {
        list.push(plugin(
            Format::Clap,
            &format!("com.vendor{}.fx{i}", i % 7),
            &format!("Effect {i}"),
            &format!("Vendor {}", i % 7),
            PluginKind::Effect,
            &["audio-effect", kinds[i % 5]],
        ));
    }
    let mut a = app(starter_project());
    a.plugins.db = db_with(&list);
    let ctx = ctx();
    let strip = first_strip(&a);
    a.open_picker(PickPurpose::AddEffect { strip });
    let input = |t: f64| egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0))),
        time: Some(t),
        ..Default::default()
    };
    let mut meshes = 0;
    let mut texts = 0;
    let mut ms = Vec::new();
    for f in 0..8 {
        let t0 = std::time::Instant::now();
        let mut out = ctx.run_ui(input(f as f64 / 60.0), |ui| {
            let c = ui.ctx().clone();
            picker::frame(&mut a, &c);
        });
        out.textures_delta.clear();
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        meshes = out.shapes.iter().filter(|s| matches!(s.shape, egui::Shape::Mesh(_))).count();
        texts = out.shapes.iter().filter(|s| matches!(s.shape, egui::Shape::Text(_))).count();
    }
    let v = &a.picker_view;
    assert!(v.visible().len() >= 75, "every effect: {}", v.visible().len());
    assert!(v.geo.shown.len() < v.geo.tiles.len(), "only what is on screen: {} of {}", v.geo.shown.len(), v.geo.tiles.len());
    assert!(meshes <= 24, "a handful of meshes whatever the count: {meshes}");
    assert!(texts <= 60 + 5 * v.geo.shown.len(), "text: {texts}");
    eprintln!("picker frame: {:.2} ms (frames {ms:?}), {} tiles shown, {meshes} meshes", ms[ms.len() - 1], v.geo.shown.len());
}

#[test]
fn thumbnails_are_made_once_then_read_back_from_disk() {
    use cantus_ui::views::picker_snap::Subject;
    use cantus_ui::views::picker_thumbs::Thumbs;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx();
    let subject = Subject::Instrument(InstrumentKind::Organ);
    let wait = |th: &mut Thumbs| {
        for _ in 0..600 {
            th.pump(&ctx);
            if th.get(subject).is_some() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    };
    let mut th = Thumbs::default();
    assert!(th.get(subject).is_none(), "off until the app turns it on");
    th.enable(Some(dir.path().to_path_buf()));
    assert!(th.get(subject).is_none(), "asked for, not yet made");
    assert!(wait(&mut th), "the picture arrives");
    assert_eq!((th.stats.captures, th.stats.uploads, th.stats.disk_loads), (1, 1, 0));
    // Asked again: from memory, nothing made.
    for _ in 0..3 {
        th.pump(&ctx);
        assert!(th.get(subject).is_some());
    }
    assert_eq!(th.stats.captures, 1);
    let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
    assert_eq!(files.len(), 1, "one PNG on disk");
    // A new start reads the file and draws no editor.
    let mut again = Thumbs::default();
    again.enable(Some(dir.path().to_path_buf()));
    assert!(wait(&mut again));
    assert_eq!((again.stats.captures, again.stats.disk_loads), (0, 1));
}

#[test]
fn the_devices_panel_drags_a_payload_and_double_click_adds() {
    use cantus_ui::picker::DevicePayload;
    let mut a = app_with_plugins();
    let ctx = ctx();
    let run = |a: &mut CantusApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 1400.0))),
            events,
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| cantus_ui::views::picker_panel::show(a, ui));
        })
        .textures_delta
        .clear();
    };
    run(&mut a, vec![]);
    run(&mut a, vec![]);
    let (_, r) = a.picker_view.panel.rows.iter().find(|r| r.0 == "cantus:organum").cloned().expect("Organum listed");
    let b = egui::PointerButton::Primary;
    let p = r.center();
    run(&mut a, vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: b, pressed: true, modifiers: Default::default() }]);
    run(&mut a, vec![egui::Event::PointerMoved(p + egui::vec2(30.0, 0.0))]);
    run(&mut a, vec![egui::Event::PointerMoved(p + egui::vec2(60.0, 10.0))]);
    let payload = egui::DragAndDrop::payload::<DevicePayload>(&ctx).expect("a payload while dragging");
    assert_eq!(payload.0, PickChoice::Instrument(InstrumentKind::Organ));
    assert!(payload.is_instrument());
    run(&mut a, vec![egui::Event::PointerButton { pos: p + egui::vec2(60.0, 10.0), button: b, pressed: false, modifiers: Default::default() }]);
    // A double-click adds it on a new track (one undo step).
    let tracks = a.project().tracks.len();
    cantus_ui::views::picker_panel::add(&mut a, "cantus:organum");
    assert_eq!(a.project().tracks.len(), tracks + 1);
    assert_eq!(a.picker_view.prefs.recent.first().map(String::as_str), Some("cantus:organum"));
}
