//! A hosted plug-in in its own editor or in Cantus's view of its parameters (ADR 0073,
//! BACKLOG §6h point 2), in process: it opens its own editor when inserted; the switch
//! changes views without touching the project, its state or its automation; the default
//! per plug-in is the settings', kept across a save and a load, a rescan and a new version;
//! a plug-in with no editor of its own has Cantus's view and no switch; the search filters;
//! meters show what the plug-in reports.

mod hosted_support;
mod support;

use cantus_core::*;
use cantus_ui::hosted_view::{HostedView, ViewDefaults};
use cantus_ui::views::plugin_notice::Target;
use hosted_support::*;
use support::*;

/// The plug-in's own window opened (on Windows: a real window); else — no display, as on
/// a Linux runner — the open fell back to Cantus's view, which is what the user then sees.
fn native_or_fallback(a: &cantus_ui::CantusApp, target: Target) {
    if a.native_open(target) {
        assert!(!a.cantus_view_open(target), "one view at a time");
        assert_eq!(a.hosted_view(target), HostedView::Native);
    } else {
        if cfg!(windows) {
            panic!("Windows opens the plug-in's own window");
        }
        assert!(a.cantus_view_open(target), "its parameters instead");
    }
}

#[test]
fn an_inserted_plugin_opens_its_own_editor_and_its_default_once_remembered() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    a.settings_path = dir.path().join("settings.json");
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    assert!(info.editor, "the test gain has an editor");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    assert!(a.hosted_view_switchable(target));
    native_or_fallback(&a, target);
    // *Use as default for this plug-in* on Cantus's view: the next insert opens there.
    a.set_hosted_view(target, HostedView::Cantus);
    assert!(a.cantus_view_open(target) && !a.native_open(target));
    a.set_hosted_view_default(target, Some(HostedView::Cantus));
    let saved = cantus_ui::settings::Settings::load(&a.settings_path);
    assert_eq!(
        saved.hosted_views.get(PluginFormat::Clap, &info.id),
        Some(HostedView::Cantus),
        "saved at once"
    );
    a.close_cantus_view(target);
    let fx2 = insert(&mut a, strip, &info);
    assert!(a.cantus_view_open(Target::Insert(fx2)), "by its default");
    assert!(!a.native_open(Target::Insert(fx2)));
    // The inverse: forgotten, the plug-in's own editor again.
    a.set_hosted_view_default(Target::Insert(fx2), None);
    a.close_cantus_view(Target::Insert(fx2));
    let fx3 = insert(&mut a, strip, &info);
    native_or_fallback(&a, Target::Insert(fx3));
    for e in [fx, fx2, fx3] {
        a.plugins.close_window(e);
    }
}

fn starter_project_for_tests() -> Project {
    cantus_ui::starter_project()
}

#[test]
fn switching_views_keeps_the_project_the_plugins_state_and_its_automation() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    // A value moved and its automation drawn, before any switch.
    a.set_plugin_param(strip, fx, 0, -6.0, false);
    a.automate(AutoTarget::Plugin { effect: fx, id: 0 });
    a.sync_engine();
    let project = a.project().clone();
    let (revision, steps) = (a.bus.revision(), a.bus.history().len());
    let state = a.plugins.capture(fx).expect("loaded");
    for view in [
        HostedView::Cantus,
        HostedView::Native,
        HostedView::Cantus,
        HostedView::Native,
    ] {
        a.set_hosted_view(target, view);
        let _ = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        assert_eq!(a.bus.revision(), revision, "no command: {view:?}");
        assert_eq!(a.bus.history().len(), steps);
        assert_eq!(*a.project(), project, "the project as it was");
        assert_eq!(a.plugins.capture(fx).as_ref(), Some(&state), "its state");
    }
    let lanes = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::Plugin { effect: fx, id: 0 })
        })
        .count();
    assert_eq!(lanes, 1, "its automation clip is there");
    a.plugins.close_window(fx);
}

