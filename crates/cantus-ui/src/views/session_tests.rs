//! The Session pane's tests (ADR 0077): what the engine says folded into what the view
//! draws, a performance recorded, the edits and the keys, what the view tells the engine,
//! the layout's arithmetic, the grids and MIDI learn — and the pane drawn headless.

use super::*;
use cantus_core::session::{SessionCommand as S, SlotRef};
use cantus_core::Ticks;
use cantus_engine::messages::{ScenePhase, SessionEvent, SlotPhase};
use cantus_engine::EngineHandle;

fn app(tracks: usize, scenes: usize) -> CantusApp {
    let mut a = CantusApp::from_parts(
        Some(demo::project(tracks, scenes)),
        EngineHandle::dummy(),
        false,
    );
    a.session_view.record_sent();
    a
}

fn slot(a: &CantusApp, i: usize, j: usize) -> SlotRef {
    SlotRef::new(a.project().tracks[i].id, a.project().session.scenes[j].id)
}

fn sent(a: &CantusApp) -> Vec<SessionMsg> {
    a.session_view.sent.clone().unwrap_or_default()
}

fn feed(a: &mut CantusApp, _now: Ticks, events: Vec<SessionEvent>) {
    for e in events {
        a.session_view.live.apply(e);
    }
}

fn empty_slots(a: &CantusApp) -> Vec<SlotRef> {
    let (n, m) = (a.project().tracks.len(), a.project().session.scenes.len());
    (0..n)
        .flat_map(|i| (0..m).map(move |j| (i, j)))
        .map(|(i, j)| slot(a, i, j))
        .filter(|s| a.project().slot_clip(*s).is_none())
        .collect()
}

const BAR: Ticks = Ticks(cantus_core::PPQ * 4);

#[test]
fn the_engines_events_become_what_the_grid_draws() {
    let mut a = app(3, 3);
    let (s00, s01, s10) = (slot(&a, 0, 0), slot(&a, 0, 1), slot(&a, 1, 0));
    feed(
        &mut a,
        Ticks::ZERO,
        vec![
            SessionEvent::Slot {
                slot: s00,
                phase: SlotPhase::Queued { at: BAR },
            },
            SessionEvent::Slot {
                slot: s10,
                phase: SlotPhase::Playing {
                    since: Ticks::ZERO,
                    from: Ticks::ZERO,
                },
            },
            SessionEvent::Track {
                track: s10.track,
                arrangement: false,
            },
        ],
    );
    assert_eq!(
        a.session_view.live.phase(s00),
        SlotPhase::Queued { at: BAR }
    );
    assert!(a.session_view.live.blinking());
    assert!(a.session_view.live.holds(s10.track));
    assert_eq!(a.session_view.live.playing(), vec![s10]);
    // A clip that starts hands its track over: the one before is no longer playing.
    feed(
        &mut a,
        BAR,
        vec![SessionEvent::Slot {
            slot: s00,
            phase: SlotPhase::Playing {
                since: BAR,
                from: Ticks::ZERO,
            },
        }],
    );
    feed(
        &mut a,
        BAR * 2,
        vec![SessionEvent::Slot {
            slot: s01,
            phase: SlotPhase::Playing {
                since: BAR * 2,
                from: Ticks::ZERO,
            },
        }],
    );
    assert_eq!(a.session_view.live.phase(s00), SlotPhase::Stopped);
    assert_eq!(
        a.session_view.live.playing_on(s01.track).map(|x| x.0),
        Some(s01)
    );
    // Stopped: gone; the track still held until it goes back to its arrangement.
    feed(
        &mut a,
        BAR * 3,
        vec![SessionEvent::Slot {
            slot: s01,
            phase: SlotPhase::Stopped,
        }],
    );
    assert_eq!(a.session_view.live.playing_on(s01.track), None);
    feed(
        &mut a,
        BAR * 3,
        vec![SessionEvent::Track {
            track: s10.track,
            arrangement: true,
        }],
    );
    assert!(!a.session_view.live.holds(s10.track));
    // Scenes, and an overflow that asks for everything again.
    let sc = a.project().session.scenes[0].id;
    feed(
        &mut a,
        BAR * 3,
        vec![
            SessionEvent::Scene {
                scene: sc,
                phase: ScenePhase::Playing { since: BAR },
            },
            SessionEvent::Overflow,
        ],
    );
    assert_eq!(
        a.session_view.live.scene_phase(sc),
        ScenePhase::Playing { since: BAR }
    );
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert!(sent(&a).contains(&SessionMsg::Resend));
    assert!(
        !a.session_view.live.busy(),
        "the state is forgotten until it is sent again"
    );
}

#[test]
fn the_playing_position_is_drawn_from_the_event_and_the_transport() {
    let a = app(2, 2);
    let s = slot(&a, 0, 0);
    let c = a.project().slot_clip(s).unwrap().clone();
    let phase = SlotPhase::Playing {
        since: BAR,
        from: Ticks::ZERO,
    };
    let half = BAR + Ticks(c.pass().0 / 2);
    let p = input::grid_progress(&a, s, phase, half).unwrap();
    assert!((p - 0.5).abs() < 1e-3, "{p}");
    // A stopping clip goes on showing where it plays until its stop.
    let stopping = SlotPhase::Stopping {
        since: BAR,
        from: Ticks::ZERO,
        at: BAR * 9,
    };
    assert!(input::grid_progress(&a, s, stopping, half).is_some());
    assert_eq!(
        input::grid_progress(&a, s, SlotPhase::Queued { at: BAR }, half),
        None
    );
}

