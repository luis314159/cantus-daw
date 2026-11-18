//! *Route the selected to a new bus* (ADR 0150) the way a user does it, through
//! `CantusApp::assemble` over the real engine, on a 2 880 × 1 800 screen at the system's
//! 100 % and 200 %:
//!
//! - on the desk: a strip's head clicked, another Ctrl+clicked, **Ctrl+G** — one new strip
//!   right after them, *Bus 1*, fed by both, and their tracks in an arrangement folder
//!   named *Bus 1* (ADR 0150 §5); the band heard through it (its mute silences them, its
//!   unmute gives them back), and one undo gives the project back to the bit, routing and
//!   folders both;
//! - in the arrangement: a track's header clicked, another Ctrl+clicked (both lit), the
//!   header's menu → *Route the selected to a new bus* — the same, as one step of undo.

mod paths_rig;

use cantus_core::{Command, Project, StripId};
use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::workspace::Pane;
use egui::{Key, Vec2};
use paths_rig::*;

/// The band the preset browser plays under a bus: drums, bass and keys, each on its strip.
fn band() -> Project {
    cantus_ui::preset_audition::source_project(cantus_ui::preset_tags::Role::Bus)
}

/// The loudest sample the engine's output carries over `blocks` blocks of 512.
fn heard(proc: &mut cantus_engine::Processor, blocks: usize) -> f32 {
    let mut out = vec![0f32; 512 * 2];
    let mut most = 0f32;
    for _ in 0..blocks {
        out.iter_mut().for_each(|x| *x = 0.0);
        proc.process(&mut out, 2);
        most = out.iter().fold(most, |m, x| m.max(x.abs()));
    }
    most
}

fn rig(native: f32) -> (Rig, cantus_engine::Processor) {
    let (mut r, proc) = Rig::with_engine(band());
    r.screen = Some(Physical::maximised(Vec2::new(2880.0, 1800.0), native));
    r.idle(4);
    (r, proc)
}

/// The tracks' strips, the first two the sources; every other strip muted (the other
/// parts' and any return the band sends to), so what is heard is the sources alone.
fn sources(r: &mut Rig) -> (StripId, StripId) {
    let strips: Vec<StripId> = r
        .app
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.strip)
        .collect();
    assert!(
        strips.len() >= 3,
        "the band has a strip per part: {strips:?}"
    );
    assert!(strips[0] != strips[1]);
    let others: Vec<StripId> = r
        .app
        .project()
        .strips
        .iter()
        .map(|s| s.id)
        .filter(|s| *s != strips[0] && *s != strips[1])
        .collect();
    for s in others {
        r.app.exec(Command::SetStripMute { id: s, muted: true });
    }
    assert!(
        r.app.project().tracks.iter().all(|t| t.strip.is_some()),
        "every part on a strip: none reaches the master around them"
    );
    (strips[0], strips[1])
}

/// The same project to the bit, but for the id counter: a new strip's id is drawn before
/// its command runs, and undo never gives an id back (ids are never reused).
fn same(a: &Project, b: &Project) -> bool {
    let (mut a, mut b) = (a.clone(), b.clone());
    a.next_id = 0;
    b.next_id = 0;
    a == b
}

fn the_bus_is_made(r: &Rig, before: &Project, a: StripId, b: StripId) -> StripId {
    let p = r.app.project();
    assert_eq!(p.strips.len(), before.strips.len() + 1, "one new strip");
    let bus = r.app.selected_strip.expect("the bus is selected");
    let st = p.strip(bus).expect("the bus");
    assert_eq!(st.name, "Bus 1");
    assert_eq!(p.strip(a).unwrap().output, Some(bus));
    assert_eq!(p.strip(b).unwrap().output, Some(bus));
    assert_eq!(st.output, None, "the bus to the master");
    let last = p.strip_index(a).max(p.strip_index(b)).unwrap();
    assert_eq!(
        p.strip_index(bus),
        Some(last + 1),
        "right after the last source"
    );
    // ADR 0150 §5: the tracks reaching it in a folder named as it, the rest outside.
    let folder = p
        .folders
        .iter()
        .find(|f| f.name == "Bus 1")
        .expect("a folder named as the bus");
    for t in &p.tracks {
        let reaches = t.strip == Some(a) || t.strip == Some(b);
        assert_eq!(
            t.folder == Some(folder.id),
            reaches,
            "{}: in the bus's folder exactly when it reaches the bus",
            t.name
        );
    }
    bus
}

