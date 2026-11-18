//! The *Faders* choice reaches every fader (beta.14, the owner: "the fader selector does
//! nothing"). Round 39 redrew the desk's faders (`mixer_look::cap`) and the recessed looks'
//! (Slate's family, the default) without reading the choice. Its own test binary: a look and
//! the cap are installed for the process.
//!
//! Through `CantusApp::assemble`, in Slate Paper (the owner's light look), Slate Night and
//! Chalk (`paper`, a classic look): Settings ▸ Appearance ▸ *Faders* is clicked the way a user
//! does, to each of *The theme's*, *Classic* and *Metal*; then
//!
//! - every cap the desk's strips, its VCA and an instrument's own mixer (Tympanum's) draw is
//!   the chosen one (draw-level evidence: `kit::fader::record`), and the pixels of a strip's
//!   and the VCA's fader differ between the three (pixel evidence);
//! - after a restart over the same folder (`Rig::in_dir`), the choice is still there and the
//!   desk still draws it.

mod paths_rig;

use cantus_core::vca::StripKind;
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, StripId, TrackId};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::kit::fader::{self, FaderCap, Host};
use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::{self, SettingsPage};
use egui::Rect;
use paths_rig::*;

/// A song with Tympanum on a track, a VCA its strip follows, and the master.
fn song() -> (Project, TrackId, StripId, StripId) {
    let mut p = cantus_ui::welcome_project();
    let mut t = p.make_track("Drums");
    let strip = p.make_strip("Drums");
    let member = strip.id;
    t.strip = Some(strip.id);
    p.strips.push(strip);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
    let track = t.id;
    p.tracks.push(t);
    let mut v = p.make_strip("Drums VCA");
    v.kind = StripKind::Vca;
    let vca = v.id;
    p.strips.push(v);
    if let Some(s) = p.strips.iter_mut().find(|s| s.id == member) {
        s.vcas.push(vca);
    }
    (p, track, member, vca)
}

/// Settings ▸ Appearance ▸ Faders, segment `i` clicked; Settings closed again.
fn choose(r: &mut Rig, c: FaderCap) {
    workspace_settings::open(&mut r.app, SettingsPage::Appearance);
    r.idle(3);
    let tip = cantus_ui::i18n::t("look.fader.tip");
    let label = cantus_ui::i18n::t("look.fader");
    let mut seg = None;
    // The row may lie below the page's fold: the wheel brings it up.
    for _ in 0..40 {
        let window = r
            .ctx
            .memory(|m| m.area_rect(egui::Id::new("cantus-settings")))
            .expect("Settings is open");
        if let Some(at) = r
            .texts
            .iter()
            .rev()
            .find(|t| t.text.trim() == label && t.seen == t.rect && window.contains_rect(t.rect))
            .map(|t| t.rect)
        {
            let row = Rect::from_min_max(
                egui::pos2(at.right(), at.top() - 6.0),
                egui::pos2(window.right(), at.bottom() + 6.0),
            );
            seg = r
                .widgets_in(row)
                .into_iter()
                .filter(|w| w.left() > at.right())
                .max_by(|a, b| a.width().total_cmp(&b.width()));
            if let Some(w) = seg {
                assert_eq!(r.hint_at(w.center()).as_deref(), Some(tip.as_str()));
                break;
            }
        }
        r.wheel(window.center() + egui::vec2(120.0, 0.0), -3.0, NONE);
    }
    let seg = seg.expect("the Faders row is on Settings ▸ Appearance");
    let i = FaderCap::ALL.iter().position(|x| *x == c).unwrap();
    let at = cantus_ui::kit::segmented::segment(seg, FaderCap::ALL.len(), i);
    r.click(at.center());
    assert_eq!(r.app.appearance.fader, c, "the click chose {c:?}");
    r.key(egui::Key::Escape, NONE);
    r.idle(3);
    assert!(!r.app.show_settings, "Settings closed");
}

