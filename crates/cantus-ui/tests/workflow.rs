//! Round 31's workflow (ADR 0072), through the application: the command palette finds
//! everything and runs it, the undo history goes anywhere in the history and marks the
//! save, project templates make new projects, track templates insert as one undo step.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::keymap::Action;
use cantus_ui::palette_index::{Kind, Target, Window};
use cantus_ui::templates::{Starter, TemplateRef};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::SettingsPage;
use cantus_ui::CantusApp;

/// The locale is the process's: tests that set it hold this.
fn lang() -> std::sync::MutexGuard<'static, ()> {
    static L: std::sync::Mutex<()> = std::sync::Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}

/// An app on `project` whose files are all in `dir`.
fn app_in(dir: &std::path::Path, project: Option<Project>) -> CantusApp {
    let mut a = CantusApp::from_parts(project, EngineHandle::dummy(), false);
    a.settings_path = dir.join("settings.json");
    a.layouts_path = dir.join("layouts.json");
    a.keymap_path = dir.join("keymap.toml");
    a.workflow.wire_at(dir, dir);
    a
}

#[test]
fn every_action_has_a_palette_entry_and_a_name_in_both_languages() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let entries = cantus_ui::views::palette::entries(&mut a);
    for action in Action::ALL {
        let key = action.label_key();
        assert!(
            i18n::has(Locale::En, &key),
            "{action:?} has no English name"
        );
        assert!(
            i18n::has(Locale::Es, &key),
            "{action:?} has no Spanish name"
        );
        if action == Action::CommandPalette {
            continue;
        }
        let e = entries
            .iter()
            .find(|e| e.target == Target::Action(action))
            .unwrap_or_else(|| panic!("{action:?} is not in the palette"));
        assert!(!e.name.is_empty() && !e.english.is_empty());
        assert_eq!(e.detail, a.keymap.display_for(action), "its shortcut shown");
    }
    for page in SettingsPage::ALL {
        assert!(
            entries.iter().any(|e| e.target == Target::Settings(page)),
            "{page:?}"
        );
    }
    for w in Window::ALL {
        assert!(
            entries.iter().any(|e| e.target == Target::Window(w)),
            "{w:?}"
        );
    }
    // Every area: by its action, or as a view of its own.
    for p in [
        Pane::Arrangement,
        Pane::PianoRoll,
        Pane::ChannelRack,
        Pane::Mixer,
        Pane::Inspector,
        Pane::Devices,
        Pane::Browser,
        Pane::Clips,
        Pane::Routing,
        Pane::Controllers,
        Pane::History,
    ] {
        assert!(
            cantus_ui::views::palette::reaches(p),
            "{p:?} cannot be reached from the palette"
        );
    }
    // Every effect and instrument of the catalogue, and the starters.
    assert!(entries.iter().any(|e| e.kind == Kind::Effect));
    assert!(entries.iter().any(|e| e.kind == Kind::Instrument));
    for s in Starter::ALL {
        assert!(entries
            .iter()
            .any(|e| e.target == Target::Template(TemplateRef::Starter(s))));
    }
}

#[test]
fn the_palette_finds_in_both_languages_and_learns_what_is_used() {
    let _l = lang();
    let dir = tempfile::tempdir().unwrap();
    i18n::set_thread_locale(Locale::Es);
    let mut a = app_in(dir.path(), None);
    let entries = cantus_ui::views::palette::entries(&mut a);
    let uses = cantus_ui::palette_index::Uses::default();
    let first = |q: &str, u: &cantus_ui::palette_index::Uses| {
        cantus_ui::palette_index::rank(&entries, q, u)
            .first()
            .map(|&i| entries[i].target.clone())
    };
    // Spanish words, English words, and a fuzzy abbreviation all find the mixer's key.
    assert_eq!(
        first("mezclador", &uses),
        Some(Target::Action(Action::ViewMixer))
    );
    assert_eq!(
        first("show the mixer", &uses),
        Some(Target::Action(Action::ViewMixer))
    );
    assert_eq!(first("deshacer", &uses), Some(Target::Action(Action::Undo)));
    // A thing used often comes first among close matches.
    let mut learnt = cantus_ui::palette_index::Uses::default();
    let wanted = cantus_ui::palette_index::Entry::action_key(Action::ToggleMetronome);
    for _ in 0..8 {
        learnt.used(&wanted);
    }
    let hits = cantus_ui::palette_index::rank(&entries, "met", &learnt);
    assert_eq!(
        entries[hits[0]].target,
        Target::Action(Action::ToggleMetronome)
    );
    // Nothing typed: the last used first.
    let empty = cantus_ui::palette_index::rank(&entries, "", &learnt);
    assert_eq!(entries[empty[0]].key, wanted);
    i18n::set_thread_locale(Locale::En);
}

