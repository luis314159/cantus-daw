//! Where plug-ins run and what the user sees when one stops working (round 29, ADR 0063):
//! the machine's default reaches the host on the boot path; a crash becomes one card in the
//! corner stack it shares with the audio notices, never over another card, never about a
//! plug-in that is gone; its answers work; an insert's menu offers where it runs and
//! *Reload*, as one undo step.

mod support;

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_host::{Format, HostNotice};
use cantus_ui::app::Files;
use cantus_ui::views::plugin_notice::{self, Answer, MenuAct, Target};
use cantus_ui::CantusApp;
use support::*;

const GLACIER: &str = "org.example.glacier";
const REASON: &str = "it crashed: an access violation (0xC0000005)";

/// The starter project with a hosted insert (Glacier) on the first strip, and a hosted
/// instrument (Odin 2) on the first track.
fn hosted_project() -> (Project, StripId, EffectId) {
    let mut p = cantus_ui::starter_project();
    let strip = p.tracks[0].strip.unwrap();
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(ExternalPlugin::new(PluginFormat::Clap, GLACIER, "Glacier"));
    let id = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    p.tracks[0].instrument = InstrumentSpec::External(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.example.odin",
        "Odin 2",
    ));
    (p, strip, id)
}

/// The host marks Glacier stopped (`times` crashes this session), and says so.
fn crash(a: &mut CantusApp, id: EffectId, times: u32) {
    a.plugins
        .mark_crashed_for_tests((Some(id), None), (Format::Clap, GLACIER), REASON, times);
    plugin_notice::on_notice(
        a,
        HostNotice::PluginCrashed {
            effect: id,
            name: "Glacier".into(),
            reason: REASON.into(),
        },
    );
}

/// The same for the first track's Odin 2.
fn crash_instrument(a: &mut CantusApp) -> TrackId {
    let track = a.project().tracks[0].id;
    let key = a.project().tracks[0]
        .instrument
        .external()
        .map(cantus_engine::hosted_key)
        .unwrap();
    a.plugins.mark_crashed_for_tests(
        (None, Some((track, key))),
        (Format::Clap, "org.example.odin"),
        "it stopped answering",
        1,
    );
    plugin_notice::on_notice(
        a,
        HostNotice::InstrumentCrashed {
            track,
            name: "Odin 2".into(),
            reason: "the plug-in sandbox stopped answering".into(),
        },
    );
    track
}

#[test]
fn the_boot_path_gives_the_host_this_computers_default() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    assert_eq!(
        files.settings.plugin_isolation,
        cantus_ui::settings::DEFAULT_PLUGIN_ISOLATION
    );
    let mut files = files;
    files.settings.plugin_isolation = PluginIsolation::Individually;
    let ctx = egui::Context::default();
    let app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        app.plugins.default_isolation,
        PluginIsolation::Individually,
        "assemble applies the setting"
    );
}

#[test]
fn a_crash_is_one_card_with_reload_and_keep_it_off() {
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    crash(&mut a, id, 1);
    let card = plugin_notice::card(&a).expect("a card");
    assert_eq!(card.sentence(), "Glacier stopped working");
    assert_eq!(card.name.as_deref(), Some("Glacier"));
    assert!(card.body.contains("kept playing"), "{}", card.body);
    // Why, in the body too (#36).
    assert!(card.body.contains("touched memory"), "{}", card.body);
    let labels: Vec<&str> = card.buttons.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Reload", "Keep it off"]);
    // The host's account, in the interface's words, its code kept.
    assert!(
        card.reasons
            .contains("Glacier: it crashed: it touched memory that was not its own (0xC0000005)"),
        "{}",
        card.reasons
    );
    // The same plug-in told twice is still one entry; another makes the card plural.
    crash(&mut a, id, 0);
    assert_eq!(a.plugin_crashes.waiting.len(), 1);
    crash_instrument(&mut a);
    let card = plugin_notice::card(&a).unwrap();
    assert_eq!(card.sentence(), "2 plug-ins stopped working");
    assert_eq!(card.buttons[0].0, "Reload all");
    plugin_notice::answer(&mut a, Answer::KeepOff);
    assert!(plugin_notice::card(&a).is_none(), "answered");
}