#[test]
fn ctrl_g_routes_the_selected_strips_to_a_new_bus_heard_through_it_and_undone_to_the_bit() {
    for native in [1.0f32, 2.0] {
        let (mut r, mut proc) = rig(native);
        r.app.workspace.reveal(Pane::Mixer);
        r.idle(4);
        let (a, b) = sources(&mut r);
        r.app.toggle_play();
        r.idle(2);
        assert!(
            heard(&mut proc, 60) > 1e-3,
            "{native}: the two sources play"
        );
        let before = r.app.project().clone();
        // Selected as a user selects: a click on one head, a Ctrl+click on the other.
        let head = |r: &Rig, s| mixer::control_rect(&r.ctx, Some(s), Control::Head).unwrap();
        let ha = head(&r, a);
        r.click(ha.center());
        let hb = head(&r, b);
        r.mods = CTRL;
        r.click(hb.center());
        r.mods = NONE;
        assert_eq!(
            r.app.strip_selection(),
            vec![a, b],
            "{native}: both selected"
        );
        r.key(Key::G, CTRL);
        r.idle(2);
        let bus = the_bus_is_made(&r, &before, a, b);
        r.shot(&format!("bus-desk-{:.0}", native * 100.0));
        // Heard through it: its mute silences both, one undo gives them back.
        assert!(
            heard(&mut proc, 30) > 1e-3,
            "{native}: still heard, through the bus"
        );
        r.app.exec(Command::SetStripMute {
            id: bus,
            muted: true,
        });
        // The app hands the engine its changes in its frame.
        r.idle(1);
        heard(&mut proc, 8);
        assert!(
            heard(&mut proc, 40) < 1e-6,
            "{native}: the bus muted, nothing of its sources is heard"
        );
        r.app.undo();
        r.idle(1);
        heard(&mut proc, 4);
        assert!(
            heard(&mut proc, 40) > 1e-3,
            "{native}: unmuted, heard again"
        );
        // One step of undo: the project as it was, to the bit, and the sources still heard.
        r.app.undo();
        r.idle(2);
        assert!(
            same(r.app.project(), &before),
            "{native}: undo restores the project exactly"
        );
        assert!(
            heard(&mut proc, 40) > 1e-3,
            "{native}: heard straight to the master"
        );
        r.app.redo();
        assert_eq!(
            r.app.project().strips.len(),
            before.strips.len() + 1,
            "redo"
        );
    }
}

#[test]
fn the_track_headers_select_several_and_their_menu_routes_them_to_a_new_bus() {
    for native in [1.0f32, 2.0] {
        let (mut r, _proc) = rig(native);
        r.app.workspace.reveal(Pane::Mixer);
        r.idle(4);
        let (a, b) = sources(&mut r);
        let before = r.app.project().clone();
        let (ta, tb) = {
            let t = &r.app.project().tracks;
            ((t[0].id, t[0].name.clone()), (t[1].id, t[1].name.clone()))
        };
        let leaf = r
            .leaf_of(Pane::Arrangement)
            .expect("the arrangement is shown");
        let name_a = r.must_in(&ta.1, leaf);
        r.click(name_a.right_center() + egui::vec2(15.0, 0.0));
        let name_b = r.must_in(&tb.1, leaf);
        r.mods = CTRL;
        r.click(name_b.right_center() + egui::vec2(15.0, 0.0));
        r.mods = NONE;
        assert_eq!(
            r.app.track_selection(),
            vec![ta.0, tb.0],
            "{native}: both selected"
        );
        assert!(
            r.app.track_selected(ta.0) && r.app.track_selected(tb.0),
            "both lit"
        );
        r.shot(&format!("bus-headers-selected-{:.0}", native * 100.0));
        // The header's menu, by the right button on one of them.
        r.right_click(name_b.right_center() + egui::vec2(15.0, 0.0));
        let words = cantus_ui::i18n::t("routing.menu.new_bus");
        let row = r.must(&words);
        r.shot(&format!("bus-header-menu-{:.0}", native * 100.0));
        r.click(row.center());
        r.idle(2);
        the_bus_is_made(&r, &before, a, b);
        r.shot(&format!("bus-headers-done-{:.0}", native * 100.0));
        r.app.undo();
        assert!(
            same(r.app.project(), &before),
            "{native}: one undo step, to the bit"
        );
        // A plain click selects one again.
        r.click(name_a.right_center() + egui::vec2(15.0, 0.0));
        assert_eq!(r.app.track_selection(), vec![ta.0]);
    }
}