#[test]
fn the_palette_opens_on_its_key_runs_a_row_and_counts_it() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    a.run_action(Action::CommandPalette);
    assert!(a.workflow.palette.is_open());
    a.workflow.palette.query = "metronome".into();
    let rows = a.workflow.palette.rows().len();
    assert!(rows > 0);
    // Run the metronome's row as Enter would.
    let entry = cantus_ui::palette_index::rank(
        a.workflow.palette.entries(),
        "metronome on",
        &a.workflow.uses,
    )
    .first()
    .map(|&i| a.workflow.palette.entries()[i].clone())
    .unwrap();
    assert_eq!(entry.target, Target::Action(Action::ToggleMetronome));
    let before = a.project().metronome;
    a.workflow.palette.close();
    cantus_ui::views::palette::run(&mut a, &entry);
    assert_ne!(a.project().metronome, before, "it ran");
    assert_eq!(a.workflow.uses.of(&entry.key).map(|u| u.count), Some(1));
    // Kept beside the settings.
    let kept =
        cantus_ui::palette_index::Uses::load(&cantus_ui::palette_index::uses_path(dir.path()));
    assert_eq!(kept.of(&entry.key).map(|u| u.count), Some(1));
    // An effect from the palette goes on the selected track's strip, one step.
    let strip = a.project().tracks[0].strip.unwrap();
    a.selected_track = Some(a.project().tracks[0].id);
    let comp = a
        .picker_view
        .catalog
        .iter()
        .find(|d| d.choice == cantus_ui::picker::PickChoice::Effect(EffectKind::Compressor))
        .map(|d| d.key.clone())
        .unwrap();
    let e = cantus_ui::views::palette::entries(&mut a)
        .into_iter()
        .find(|e| e.target == Target::Device(comp.clone()))
        .unwrap();
    let steps = a.bus.history().len();
    cantus_ui::views::palette::run(&mut a, &e);
    assert!(a
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .iter()
        .any(|x| x.spec.kind == EffectKind::Compressor));
    assert_eq!(a.bus.history().len(), steps + 1);
}

#[test]
fn the_history_goes_to_any_step_and_marks_the_save() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let t = a.project().tracks[0].id;
    // Review 6: a project never saved has no step that is its file's.
    assert!(!cantus_ui::views::history::rows(&a, 0..1)[0].saved);
    for (i, name) in ["One", "Two", "Three", "Four"].iter().enumerate() {
        assert!(a.exec(Command::SetTrackName {
            id: t,
            name: name.to_string(),
        }));
        if i == 1 {
            a.save_to_path(&dir.path().join("song.cantus"));
        }
    }
    let rows = cantus_ui::views::history::rows(&a, 0..usize::MAX);
    assert_eq!(rows.len(), 5, "the start and four steps");
    assert_eq!(rows[0].name, "Project opened");
    assert_eq!(rows[2].name, "Rename track to Two");
    assert!(rows[4].current && !rows[3].current);
    assert!(rows[2].saved, "the second step is the file's");
    assert!(!rows[2].unsaved && rows[3].unsaved && rows[4].unsaved);
    // A click on the first step: three undos, one per step; the history stays.
    assert!(cantus_ui::views::history::go_to(&mut a, rows[1].step));
    assert_eq!(a.project().track(t).unwrap().name, "One");
    assert_eq!(a.bus.history().len(), 1);
    assert_eq!(a.bus.redo_history().len(), 3);
    let rows = cantus_ui::views::history::rows(&a, 0..usize::MAX);
    assert!(rows[1].current && rows[2].ahead && rows[4].ahead);
    assert!(
        rows[2].saved && rows[2].unsaved,
        "a step to redo the file holds"
    );
    // A click on the last: redone, all of it.
    assert!(cantus_ui::views::history::go_to(&mut a, rows[4].step));
    assert_eq!(a.project().track(t).unwrap().name, "Four");
    // To the start: the project as it was opened.
    let start = cantus_ui::views::history::rows(&a, 0..1)[0].step;
    assert!(cantus_ui::views::history::go_to(&mut a, start));
    assert_eq!(a.bus.history().len(), 0);
    assert_eq!(a.bus.redo_history().len(), 4);
    // Alt+H shows and hides the pane.
    a.run_action(Action::ViewHistory);
    assert!(a.workspace.contains(Pane::History));
    a.run_action(Action::ViewHistory);
    assert!(!a.workspace.contains(Pane::History));
}

#[test]
fn every_starter_is_a_project_that_saves_loads_and_opens_untitled() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    for s in Starter::ALL {
        let p = s.build();
        let file = dir.path().join(format!("{}.cantus", s.id()));
        cantus_project::save(&p, &file, "test").unwrap();
        let back = cantus_project::load(&file, true).unwrap().project;
        assert_eq!(back, p, "{s:?} survives its file");
        // Every id unique, every strip it names there.
        let mut ids = std::collections::HashSet::new();
        for t in &p.tracks {
            assert!(ids.insert(t.id.0), "{s:?}");
            if let Some(st) = t.strip {
                assert!(p.strip(st).is_some(), "{s:?}: its strip");
            }
        }
        for st in &p.strips {
            assert!(ids.insert(st.id.0), "{s:?}");
            for x in &st.sends {
                assert!(p.strip(x.to).is_some(), "{s:?}: a send's return");
            }
        }
    }
    assert!(!Starter::Mastering.build().master.effects.is_empty());
    assert!(Starter::Beat
        .build()
        .tracks
        .iter()
        .all(|t| !t.clips.is_empty()));
    // New from template: an untitled project in place, nothing to save yet.
    let mut a = app_in(dir.path(), None);
    a.request_template(TemplateRef::Starter(Starter::Band));
    assert_eq!(a.project().tracks.len(), 4);
    assert_eq!(a.project().name, i18n::t("app.untitled"));
    assert!(a.path.is_none() && !a.bus.is_dirty());
}