#[test]
fn launching_tells_the_engine_and_shows_the_press_at_once() {
    let mut a = app(2, 2);
    let s = slot(&a, 0, 0);
    exec(
        &mut a,
        S::SetSlotLaunch {
            slot: s,
            launch: cantus_core::session::LaunchSettings {
                velocity: 1.0,
                ..Default::default()
            },
        },
    );
    launch(&mut a, s, 0.5);
    release(&mut a, s);
    let sc = a.project().session.scenes[1].id;
    launch_scene(&mut a, sc, 1.0);
    stop_track(&mut a, s.track);
    stop_all(&mut a);
    back_to_arrangement(&mut a, None);
    set_arrangement_record(&mut a, true);
    assert_eq!(
        sent(&a),
        vec![
            SessionMsg::Launch {
                slot: s,
                velocity: 0.5
            },
            SessionMsg::Release { slot: s },
            SessionMsg::LaunchScene {
                scene: sc,
                velocity: 1.0
            },
            SessionMsg::StopTrack { track: s.track },
            SessionMsg::StopAll,
            SessionMsg::BackToArrangement { track: None },
            SessionMsg::SetArrangementRecord(true),
        ]
    );
    // The press shows queued before the conductor answers, and lets go unanswered.
    assert!(matches!(
        a.session_view.live.phase(s),
        SlotPhase::Queued { .. }
    ));
    assert!(matches!(
        a.session_view.live.scene_phase(sc),
        ScenePhase::Queued { .. }
    ));
    a.session_view
        .live
        .expire(std::time::Instant::now() + live::PRESS_SHOWN);
    assert_eq!(a.session_view.live.phase(s), SlotPhase::Stopped);
}

#[test]
fn recording_into_a_slot_needs_the_track_armed_and_a_second_press_ends_it() {
    let mut a = app(4, 3);
    let s = slot(&a, 0, 2);
    record(&mut a, s);
    assert!(sent(&a).is_empty(), "not armed: refused, and said");
    let t3 = slot(&a, 3, 2);
    assert!(a.project().track(t3.track).unwrap().armed);
    record(&mut a, t3);
    feed(
        &mut a,
        Ticks::ZERO,
        vec![SessionEvent::Slot {
            slot: t3,
            phase: SlotPhase::Recording { since: Ticks::ZERO },
        }],
    );
    record(&mut a, t3);
    assert_eq!(
        sent(&a),
        vec![
            SessionMsg::Record { slot: t3 },
            SessionMsg::StopRecording { slot: t3 }
        ]
    );
}

#[test]
fn the_keys_move_the_cursor_and_act_on_the_slot_only_while_the_pane_has_the_focus() {
    use crate::keymap::Action;
    let mut a = app(3, 3);
    a.session_view.cursor = Some(slot(&a, 0, 0));
    // Without the focus the arrows are the arrangement's.
    assert!(!action(&mut a, Action::NudgeRight));
    a.session_view.focused = true;
    assert!(action(&mut a, Action::NudgeRight));
    assert!(action(&mut a, Action::TransposeDown));
    assert_eq!(a.session_view.cursor, Some(slot(&a, 1, 1)));
    // Kept inside the grid.
    for _ in 0..5 {
        action(&mut a, Action::NudgeRight);
    }
    assert_eq!(a.session_view.cursor, Some(slot(&a, 2, 1)));
    // Without the focus a bare Enter is not the session's (a dialog's, the arrangement's).
    a.session_view.focused = false;
    assert!(!action(&mut a, Action::SessionLaunch));
    assert!(!action(&mut a, Action::SessionLaunchScene));
    assert!(sent(&a).is_empty());
    // With it, Enter launches the cursor's slot; Shift+Enter its scene.
    a.session_view.focused = true;
    assert!(action(&mut a, Action::SessionLaunch));
    assert!(action(&mut a, Action::SessionLaunchScene));
    let c = slot(&a, 2, 1);
    assert_eq!(
        sent(&a),
        vec![
            SessionMsg::Launch {
                slot: c,
                velocity: 1.0
            },
            SessionMsg::LaunchScene {
                scene: c.scene,
                velocity: 1.0
            }
        ]
    );
}

#[test]
fn the_grid_edits_are_one_undo_step_each() {
    use crate::keymap::Action;
    let mut a = app(3, 3);
    a.session_view.focused = true;
    // An empty slot: a new MIDI clip, a bar long, looping.
    let empties = empty_slots(&a);
    let empty = empties[0];
    keys::new_clip(&mut a, empty);
    let c = a.project().slot_clip(empty).expect("made").clone();
    assert_eq!(c.clip.length, BAR);
    assert!(c.looping());
    assert_eq!(a.session_view.cursor, Some(empty));
    // Its loop off and on again.
    action(&mut a, Action::ToggleClipLoop);
    assert!(!a.project().slot_clip(empty).unwrap().looping());
    a.undo();
    assert!(a.project().slot_clip(empty).unwrap().looping());
    // Copy, then paste into another empty slot (fresh ids).
    action(&mut a, Action::Copy);
    let before = a.project().clone();
    if let Some(o) = empties.get(1).copied() {
        a.session_view.cursor = Some(o);
        action(&mut a, Action::Paste);
        let pasted = a.project().slot_clip(o).expect("pasted");
        assert_ne!(pasted.id(), c.id());
        a.undo();
        assert!(a.project().slot_clip(o).is_none());
        assert_eq!(a.project().tracks, before.tracks);
    }
    // Duplicate: into the next free slot below (a scene made when there is none).
    a.session_view.cursor = Some(slot(&a, 0, 0));
    let col = a.project().tracks[0].session_clips().count();
    action(&mut a, Action::Duplicate);
    assert_eq!(a.project().tracks[0].session_clips().count(), col + 1);
    // Delete the cursor's clip; then the scene, once it is the scene that is chosen.
    let cur = a.session_view.cursor.unwrap();
    action(&mut a, Action::Delete);
    assert!(a.project().slot_clip(cur).is_none());
    a.session_view.scene_chosen = true;
    let n = a.project().session.scenes.len();
    action(&mut a, Action::Delete);
    assert_eq!(a.project().session.scenes.len(), n - 1);
    a.undo();
    assert_eq!(a.project().session.scenes.len(), n);
}