#[test]
fn the_default_is_kept_by_the_plugins_id_through_a_save_a_rescan_and_a_new_version() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    a.settings_path = dir.path().join("settings.json");
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    a.set_hosted_view(target, HostedView::Cantus);
    a.set_hosted_view_default(target, Some(HostedView::Cantus));
    // A restart: the settings read back from their file.
    let back = cantus_ui::settings::Settings::load(&a.settings_path);
    assert_eq!(back, a.settings, "the whole settings round-trip");
    assert_eq!(
        back.hosted_views.get(PluginFormat::Clap, &info.id),
        Some(HostedView::Cantus)
    );
    // An update and a rescan: the database forgotten and read again, finding version 2 of
    // it in another file — the same plug-in, the same default; inserted again, it opens in
    // Cantus's view.
    a.settings = back;
    a.plugins.db = Default::default();
    a.plugins.db_path = None;
    let v1_file = dir.path().join("Cantus Test Gain.clap");
    let v2_file = dir.path().join("Cantus Test Gain 2.clap");
    std::fs::copy(&v1_file, &v2_file).unwrap();
    let mut v2 = info.clone();
    v2.version = "2.0.0".into();
    v2.path = v2_file.clone();
    a.plugins.db.put(cantus_host::DbEntry {
        path: v2_file,
        format: cantus_host::Format::Clap,
        modified: 1,
        size: 1,
        status: cantus_host::ScanStatus::Ok,
        plugins: vec![v2.clone()],
    });
    assert_eq!(a.plugins.db.plugins().len(), 1, "only version 2 is known");
    assert_eq!(a.hosted_view_default(target), Some(HostedView::Cantus));
    a.close_cantus_view(target);
    let fx2 = insert(&mut a, strip, &v2);
    assert!(
        a.cantus_view_open(Target::Insert(fx2)),
        "version 2 opens by the default"
    );
    assert!(!a.native_open(Target::Insert(fx2)));
    // An older settings file, before the field: every plug-in opens its own editor.
    std::fs::write(&a.settings_path, r#"{"autosave_seconds": 30}"#).unwrap();
    let old = cantus_ui::settings::Settings::load(&a.settings_path);
    assert_eq!(old.hosted_views, ViewDefaults::default());
    // A file whose views are not readable keeps every other choice (the lenient reader).
    std::fs::write(
        &a.settings_path,
        r#"{"autosave_seconds": 30, "hosted_views": {"by_plugin": 7}}"#,
    )
    .unwrap();
    let broken = cantus_ui::settings::Settings::load(&a.settings_path);
    assert_eq!(broken.autosave_seconds, 30);
    assert_eq!(broken.hosted_views, ViewDefaults::default());
    a.plugins.close_window(fx);
}

#[test]
fn a_plugin_with_no_editor_has_cantus_view_and_no_switch() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(
        &mut a,
        &gain_library(),
        dir.path(),
        "Cantus Test Gain nogui.clap",
    );
    assert!(!info.editor, "a copy named nogui has no editor");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    assert!(a.cantus_view_open(target), "its only view");
    assert!(!a.native_open(target));
    assert!(!a.hosted_view_switchable(target));
    assert_eq!(a.hosted_view(target), HostedView::Cantus);
    assert!(cantus_ui::hosted_view::menu_rows(&a, target).is_empty());
    // Its own window is never asked for, even when chosen.
    a.set_hosted_view(target, HostedView::Native);
    assert!(!a.native_open(target) && a.cantus_view_open(target));
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(
        text.contains(&cantus_ui::i18n::t("hostedview.only_cantus")),
        "{text}"
    );
    assert!(
        !text.contains(&cantus_ui::i18n::t("hostedview.native")),
        "no switch: {text}"
    );
    assert!(text.contains("GAIN"), "its parameters: {text}");
}

#[test]
fn the_switch_and_the_default_are_in_the_frame_and_the_menus() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    a.settings_path = dir.path().join("settings.json");
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    a.set_hosted_view(target, HostedView::Cantus);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    // The lamp's word is set in capitals, as every lamp's.
    let lower = text.to_lowercase();
    for key in [
        "hostedview.native",
        "hostedview.cantus",
        "hostedview.default",
    ] {
        let word = cantus_ui::i18n::t(key).to_lowercase();
        assert!(lower.contains(&word), "{key}: {text}");
    }
    // The menus' rows: the two views (Cantus's lit) and the default (off).
    let rows = cantus_ui::hosted_view::menu_rows(&a, target);
    let lit: Vec<(&str, bool)> = rows.iter().map(|r| (r.label_key, r.on)).collect();
    assert_eq!(
        lit,
        [
            ("hostedview.native", false),
            ("hostedview.cantus", true),
            ("hostedview.use_default", false)
        ]
    );
    cantus_ui::hosted_view::run(&mut a, target, cantus_ui::hosted_view::ViewAct::Default);
    assert_eq!(a.hosted_view_default(target), Some(HostedView::Cantus));
    cantus_ui::hosted_view::run(&mut a, target, cantus_ui::hosted_view::ViewAct::Default);
    assert_eq!(a.hosted_view_default(target), None, "its inverse");
    cantus_ui::hosted_view::run(
        &mut a,
        target,
        cantus_ui::hosted_view::ViewAct::Show(HostedView::Native),
    );
    native_or_fallback(&a, target);
    a.plugins.close_window(fx);
}