#[test]
fn a_users_project_template_is_kept_listed_opened_and_deleted() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), Some(Starter::Beat.build()));
    a.run_action(Action::SaveAsTemplate);
    let naming = a.workflow.naming.as_mut().expect("the name is asked");
    naming.name = "My beat".into();
    let base = a.audio_base().map(|p| p.to_path_buf());
    let path = a
        .workflow
        .store
        .save_project(a.project(), base.as_deref(), "My beat")
        .unwrap();
    assert!(path.starts_with(dir.path().join("templates").join("projects")));
    let listed = a.workflow.store.projects();
    let mine = listed
        .iter()
        .find(|t| t.name == "My beat")
        .expect("listed after the starters");
    assert_eq!(mine.template, TemplateRef::User(path.clone()));
    // Unsaved changes: the question first; Don't save, then the template is there.
    let t = a.project().tracks[0].id;
    a.exec(Command::SetTrackName {
        id: t,
        name: "Changed".into(),
    });
    a.request_template(TemplateRef::User(path.clone()));
    assert!(a.unsaved_prompt.is_some(), "asked first");
    a.answer_unsaved(cantus_ui::views::unsaved::Answer::DontSave);
    assert_eq!(a.project().tracks.len(), Starter::Beat.build().tracks.len());
    assert_ne!(a.project().tracks[0].name, "Changed");
    // Deleted from the start page's ×: only a file of the store's.
    assert!(a
        .workflow
        .store
        .delete(&dir.path().join("elsewhere.cantus"))
        .is_err());
    a.workflow.store.delete(&path).unwrap();
    assert!(!a
        .workflow
        .store
        .projects()
        .iter()
        .any(|t| t.name == "My beat"));
}

/// A project with a lead whose strip has a compressor, a send to *Reverb* and a modulator
/// on its own fader; a *Reverb* return.
fn lead_project() -> Project {
    let mut p = cantus_ui::starter_project();
    let mut ret = p.make_strip("Reverb");
    ret.dock = Dock::Right;
    ret.effects.push(p.make_effect(EffectKind::Space));
    let ret_id = ret.id;
    p.strips.push(ret);
    let t = &p.tracks[0];
    let (track, strip) = (t.id, t.strip.unwrap());
    let comp = p.make_effect(EffectKind::Compressor);
    let mid = ModulatorId(p.alloc_id());
    let mut m = Modulator::new(mid, ModKind::Lfo);
    m.routes.push(ModRoute {
        target: automation::AutoTarget::StripPan(strip),
        depth: 0.5,
    });
    let s = p.strip_mut(strip).unwrap();
    s.effects.push(comp);
    let mut send = Send::new(ret_id);
    send.level = 0.4;
    s.sends.push(send);
    s.modulators.push(m);
    let tr = p.track_mut(track).unwrap();
    tr.name = "Lead".into();
    tr.color = Color::rgb(200, 40, 90);
    p
}

#[test]
fn a_track_template_inserts_as_one_step_with_new_ids_and_finds_its_return_by_name() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), Some(lead_project()));
    let lead = a.project().tracks[0].id;
    a.selected_track = Some(lead);
    a.run_action(Action::SaveTrackTemplate);
    assert_eq!(
        a.workflow.naming.as_ref().map(|n| n.name.as_str()),
        Some("Lead")
    );
    a.workflow.naming = None;
    let path = a
        .workflow
        .store
        .save_track(a.project(), lead, "Lead")
        .unwrap();
    let tpl = cantus_ui::templates::load_track_template(&path).unwrap();
    assert_eq!(tpl.tracks.len(), 1);
    assert!(
        tpl.tracks[0].clips.is_empty(),
        "a sound and a channel, not a song"
    );
    // Into another project with a return of that name: one step, every id new.
    let mut other = cantus_ui::starter_project();
    let mut ret = other.make_strip("Reverb");
    ret.dock = Dock::Right;
    let other_ret = ret.id;
    other.strips.push(ret);
    let mut b = app_in(dir.path(), Some(other));
    let before = b.project().clone();
    let steps = b.bus.history().len();
    assert!(b.insert_track_template(&path, 1));
    assert_eq!(b.bus.history().len(), steps + 1, "one undo step");
    let p = b.project();
    let t = &p.tracks[1];
    assert_eq!(t.name, "Lead");
    assert_eq!(t.color, Color::rgb(200, 40, 90));
    assert_eq!(b.selected_track, Some(t.id));
    let s = p.strip(t.strip.unwrap()).unwrap();
    assert!(s
        .effects
        .iter()
        .any(|e| e.spec.kind == EffectKind::Compressor));
    assert_eq!(s.sends.len(), 1);
    assert_eq!(s.sends[0].to, other_ret, "found by its name");
    assert_eq!(s.sends[0].level, 0.4);
    let route = &s.modulators[0].routes[0];
    assert_eq!(
        route.target,
        automation::AutoTarget::StripPan(s.id),
        "the modulator rides the new strip"
    );
    assert_eq!(
        p.strips.len(),
        before.strips.len(),
        "a blank strip's place taken"
    );
    let ids: Vec<u64> = p
        .tracks
        .iter()
        .map(|t| t.id.0)
        .chain(p.strips.iter().map(|s| s.id.0))
        .chain(
            p.strips
                .iter()
                .flat_map(|s| s.effects.iter().map(|e| e.id.0)),
        )
        .collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "ids unique");
    // Undo: the project exactly as it was.
    b.undo();
    // Ids handed out are never handed out again (ADR 0004): all else as it was.
    let mut now = b.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before);
    // Without a return of that name: inserted, the send left out and said.
    let mut c = app_in(dir.path(), None);
    assert!(c.insert_track_template(&path, 0));
    let t = &c.project().tracks[0];
    assert!(c
        .project()
        .strip(t.strip.unwrap())
        .unwrap()
        .sends
        .is_empty());
    assert!(c.status.text.contains("Reverb"), "{}", c.status.text);
}