#[test]
fn scenes_are_inserted_and_captured_from_what_plays() {
    let mut a = app(3, 2);
    keys::insert_scene(&mut a);
    assert_eq!(a.project().session.scenes.len(), 3);
    // Nothing plays: nothing captured (said).
    keys::capture_scene(&mut a);
    assert_eq!(a.project().session.scenes.len(), 3);
    let s = slot(&a, 0, 0);
    feed(
        &mut a,
        Ticks::ZERO,
        vec![SessionEvent::Slot {
            slot: s,
            phase: SlotPhase::Playing {
                since: Ticks::ZERO,
                from: Ticks::ZERO,
            },
        }],
    );
    a.session_view.cursor = Some(s);
    keys::capture_scene(&mut a);
    assert_eq!(a.project().session.scenes.len(), 4);
    let new = a.project().session.scenes[1].id;
    assert!(a.project().tracks[0].slot_clip(new).is_some());
    assert!(a.project().tracks[1].slot_clip(new).is_none());
}

fn geometry(a: &CantusApp) -> layout::Geometry {
    layout::Geometry::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 600.0)),
        1.0,
        track_ids(a),
        scene_ids(a),
        layout::COL_W,
        egui::Vec2::ZERO,
        None,
    )
}

#[test]
fn a_slot_dragged_to_another_moves_or_copies_and_onto_nothing_does_nothing() {
    let mut a = app(3, 3);
    let from = slot(&a, 0, 0);
    let to = empty_slots(&a)[0];
    let geo = geometry(&a);
    let (i, j) = geo.index_of(to).unwrap();
    let at = geo.cell(i, j).center();
    a.session_view.geo = Some(geo);
    let id = a.project().slot_clip(from).unwrap().id();
    drop::finish_slot_drag(&mut a, from, Some(at), false);
    assert_eq!(a.project().slot_clip(to).map(|c| c.id()), Some(id));
    assert!(a.project().slot_clip(from).is_none());
    a.undo();
    drop::finish_slot_drag(&mut a, from, Some(at), true);
    assert!(a.project().slot_clip(from).is_some() && a.project().slot_clip(to).is_some());
    let n = a.bus.revision();
    drop::finish_slot_drag(&mut a, from, Some(egui::pos2(-50.0, -50.0)), false);
    assert_eq!(a.bus.revision(), n);
}

#[test]
fn arrangement_clips_dropped_on_a_slot_go_into_their_columns() {
    let mut a = app(2, 2);
    // An arrangement clip on track 1.
    let t = a.project().tracks[1].id;
    let mut c = a.bus.new_clip("Arr", Ticks::ZERO, BAR);
    c.notes.clear();
    let cid = c.id;
    assert!(a.exec(cantus_core::Command::InsertClip {
        track: t,
        clip: Box::new(c)
    }));
    a.set_clip_selection(vec![(t, cid)]);
    let geo = geometry(&a);
    let target = slot(&a, 0, 0);
    let (i, j) = geo.index_of(target).unwrap();
    let at = geo.cell(i, j).center();
    a.session_view.geo = Some(geo);
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(at)],
            ..Default::default()
        },
        |_| {},
    );
    out.textures_delta.clear();
    let scenes = a.project().session.scenes.len();
    assert!(drop::from_arrangement(&mut a, &ctx));
    // Its own column, from the row under the pointer down (a new row if the column is full).
    let col: Vec<_> = a.project().tracks[1]
        .session_clips()
        .filter(|c| c.clip.name == "Arr")
        .collect();
    assert_eq!(col.len(), 1);
    assert!(a.project().session.scenes.len() >= scenes);
    assert!(
        a.project().tracks[1].clips.iter().any(|c| c.id == cid),
        "the arrangement keeps its clip"
    );
}

#[test]
fn the_layout_hits_what_it_draws_and_scrolls_to_show_the_cursor() {
    let geo = layout::Geometry::new(
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 500.0)),
        1.0,
        (0..20).map(|i| cantus_core::TrackId(i + 1)).collect(),
        (0..30)
            .map(|i| cantus_core::session::SceneId(i + 100))
            .collect(),
        layout::COL_W,
        egui::vec2(1.0e6, 1.0e6),
        Some(layout::PANEL_H),
    );
    // The scroll is clamped to what there is.
    let m = geo.max_scroll();
    assert_eq!(geo.scroll, m);
    // Each cell is hit as itself.
    let (cols, rows) = geo.visible();
    for i in cols.clone() {
        for j in rows.clone() {
            if j >= 30 {
                continue;
            }
            let c = geo.cell(i, j).center();
            if geo.grid.contains(c) {
                assert_eq!(
                    geo.hit(c),
                    Some(layout::Hit::Slot(SlotRef::new(
                        geo.tracks[i],
                        geo.scenes[j]
                    )))
                );
            }
        }
    }
    assert_eq!(geo.scroll_to_show(0, 0), egui::Vec2::ZERO);
    // A cell already in view does not move the grid.
    let (i, j) = (cols.start + 1, rows.start + 1);
    assert_eq!(geo.scroll_to_show(i, j), geo.scroll);
    // Nothing overlaps: the panel is under the stop row, the master column right of the grid.
    let p = geo.panel.unwrap();
    assert!(geo.stops.bottom() <= p.top() + 0.01);
    assert!(geo.grid.right() <= geo.master.left() + 0.01);
}

fn press(a: &mut CantusApp, port: u8, status: u8, d1: u8, d2: u8) {
    let h = cantus_engine::midi::Heard {
        port,
        event: cantus_engine::MidiEvent {
            status,
            data1: d1,
            data2: d2,
        },
        at: std::time::Instant::now(),
    };
    a.controller_heard(&[h]);
}