fn param(id: u32, name: &str, group: &str) -> cantus_host::ParamInfo {
    cantus_host::ParamInfo {
        id,
        name: name.into(),
        group: group.into(),
        min: 0.0,
        max: 1.0,
        default: 0.0,
        stepped: false,
        automatable: true,
        hidden: false,
        read_only: false,
        steps: 0,
    }
}

#[test]
fn the_search_filters_by_name_and_by_group() {
    use cantus_ui::views::plugin_view::group;
    let params = vec![
        param(0, "Cutoff", "Filter"),
        param(1, "Resonance", "Filter"),
        param(2, "Attack", "Amp/Envelope"),
        param(3, "Release", "Amp/Envelope"),
        param(4, "Drive", ""),
    ];
    let names = |g: &[(String, Vec<usize>)]| -> Vec<(String, Vec<usize>)> { g.to_vec() };
    // Nothing typed: every group, in the plug-in's order; a page path reads as a path.
    let all = group(&params, "");
    assert_eq!(all.len(), 3);
    assert_eq!(all[1].0, "Amp \u{203a} Envelope");
    assert_eq!(all[2].0, cantus_ui::i18n::t("host.editor.ungrouped"));
    // By a group's name: its parameters; by a name: that one; every word must appear.
    assert_eq!(
        names(&group(&params, "filter")),
        vec![("Filter".into(), vec![0, 1])]
    );
    assert_eq!(
        names(&group(&params, "  ATTACK ")),
        vec![("Amp \u{203a} Envelope".into(), vec![2])]
    );
    assert_eq!(group(&params, "envelope rel")[0].1, vec![3]);
    assert!(group(&params, "filter attack").is_empty());
    assert!(group(&params, "zzz").is_empty());
    // Through the editor: what is typed narrows the cards and the count says so.
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    a.set_hosted_view(Target::Insert(fx), HostedView::Cantus);
    a.fx.hosted.filter = "lat".into();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("| LATENCY |"), "{text}");
    assert!(!text.contains("| GAIN |"), "filtered out: {text}");
    assert!(text.contains("1 of 3 parameters"), "{text}");
    a.fx.hosted.filter = "nothing like it".into();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("No parameter matches"), "{text}");
}

#[test]
fn a_hosted_instruments_cantus_view_has_its_meters_its_switch_and_keeps_its_state() {
    use cantus_ui::picker::{PickChoice, PickPurpose};
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(
        &mut a,
        &synth_library(),
        dir.path(),
        "Cantus Test Synth.clap",
    );
    let track = a.project().tracks[0].id;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info.clone())),
    );
    let target = Target::Instrument(track);
    assert!(a.is_hosted_instrument(track));
    // Picked: its own editor.
    native_or_fallback(&a, target);
    let project = a.project().clone();
    let revision = a.bus.revision();
    let state = a.plugins.capture_instrument(track).expect("loaded");
    a.set_hosted_view(target, HostedView::Cantus);
    assert!(a.cantus_view_open(target) && !a.native_open(target));
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(1400.0, 900.0), 3, |ui| {
        let c = ui.ctx().clone();
        cantus_ui::views::plugin_view_inst::window(&mut a, &c);
    });
    assert!(text.to_lowercase().contains("cantus test synth"), "{text}");
    assert!(text.contains("| GAIN |"), "{text}");
    assert!(
        text.contains("| TEMPO SEEN |"),
        "a meter, its name whole: {text}"
    );
    assert!(text.contains("| PLAYING SEEN |"), "{text}");
    let (drawn, meters) = cantus_ui::views::plugin_view::drawn(&a);
    assert_eq!(
        meters, 2,
        "Tempo seen and Playing seen are read-only: meters"
    );
    assert!(drawn >= 4, "{drawn}");
    // Back to its own window: nothing of the project changed.
    a.set_hosted_view(target, HostedView::Native);
    native_or_fallback(&a, target);
    assert_eq!(a.bus.revision(), revision);
    assert_eq!(*a.project(), project);
    assert_eq!(a.plugins.capture_instrument(track), Some(state));
    // The inspector's rows: the editor and *Remember*.
    a.set_hosted_view(target, HostedView::Cantus);
    a.selected_track = Some(track);
    let text = paint(&ctx, egui::vec2(420.0, 6000.0), 3, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    for key in [
        "hostedview.row",
        "hostedview.default_row",
        "hostedview.cantus",
    ] {
        let word = cantus_ui::i18n::t(key).to_lowercase();
        assert!(text.to_lowercase().contains(&word), "{key}: {text}");
    }
    a.plugins.close_instrument_window(track);
}