#[test]
fn a_track_template_is_offered_in_the_picker_and_the_palette() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), Some(lead_project()));
    let lead = a.project().tracks[0].id;
    let path = a
        .workflow
        .store
        .save_track(a.project(), lead, "Lead")
        .unwrap();
    a.workflow.templates_changed();
    let entries = cantus_ui::views::palette::entries(&mut a);
    let e = entries
        .iter()
        .find(|e| e.target == Target::TrackTemplate(path.clone()))
        .expect("in the palette");
    assert_eq!(e.kind, Kind::TrackTemplate);
    let d = a
        .picker_view
        .catalog
        .iter()
        .find(|d| d.choice == cantus_ui::picker::PickChoice::TrackTemplate(path.clone()))
        .cloned()
        .expect("in the picker's catalogue");
    assert_eq!(d.source, cantus_ui::picker_catalog::Source::Template);
    let shelves = cantus_ui::picker_query::shelves(
        &a.picker_view.catalog,
        Some(cantus_ui::picker_catalog::Role::Instrument),
    );
    assert!(shelves.contains(&cantus_ui::picker_query::Shelf::Source(
        cantus_ui::picker_catalog::Source::Template
    )));
    let n = a.project().tracks.len();
    // From the picker (no place said): after the selected track (review 4).
    a.selected_track = Some(lead);
    a.apply_pick(
        cantus_ui::picker::PickPurpose::AddInstrument { index: None },
        d.choice.clone(),
    );
    assert_eq!(a.project().tracks.len(), n + 1);
    assert_eq!(a.project().tracks[1].name, "Lead", "after the selected one");
    // Never swapped in for a track's instrument.
    assert!(!cantus_ui::swap::available(
        &cantus_ui::picker::PickPurpose::ReplaceInstrument { track: lead },
        &d.choice
    ));
    // Deleted (the start page's ×): gone from the store, and from the picker next frame.
    a.workflow.store.delete(&path).unwrap();
    a.workflow.templates_changed();
    cantus_ui::workflow::sync_templates(&mut a);
    assert!(a.workflow.store.tracks().is_empty());
    let entries = cantus_ui::views::palette::entries(&mut a);
    assert!(!entries.iter().any(|e| e.kind == Kind::TrackTemplate));
}

#[test]
fn opened_and_saved_projects_become_recent() {
    let _l = lang();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let file = dir.path().join("first.cantus");
    a.save_to_path(&file);
    let second = dir.path().join("second.cantus");
    a.save_to_path(&second);
    a.open(&file);
    let r = &a.workflow.recent.projects;
    assert_eq!(r.len(), 2);
    assert!(r[0].ends_with("first.cantus") && r[1].ends_with("second.cantus"));
    // A recent file that is gone leaves the list when it is opened.
    std::fs::remove_file(&second).unwrap();
    a.open(&second);
    assert!(!a
        .workflow
        .recent
        .projects
        .iter()
        .any(|p| p.ends_with("second.cantus")));
    // The palette lists what is there.
    let entries = cantus_ui::views::palette::entries(&mut a);
    assert!(entries.iter().any(|e| e.kind == Kind::Recent));
}