#[test]
fn a_launchpad_is_found_greeted_lit_and_launches_from_its_pads() {
    let mut a = app(10, 10);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["Keys".into(), "MIDIIN2 (LPMiniMK3 MIDI)".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    let log = a.controllers.feedback_log.clone().unwrap();
    let lp = "MIDIIN2 (LPMiniMK3 MIDI)";
    // Its programmer mode first, then every light.
    assert_eq!(
        log[0],
        (
            lp.to_string(),
            vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x0E, 0x01, 0xF7]
        )
    );
    assert!(log.len() >= 2 && log.iter().all(|(n, _)| n == lp));
    assert_eq!(a.session_view.rings, vec![(0, 0, 8, 8)]);
    // Nothing changed: nothing more sent.
    frame(&mut a, &ctx);
    assert_eq!(
        a.controllers.feedback_log.as_ref().unwrap().len(),
        log.len()
    );
    // The upper-left pad launches the ring's first slot; its release releases it.
    press(&mut a, 1, 0x90, 81, 100);
    press(&mut a, 1, 0x90, 81, 0);
    let s = slot(&a, 0, 0);
    let told: Vec<SessionMsg> = sent(&a)
        .into_iter()
        .filter(|m| *m != SessionMsg::Resend)
        .collect();
    assert_eq!(
        told[..2],
        [
            SessionMsg::Launch {
                slot: s,
                velocity: 100.0 / 127.0
            },
            SessionMsg::Release { slot: s }
        ]
    );
    // Right moves the ring; a playing clip turns its pad's light green and pulsing.
    press(&mut a, 1, 0xB0, 94, 127);
    frame(&mut a, &ctx);
    assert_eq!(a.session_view.rings, vec![(1, 0, 8, 8)]);
    let s1 = slot(&a, 1, 0);
    feed(
        &mut a,
        Ticks::ZERO,
        vec![SessionEvent::Slot {
            slot: s1,
            phase: SlotPhase::Playing {
                since: Ticks::ZERO,
                from: Ticks::ZERO,
            },
        }],
    );
    a.controllers.feedback_log = Some(Vec::new());
    frame(&mut a, &ctx);
    let lit = a.controllers.feedback_log.clone().unwrap();
    // Pulsing (type 2) at LED 81, the palette's green.
    assert!(
        lit.iter()
            .any(|(_, m)| m.windows(3).any(|w| w == [2, 81, 21])),
        "{lit:?}"
    );
    // A scene button launches the ring's row; the Session button stops all.
    press(&mut a, 1, 0xB0, 89, 127);
    press(&mut a, 1, 0xB0, 95, 127);
    let sc = a.project().session.scenes[0].id;
    let msgs = sent(&a);
    assert!(msgs.contains(&SessionMsg::LaunchScene {
        scene: sc,
        velocity: 1.0
    }));
    assert!(msgs.contains(&SessionMsg::StopAll));
}