#[test]
fn the_crash_card_and_the_audio_notices_share_one_stack_and_never_overlap() {
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    a.audio_setup
        .notice(cantus_ui::audio_setup::Notice::Lost("Focusrite USB".into()));
    a.audio_setup
        .notice(cantus_ui::audio_setup::Notice::Connected("UMC204HD".into()));
    // Three crashes: the card has its third button (#28).
    crash(&mut a, id, 3);
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        for size in [egui::vec2(1280.0, 720.0), egui::vec2(420.0, 600.0)] {
            let ctx = ctx();
            let mut layout = Vec::new();
            paint(&ctx, size, 2, |ui| {
                layout = cantus_ui::views::audio_notice::card_layout(&a, ui.ctx());
            });
            assert_eq!(layout.len(), 3, "two audio cards and the plug-in card");
            assert_eq!(
                layout[0].1.len(),
                3,
                "{locale:?}: Reload, Keep it off, Run individually"
            );
            for (i, (r, buttons)) in layout.iter().enumerate() {
                assert!(
                    r.left() >= 0.0 && r.right() <= size.x && r.top() >= 0.0,
                    "{locale:?} {size:?}: a card off the screen: {r:?}"
                );
                for b in buttons {
                    assert!(
                        r.contains_rect(*b),
                        "{locale:?} {size:?}: a button off its card"
                    );
                }
                for (j, b) in buttons.iter().enumerate() {
                    for c in &buttons[j + 1..] {
                        assert!(!b.intersects(*c), "{locale:?} {size:?}: buttons overlap");
                    }
                }
                for (q, _) in &layout[i + 1..] {
                    assert!(
                        !r.intersects(*q),
                        "{locale:?} {size:?}: cards overlap: {r:?} and {q:?}"
                    );
                }
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// A card never speaks of a plug-in that is gone: its slot removed, or the project
/// replaced (#16, #30).
#[test]
fn the_crash_card_leaves_with_the_plugin_it_names() {
    let (p, strip, id) = hosted_project();
    let mut a = app(p.clone());
    crash(&mut a, id, 1);
    plugin_notice::prune(&mut a);
    assert!(
        plugin_notice::card(&a).is_some(),
        "still stopped: still told"
    );
    a.exec(Command::RemoveEffect { strip, effect: id });
    a.sync_engine();
    plugin_notice::prune(&mut a);
    assert!(plugin_notice::card(&a).is_none(), "the slot is gone");
    let ctx = ctx();
    let mut rects = Vec::new();
    paint(&ctx, egui::vec2(1280.0, 720.0), 2, |ui| {
        rects = cantus_ui::views::audio_notice::card_rects(&a, ui.ctx());
    });
    assert!(rects.is_empty());
    // A crash, then another project: the card goes with the old one.
    let mut a = app(p);
    crash(&mut a, id, 1);
    a.replace_project(cantus_ui::starter_project());
    assert!(plugin_notice::card(&a).is_none(), "a new project");
}

/// The card's answers: *Run individually* is one undo step on the plug-in that keeps
/// crashing; *Reload* asks the host (#28).
#[test]
fn the_cards_answers_do_what_they_say() {
    let (p, strip, id) = hosted_project();
    let mut a = app(p);
    crash(&mut a, id, 3);
    let before = a.bus.history().len();
    plugin_notice::answer(&mut a, Answer::RunIndividually);
    assert_eq!(a.bus.history().len(), before + 1, "one undo step");
    let chosen = a
        .project()
        .effect(strip, id)
        .and_then(|e| e.spec.external.as_deref())
        .and_then(|p| p.isolation);
    assert_eq!(chosen, Some(PluginIsolation::Individually));
    assert!(plugin_notice::card(&a).is_none());
    // Reload: the host tries (Glacier is not on this computer, so it says why).
    crash(&mut a, id, 0);
    plugin_notice::answer(&mut a, Answer::Reload);
    assert!(plugin_notice::card(&a).is_none());
    assert!(a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("Glacier"), "{}", a.status.text);
}

/// In a card that lists several plug-ins, *Run individually* is offered for the one
/// that keeps crashing, by name, and moves only that one (#25, #49).
#[test]
fn run_individually_names_the_plugin_that_keeps_crashing() {
    let (p, strip, id) = hosted_project();
    let mut a = app(p);
    crash(&mut a, id, 3);
    let track = crash_instrument(&mut a);
    let card = plugin_notice::card(&a).unwrap();
    let labels: Vec<&str> = card.buttons.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        ["Reload all", "Keep them off", "Run Glacier individually"]
    );
    plugin_notice::answer(&mut a, Answer::RunIndividually);
    let glacier = a
        .project()
        .effect(strip, id)
        .and_then(|e| e.spec.external.as_deref())
        .and_then(|p| p.isolation);
    assert_eq!(glacier, Some(PluginIsolation::Individually));
    let odin = a
        .project()
        .track(track)
        .and_then(|t| t.instrument.external())
        .and_then(|p| p.isolation);
    assert_eq!(odin, None, "its neighbour stays where it was");
}

/// A stopped plug-in's window: the status line says why in the interface's words, and
/// the card comes back with *Reload* (#31).
#[test]
fn asking_for_a_stopped_plugins_window_brings_its_card_back() {
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    crash(&mut a, id, 1);
    plugin_notice::answer(&mut a, Answer::KeepOff);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    a.show_plugin_window(id);
    let text = a.status.text.clone();
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(a.status.error);
    assert!(text.contains("Glacier dejó de funcionar"), "{text}");
    assert!(!text.contains("crashed"), "no English: {text}");
    assert!(plugin_notice::card(&a).is_some(), "its card is back");
}

/// Why a plug-in stopped, from the forms the host writes on Windows and on Linux (#37).
#[test]
fn the_reasons_keep_their_detail_on_every_system() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let w = plugin_notice::reason_words;
    assert_eq!(
        w("it crashed (signal 11)"),
        "it crashed: it touched memory that was not its own (signal 11)"
    );
    assert_eq!(
        w("it crashed (signal 6)"),
        "it crashed: it aborted (signal 6)"
    );
    assert_eq!(
        w("it crashed (signal 4)"),
        "it crashed: an instruction the processor refused (signal 4)"
    );
    assert_eq!(
        w("it crashed (signal 8)"),
        "it crashed: a division by zero (signal 8)"
    );
    assert_eq!(w("it crashed (signal 15)"), "it crashed (signal 15)");
    assert_eq!(
        w("it crashed (exit code 0xC0000135)"),
        "it crashed (0xC0000135)"
    );
    assert_eq!(
        w("it crashed: an access violation (0xC0000005)"),
        "it crashed: it touched memory that was not its own (0xC0000005)"
    );
}

#[test]
fn where_a_plugin_runs_is_its_menus_choice_and_one_undo_step() {
    let (p, strip, id) = hosted_project();
    let mut a = app(p);
    let rows = plugin_notice::menu_rows(&a, Target::Insert(id));
    // The machine's default first, then the four; no Reload while it works.
    assert_eq!(rows.len(), 5, "{rows:?}");
    assert_eq!(rows[0].1, Some(true), "it follows the default");
    assert!(rows.iter().all(|r| r.2 != MenuAct::Reload));
    let before = a.bus.history().len();
    plugin_notice::run_menu(
        &mut a,
        Target::Insert(id),
        MenuAct::Run(Some(PluginIsolation::Individually)),
    );
    assert_eq!(a.bus.history().len(), before + 1);
    let chosen = a
        .project()
        .effect(strip, id)
        .and_then(|e| e.spec.external.as_deref())
        .and_then(|p| p.isolation);
    assert_eq!(chosen, Some(PluginIsolation::Individually));
    let rows = plugin_notice::menu_rows(&a, Target::Insert(id));
    assert_eq!(rows[4].1, Some(true));
    a.undo();
    assert_eq!(
        plugin_notice::chosen(&a, Target::Insert(id)),
        Some(None),
        "undo gives the default back"
    );
}

#[test]
fn the_strings_of_isolation_exist_in_both_languages() {
    for key in [
        "isolation.crashed.one",
        "isolation.crashed.suffix",
        "isolation.reload",
        "isolation.keep_off",
        "isolation.run_individually",
        "isolation.run_name_individually",
        "isolation.slot.band",
        "isolation.slot.stopped_band",
        "isolation.window.stopped",
        "isolation.export.stopped",
        "isolation.bake.stopped",
        "isolation.manager.band",
        "isolation.status.late",
        "cmd.set_plugin_isolation",
    ] {
        for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(locale);
            assert_ne!(cantus_ui::i18n::t(key), key, "{key} in {locale:?}");
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// A plug-in whose name has parentheses keeps them in its name: they are no system code
/// (review #26); the detail a system gives is said in the interface's words, in both
/// languages (#32).
#[test]
fn a_reasons_detail_is_a_system_code_only() {
    let w = plugin_notice::reason_words;
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(
        w("Kontakt 7 (16 out) stopped responding while it processed audio"),
        cantus_ui::i18n::t("isolation.why.hung_audio")
    );
    assert_eq!(w("it crashed (exit status: 1)"), "it crashed (exit code 1)");
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let es = [
        w("it crashed (signal 11)"),
        w("it crashed (exit status: 1)"),
        w("Serum (x64) stopped responding while it rendered"),
    ];
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(es[0].contains("señal 11"), "{}", es[0]);
    assert!(es[1].contains("código de salida 1"), "{}", es[1]);
    assert!(!es[2].contains("x64"), "{}", es[2]);
    for s in &es {
        assert!(!s.contains("signal") && !s.contains("exit"), "{s}");
    }
}

/// The Run button of a card that lists several plug-ins names the one that keeps
/// crashing, shortened in its middle, its verb whole; in Spanish at 420 × 600 every
/// button stays on its card (review #27).
#[test]
fn a_long_name_never_cuts_the_run_buttons_words() {
    let mut p = cantus_ui::starter_project();
    let strip = p.tracks[0].strip.unwrap();
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        GLACIER,
        "Spectrasonics Omnisphere Orchestral Edition",
    ));
    let id = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    p.tracks[0].instrument = InstrumentSpec::External(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.example.odin",
        "Odin 2",
    ));
    let mut a = app(p);
    a.plugins
        .mark_crashed_for_tests((Some(id), None), (Format::Clap, GLACIER), REASON, 3);
    plugin_notice::on_notice(
        &mut a,
        HostNotice::PluginCrashed {
            effect: id,
            name: "Spectrasonics Omnisphere Orchestral Edition".into(),
            reason: REASON.into(),
        },
    );
    crash_instrument(&mut a);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let card = plugin_notice::card(&a).unwrap();
    let run = card.buttons[2].0.clone();
    let ctx = ctx();
    let mut layout = Vec::new();
    paint(&ctx, egui::vec2(420.0, 600.0), 2, |ui| {
        layout = cantus_ui::views::audio_notice::card_layout(&a, ui.ctx());
    });
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(
        run.starts_with("Ejecutar ") && run.ends_with(" por separado"),
        "{run}"
    );
    assert!(run.contains('\u{2026}'), "the name shortened: {run}");
    let (r, buttons) = &layout[0];
    assert_eq!(buttons.len(), 3);
    for b in buttons {
        assert!(r.contains_rect(*b), "a button off its card");
    }
    // A button no wider than its card's room shows its words whole (≤ the measured room).
    let room = r.width() - 96.0;
    let k = ctx.pixels_per_point();
    let w = cantus_ui::kit::text::measure(
        &ctx,
        &cantus_ui::kit::text::Text::new(
            &run,
            cantus_ui::kit::text::Role::Control,
            egui::Color32::WHITE,
        ),
        k,
    )
    .x;
    assert!(w + 24.0 <= room, "{run}: {w} in {room}");
}