/// ADR 0069 meets ADR 0072: a track template with an arpeggiator and a modulator on one of
/// its parameters, inserted twice, gives two chains of their own — distinct ids, each
/// modulator riding its own track's arpeggiator — and each insert undoes as one step.
#[test]
fn a_track_template_carries_its_note_effects_with_ids_of_their_own() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut p = cantus_ui::starter_project();
    let track = p.tracks[0].id;
    let arp_id = cantus_core::notefx::NoteFxId(p.alloc_id());
    let arp = NoteEffect::new(arp_id, NoteFxType::Arp);
    let key = arp.kind.params()[0].key.to_string();
    let mid = ModulatorId(p.alloc_id());
    let mut m = Modulator::new(mid, ModKind::Lfo);
    m.routes.push(ModRoute {
        target: automation::AutoTarget::NoteFx {
            track,
            effect: arp_id,
            key: key.clone(),
        },
        depth: 0.3,
    });
    let t = p.track_mut(track).unwrap();
    t.note_fx.push(arp);
    t.modulators.push(m);
    let mut a = app_in(dir.path(), Some(p));
    let path = a
        .workflow
        .store
        .save_track(a.project(), track, "Arp lead")
        .unwrap();
    let n = a.project().tracks.len();
    assert!(a.insert_track_template(&path, n));
    assert!(
        a.insert_track_template(&path, n + 1),
        "the second insert is not refused"
    );
    let p = a.project();
    let (one, two) = (&p.tracks[n], &p.tracks[n + 1]);
    let mut ids: Vec<u64> = p
        .tracks
        .iter()
        .flat_map(|t| t.note_fx.iter().map(|e| e.id.0))
        .collect();
    assert_eq!(ids.len(), 3, "the source's and two copies");
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "every chain's ids its own");
    for t in [one, two] {
        assert_eq!(t.note_fx.len(), 1);
        assert_eq!(t.note_fx[0].kind.kind(), NoteFxType::Arp);
        assert_eq!(
            t.modulators[0].routes[0].target,
            automation::AutoTarget::NoteFx {
                track: t.id,
                effect: t.note_fx[0].id,
                key: key.clone(),
            },
            "the modulator rides its own track's arpeggiator"
        );
    }
    assert_ne!(one.modulators[0].id, two.modulators[0].id);
    let steps = a.bus.history().len();
    a.undo();
    assert_eq!(a.bus.history().len(), steps - 1);
    assert_eq!(a.project().tracks.len(), n + 1, "one step, one track");
}

/// The test CLAP plug-in, built where the host's tests build it.
fn test_gain() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

/// Review 2: a template keeps a hosted plug-in as it is now — its state asked for and
/// waited for, as a save does — never the project's last copy of it.
#[test]
fn a_template_keeps_a_hosted_plug_in_as_it_is_now() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    cantus_ui::views::plugins::demo_setup(&mut a, &test_gain(), false);
    let info = a
        .plugins
        .db
        .effects()
        .first()
        .map(|p| (*p).clone())
        .unwrap();
    let strip = a.project().tracks[0].strip.unwrap();
    a.add_plugin(strip, &info);
    a.sync_engine();
    let effect = a
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .iter()
        .find(|e| e.spec.external.is_some())
        .map(|e| e.id)
        .unwrap();
    a.set_plugin_param(strip, effect, 0, -12.0, false);
    a.sync_engine();
    let state_in = |p: &Project| {
        p.strip(strip)
            .and_then(|s| s.effect(effect))
            .and_then(|e| e.spec.external.as_ref())
            .map(|x| x.state.bytes().to_vec())
            .unwrap_or_default()
    };
    let now = state_in(&a.plugins.project_for_save(a.bus.project()));
    assert_ne!(now, state_in(a.project()), "the project's copy is older");
    let track = a.project().tracks[0].id;
    cantus_ui::views::template_name::begin(
        &mut a,
        cantus_ui::views::template_name::Keep::Track(track),
        "Gain lead".into(),
    );
    let t0 = std::time::Instant::now();
    while a.workflow.gathering.is_some() {
        assert!(t0.elapsed() < std::time::Duration::from_secs(20));
        a.plugins.idle(&mut a.bus, &mut a.engine);
        cantus_ui::views::template_name::poll(&mut a, false);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let path = a.workflow.store.tracks()[0].path.clone();
    let kept = cantus_ui::templates::load_track_template(&path).unwrap();
    assert_eq!(state_in(&kept), now, "as it is now");
}

/// Review 3: a template's route to a strip with no name is not kept (its number means
/// another strip elsewhere), and no route lands on the blank strip the inserted one
/// replaces — every send and output names a strip that is there after the insert.
#[test]
fn a_track_template_never_routes_by_number_or_to_the_strip_it_replaces() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut p = cantus_ui::starter_project();
    let lead = p.tracks[0].id;
    let own = p.tracks[0].strip.unwrap();
    // A send into the second strip, which has no name and no track: "2" by number.
    let unnamed = p.strips[1].id;
    p.strip_mut(own).unwrap().sends.push(Send::new(unnamed));
    p.strip_mut(own).unwrap().output = Some(unnamed);
    let a = app_in(dir.path(), Some(p));
    let path = a
        .workflow
        .store
        .save_track(a.project(), lead, "Lead")
        .unwrap();
    let tpl = cantus_ui::templates::load_track_template(&path).unwrap();
    assert_eq!(
        tpl.strips.len(),
        1,
        "no stand-in for a strip known only by its number"
    );
    // Into a fresh project, whose second strip is blank and would be "2".
    let mut b = app_in(dir.path(), None);
    let n = b.project().tracks.len();
    assert!(b.insert_track_template(&path, n));
    let p = b.project();
    let s = p.strip(p.tracks[n].strip.unwrap()).unwrap();
    assert!(
        s.sends.is_empty() && s.output.is_none(),
        "{:?} {:?}",
        s.sends,
        s.output
    );
    for st in &p.strips {
        for x in &st.sends {
            assert!(p.strip(x.to).is_some());
        }
        if let Some(o) = st.output {
            assert!(p.strip(o).is_some());
        }
    }
}