#[test]
fn a_grid_turned_off_is_given_back_to_its_own_mode() {
    let mut a = app(2, 2);
    a.controllers.feedback_log = Some(Vec::new());
    let dir = std::env::temp_dir().join("cantus-session-grid-test");
    let _ = std::fs::create_dir_all(&dir);
    a.settings_path = dir.join("settings.json");
    let name = "APC40 mkII".to_string();
    a.midi_port_names = vec![name.clone()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert_eq!(a.controllers.grids.ports.len(), 1);
    crate::controllers::grid::choose(&mut a, &name, None);
    a.controllers.feedback_log = Some(Vec::new());
    frame(&mut a, &ctx);
    assert!(a.controllers.grids.ports.is_empty());
    let log = a.controllers.feedback_log.clone().unwrap();
    assert_eq!(
        log.last().unwrap().1,
        vec![0xF0, 0x47, 0x7F, 0x29, 0x60, 0x00, 0x04, 0x40, 0x00, 0x00, 0x00, 0xF7]
    );
}

#[test]
fn a_learnt_pad_launches_its_slot_and_lets_go_of_it() {
    use cantus_core::controller::{Control, MapMode, Mapping, MappingTarget, MidiSource, PortRef};
    let mut a = app(2, 2);
    a.midi_port_names = vec!["Pads".into()];
    let s = slot(&a, 1, 1);
    let id = a.new_mapping_id();
    let m = Mapping::new(
        id,
        MidiSource {
            port: PortRef {
                name: "Pads".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Note(36),
        },
        MappingTarget::SessionSlot {
            track: s.track,
            scene: s.scene,
        },
        MapMode::Trigger,
    );
    assert!(a.add_mapping(crate::controllers::Scope::Project, m));
    press(&mut a, 0, 0x90, 36, 127);
    press(&mut a, 0, 0x80, 36, 0);
    assert_eq!(
        sent(&a),
        vec![
            SessionMsg::Launch {
                slot: s,
                velocity: 1.0
            },
            SessionMsg::Release { slot: s }
        ]
    );
    // Its light: a quarter while it holds a clip and stops, full while it plays.
    let target = MappingTarget::SessionSlot {
        track: s.track,
        scene: s.scene,
    };
    let has = a.project().slot_clip(s).is_some();
    // Pressed: queued (half) until the conductor answers; unanswered, back to resting.
    if has {
        assert_eq!(learn::launcher_pos(&a, &target), Some(0.5));
    }
    a.session_view
        .live
        .expire(std::time::Instant::now() + live::PRESS_SHOWN);
    assert_eq!(
        learn::launcher_pos(&a, &target),
        Some(if has { 0.25 } else { 0.0 })
    );
    feed(
        &mut a,
        Ticks::ZERO,
        vec![SessionEvent::Slot {
            slot: s,
            phase: SlotPhase::Playing {
                since: Ticks::ZERO,
                from: Ticks::ZERO,
            },
        }],
    );
    assert_eq!(a.controller_pos(&target), Some(1.0));
}

/// In the default theme (the light theme's case is `tests/session_view.rs`'s: installing a
/// palette is process-wide, and the lib tests read it in parallel).
#[test]
fn the_pane_draws_headless_with_and_without_a_session() {
    for p in [demo::project(8, 8), cantus_core::Project::new("none")] {
        let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
        demo::showcase(&mut a);
        a.session_view.cursor = a
            .project()
            .tracks
            .first()
            .zip(a.project().session.scenes.first())
            .map(|(t, s)| SlotRef::new(t.id, s.id));
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        crate::theme::apply(&crate::theme_family::cantus_night(), &ctx);
        for _ in 0..3 {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ui| show(&mut a, ui),
            );
            out.textures_delta.clear();
        }
        assert!(a.session_view.geo.is_some());
    }
}

#[test]
fn a_press_goes_through_the_conductor_and_comes_back_as_the_slots_light() {
    // The whole loop with the session's conductor: the view's press → the engine handle →
    // `perform_frame` (the conductor plans it and tells what plays) → the view's frame.
    let mut a = app(3, 3);
    let s = slot(&a, 1, 0);
    a.session_view.sent = None;
    launch(&mut a, s, 1.0);
    a.perform_frame();
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert!(a.session_view.live.heard, "the conductor spoke");
    let p = a.session_view.live.phase(s);
    assert!(
        matches!(p, SlotPhase::Playing { .. } | SlotPhase::Queued { .. }),
        "{p:?}"
    );
    // Stop all: the track's clip stops (at once, or at its boundary).
    stop_all(&mut a);
    a.perform_frame();
    frame(&mut a, &ctx);
    let p = a.session_view.live.phase(s);
    assert!(
        matches!(
            p,
            SlotPhase::Stopping { .. } | SlotPhase::Stopped | SlotPhase::Playing { .. }
        ),
        "{p:?}"
    );
}

/// Press → `EngineMessage::Session` for a Launchpad pad hit, measured on the interface
/// thread (`cargo test -p cantus-ui --lib pad_press_latency -- --ignored --nocapture`): the
/// pad's message read and turned into the press (`controller_heard`), then the conductor's
/// frame that plans it (`perform_frame`) — with a small session (idle) and a 16 × 32 one
/// playing (busy). The wake itself is the MIDI tap's: a taken message asks the window for a
/// frame at once (`controllers::waking_tap`).
#[test]
#[ignore]
fn pad_press_latency() {
    use std::time::Instant;
    for (name, tracks, scenes) in [("idle 4 x 4", 4, 4), ("busy 16 x 32", 16, 32)] {
        let mut heard = Vec::new();
        let mut planned = Vec::new();
        for n in 0..50 {
            let mut a = CantusApp::from_parts(
                Some(demo::project(tracks, scenes)),
                EngineHandle::dummy(),
                false,
            );
            demo::showcase(&mut a);
            a.controllers.feedback_log = Some(Vec::new());
            a.midi_port_names = vec!["MIDIIN2 (LPMiniMK3 MIDI)".into()];
            let ctx = egui::Context::default();
            frame(&mut a, &ctx);
            let _ = a.engine.take_session_msgs();
            let pad = 81 - 10 * (n % 4) as u8;
            let t0 = Instant::now();
            press(&mut a, 0, 0x90, pad, 100);
            let t1 = Instant::now();
            assert!(
                !a.engine.take_session_msgs().is_empty(),
                "the press reached the handle"
            );
            let s0 = slot(&a, 0, 0);
            launch(&mut a, s0, 1.0);
            let t2 = Instant::now();
            a.perform_frame();
            let t3 = Instant::now();
            heard.push((t1 - t0).as_secs_f64() * 1e6);
            planned.push((t3 - t2).as_secs_f64() * 1e3);
        }
        heard.sort_by(f64::total_cmp);
        planned.sort_by(f64::total_cmp);
        println!(
            "{name}: pad → press median {:.0} µs (p95 {:.0} µs); conductor's frame median {:.2} ms (p95 {:.2} ms)",
            heard[25], heard[47], planned[25], planned[47]
        );
    }
}

// ---- the review's fixes (round 33) -----------------------------------------------------

fn draw(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>, pane: bool) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            frame(a, ui.ctx());
            if pane {
                show(a, ui);
            }
        },
    );
    out.textures_delta.clear();
}

fn themed() -> egui::Context {
    let ctx = egui::Context::default();
    crate::design::install_fonts(&ctx);
    crate::design::install_style(&ctx);
    crate::theme::apply(&crate::theme_family::cantus_night(), &ctx);
    ctx
}

/// Fails before the fix: the slots of a pane no longer drawn stayed where a drop could
/// land, so an arrangement clip let go there went into a hidden slot.
#[test]
fn a_hidden_pane_has_no_slots_to_drop_into() {
    let mut a = app(3, 3);
    let ctx = themed();
    draw(&mut a, &ctx, Vec::new(), true);
    let geo = a.session_view.geo.clone().expect("drawn");
    let target = empty_slots(&a)[0];
    let (i, j) = geo.index_of(target).unwrap();
    let at = geo.cell(i, j).center();
    assert_eq!(drop::slot_at(&a, at), Some(target));
    // Two passes without the pane (the arrangement's tab in front): nothing to land in.
    draw(&mut a, &ctx, Vec::new(), false);
    draw(&mut a, &ctx, Vec::new(), false);
    assert_eq!(drop::slot_at(&a, at), None);
    // Tab back to the arrangement clears them at once.
    draw(&mut a, &ctx, Vec::new(), true);
    a.workspace.reveal(crate::views::workspace::Pane::Session);
    keys::toggle_view(&mut a);
    assert_eq!(drop::slot_at(&a, at), None);
}

