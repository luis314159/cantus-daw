//! The owner's daily click paths (round 39, ADR 0110), each driven through
//! `CantusApp::assemble` the way a person takes it and counted — clicks, keys and pointer
//! travel (`paths_rig`). Each test holds its path's count: a change that makes a daily
//! path longer fails here. The table, with FL Studio's and Ableton Live's ways beside, is
//! in `docs/round39/ux-b.md`; every path starts with the pointer in the middle of the
//! arrangement, at 1600 × 900, the welcome song open.
//!
//! `CANTUS_PATHS_SHOTS=<dir>` writes a picture of each path's last frame.

mod paths_rig;

use cantus_core::*;
use cantus_ui::picker_catalog::Role;
use cantus_ui::views::workspace::Pane;
use egui::{pos2, Key, Pos2, Rect};
use paths_rig::*;

/// Where a person works: the middle of the arrangement. Every path starts there.
const HOME: Pos2 = Pos2::new(700.0, 300.0);

/// What a path cost, printed for the table, and held: no more clicks or keys than `max`,
/// and travel within 10 % of it (a layout's pixel moves do not fail it).
fn held(name: &str, got: Count, max: Count) {
    println!("PATH {name}: {got}");
    assert!(
        got.clicks <= max.clicks && got.keys <= max.keys && got.travel <= max.travel * 1.1 + 1.0,
        "{name} got longer: {got}, held at {max}"
    );
}

fn c(clicks: u32, keys: u32, travel: f32) -> Count {
    Count {
        clicks,
        keys,
        travel,
    }
}

fn region(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
}

/// The device picker's grid of tiles.
fn picker_grid() -> Rect {
    region(330.0, 80.0, 1160.0, 830.0)
}

fn rig() -> Rig {
    let mut r = Rig::new(cantus_ui::welcome_project());
    r.idle(3);
    r
}

/// The arrangement's header row of track `i`.
fn header_row(r: &Rig, i: usize) -> Rect {
    let g = r
        .app
        .arrangement
        .geometry()
        .expect("the arrangement is drawn");
    let row = g.rows[i];
    region(0.0, row.top, g.left, row.top + row.height)
}

/// The desk's column of the strip whose name is painted as `name`.
fn strip_column(r: &Rig, name: &str) -> Rect {
    let n = r.must(name);
    region(n.center().x - 12.0, 490.0, n.center().x + 12.0, 880.0)
}

/// The add-track key at the top of the track headers: since round 39 (el-arrange) the plus
/// in the corner over the headers, in the ruler's row — the same click (the picker) and
/// right-click (every kind of track, the instruments chosen last).
fn add_track_key(r: &mut Rig) -> Rect {
    r.must_control(region(0.0, 44.0, 222.0, 166.0), "Add a track")
}

/// The device named `name` used once before (yesterday's session): first in the recent
/// list.
fn used_before(r: &mut Rig, role: Role, name: &str) {
    cantus_ui::quick_devices::warm(&mut r.app);
    let key = r
        .app
        .picker_view
        .catalog
        .iter()
        .find(|d| d.role == role && d.name == name)
        .map(|d| d.key.clone())
        .expect("in the catalogue");
    r.app.picker_view.prefs.used(&key);
}

// ---- 1. Add an instrument ----------------------------------------------------------------

/// The picker's way: the add-track key, a double-click on the tile (FL: + and a click in its
/// list, 2; Live: a double-click in the browser, 2).
#[test]
fn p01a_add_an_instrument_from_the_picker() {
    let mut r = rig();
    let before = r.app.project().tracks.len();
    r.begin(HOME);
    let plus = add_track_key(&mut r);
    r.click(plus.center());
    r.shot("p01a-picker");
    let tile = r.must_in("Chorda", picker_grid());
    r.double_click(tile.center());
    assert_eq!(r.app.project().tracks.len(), before + 1);
    held("p01a add an instrument (picker)", r.count, c(3, 0, 1800.0));
}

/// Round 39: the instruments used last are in the add-track key's right-click menu — two
/// clicks, the pointer staying where the key is.
#[test]
fn p01b_add_a_recent_instrument() {
    let mut r = rig();
    used_before(&mut r, Role::Instrument, "Chorda");
    let before = r.app.project().tracks.len();
    r.begin(HOME);
    let plus = add_track_key(&mut r);
    r.right_click(plus.center());
    r.shot("p01b-menu");
    let row = r.must("New track with Chorda");
    r.click(row.center());
    let p = r.app.project();
    assert_eq!(p.tracks.len(), before + 1);
    let new = p.tracks.iter().max_by_key(|t| t.id.0).unwrap();
    assert_eq!(
        new.instrument.native().map(|n| n.kind),
        Some(InstrumentKind::Chorda),
        "the new track plays Chorda"
    );
    held("p01b add a recent instrument", r.count, c(2, 0, 800.0));
}

