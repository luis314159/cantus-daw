//! Where plug-ins run and what the user sees when one stops working (round 29, ADR 0063):
//! the machine's default reaches the host on the boot path; a crash becomes one card in the
//! corner stack it shares with the audio notices, never over another card; its answers
//! work; an insert's menu offers where it runs and *Reload*, as one undo step.

mod support;

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_host::HostNotice;
use cantus_ui::app::Files;
use cantus_ui::views::plugin_notice::{self, Answer, MenuAct, Target};
use cantus_ui::CantusApp;
use support::*;

fn hosted_project() -> (Project, StripId, EffectId) {
    let mut p = cantus_ui::starter_project();
    let strip = p.tracks[0].strip.unwrap();
    let mut slot = p.make_effect(EffectKind::External);
    slot.spec = EffectSpec::external(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.example.glacier",
        "Glacier",
    ));
    let id = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot);
    (p, strip, id)
}

fn crashed(effect: EffectId, name: &str) -> HostNotice {
    HostNotice::PluginCrashed {
        effect,
        name: name.into(),
        reason: "it crashed: an access violation (0xC0000005)".into(),
    }
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
    plugin_notice::on_notice(&mut a, crashed(id, "Glacier"));
    let (title, body, buttons, reasons) = plugin_notice::card(&a).expect("a card");
    assert_eq!(title, "Glacier stopped working");
    assert!(body.contains("kept playing"), "{body}");
    let labels: Vec<&str> = buttons.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Reload", "Keep it off"]);
    assert!(reasons.contains("access violation"), "{reasons}");
    // The same plug-in told twice is still one entry; another makes the card plural.
    plugin_notice::on_notice(&mut a, crashed(id, "Glacier"));
    assert_eq!(a.plugin_crashes.waiting.len(), 1);
    plugin_notice::on_notice(
        &mut a,
        HostNotice::InstrumentCrashed {
            track: a.project().tracks[0].id,
            name: "Odin 2".into(),
            reason: "it stopped answering".into(),
        },
    );
    let (title, _, buttons, _) = plugin_notice::card(&a).unwrap();
    assert_eq!(title, "2 plug-ins stopped working");
    assert_eq!(buttons[0].0, "Reload all");
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
    plugin_notice::on_notice(&mut a, crashed(id, "Glacier"));
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        let ctx = ctx();
        let mut rects = Vec::new();
        paint(&ctx, egui::vec2(1280.0, 720.0), 2, |ui| {
            rects = cantus_ui::views::audio_notice::card_rects(&a, ui.ctx());
        });
        assert_eq!(rects.len(), 3, "two audio cards and the plug-in card");
        for (i, r) in rects.iter().enumerate() {
            for q in &rects[i + 1..] {
                assert!(
                    !r.intersects(*q),
                    "{locale:?}: cards overlap: {r:?} and {q:?}"
                );
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
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
        "isolation.reload",
        "isolation.keep_off",
        "isolation.run_individually",
        "isolation.slot.band",
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