/// Fails before the fix: the grid's release took the scene's press, so a gate scene was
/// never let go.
#[test]
fn a_scene_launcher_let_go_releases_its_scene() {
    let mut a = app(3, 3);
    let ctx = themed();
    draw(&mut a, &ctx, Vec::new(), true);
    let geo = a.session_view.geo.clone().unwrap();
    let r = geo.scene(1);
    let p = egui::pos2(r.left() + 8.0, r.center().y);
    let button = |pressed| egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    draw(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(p), button(true)],
        true,
    );
    draw(&mut a, &ctx, vec![button(false)], true);
    let sc = a.project().session.scenes[1].id;
    let told: Vec<SessionMsg> = sent(&a)
        .into_iter()
        .filter(|m| *m != SessionMsg::Resend)
        .collect();
    assert_eq!(
        told,
        vec![
            SessionMsg::LaunchScene {
                scene: sc,
                velocity: 1.0
            },
            SessionMsg::ReleaseScene { scene: sc }
        ]
    );
}

/// A port that keeps what it is sent, for the drop's goodbye.
struct Kept;
static KEPT: std::sync::Mutex<Vec<Vec<u8>>> = std::sync::Mutex::new(Vec::new());
impl cantus_engine::midi::OutPort for Kept {
    fn write(&mut self, bytes: &[u8]) -> bool {
        KEPT.lock().unwrap().push(bytes.to_vec());
        true
    }
}

/// Fails before the fix: nothing gave a grid back when Cantus closed — a Launchpad stayed
/// in programmer mode, an APC40 in Live's. The close path calls `release_all`; a panic's
/// unwinding drops the controllers, which does it too.
#[test]
fn closing_gives_every_grid_back_to_its_own_mode() {
    let mut a = app(2, 2);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["APC40 mkII".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    a.controllers.feedback_log = Some(Vec::new());
    crate::controllers::grid::release_all(&mut a);
    let mode_40 = vec![
        0xF0, 0x47, 0x7F, 0x29, 0x60, 0x00, 0x04, 0x40, 0x00, 0x00, 0x00, 0xF7,
    ];
    assert_eq!(
        a.controllers
            .feedback_log
            .as_ref()
            .unwrap()
            .last()
            .unwrap()
            .1,
        mode_40
    );
    // Dropped without the close path: the drop says goodbye through the outputs.
    let mut b = app(2, 2);
    b.controllers.feedback_log = Some(Vec::new());
    b.midi_port_names = vec!["APC40 mkII".into()];
    frame(&mut b, &ctx);
    b.controllers.feedback_log = None;
    b.controllers.outputs = cantus_engine::midi::MidiOutputs::with_connect(|_| {
        Some(Box::new(Kept) as Box<dyn cantus_engine::midi::OutPort>)
    });
    KEPT.lock().unwrap().clear();
    drop(b);
    assert_eq!(KEPT.lock().unwrap().last(), Some(&mode_40));
}

/// Fails before the fix: a grid's port swallowed every message it did not decode, so an
/// APC mini's faders could neither be learnt nor drive a mapping.
#[test]
fn an_apc_minis_fader_drives_a_learnt_mapping() {
    use cantus_core::controller::{Control, MapMode, Mapping, MappingTarget, MidiSource, PortRef};
    let mut a = app(2, 2);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["APC mini mk2".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert_eq!(a.controllers.grids.ports.len(), 1);
    let id = a.new_mapping_id();
    let m = Mapping::new(
        id,
        MidiSource {
            port: PortRef {
                name: "APC mini mk2".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Cc(48),
        },
        MappingTarget::Tempo,
        MapMode::Absolute,
    );
    let m = Mapping {
        takeover: cantus_core::controller::Takeover::Jump,
        ..m
    };
    assert!(a.add_mapping(crate::controllers::Scope::Project, m));
    let before = a.project().tempo_bpm;
    press(&mut a, 0, 0xB0, 48, 127);
    assert_ne!(a.project().tempo_bpm, before, "the fader moved the tempo");
}

/// Fails before the fix: Ctrl+D with no free slot below made a scene, then the copy — two
/// undo steps.
#[test]
fn duplicating_into_a_new_scene_is_one_step() {
    use crate::keymap::Action;
    let mut a = app(3, 3);
    a.session_view.focused = true;
    let last = a.project().session.scenes.len() - 1;
    let i = (0..3)
        .find(|i| a.project().slot_clip(slot(&a, *i, last)).is_some())
        .expect("a clip in the last row");
    a.session_view.cursor = Some(slot(&a, i, last));
    let before = a.project().clone();
    assert!(action(&mut a, Action::Duplicate));
    assert_eq!(
        a.project().session.scenes.len(),
        before.session.scenes.len() + 1
    );
    a.undo();
    assert_eq!(a.project().session.scenes, before.session.scenes);
    assert_eq!(a.project().tracks, before.tracks);
}

/// Fails before the fix: a clip pasted on another track kept its envelope of the first
/// track's level, so pasting Bass's clip on Keys automated Bass.
#[test]
fn a_pasted_clip_moves_its_envelopes_to_its_new_track() {
    use crate::keymap::Action;
    use cantus_core::automation::{AutoPoint, AutoTarget, Automation};
    let mut a = app(3, 3);
    a.session_view.focused = true;
    let bass = slot(&a, 1, 0);
    let env = Automation::new(
        AutoTarget::TrackGain(bass.track),
        vec![AutoPoint::new(Ticks::ZERO, 0.4)],
    );
    exec(
        &mut a,
        S::SetSlotEnvelopes {
            slot: bass,
            envelopes: vec![env],
        },
    );
    a.session_view.cursor = Some(bass);
    action(&mut a, Action::Copy);
    let keys = empty_slots(&a)
        .into_iter()
        .find(|s| s.track != bass.track)
        .expect("an empty slot on another track");
    a.session_view.cursor = Some(keys);
    action(&mut a, Action::Paste);
    let pasted = a.project().slot_clip(keys).expect("pasted");
    assert_eq!(
        pasted
            .envelopes
            .iter()
            .map(|e| e.target.clone())
            .collect::<Vec<_>>(),
        vec![AutoTarget::TrackGain(keys.track)]
    );
}

/// Fails before the fix: Enter launched, its key-up never released — a gate clip played on.
#[test]
fn a_key_held_launch_is_released_with_the_key() {
    use crate::keymap::Action;
    let mut a = app(2, 2);
    let s = slot(&a, 0, 0);
    a.session_view.cursor = Some(s);
    a.session_view.focused = true;
    action(&mut a, Action::SessionLaunch);
    assert!(a.session_view.key_held.is_some());
    // The next frame, no key down: let go.
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert!(sent(&a).contains(&SessionMsg::Release { slot: s }));
    assert!(a.session_view.key_held.is_none());
}

/// A learnt slot behaves as a pad or the mouse: on an armed track's empty slot it records.
#[test]
fn a_learnt_slot_records_into_an_armed_tracks_empty_slot() {
    let mut a = app(4, 3);
    let armed = empty_slots(&a)
        .into_iter()
        .find(|s| a.project().track(s.track).is_some_and(|t| t.armed))
        .expect("an armed empty slot");
    let target = cantus_core::controller::MappingTarget::SessionSlot {
        track: armed.track,
        scene: armed.scene,
    };
    learn::drive(&mut a, &target, true, 1.0);
    assert_eq!(sent(&a), vec![SessionMsg::Record { slot: armed }]);
}

/// Fails before the fix: Ctrl+X with the scene chosen copied one clip and deleted the row.
#[test]
fn cutting_a_scene_copies_its_row() {
    use crate::keymap::Action;
    let mut a = app(3, 3);
    a.session_view.focused = true;
    let sc = a.project().session.scenes[0].id;
    let row = a
        .project()
        .tracks
        .iter()
        .filter(|t| t.slot_clip(sc).is_some())
        .count();
    assert!(row >= 2);
    a.session_view.cursor = Some(slot(&a, 0, 0));
    a.session_view.scene_chosen = true;
    action(&mut a, Action::Cut);
    assert!(a.project().session.scene(sc).is_none());
    assert_eq!(a.session_view.clipboard.len(), row);
}

/// Fails before the fix: a held pad was let go through where the ring had moved, so a
/// gate clip was released in another slot.
#[test]
fn a_held_pad_lets_go_of_the_slot_it_launched() {
    let mut a = app(10, 10);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["MIDIIN2 (LPMiniMK3 MIDI)".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    press(&mut a, 0, 0x90, 81, 100);
    // The ring a track to the right, then the pad let go.
    press(&mut a, 0, 0xB0, 94, 127);
    press(&mut a, 0, 0x90, 81, 0);
    let s = slot(&a, 0, 0);
    assert!(sent(&a).contains(&SessionMsg::Release { slot: s }));
}

/// Fails before the fix: a new grid port was taken from the engine a frame after it
/// appeared (the filter published before the grid was found).
#[test]
fn a_new_grid_port_is_taken_in_the_same_frame() {
    let mut a = app(2, 2);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["MIDIIN2 (LPX MIDI)".into()];
    a.controller_input();
    assert_eq!(
        a.controllers.grids.ports.len(),
        1,
        "found before the filter"
    );
    assert!(a.controllers.published.is_some());
}

fn draw_arrangement(a: &mut CantusApp, ctx: &egui::Context, shown: bool) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| {
            frame(a, ui.ctx());
            if shown {
                crate::views::arrangement::show(a, ui);
            }
        },
    );
    out.textures_delta.clear();
}

/// Fails before the fix: the arrangement's timeline stayed where a drop could land once
/// its tab was hidden, so a slot's clip let go outside the grid went into it unseen.
#[test]
fn a_hidden_arrangement_has_no_time_to_drop_into() {
    let mut a = app(3, 3);
    let ctx = themed();
    draw_arrangement(&mut a, &ctx, true);
    let g = a.arrangement.geometry().expect("drawn");
    let at = egui::pos2(g.left + g.width * 0.5, g.body_top + 12.0);
    // Two passes with the Session pane's tab in front instead.
    draw_arrangement(&mut a, &ctx, false);
    draw_arrangement(&mut a, &ctx, false);
    assert!(a.arrangement.geometry().is_none());
    let from = slot(&a, 0, 0);
    let before = a.project().tracks.clone();
    drop::finish_slot_drag(&mut a, from, Some(at), false);
    assert_eq!(
        a.project().tracks,
        before,
        "nothing landed in the hidden arrangement"
    );
    // Shown again, it is a place to land in again.
    draw_arrangement(&mut a, &ctx, true);
    assert!(a.arrangement.geometry().is_some());
}

/// One pass's keys, as the app reads them: `global_shortcuts` with the given events.
fn keys_pass(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            frame(a, ui.ctx());
            // As the app: the keys before the panes are laid out, the pane drawn.
            a.global_shortcuts(ui.ctx());
            show(a, ui);
        },
    );
    out.textures_delta.clear();
}