/// One render at a time: a modulator bake waits for an export, and an export for a bake
/// — they share the plug-ins' render instances (review #5, #10, #13, #21, #48).
#[test]
fn a_bake_and_an_export_never_render_at_once() {
    let mut p = cantus_ui::starter_project();
    let second = p.make_track("Second");
    p.tracks.push(second);
    let listen = p.tracks[0].strip.unwrap();
    let target_strip = p.tracks[1].strip.unwrap();
    let mut f = cantus_core::Modulator::new(
        cantus_core::ModulatorId(p.alloc_id()),
        cantus_core::ModKind::Follower,
    );
    if let cantus_core::ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(listen);
    }
    let target = AutoTarget::StripPan(target_strip);
    f.routes.push(cantus_core::ModRoute {
        target: target.clone(),
        depth: 0.4,
    });
    let mid = f.id;
    p.strip_mut(target_strip).unwrap().modulators.push(f);
    let mut a = app(p);
    let dir = tempfile::tempdir().unwrap();
    assert!(cantus_ui::views::export::start_with(
        &mut a,
        &dir.path().join("mix.wav"),
        None
    ));
    assert!(!a.bake_route(mid, &target), "the bake waits for the export");
    assert!(a.status.error);
    cantus_ui::views::export::wait(&mut a);
    assert!(a.bake_route(mid, &target), "then it runs");
    assert!(
        !cantus_ui::views::export::start_with(&mut a, &dir.path().join("b.wav"), None),
        "the export waits for the bake"
    );
    assert!(a.status.error);
    let start = std::time::Instant::now();
    while a.modulators.pending_bake.is_some() && start.elapsed().as_secs() < 30 {
        a.poll_bake();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// A render that lost a plug-in is never reported done: the export says so, the bake
/// bakes nothing, the freeze freezes nothing (review #6, #21, #47).
#[test]
fn every_render_that_lost_a_plugin_says_so() {
    let dir = tempfile::tempdir().unwrap();
    // The export.
    let mut a = app(cantus_ui::starter_project());
    assert!(cantus_ui::views::export::start_with(
        &mut a,
        &dir.path().join("mix.wav"),
        None
    ));
    a.plugins
        .lose_in_render_for_tests(
            "Glacier",
            "it crashed: an access violation (0xC0000005)",
            cantus_host::RenderLoss::Stopped,
        );
    cantus_ui::views::export::wait(&mut a);
    assert!(a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("Glacier"), "{}", a.status.text);
    assert!(matches!(a.export.last, Some(Err(_))));
    // The freeze.
    let mut a = app(cantus_ui::starter_project());
    a.engine.frozen.set_app_dir(dir.path().join("renders"));
    a.path = Some(dir.path().join("song.cantus"));
    let strip = a.project().tracks[0].strip.unwrap();
    a.freeze_target(cantus_core::FreezeTarget::Strip(strip));
    assert!(a.freeze.job.is_some());
    a.plugins
        .lose_in_render_for_tests(
            "Glacier",
            "it crashed: an access violation (0xC0000005)",
            cantus_host::RenderLoss::Stopped,
        );
    let ctx = ctx();
    let start = std::time::Instant::now();
    while a.freeze.job.is_some() && start.elapsed().as_secs() < 30 {
        cantus_ui::freeze::tick(&mut a, &ctx);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("Glacier"), "{}", a.status.text);
    assert!(
        a.project().strip(strip).is_some_and(|s| s.frozen.is_none()),
        "nothing frozen"
    );
}

/// A modulator bake whose render lost a plug-in bakes nothing, and says why (#21).
#[test]
fn a_bake_that_lost_a_plugin_bakes_nothing() {
    let mut p = cantus_ui::starter_project();
    let second = p.make_track("Second");
    p.tracks.push(second);
    let listen = p.tracks[0].strip.unwrap();
    let target_strip = p.tracks[1].strip.unwrap();
    let mut f = cantus_core::Modulator::new(
        cantus_core::ModulatorId(p.alloc_id()),
        cantus_core::ModKind::Follower,
    );
    if let cantus_core::ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(listen);
    }
    let target = AutoTarget::StripPan(target_strip);
    f.routes.push(cantus_core::ModRoute {
        target: target.clone(),
        depth: 0.4,
    });
    let mid = f.id;
    p.strip_mut(target_strip).unwrap().modulators.push(f);
    let mut a = app(p);
    let before = a.bus.history().len();
    assert!(a.bake_route(mid, &target));
    a.plugins
        .lose_in_render_for_tests(
            "Glacier",
            "it crashed: an access violation (0xC0000005)",
            cantus_host::RenderLoss::Stopped,
        );
    let start = std::time::Instant::now();
    while a.modulators.pending_bake.is_some() && start.elapsed().as_secs() < 30 {
        a.poll_bake();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("Glacier"), "{}", a.status.text);
    assert_eq!(a.bus.history().len(), before, "nothing baked");
}