/// The desk shown: the caps it drew, and the pixels of the member's and the VCA's faders.
fn desk(r: &mut Rig, member: StripId, vca: StripId) -> (Vec<fader::Drawn>, Vec<[u8; 4]>) {
    r.app.workspace.reveal(Pane::Mixer);
    r.idle(3);
    fader::record(true);
    r.idle(1);
    let log = fader::take_log();
    fader::record(false);
    let c = r.canvas();
    let ppp = r.ctx.pixels_per_point();
    let mut px = Vec::new();
    for s in [member, vca] {
        let f: Rect = mixer::control_rect(&r.ctx, Some(s), Control::Fader)
            .unwrap_or_else(|| panic!("{s:?}'s fader is drawn"));
        let mut y = f.top();
        while y < f.bottom() {
            let mut x = f.left();
            while x < f.right() {
                px.push(c.pixel((x * ppp) as usize, (y * ppp) as usize));
                x += 1.0;
            }
            y += 1.0;
        }
    }
    (log, px)
}

/// Tympanum's own mixer shown: the caps it drew.
fn inst_mixer(r: &mut Rig, track: TrackId) -> Vec<fader::Drawn> {
    r.app.open_inst_editor(track);
    r.app.inst.view = cantus_ui::views::inst::View::Mixer;
    r.idle(4);
    fader::record(true);
    r.idle(1);
    let log = fader::take_log();
    fader::record(false);
    r.app.close_inst_editor();
    r.idle(2);
    log
}

fn all_are(log: &[fader::Drawn], host: Host, c: FaderCap, look: &str) {
    let of: Vec<_> = log.iter().filter(|d| d.0 == host).collect();
    assert!(
        !of.is_empty(),
        "{look}: no {host:?} fader was drawn: {log:?}"
    );
    let wrong: Vec<_> = of.iter().filter(|d| d.2 != c).collect();
    assert!(
        wrong.is_empty(),
        "{look}: {} of {} {host:?} caps are not {c:?}: {wrong:?}",
        wrong.len(),
        of.len()
    );
}

#[test]
fn the_faders_choice_reaches_the_desk_the_vcas_and_the_instruments_mixers_and_is_kept() {
    let (project, track, member, vca) = song();
    // A folder of the test's own: the second session reads what the first one kept.
    let tmp = tempfile::tempdir().unwrap();
    let mut r = Rig::in_dir(tmp.path(), Some(project));
    r.idle(4);
    for look in ["slate_paper", "slate_night", "paper"] {
        let light = look != "slate_night";
        r.app.appearance.mode = if light { Mode::Light } else { Mode::Dark };
        if light {
            r.app.appearance.light.base = format!("preset:{look}");
        } else {
            r.app.appearance.dark.base = format!("preset:{look}");
        }
        r.app.apply_appearance();
        r.idle(3);
        let mut seen: Vec<(FaderCap, Vec<[u8; 4]>)> = Vec::new();
        for c in FaderCap::ALL {
            choose(&mut r, c);
            let (log, px) = desk(&mut r, member, vca);
            r.shot(&format!("faders-{look}-{c:?}"));
            all_are(&log, Host::Desk, c, look);
            all_are(&log, Host::Vca, c, look);
            let inst = inst_mixer(&mut r, track);
            all_are(&inst, Host::InstMixer, c, look);
            seen.push((c, px));
        }
        for i in 0..seen.len() {
            for j in i + 1..seen.len() {
                let differ = seen[i]
                    .1
                    .iter()
                    .zip(&seen[j].1)
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(
                    differ > 20,
                    "{look}: the desk's faders look the same as {:?} and as {:?} ({differ} pixels differ)",
                    seen[i].0,
                    seen[j].0
                );
            }
        }
    }
    // Metal chosen, then a restart over the same folder: still Metal, on the desk too.
    choose(&mut r, FaderCap::Metal);
    r.close_window();
    drop(r);
    let (project, _, member, vca) = song();
    let mut again = Rig::in_dir(tmp.path(), Some(project));
    assert_eq!(again.app.appearance.fader, FaderCap::Metal, "kept");
    assert_eq!(fader::cap(), FaderCap::Metal, "applied at start");
    let (log, _) = desk(&mut again, member, vca);
    all_are(&log, Host::Desk, FaderCap::Metal, "after a restart");
    all_are(&log, Host::Vca, FaderCap::Metal, "after a restart");
}