/// Fails before the fix: holding Enter, the OS's repeats each launched again — on an armed
/// empty slot, the first repeat ended the take the press began.
#[test]
fn a_held_enter_launches_once_not_with_every_repeat() {
    let mut a = app(4, 3);
    let armed = empty_slots(&a)
        .into_iter()
        .find(|s| a.project().track(s.track).is_some_and(|t| t.armed))
        .expect("an armed empty slot");
    let ctx = themed();
    draw(&mut a, &ctx, Vec::new(), true);
    a.session_view.cursor = Some(armed);
    a.session_view.focused = true;
    let enter = |repeat| egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: egui::Modifiers::NONE,
    };
    keys_pass(&mut a, &ctx, vec![enter(false)]);
    keys_pass(&mut a, &ctx, vec![enter(true)]);
    keys_pass(&mut a, &ctx, vec![enter(true), enter(true)]);
    let told: Vec<SessionMsg> = sent(&a)
        .into_iter()
        .filter(|m| matches!(m, SessionMsg::Record { .. } | SessionMsg::Launch { .. }))
        .collect();
    assert_eq!(told, vec![SessionMsg::Record { slot: armed }]);
}

/// Fails before the fix: a copy dragged kept an Unda clip's envelope of Unda's parameter
/// on Calor's track (the session's rule) where a paste dropped it (the view's own) — two
/// rules for one copy.
#[test]
fn a_clip_copied_to_another_instrument_keeps_the_same_envelopes_either_way() {
    use crate::keymap::Action;
    use cantus_core::automation::{AutoPoint, AutoTarget, Automation};
    use cantus_core::instrument::InstrumentSpec;
    use cantus_core::{InstrumentKind, NativeSpec};
    let mut p = demo::project(3, 7);
    p.tracks[0].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    p.tracks[1].instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog));
    let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    a.session_view.record_sent();
    a.session_view.focused = true;
    let unda = slot(&a, 0, 0);
    let calor = a.project().tracks[1].id;
    let point = |v| vec![AutoPoint::new(Ticks::ZERO, v)];
    let envelopes = vec![
        Automation::new(AutoTarget::TrackGain(unda.track), point(0.4)),
        Automation::new(
            AutoTarget::Instrument {
                track: unda.track,
                key: "cutoff".into(),
            },
            point(0.7),
        ),
    ];
    exec(
        &mut a,
        S::SetSlotEnvelopes {
            slot: unda,
            envelopes,
        },
    );
    let free: Vec<SlotRef> = empty_slots(&a)
        .into_iter()
        .filter(|s| s.track == calor)
        .collect();
    assert!(free.len() >= 2);
    // A copy dragged (Ctrl held) …
    let ctx = themed();
    draw(&mut a, &ctx, Vec::new(), true);
    let geo = a.session_view.geo.clone().expect("drawn");
    let (i, j) = geo.index_of(free[0]).unwrap();
    drop::finish_slot_drag(&mut a, unda, Some(geo.cell(i, j).center()), true);
    // … and one pasted (the pane holding the keyboard).
    a.session_view.focused = true;
    a.session_view.cursor = Some(unda);
    action(&mut a, Action::Copy);
    a.session_view.cursor = Some(free[1]);
    action(&mut a, Action::Paste);
    let targets = |s: SlotRef| {
        a.project()
            .slot_clip(s)
            .expect("copied")
            .envelopes
            .iter()
            .map(|e| e.target.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(targets(free[0]), vec![AutoTarget::TrackGain(calor)]);
    assert_eq!(targets(free[1]), targets(free[0]));
}

/// Fails before the fix: a pad's aftertouch went on to MIDI learn, so pressing a pad of a
/// Launchpad X while learning learnt its pressure.
#[test]
fn a_pads_pressure_learns_nothing() {
    let mut a = app(10, 10);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["MIDIIN2 (LPX MIDI)".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    assert_eq!(a.controllers.grids.ports.len(), 1);
    a.learn_target(cantus_core::controller::MappingTarget::Tempo);
    press(&mut a, 0, 0x90, 11, 100);
    press(&mut a, 0, 0xA0, 11, 60);
    press(&mut a, 0, 0xD0, 60, 0);
    press(&mut a, 0, 0x80, 11, 0);
    let l = a.controllers.learning.as_ref().expect("still armed");
    assert!(l.heard.is_empty() && l.source.is_none(), "nothing learnt");
}

/// The Pro's track select row: the tracks in their colours, the stops while Stop Clip is
/// held.
#[test]
fn a_launchpad_pros_bottom_row_selects_tracks_or_stops_them() {
    use cantus_core::controller::grid::{Light, Rgb};
    let mut a = app(10, 10);
    a.controllers.feedback_log = Some(Vec::new());
    a.midi_port_names = vec!["LPProMK3 MIDI".into()];
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    let mut p = a.controllers.grids.ports[0].clone();
    let idle_stop = Light::On(Rgb::RED.dim(0.15));
    let f = crate::controllers::grid::picture(&a, &p);
    assert_ne!(f.stops[0], idle_stop);
    assert_ne!(f.stop_all, Light::On(Rgb::RED));
    p.stop_mode = true;
    let f = crate::controllers::grid::picture(&a, &p);
    assert_eq!(f.stops[0], idle_stop);
    assert_eq!(f.stop_all, Light::On(Rgb::RED));
}

/// Fails before the fix: a scene cut and pasted back came back as clips in the cursor's
/// row — its name, tempo, metre and follow actions lost.
#[test]
fn a_cut_scene_pasted_comes_back_whole_in_one_step() {
    use crate::keymap::Action;
    let mut a = app(3, 5);
    a.session_view.focused = true;
    let sc = a.project().session.scenes[3].clone();
    assert!(sc.tempo.is_some() && sc.metre.is_some());
    let row: Vec<_> = a
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.slot_clip(sc.id).map(|c| (t.id, c.clip.notes.len())))
        .collect();
    a.session_view.cursor = Some(slot(&a, 0, 3));
    a.session_view.scene_chosen = true;
    action(&mut a, Action::Cut);
    assert!(a.project().session.scene(sc.id).is_none());
    a.session_view.cursor = Some(slot(&a, 0, 1));
    a.session_view.scene_chosen = true;
    let scenes = a.project().session.scenes.len();
    action(&mut a, Action::Paste);
    assert_eq!(a.project().session.scenes.len(), scenes + 1);
    let back = a.project().session.scenes[2].clone();
    assert_ne!(back.id, sc.id);
    assert_eq!(
        (&back.name, back.color, back.tempo, back.metre, &back.follow),
        (&sc.name, sc.color, sc.tempo, sc.metre, &sc.follow)
    );
    let again: Vec<_> = a
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.slot_clip(back.id).map(|c| (t.id, c.clip.notes.len())))
        .collect();
    assert_eq!(again, row);
    // One step: undone, the scene is gone again.
    a.undo();
    assert_eq!(a.project().session.scenes.len(), scenes);
}