/// A modulator that follows a strip's sound, routed to the second strip's pan.
fn follower_project() -> (Project, cantus_core::ModulatorId, AutoTarget) {
    let mut p = cantus_ui::starter_project();
    let second = p.make_track("Second");
    p.tracks.push(second);
    let listen = p.tracks[0].strip.unwrap();
    let target_strip = p.tracks[1].strip.unwrap();
    let mut f = cantus_core::Modulator::new(
        cantus_core::ModulatorId(p.alloc_id()),
        cantus_core::ModKind::Follower,
    );
    if let cantus_core::ModSource::Follower(src) = &mut f.source {
        src.listen = cantus_core::modulator::Listen::Strip(listen);
    }
    let target = AutoTarget::StripPan(target_strip);
    f.routes.push(cantus_core::ModRoute {
        target: target.clone(),
        depth: 0.4,
    });
    let mid = f.id;
    p.strip_mut(target_strip).unwrap().modulators.push(f);
    (p, mid, target)
}

/// Two bakes in a row: the second waits for the first, which is never thrown away nor
/// has its render ended under it (review isolation-4 #5, #21).
#[test]
fn two_bakes_in_a_row_never_overlap() {
    let (p, mid, target) = follower_project();
    let mut a = app(p);
    let before = a.bus.history().len();
    assert!(a.bake_route(mid, &target), "the first bake traces");
    assert!(!a.bake_route(mid, &target), "the second waits for it");
    assert!(a.status.error, "and says so: {}", a.status.text);
    assert!(a.modulators.pending_bake.is_some(), "the first is kept");
    let start = std::time::Instant::now();
    while a.modulators.pending_bake.is_some() && start.elapsed().as_secs() < 30 {
        a.poll_bake();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(a.bus.history().len(), before + 1, "the first bake was made");
}

/// An export that never had a plug-in says so in the interface's words — the file was
/// made without it, never "from that point on" — and one begun from a busy plug-in's last
/// saved state is done, with a word (review isolation-4 #10, #41).
#[test]
fn a_render_that_never_had_a_plugin_says_so_in_its_words() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(cantus_ui::starter_project());
    assert!(cantus_ui::views::export::start_with(
        &mut a,
        &dir.path().join("mix.wav"),
        None
    ));
    a.plugins.lose_in_render_for_tests(
        "Glacier",
        "C:\\Plugins\\glacier.clap: init refused",
        cantus_host::RenderLoss::NeverLoaded,
    );
    cantus_ui::views::export::wait(&mut a);
    let text = a.status.text.clone();
    assert!(a.status.error, "{text}");
    assert!(text.contains("No se pudo cargar Glacier"), "{text}");
    assert!(text.contains("no se pudo cargar"), "the reason in its words: {text}");
    assert!(!text.contains("desde ese punto"), "{text}");
    assert!(!text.contains("init refused"), "{text}");
    assert!(matches!(a.export.last, Some(Err(_))));
    // Begun from its last-saved state: done, and said.
    let mut a = app(cantus_ui::starter_project());
    assert!(cantus_ui::views::export::start_with(
        &mut a,
        &dir.path().join("mix2.wav"),
        None
    ));
    a.plugins.lose_in_render_for_tests(
        "Glacier",
        "the plug-in is busy: its own window holds it",
        cantus_host::RenderLoss::Stale,
    );
    cantus_ui::views::export::wait(&mut a);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(matches!(a.export.last, Some(Ok(_))), "done");
    assert!(
        a.status.text.contains("estaba ocupado"),
        "{}",
        a.status.text
    );
}