/// The keyboard's way: the palette, the name, Enter.
#[test]
fn p01c_add_an_instrument_by_keyboard() {
    let mut r = rig();
    let before = r.app.project().tracks.len();
    r.begin(HOME);
    r.key(Key::P, ctrl_shift());
    r.type_text("chorda");
    r.key(Key::Enter, NONE);
    assert_eq!(r.app.project().tracks.len(), before + 1);
    held("p01c add an instrument (keyboard)", r.count, c(0, 8, 0.0));
}

// ---- 2. Add an effect to a strip ---------------------------------------------------------

/// The picker's way: select the strip, an empty insert row of the inspector, a double-click
/// on the tile (FL: select the strip, its slot, the plug-in in the list, 3).
#[test]
fn p02a_add_an_effect_from_the_picker() {
    let mut r = rig();
    r.begin(HOME);
    let col = strip_column(&r, "Kick");
    let name = r.control_named(col, "Strip 2");
    r.click(name.center());
    let slot = r.must_control(region(1320.0, 90.0, 1600.0, 880.0), "Add an insert effect");
    r.click(slot.center());
    let tile = r.must_in("Aequalis", picker_grid());
    r.double_click(tile.center());
    let s = r.app.project().tracks[1].strip.unwrap();
    assert_eq!(r.app.project().strip(s).unwrap().effects.len(), 1);
    held("p02a add an effect (picker)", r.count, c(4, 0, 3000.0));
}

/// Round 39: the effects inserted last head the strip's *Add effect ▸*, now near the top of
/// its right-click menu — two clicks and a hover, no selection first.
#[test]
fn p02b_insert_a_recent_effect() {
    let mut r = rig();
    used_before(&mut r, Role::Effect, "Comprimo");
    r.begin(HOME);
    let col = strip_column(&r, "Kick");
    let name = r.control_named(col, "Strip 2");
    r.right_click(name.center());
    let add = r.must(&cantus_ui::i18n::t("effect.add"));
    r.hover(add.center());
    r.idle(3);
    r.shot("p02b-menu");
    let row = r.must("Insert Comprimo");
    r.click(row.center());
    let s = r.app.project().tracks[1].strip.unwrap();
    let fx = &r.app.project().strip(s).unwrap().effects;
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].spec.kind, EffectKind::Compressor);
    // Round 39: `el-mixer`'s new desk moved the strip's name, so the
    // pointer's way to it is longer (measured 1 029 pt); the clicks are the same two.
    held("p02b insert a recent effect", r.count, c(2, 0, 1030.0));
}

// ---- 3–6. Piano roll, mute and solo, the loop, arm and record ---------------------------

/// A double-click on a clip opens its editor (FL's and Live's double-click).
#[test]
fn p03_open_the_piano_roll_on_a_clip() {
    let mut r = rig();
    r.app.workspace.reveal(Pane::Mixer);
    r.idle(2);
    r.begin(HOME);
    let clip = r.must("Motif");
    r.double_click(clip.center() + egui::vec2(30.0, 8.0));
    assert!(r.app.workspace.is_visible(Pane::PianoRoll));
    held("p03 piano roll on a clip", r.count, c(2, 0, 460.0));
}

/// The header's lamps: one click each (FL's and Live's).
#[test]
fn p04_mute_and_solo() {
    let mut r = rig();
    r.begin(HOME);
    let row = header_row(&r, 1);
    let m = r.must_control(row, "Mute");
    r.click(m.center());
    let s = r.must_control(row, "Solo");
    r.click(s.center());
    let t = &r.app.project().tracks[1];
    assert!(t.muted && t.soloed);
    held("p04 mute and solo a track", r.count, c(2, 0, 560.0));
}

