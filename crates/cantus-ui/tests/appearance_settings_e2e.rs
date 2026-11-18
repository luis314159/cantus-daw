//! Every choice on Settings ▸ Appearance takes effect where it says, and is kept (beta.14,
//! the owner, after *Faders* did nothing: "check that other options are not like this").
//! Its own test binary: a look, a language and the painters' globals are the process's.
//!
//! Through `CantusApp::assemble`, table-driven: for each row of choices on the page — the
//! mode, the language, the interface's size, the units' faces, the cables, the faders, the
//! instruments' pictures, and *Layout and controls*' five — every choice is clicked the way
//! a user does, and the state its painters read (not the setting the click wrote) is
//! asked: each choice gives its own (the mode's *System* follows the system, so it is
//! only asked to be one of the two). Then a restart over the same folder: each row's last
//! choice is still what the painters read.

mod paths_rig;

use cantus_ui::views::workspace_settings::{self, SettingsPage};
use egui::Rect;
use paths_rig::*;

/// What the painters read for a row, in words (compared, never parsed).
type Probe = fn(&Rig) -> String;

/// One row of choices: its label's key, its choices' count, and its painters' state.
struct Row {
    label: &'static str,
    choices: usize,
    /// How many of its choices must read differently (*System* is one of Night or Paper).
    distinct: usize,
    probe: Probe,
}

fn rows() -> Vec<Row> {
    use cantus_ui::views::owner_look;
    vec![
        Row {
            label: "topbar.look.mode",
            choices: 3,
            distinct: 2,
            probe: |_| format!("dark {}", cantus_ui::kit::Style::current().t.dark),
        },
        Row {
            label: "settings.ui_scale",
            choices: 9,
            distinct: 9,
            probe: |r| format!("zoom {:.2}", r.ctx.zoom_factor()),
        },
        Row {
            label: "skin.settings.faces",
            choices: 2,
            distinct: 2,
            probe: |_| format!("faces {}", cantus_ui::kit::skin::enabled()),
        },
        Row {
            label: "cables.style",
            choices: 5,
            distinct: 4,
            probe: |r| {
                let sys = cantus_ui::kit::Style::current().t.system;
                format!("{:?}", r.app.settings.cable_style_in(&sys))
            },
        },
        Row {
            label: "look.fader",
            choices: 3,
            distinct: 3,
            probe: |_| format!("{:?}", cantus_ui::kit::fader::cap()),
        },
        Row {
            label: "scene.pictures",
            choices: 2,
            distinct: 2,
            probe: |_| format!("{:?}", cantus_ui::scene::backend::current()),
        },
        Row {
            label: "owner_look.maximise",
            choices: 2,
            distinct: 2,
            probe: |r| format!("{:?}", r.app.workspace.maximise_keeps()),
        },
        Row {
            label: "owner_look.topbar",
            choices: 4,
            distinct: 4,
            probe: |r| {
                format!(
                    "typing {} arm {}",
                    owner_look::typing_key_shown(),
                    r.app.settings.look.topbar_automation
                )
            },
        },
        Row {
            label: "owner_look.dock",
            choices: 2,
            distinct: 2,
            probe: |r| format!("{:?}", r.app.settings.look.mixer_dock),
        },
        Row {
            label: "owner_look.values",
            choices: 2,
            distinct: 2,
            probe: |_| format!("{:?}", owner_look::value_style()),
        },
        Row {
            label: "owner_look.mute",
            choices: 2,
            distinct: 2,
            probe: |_| {
                let grey = egui::Color32::from_gray(128);
                let accent = egui::Color32::from_rgb(1, 2, 3);
                format!("{:?}", owner_look::mute_fill(grey, accent))
            },
        },
    ]
}

/// Settings ▸ Appearance open, the row labelled `label` scrolled into view: its choices'
/// rect.
fn find(r: &mut Rig, label: &str) -> Rect {
    workspace_settings::open(&mut r.app, SettingsPage::Appearance);
    r.settle();
    let words = cantus_ui::i18n::t(label);
    for _ in 0..40 {
        let window = r
            .ctx
            .memory(|m| m.area_rect(egui::Id::new("cantus-settings")))
            .expect("Settings is open");
        let body = Rect::from_min_max(
            egui::pos2(window.left() + 200.0, window.top() + 50.0),
            egui::pos2(window.right(), window.bottom() - 56.0),
        );
        if let Some(at) = r
            .texts
            .iter()
            .rev()
            .find(|t| t.text.trim() == words && t.seen == t.rect && body.contains_rect(t.rect))
            .map(|t| t.rect)
        {
            let row = Rect::from_min_max(
                egui::pos2(at.right(), at.top() - 6.0),
                egui::pos2(window.right(), at.bottom() + 6.0),
            );
            if let Some(w) = r
                .widgets_in(row)
                .into_iter()
                .filter(|w| w.left() > at.right() && body.contains_rect(*w))
                .max_by(|a, b| a.width().total_cmp(&b.width()))
            {
                return w;
            }
        }
        r.wheel(body.center(), -3.0, NONE);
    }
    panic!("the row “{words}” is not on Settings ▸ Appearance");
}

fn click_choice(r: &mut Rig, row: &Row, i: usize) {
    let seg = find(r, row.label);
    let at = cantus_ui::kit::segmented::segment(seg, row.choices, i);
    r.click(at.center());
    r.idle(3);
}

#[test]
fn every_appearance_choice_reaches_its_painters_and_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let mut r = Rig::in_dir(tmp.path(), Some(cantus_ui::starter_project()));
    let mut last: Vec<String> = Vec::new();
    for row in rows() {
        let mut seen: Vec<String> = Vec::new();
        for i in 0..row.choices {
            click_choice(&mut r, &row, i);
            seen.push((row.probe)(&r));
        }
        // The interface's size back to 100 %, the others left on their last choice.
        if row.label == "settings.ui_scale" {
            click_choice(&mut r, &row, 2);
        }
        let mut d = seen.clone();
        d.sort();
        d.dedup();
        assert!(
            d.len() >= row.distinct,
            "{}: its choices read {seen:?} — {} of {} differ",
            row.label,
            d.len(),
            row.distinct
        );
        last.push((row.probe)(&r));
    }
    // The language: Español paints the page in Spanish; English back.
    let lang = Row {
        label: "menu.language",
        choices: cantus_ui::i18n::Locale::ALL.len(),
        distinct: 2,
        probe: |_| format!("{:?}", cantus_ui::i18n::locale()),
    };
    let es = cantus_ui::i18n::Locale::ALL
        .iter()
        .position(|l| l.code() == "es")
        .expect("Spanish is offered");
    let en = cantus_ui::i18n::Locale::ALL
        .iter()
        .position(|l| l.code() == "en")
        .expect("English is offered");
    click_choice(&mut r, &lang, es);
    assert_eq!(cantus_ui::i18n::locale().code(), "es");
    r.idle(2);
    let spanish = cantus_ui::i18n::t("settings.appearance");
    assert!(
        r.text(&spanish).is_some(),
        "the page says “{spanish}” in Spanish"
    );
    click_choice(&mut r, &lang, en);
    assert_eq!(cantus_ui::i18n::locale().code(), "en");
    // A restart over the same folder: each row's last choice is what its painters read.
    r.app.show_settings = false;
    r.idle(2);
    r.close_window();
    drop(r);
    let again = Rig::in_dir(tmp.path(), Some(cantus_ui::starter_project()));
    for (row, was) in rows().iter().zip(&last) {
        assert_eq!(
            &(row.probe)(&again),
            was,
            "{}: not kept over a restart",
            row.label
        );
    }
}