/// A reload that crashes again: its card comes back, and the status line says why in the
/// interface's words (review isolation-4 #38, #64).
#[test]
fn a_reload_that_crashes_again_brings_its_card_back_in_its_words() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    crash(&mut a, id, 1);
    a.plugins
        .fail_next_reload_for_tests("it crashed: an abort (0x00000003)", true);
    plugin_notice::reload(&mut a, Target::Insert(id), "Glacier");
    let text = a.status.text.clone();
    let card = plugin_notice::card(&a);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(a.status.error, "{text}");
    assert!(text.contains("se abortó"), "{text}");
    assert!(!text.contains("crashed") && !text.contains("abort ("), "{text}");
    let card = card.expect("the card is back");
    assert!(card.body.contains("se abortó"), "why it is off now: {}", card.body);
}

/// A reload that fails without crashing says so in the interface's words, never the host's
/// English (review isolation-4 #25, #40).
#[test]
fn a_reload_that_fails_without_crashing_says_so_in_its_words() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    a.plugins.mark_missing_for_tests(id, "not loaded");
    a.plugins
        .fail_next_reload_for_tests("C:\\Plugins\\glacier.clap: init refused", false);
    plugin_notice::reload(&mut a, Target::Insert(id), "Glacier");
    let text = a.status.text.clone();
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(text.contains("no se pudo volver a cargar"), "{text}");
    assert!(!text.contains("init refused"), "{text}");
}