/// Round 39: a loop drawn in the loop lane loops at once — one drag, where it took the drag
/// and the loop key (FL and Logic: the drawn region loops).
#[test]
fn p05_set_the_loop() {
    let mut r = rig();
    r.app.exec(Command::SetLoopEnabled { enabled: false });
    let steps = r.app.bus.history().len();
    r.begin(HOME);
    let g = r.app.arrangement.geometry().unwrap();
    let y = (g.ruler_top + g.loop_lane_bottom) * 0.5;
    let x = |beat: f32| g.left + (beat - g.scroll_beats) * g.px_per_beat;
    r.drag(pos2(x(4.0), y), pos2(x(12.0), y));
    let p = r.app.project();
    let l = p.loop_region.clone().expect("a loop region");
    assert_eq!((l.start, l.end), (Ticks::beats(4), Ticks::beats(12)));
    assert!(p.loop_enabled, "looping is on");
    assert_eq!(r.app.bus.history().len(), steps + 1, "one step of undo");
    r.app.undo();
    assert!(!r.app.project().loop_enabled, "undone together");
    held("p05 set the loop", r.count, c(1, 0, 640.0));
}

/// The header's arm lamp and R (FL: arm, record, play — 3; Live: arm and F9, 2).
#[test]
fn p06_arm_and_record() {
    let mut r = rig();
    r.begin(HOME);
    let row = header_row(&r, 0);
    let arm = r.must_control(row, "Arm");
    r.click(arm.center());
    assert!(r.app.project().tracks[0].armed);
    r.key(Key::R, NONE);
    assert!(r.app.is_recording());
    held("p06 arm and record", r.count, c(1, 1, 570.0));
}

// ---- 7–9. A send, an insert's editor, the tempo -----------------------------------------

/// The strip's routing dot, then the destination strip (FL: select the source, the
/// destination's arrow, 2).
#[test]
fn p07_route_a_send() {
    let mut r = rig();
    r.begin(HOME);
    // Round 39 (the owner's option C): the cables show on demand — the desk's Cables key,
    // then the strip's socket, then the destination.
    let cables = cantus_ui::views::mixer::title_rect(&r.ctx, "cables").expect("the Cables key");
    r.click(cables.center());
    let col = strip_column(&r, "Kick");
    let dot = r.must_control(col, "Drag to a strip");
    r.click(dot.center());
    let target = r.must("14");
    r.click(target.center() + egui::vec2(0.0, 40.0));
    let kick = r.app.project().tracks[1].strip.unwrap();
    assert_eq!(r.app.project().strip(kick).unwrap().sends.len(), 1);
    // Round 39: des-mixer's strips are wider (62 pt a pitch, was 48) and the cables show
    // on demand (the owner's option C): one click more, more travel.
    held("p07 route a send", r.count, c(3, 0, 2400.0));
}

/// Select the strip, click its insert in the inspector (FL: select the strip, its slot, 2).
#[test]
fn p08_open_an_inserts_editor() {
    let mut r = rig();
    let s = r.app.project().tracks[1].strip.unwrap();
    r.app.add_effect(s, EffectKind::Compressor);
    r.app.close_comp_editor();
    r.idle(3);
    r.begin(HOME);
    let col = strip_column(&r, "Kick");
    let name = r.control_named(col, "Strip 2");
    r.click(name.center());
    // Round 39: the inserts come first in the strip's sheet (des-mixer), higher up.
    let row = r.must_control(region(1320.0, 100.0, 1600.0, 600.0), "Open the editor");
    r.click(row.center());
    assert!(
        r.app.comp_editor.open.is_some(),
        "the compressor's editor is open"
    );
    // Round 39: the inserts lead the strip's sheet (des-mixer), further up the panel.
    held("p08 open an insert's editor", r.count, c(2, 0, 2100.0));
}

/// A drag up on the tempo's well (FL's drag; Live: click, type, Enter).
#[test]
fn p09_change_the_tempo() {
    let mut r = rig();
    let before = r.app.project().tempo_bpm;
    r.begin(HOME);
    // Round 39: the kit's top bar moved the tempo (and `el-topbar` is rebuilding the bar),
    // so the well is found by what it shows — its "BPM" unit — and by its help-line name,
    // never by a fixed rect: the control named exactly "Tempo" around its unit.
    let bpm = r.must("BPM");
    let around = Rect::from_min_max(
        egui::pos2(bpm.left() - 140.0, bpm.top() - 24.0),
        egui::pos2(bpm.right() + 40.0, bpm.bottom() + 24.0),
    );
    let well = r.control_named(around, "Tempo");
    r.drag(well.center(), well.center() - egui::vec2(0.0, 24.0));
    assert!(r.app.project().tempo_bpm > before, "faster");
    held("p09 change the tempo", r.count, c(1, 0, 440.0));
}

// ---- 10–13. Save, export, a preset, Settings ▸ Audio ------------------------------------