#[test]
fn a_hosted_instrument_with_no_editor_opens_cantus_view_only() {
    use cantus_ui::picker::{PickChoice, PickPurpose};
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(
        &mut a,
        &synth_library(),
        dir.path(),
        "Cantus Test Synth nogui.clap",
    );
    assert!(!info.editor);
    let track = a.project().tracks[0].id;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info)),
    );
    let target = Target::Instrument(track);
    assert!(a.cantus_view_open(target));
    assert!(!a.hosted_view_switchable(target));
    assert!(cantus_ui::hosted_view::menu_rows(&a, target).is_empty());
    a.selected_track = Some(track);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(420.0, 6000.0), 3, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    assert!(
        !text
            .to_lowercase()
            .contains(&cantus_ui::i18n::t("hostedview.default_row").to_lowercase()),
        "no default to remember: {text}"
    );
}

/// The views chosen this session belong to the project they were chosen in: another project
/// (whose ids may be the same numbers) opens each plug-in by its default.
#[test]
fn another_project_forgets_the_views_chosen_in_this_one() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    a.set_hosted_view(Target::Insert(fx), HostedView::Cantus);
    assert_eq!(
        a.fx.hosted.views.chosen(Target::Insert(fx)),
        Some(HostedView::Cantus)
    );
    a.plugins.close_window(fx);
    a.replace_project(starter_project_for_tests());
    assert_eq!(a.fx.hosted.views.chosen(Target::Insert(fx)), None);
    assert!(a.fx.hosted.views.pending().is_empty());
}

/// Settings' plug-in manager lists the plug-ins whose view was remembered, by name, with
/// the view and *Forget*; with none, a line says how to make one.
#[test]
fn the_plugin_manager_lists_the_remembered_views() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    a.settings_path = dir.path().join("settings.json");
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let ctx = ctx();
    let size = egui::vec2(900.0, 2400.0);
    let text = paint(&ctx, size, 2, |ui| {
        cantus_ui::views::plugins::manager(&mut a, ui)
    });
    assert!(
        text.contains(&cantus_ui::i18n::t("hostedview.manager.none")),
        "{text}"
    );
    a.settings
        .hosted_views
        .set(PluginFormat::Clap, &info.id, Some(HostedView::Cantus));
    // One a scan no longer knows is listed by its id.
    a.settings
        .hosted_views
        .set(PluginFormat::Vst3, "0123ABCD", Some(HostedView::Native));
    let text = paint(&ctx, size, 2, |ui| {
        cantus_ui::views::plugins::manager(&mut a, ui)
    });
    assert!(text.contains("Cantus Test Gain"), "{text}");
    assert!(text.contains("vst3:0123ABCD"), "{text}");
    assert!(
        text.contains(&cantus_ui::i18n::t("hostedview.cantus")),
        "{text}"
    );
    assert!(
        text.contains(&cantus_ui::i18n::t("hostedview.manager.forget")),
        "{text}"
    );
}

/// A knob of a hosted instrument's Cantus view sets the plug-in's parameter, and a gesture
/// is one undo step (`Subject::Instrument::set`, as the cell calls it).
#[test]
fn an_instruments_knob_sets_its_parameter_in_one_undo_step() {
    use cantus_ui::picker::{PickChoice, PickPurpose};
    use cantus_ui::views::plugin_view::Subject;
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(
        &mut a,
        &synth_library(),
        dir.path(),
        "Cantus Test Synth.clap",
    );
    let track = a.project().tracks[0].id;
    a.apply_pick(
        PickPurpose::ReplaceInstrument { track },
        PickChoice::Hosted(Box::new(info)),
    );
    a.plugins.close_instrument_window(track);
    let steps = a.bus.history().len();
    Subject::Instrument(track).set(&mut a, 0, -6.0);
    Subject::Instrument(track).set(&mut a, 0, -9.0);
    a.end_gesture();
    assert_eq!(a.bus.history().len(), steps + 1, "one step");
    let stored = |a: &cantus_ui::CantusApp| {
        a.project().tracks[0]
            .instrument
            .external()
            .and_then(|p| p.params.get(&0).copied())
    };
    assert_eq!(stored(&a), Some(-9.0));
    a.undo();
    assert_ne!(stored(&a), Some(-9.0), "undone");
}