/// One frame of the workflow's panels.
fn frame(a: &mut CantusApp, ctx: &egui::Context) {
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        cantus_ui::workflow::frame(a, ui.ctx());
    });
    out.textures_delta.clear();
}

/// Review 5: the start page reads its lists when it opens and when a template is saved or
/// deleted, never each frame — a frame touches no disk.
#[test]
fn the_start_page_reads_the_disk_when_it_opens_not_each_frame() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    a.run_action(Action::NewFromTemplate);
    frame(&mut a, &ctx);
    let n = a.workflow.start.templates.len();
    // A template that lands on disk behind the page's back is not looked for each frame…
    let folder = dir.path().join("templates").join("projects");
    std::fs::create_dir_all(&folder).unwrap();
    cantus_project::save(&Starter::Empty.build(), &folder.join("Sneaky.cantus"), "t").unwrap();
    frame(&mut a, &ctx);
    assert_eq!(a.workflow.start.templates.len(), n, "no reading per frame");
    // …but one saved through Cantus is listed at once.
    a.workflow.templates_changed();
    frame(&mut a, &ctx);
    assert_eq!(a.workflow.start.templates.len(), n + 1);
}

/// Review 10: a track template saved over under its own name is offered as it is now.
#[test]
fn a_track_template_saved_over_is_offered_as_it_is_now() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let t = a.project().tracks[0].id;
    let what = |a: &mut CantusApp| {
        cantus_ui::workflow::sync_templates(a);
        cantus_ui::views::palette::entries(a);
        a.picker_view
            .catalog
            .iter()
            .find(|d| d.source == cantus_ui::picker_catalog::Source::Template)
            .map(|d| d.what.clone())
            .unwrap()
    };
    a.workflow.store.save_track(a.project(), t, "Lead").unwrap();
    a.workflow.templates_changed();
    let first = what(&mut a);
    a.exec(Command::SetInstrument {
        track: t,
        spec: Box::new(InstrumentSpec::Pulsus(PulsusParams::default())),
    });
    a.workflow.store.save_track(a.project(), t, "Lead").unwrap();
    a.workflow.templates_changed();
    let second = what(&mut a);
    assert_ne!(first, second, "{first} / {second}");
    assert!(second.contains("Pulsus"), "{second}");
}

/// Review 8: the recent list and the palette's uses are written whole, through a
/// temporary file that does not stay behind.
#[test]
fn the_workflow_files_are_written_whole() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = cantus_ui::recent::Recent::load(cantus_ui::recent::path(dir.path()));
    r.note(&dir.path().join("a.cantus"));
    let mut u = cantus_ui::palette_index::Uses::default();
    u.used("action:undo");
    let up = cantus_ui::palette_index::uses_path(dir.path());
    u.save(&up).unwrap();
    u.save(&up).unwrap();
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(cantus_ui::palette_index::Uses::load(&up), u);
    assert_eq!(
        cantus_ui::recent::Recent::load(cantus_ui::recent::path(dir.path()))
            .projects
            .len(),
        1
    );
}

/// Review 11: Ctrl+Shift+P closes the palette while its field has the keys.
#[test]
fn the_palettes_key_closes_it_while_its_field_has_the_keys() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_in(dir.path(), None);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    a.run_action(Action::CommandPalette);
    // Two frames: the field takes the focus.
    frame(&mut a, &ctx);
    frame(&mut a, &ctx);
    assert!(a.workflow.palette.is_open());
    let mods = egui::Modifiers {
        ctrl: true,
        shift: true,
        command: true,
        ..Default::default()
    };
    let input = egui::RawInput {
        events: vec![
            egui::Event::ModifiersChanged(mods),
            egui::Event::Key {
                key: egui::Key::P,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: mods,
            },
        ],
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| cantus_ui::workflow::frame(&mut a, ui.ctx()));
    out.textures_delta.clear();
    assert!(!a.workflow.palette.is_open(), "its key closed it");
}

/// A template writes no automation: a track saved in Write (and a project whose tracks
/// and master write) comes back reading, so the global arm cannot overwrite on Play.
#[test]
fn a_template_never_carries_an_automation_mode() {
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let mut p = lead_project();
    for tr in &mut p.tracks {
        tr.auto_mode = AutoMode::Write;
    }
    p.master.auto_mode = AutoMode::Latch;
    let lead = p.tracks[0].id;
    let a = app_in(dir.path(), Some(p));
    let path = a
        .workflow
        .store
        .save_track(a.project(), lead, "Writer")
        .unwrap();
    let tpl = cantus_ui::templates::load_track_template(&path).unwrap();
    assert_eq!(tpl.tracks[0].auto_mode, AutoMode::Read, "the track reads");
    let path = a
        .workflow
        .store
        .save_project(a.project(), None, "Writers")
        .unwrap();
    let back = cantus_project::load(&path, true).unwrap().project;
    assert!(back.tracks.iter().all(|t| t.auto_mode == AutoMode::Read));
    assert_eq!(back.master.auto_mode, AutoMode::Read, "the master reads");
    assert_eq!(
        a.project().tracks[0].auto_mode,
        AutoMode::Write,
        "the project itself keeps its mode"
    );
}