/// A window asked for while its plug-in's helper is busy: said as such, in the interface's
/// words — it opens by itself once free — never an error in the host's English with a wait
/// that did not happen (review isolation-4 #39).
#[test]
fn a_busy_window_is_said_in_its_words() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let mut a = app(cantus_ui::starter_project());
    let busy = cantus_host::HostError::Busy("its own window holds it".into());
    plugin_notice::window_failed(&mut a, "Glacier", &busy);
    let text = a.status.text.clone();
    let error = a.status.error;
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    assert!(!error, "not an error: {text}");
    assert!(text.contains("Glacier está ocupado"), "{text}");
    assert!(!text.contains("busy") && !text.contains("holds"), "{text}");
}

/// A plug-in that could not be loaded — refused, or its helper busy — can be loaded again
/// from its menu, not only one that crashed (review isolation-4 #57).
#[test]
fn a_plugin_that_never_loaded_can_be_reloaded_from_its_menu() {
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    a.plugins
        .mark_missing_for_tests(id, "the plug-in is busy: it did not answer in 30 s");
    let rows = plugin_notice::menu_rows(&a, Target::Insert(id));
    assert!(
        rows.iter().any(|(_, _, act)| *act == MenuAct::Reload),
        "{rows:?}"
    );
}

/// A plug-in waiting on a window of its own has a card of its own — *Show it*, *Stop it* —
/// that a close keeps away until it waits anew, and that goes when it is stopped (review
/// isolation-4 #54).
#[test]
fn a_plugin_waiting_on_its_window_has_a_card() {
    use plugin_notice::WaitAnswer;
    let (p, _, id) = hosted_project();
    let mut a = app(p);
    a.plugins.set_waiting_for_tests(id, true);
    let card = plugin_notice::waiting_card(&a).expect("its card");
    assert_eq!(card.name, "Glacier");
    let answers: Vec<_> = card.buttons.iter().map(|(_, a)| *a).collect();
    assert_eq!(answers, vec![WaitAnswer::Show, WaitAnswer::Stop]);
    let ctx = ctx();
    let mut layout = Vec::new();
    paint(&ctx, egui::vec2(420.0, 600.0), 2, |ui| {
        layout = cantus_ui::views::audio_notice::card_layout(&a, ui.ctx());
    });
    assert_eq!(layout.len(), 1);
    for b in &layout[0].1 {
        assert!(layout[0].0.contains_rect(*b), "a button off its card");
    }
    plugin_notice::answer_waiting(&mut a, card.target, WaitAnswer::Dismiss);
    assert!(plugin_notice::waiting_card(&a).is_none(), "closed");
    a.plugins.set_waiting_for_tests(id, false);
    plugin_notice::prune(&mut a);
    a.plugins.set_waiting_for_tests(id, true);
    assert!(plugin_notice::waiting_card(&a).is_some(), "waiting anew");
    plugin_notice::answer_waiting(&mut a, card.target, WaitAnswer::Stop);
    assert!(plugin_notice::waiting_card(&a).is_none(), "stopped");
}