/// Ctrl+S (everyone's).
#[test]
fn p10_save() {
    let mut r = rig();
    let path = r.dir.join("song.cantus");
    r.app.save_to_path(&path);
    r.app.exec(Command::SetTempo { bpm: 128.0 });
    assert!(r.app.bus.is_dirty());
    r.begin(HOME);
    r.key(Key::S, CTRL);
    for _ in 0..400 {
        if !r.app.bus.is_dirty() {
            break;
        }
        r.idle(1);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!r.app.bus.is_dirty(), "saved");
    held("p10 save", r.count, c(0, 1, 0.0));
}

/// Ctrl+Shift+E opens the export dialog on the last choices; Enter is its *Export…* and the
/// system's save dialog takes one more Enter (not driven here: it is the system's window).
#[test]
fn p11_export() {
    let mut r = rig();
    r.begin(HOME);
    r.key(Key::E, ctrl_shift());
    assert!(r.app.export.dialog.is_some(), "the dialog is open");
    held("p11 export (to the dialog)", r.count, c(0, 1, 0.0));
}

/// Round 39: Ctrl+F opens the preset browser on the selected track's instrument; typing
/// finds, ↓ plays the next one (Live: Ctrl+F, the words, ↓). It was F8, the preset well,
/// *Browse presets…*.
#[test]
fn p12_find_a_preset_and_hear_it() {
    let mut r = rig();
    let t = r.app.project().tracks[0].id;
    r.app.apply_pick(
        cantus_ui::picker::PickPurpose::ReplaceInstrument { track: t },
        cantus_ui::picker::PickChoice::Instrument(InstrumentKind::Wavetable),
    );
    r.app.select_track(t);
    r.idle(3);
    r.begin(HOME);
    r.key(Key::F, CTRL);
    assert!(r.app.presets.open, "the preset browser is open");
    r.type_text("pad");
    r.key(Key::ArrowDown, NONE);
    // The audition follows once the selection rests 150 ms of the clock on the wall.
    std::thread::sleep(std::time::Duration::from_millis(250));
    r.idle(3);
    r.shot("p12-done");
    let a = &r.app.presets.audition;
    assert!(
        a.rendering() || a.until.is_some() || a.heard.is_some(),
        "a preset is heard"
    );
    held("p12 find a preset and hear it", r.count, c(0, 5, 0.0));
}

/// Round 39: Settings opens on the page chosen last, in the next session too — Ctrl+, is
/// Audio's for whoever goes there (FL's F10 and Live's Ctrl+, open where they were left).
#[test]
fn p13_settings_audio() {
    let tmp = tempfile::tempdir().unwrap();
    {
        let mut first = Rig::in_dir(tmp.path(), Some(cantus_ui::welcome_project()));
        first.key(Key::Comma, CTRL);
        let audio = first.must("Audio");
        first.click(audio.center());
        assert_eq!(
            first.app.settings_page.page,
            cantus_ui::views::workspace_settings::SettingsPage::Audio
        );
    }
    let mut r = Rig::in_dir(tmp.path(), Some(cantus_ui::welcome_project()));
    r.begin(HOME);
    r.key(Key::Comma, CTRL);
    r.shot("p13-done");
    assert!(r.app.show_settings);
    assert_eq!(
        r.app.settings_page.page,
        cantus_ui::views::workspace_settings::SettingsPage::Audio
    );
    held("p13 Settings > Audio", r.count, c(0, 1, 0.0));
}

/// The way before round 39, still there: the instrument's window (F8), its preset well,
/// *Browse presets…*, the words, ↓.
#[test]
fn p12a_find_a_preset_from_the_instruments_window() {
    let mut r = rig();
    let t = r.app.project().tracks[0].id;
    r.app.apply_pick(
        cantus_ui::picker::PickPurpose::ReplaceInstrument { track: t },
        cantus_ui::picker::PickChoice::Instrument(InstrumentKind::Wavetable),
    );
    r.idle(3);
    r.begin(HOME);
    r.key(Key::F8, NONE);
    let well = r.must_control(region(0.0, 60.0, 1600.0, 880.0), "Browse presets");
    r.click(well.center());
    let row = r.must("Browse presets…");
    r.click(row.center());
    assert!(r.app.presets.open, "the preset browser is open");
    r.type_text("pad");
    r.key(Key::ArrowDown, NONE);
    held(
        "p12a find a preset (instrument window)",
        r.count,
        c(2, 5, 1100.0),
    );
}