/// The starters' effects are chosen by hand (ADR 0072): every return is full wet as every
/// return Cantus makes, with the reverb picked for it; the mastering chain is the equaliser,
/// the multiband, the stereo tool, the maximiser and Speculum, each with its preset.
#[test]
fn the_starters_returns_are_full_wet_and_the_mastering_chain_has_its_presets() {
    use cantus_ui::templates::{BAND_REVERB, BEAT_REVERB, MASTERING_CHAIN};
    let _l = lang();
    i18n::set_thread_locale(Locale::En);
    let preset = |kind: EffectKind, name: &str| {
        cantus_ui::views::fx::factory_presets(kind)
            .iter()
            .find(|(k, _)| *k == name)
            .unwrap_or_else(|| panic!("{name}"))
            .1
    };
    // A return: a strip some send goes to.
    let returns = |p: &Project| -> Vec<StripId> {
        let mut r: Vec<StripId> = p
            .strips
            .iter()
            .flat_map(|st| st.sends.iter().map(|x| x.to))
            .collect();
        r.dedup();
        r
    };
    for s in Starter::ALL {
        let p = s.build();
        for id in returns(&p) {
            let st = p.strip(id).unwrap();
            assert!(
                !st.effects.is_empty(),
                "{s:?}: {} holds its effect",
                st.name
            );
            for e in &st.effects {
                assert_eq!(e.spec.get("mix"), 1.0, "{s:?}: {} full wet", st.name);
            }
        }
    }
    for (s, chosen) in [(Starter::Beat, BEAT_REVERB), (Starter::Band, BAND_REVERB)] {
        let p = s.build();
        let ret = p.strip(returns(&p)[0]).unwrap();
        let e = &ret.effects[0];
        assert_eq!(e.spec.kind, EffectKind::Space, "{s:?}");
        for (k, v) in preset(EffectKind::Space, chosen) {
            if *k != "mix" {
                assert_eq!(e.spec.get(k), *v, "{s:?}: {k}");
            }
        }
    }
    let m = Starter::Mastering.build();
    assert_eq!(m.master.effects.len(), MASTERING_CHAIN.len());
    for (e, (kind, name)) in m.master.effects.iter().zip(MASTERING_CHAIN) {
        assert_eq!(e.spec.kind, kind);
        if let Some(name) = name {
            let values = preset(kind, name);
            assert!(!values.is_empty(), "{name}");
            for (k, v) in values {
                assert_eq!(e.spec.get(k), *v, "{kind:?} {name}: {k}");
            }
        }
    }
    assert!(MASTERING_CHAIN[1].1 == Some("mb.preset.glue"), "the glue");
}

/// *Audio cores* and the line under it count alike — cores at work, the audio thread and
/// its helpers — so their engines agree for every count (round 31: "4 cores helping" under
/// "Automatic (6) · V6" showed two engines on one screen).
#[test]
fn the_cores_field_and_the_now_line_count_alike() {
    use cantus_ui::engines::Engine;
    use cantus_ui::views::settings_audio::{now_line, workers_count, workers_label};
    let _l = lang();
    for l in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(l);
        for helpers in 0..=24u32 {
            let cores = helpers + 1;
            assert_eq!(workers_count(Some(helpers)), cores);
            let e = Engine::of(cores);
            let tail = format!(" · {} · {}", e.name(), e.model());
            let field = workers_label(Some(helpers));
            for parallel in [true, false] {
                let now = now_line(helpers, parallel);
                assert!(now.ends_with(&tail), "{l:?} {helpers}: {now} | {field}");
                assert!(
                    now.starts_with(&format!("{cores} ")),
                    "{l:?}: the line counts cores: {now}"
                );
            }
            assert!(field.ends_with(&tail), "{l:?} {helpers}: {field}");
            assert!(field.starts_with(&cores.to_string()), "{l:?}: {field}");
        }
        let auto = workers_label(None);
        let e = Engine::of(workers_count(None));
        assert!(auto.ends_with(&format!(" · {} · {}", e.name(), e.model())));
        assert!(
            auto.contains(&format!("({})", workers_count(None))),
            "{auto}"
        );
    }
}