/// Review of ADR 0073, #1: what a hosted instrument's Cantus view keeps — its A/B, the view
/// chosen — is the plug-in's on the track, never the track's: another load put on the
/// track (a replace, a swap) starts from its own default, and the old one's state is never
/// applied to it.
#[test]
fn an_instruments_view_state_belongs_to_its_plugin_not_its_track() {
    use cantus_ui::picker::{PickChoice, PickPurpose};
    use cantus_ui::views::plugin_view_inst::{on_b, toggle_ab};
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(
        &mut a,
        &synth_library(),
        dir.path(),
        "Cantus Test Synth.clap",
    );
    let track = a.project().tracks[0].id;
    let target = Target::Instrument(track);
    // A pick opens the plug-in: in its own window where one can open (whether it did is
    // said), else in Cantus's view, chosen for that load by the fallback itself.
    let pick = |a: &mut cantus_ui::CantusApp| {
        a.apply_pick(
            PickPurpose::ReplaceInstrument { track },
            PickChoice::Hosted(Box::new(info.clone())),
        );
        let native = a.native_open(target);
        a.plugins.close_instrument_window(track);
        native
    };
    pick(&mut a);
    a.set_hosted_view(target, HostedView::Cantus);
    toggle_ab(&mut a, track);
    assert!(on_b(&a, track), "B showing");
    // Another load of a plug-in on the track.
    a.close_cantus_view(target);
    let native = pick(&mut a);
    a.close_cantus_view(target);
    assert!(!on_b(&a, track), "the old plug-in's A/B is not this one's");
    if native {
        assert_ne!(
            a.hosted_view(target),
            HostedView::Cantus,
            "the view chosen for the old one is not kept for this one"
        );
    } else if cfg!(windows) {
        panic!("Windows opens the plug-in's own window");
    }
    // Its A/B starts from its own state: toggling applies nothing of the old one's.
    let own = a.plugins.capture_instrument(track).expect("loaded");
    toggle_ab(&mut a, track);
    let record = a.project().tracks[0]
        .instrument
        .external()
        .cloned()
        .unwrap();
    assert_eq!((record.state, record.params), own);
    // Another project forgets the view's state too.
    a.replace_project(starter_project_for_tests());
    assert!(!on_b(&a, track));
}

/// Review of ADR 0073, #4: a default set or forgotten changes what opens next, never what
/// an instance already shows.
#[test]
fn a_default_set_later_never_changes_what_shows() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    a.settings_path = dir.path().join("settings.json");
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    let shows = a.hosted_view(target);
    let other = if shows == HostedView::Native {
        HostedView::Cantus
    } else {
        HostedView::Native
    };
    a.set_hosted_view_default(target, Some(other));
    assert_eq!(a.hosted_view(target), shows, "what shows is what shows");
    let rows = cantus_ui::hosted_view::menu_rows(&a, target);
    assert!(rows
        .iter()
        .any(|r| r.label_key == shows.label_key() && r.on));
    a.set_hosted_view_default(target, None);
    assert_eq!(a.hosted_view(target), shows);
    a.plugins.close_window(fx);
}