/// The real-time priority line in every form a system gives, in each language's words:
/// no doubled words, no placeholder left, nothing of the system's English in Spanish.
#[test]
fn the_real_time_priority_line_reads_in_every_form() {
    use cantus_rt::thread::{Grant, MmcssPriority, PriorityReport, Refusal};
    use cantus_rt::Promotion;
    let _l = lang();
    let forms = [
        None,
        Some(Promotion::Granted(Grant::Mmcss {
            task: "Pro Audio",
            priority: MmcssPriority::High,
            task_index: 1,
        })),
        Some(Promotion::Granted(Grant::TimeConstraint {
            period_ns: 5_333_000,
            computation_ns: 2_666_000,
            joined_workgroup: true,
        })),
        Some(Promotion::Granted(Grant::TimeConstraint {
            period_ns: 5_333_000,
            computation_ns: 2_666_000,
            joined_workgroup: false,
        })),
        Some(Promotion::Granted(Grant::ThreadPriority { level: 15 })),
        Some(Promotion::Granted(Grant::RealTime {
            fifo: true,
            priority: 50,
            via_rtkit: true,
        })),
        Some(Promotion::Granted(Grant::RealTime {
            fifo: false,
            priority: 40,
            via_rtkit: false,
        })),
        Some(Promotion::LeftAsIs(
            "driver-owned thread: real-time already, left alone",
        )),
        Some(Promotion::LeftAsIs(
            "driver-owned thread: SCHED_FIFO, left alone",
        )),
        Some(Promotion::LeftAsIs(
            "background threads run at normal priority",
        )),
        Some(Promotion::LeftAsIs(
            "a spinning thread stays at normal priority: RLIMIT_RTTIME would kill it",
        )),
        Some(Promotion::Refused(Refusal {
            step: "AvSetMmThreadCharacteristicsW",
            code: 1552,
        })),
        Some(Promotion::Refused(Refusal {
            step: "no MMCSS",
            code: 0,
        })),
    ];
    let spinning = Promotion::LeftAsIs(
        "a spinning thread gets no time constraint: macOS demotes threads that overrun it",
    );
    let pool = [
        vec![],
        vec![Promotion::Granted(Grant::ThreadPriority { level: 15 }); 4],
        vec![
            Promotion::Granted(Grant::ThreadPriority { level: 15 }),
            spinning,
        ],
    ];
    for l in [Locale::En, Locale::Es] {
        i18n::set_thread_locale(l);
        let english: &[&str] = match l {
            Locale::En => &[],
            _ => &[
                "refused",
                "priority",
                "driver",
                "thread",
                "time constraint",
                "none",
                "not started",
            ],
        };
        let thread = if l == Locale::En {
            "audio thread"
        } else {
            "hilo de audio"
        };
        for cb in forms {
            for workers in &pool {
                let report = PriorityReport {
                    callback: cb,
                    workers: workers.clone(),
                };
                let (line, warn) = cantus_ui::audio_setup::realtime_words(&report);
                assert_eq!(line.matches(thread).count(), 1, "{l:?}: {line}");
                assert!(!line.contains('{'), "{l:?}: {line}");
                for w in english {
                    assert!(!line.contains(w), "{l:?}: English {w:?} in {line}");
                }
                assert_eq!(warn, cb.is_some_and(|p| !p.is_realtime()), "{line}");
            }
        }
    }
}

/// Settings ▸ Audio reads whole in both languages, in a roomy window and a small one: no
/// text runs past the edge it is drawn within (the real-time line's second line ran off to
/// the right), and nothing is cut to "…" (the record offset's caption was "CORR…").
#[test]
fn the_audio_page_reads_whole_in_both_languages() {
    use cantus_ui::views::{settings_audio, workspace_settings};
    type Seen = Vec<(String, bool, egui::Rect, egui::Rect)>;
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Seen) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_string(),
                t.galley.elided,
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let _l = lang();
    for l in [Locale::En, Locale::Es] {
        for size in [egui::vec2(1500.0, 940.0), egui::vec2(1100.0, 760.0)] {
            i18n::set_thread_locale(l);
            let dir = tempfile::tempdir().unwrap();
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
            let mut a = app_in(dir.path(), Some(cantus_ui::starter_project()));
            settings_audio::demo_setup(&mut a);
            workspace_settings::open(&mut a, workspace_settings::SettingsPage::Audio);
            let mut texts: Seen = Vec::new();
            for frame in 0..4 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    time: Some(frame as f64 / 60.0),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let c = ui.ctx().clone();
                    workspace_settings::window(&mut a, &c);
                });
                out.textures_delta.clear();
                texts.clear();
                for c in &out.shapes {
                    walk(&c.shape, c.clip_rect, &mut texts);
                }
            }
            assert!(texts.len() > 20, "the page was drawn");
            // The buttons that open a dialog end in "…" by their words, not by a cut.
            let dialogs = ["devices.pop.assistant", "devices.pop.routing"].map(i18n::t);
            assert!(
                texts
                    .iter()
                    .any(|x| x.0 == i18n::t("devices.offset.cell").to_uppercase()),
                "the record offset's caption whole"
            );
            for (text, elided, rect, clip) in &texts {
                let at = format!("{l:?} {size:?}: {text:?} at {rect:?} in {clip:?}");
                let cut = text.contains('…') && !dialogs.contains(text);
                assert!(!elided && !cut, "cut: {at}");
                // Only what lies inside the clip's height (a scrolled page cuts its rows).
                if rect.bottom() > clip.top() && rect.top() < clip.bottom() {
                    assert!(
                        rect.right() <= clip.right() + 1.0 && rect.left() >= clip.left() - 1.0,
                        "past the edge: {at}"
                    );
                }
            }
        }
    }
}