/// Review of ADR 0073, #2: a plug-in still loading in its helper, opened in its own view,
/// shows Cantus's frame with its loading card meanwhile, and its own window takes the
/// frame's place once it is in; one that fails while loading leaves the frame up with why
/// and *Reload*. (The loading is the host's test switch here; `hosted_view_isolated.rs`
/// has a real slow helper.)
#[test]
fn a_loading_plugin_shows_its_card_then_its_window_or_why_it_failed() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let fx = insert(&mut a, strip, &info);
    let target = Target::Insert(fx);
    a.plugins.close_window(fx);
    a.close_cantus_view(target);
    // Its own view, though no window could open at the insert (no display: the fallback
    // chose Cantus's view for this load).
    a.choose_hosted_view(target, HostedView::Native);
    a.plugins.set_loading_for_tests(fx, true);
    a.open_effect_editor(strip, fx);
    assert!(a.cantus_view_open(target), "its loading card meanwhile");
    assert!(a.fx.hosted.views.pending().contains(&target));
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(
        text.contains(&cantus_ui::i18n::t("hostedview.opens_when_loaded")),
        "{text}"
    );
    a.plugins.set_loading_for_tests(fx, false);
    a.pump_hosted_opens();
    native_or_fallback(&a, target);
    assert!(a.fx.hosted.views.pending().is_empty());
    // Loading again, and failing.
    a.plugins.close_window(fx);
    a.close_cantus_view(target);
    a.choose_hosted_view(target, HostedView::Native);
    a.plugins.set_loading_for_tests(fx, true);
    a.open_effect_editor(strip, fx);
    a.plugins
        .mark_loaded_crashed_for_tests(fx, "it crashed while it loaded");
    a.plugins.set_loading_for_tests(fx, false);
    a.pump_hosted_opens();
    assert!(
        a.cantus_view_open(target),
        "its frame stays: why it failed, with Reload"
    );
    assert!(!a.native_open(target));
    assert!(a.fx.hosted.views.pending().is_empty());
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(
        text.contains(&cantus_ui::i18n::t("isolation.slot.reload")),
        "{text}"
    );
}

/// Review of ADR 0073, #5: the count is the plug-in's showing — a stopped one's too, never
/// the cards of the one shown before.
#[test]
fn the_count_is_the_plugins_showing_even_when_it_stopped() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let first = insert(&mut a, strip, &info);
    a.set_hosted_view(Target::Insert(first), HostedView::Cantus);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("3 parameters"), "{text}");
    a.close_cantus_view(Target::Insert(first));
    a.plugins.close_window(first);
    let second = insert(&mut a, strip, &info);
    a.plugins.close_window(second);
    let five: Vec<cantus_host::ParamInfo> = (0..5)
        .map(|i| param(100 + i, &format!("P{i}"), "Main"))
        .collect();
    a.plugins.set_params_for_tests(second, five);
    a.plugins
        .mark_loaded_crashed_for_tests(second, "it crashed");
    a.open_effect_editor(strip, second);
    assert!(a.cantus_view_open(Target::Insert(second)));
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    assert!(text.contains("5 parameters"), "{text}");
}

/// Second review of ADR 0073, #3: a plug-in waiting for its helper that fails while the
/// user has opened another editor in its frame's place never replaces that editor; and
/// one whose loading frame the user closed never pops its own window up later.
#[test]
fn a_waiting_plugin_never_takes_back_a_frame_the_user_moved_on_from() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project_for_tests());
    let info = know(&mut a, &gain_library(), dir.path(), "Cantus Test Gain.clap");
    let strip = first_strip(&a);
    let first = insert(&mut a, strip, &info);
    a.plugins.close_window(first);
    let second = insert(&mut a, strip, &info);
    a.plugins.close_window(second);
    let (ta, tb) = (Target::Insert(first), Target::Insert(second));
    a.close_cantus_view(tb);
    // A loading; the user opens B's Cantus view in the same frame; A fails.
    a.plugins.set_loading_for_tests(first, true);
    a.open_effect_editor(strip, first);
    assert!(a.cantus_view_open(ta));
    a.set_hosted_view(tb, HostedView::Cantus);
    assert!(a.cantus_view_open(tb));
    a.plugins
        .mark_loaded_crashed_for_tests(first, "it crashed while it loaded");
    a.plugins.set_loading_for_tests(first, false);
    a.pump_hosted_opens();
    assert!(a.cantus_view_open(tb), "the user's editor stays");
    assert!(!a.cantus_view_open(ta));
    assert!(a.fx.hosted.views.pending().is_empty());
    // B loading; its loading frame closed by the user; B loads: nothing pops up.
    a.close_cantus_view(tb);
    a.choose_hosted_view(tb, HostedView::Native);
    a.plugins.set_loading_for_tests(second, true);
    a.open_effect_editor(strip, second);
    assert!(a.cantus_view_open(tb) && a.fx.hosted.views.pending().contains(&tb));
    a.close_fx_editor();
    assert!(
        a.fx.hosted.views.pending().is_empty(),
        "closed by hand: forgotten"
    );
    a.plugins.set_loading_for_tests(second, false);
    a.pump_hosted_opens();
    assert!(!a.native_open(tb), "its window never pops up");
    assert!(!a.cantus_view_open(tb));
}
