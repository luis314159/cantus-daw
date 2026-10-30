//! The session's MIDI learn (ADR 0077): a mapped pad or button launches a slot or a scene
//! on its press and releases it on its release (a gate clip stops), a stop button stops
//! its track; the scene cursor, *stop all*, arrangement record and *back to arrangement*
//! are keymap actions (any control fires them); the global quantisation is a stepped
//! target a knob turns. Feedback lights each control by its target's state: a slot's
//! playing (full), queued or stopping (half), holding a clip (a quarter), empty (off).

use crate::app::CantusApp;
use cantus_core::controller::MappingTarget;
use cantus_core::session::SlotRef;
use cantus_engine::messages::{ScenePhase, SlotPhase};

/// A launcher's state for its control's light (0…1): `None` for a target that is not one.
pub fn launcher_pos(app: &CantusApp, target: &MappingTarget) -> Option<f32> {
    let live = &app.session_view.live;
    Some(match target {
        MappingTarget::SessionSlot { track, scene } => {
            let slot = SlotRef::new(*track, *scene);
            match live.phase(slot) {
                SlotPhase::Playing { .. } | SlotPhase::Recording { .. } => 1.0,
                SlotPhase::Queued { .. }
                | SlotPhase::Stopping { .. }
                | SlotPhase::RecordQueued { .. } => 0.5,
                SlotPhase::Stopped if app.project().slot_clip(slot).is_some() => 0.25,
                SlotPhase::Stopped => 0.0,
            }
        }
        MappingTarget::SessionScene(id) => match live.scene_phase(*id) {
            ScenePhase::Playing { .. } => 1.0,
            ScenePhase::Queued { .. } => 0.5,
            ScenePhase::Idle => 0.25,
        },
        MappingTarget::SessionStop(t) => {
            if live.playing_on(*t).is_some() {
                1.0
            } else {
                0.0
            }
        }
        _ => return None,
    })
}

/// A launcher's control moved: pressed (at `velocity`, 0…1) or let go. Returns whether
/// `target` was a launcher.
pub fn drive(app: &mut CantusApp, target: &MappingTarget, pressed: bool, velocity: f32) -> bool {
    match *target {
        MappingTarget::SessionSlot { track, scene } => {
            let slot = SlotRef::new(track, scene);
            if pressed {
                super::press_slot(app, slot, velocity);
            } else {
                super::release(app, slot);
            }
        }
        MappingTarget::SessionScene(id) => {
            if pressed {
                super::launch_scene(app, id, velocity);
            } else {
                super::release_scene(app, id);
            }
        }
        MappingTarget::SessionStop(t) => {
            if pressed {
                super::stop_track(app, t);
            }
        }
        _ => return false,
    }
    true
}

/// A launcher target in words, for the mappings pane and the status line.
pub fn target_name(app: &CantusApp, target: &MappingTarget) -> Option<String> {
    use crate::i18n::{t, tf};
    let p = app.project();
    let track = |id: cantus_core::TrackId| {
        p.track(id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| t("auto.target.missing"))
    };
    let scene = |id: cantus_core::session::SceneId| match (
        p.session.scene_index(id),
        p.session.scene(id),
    ) {
        (Some(_), Some(s)) if !s.name.is_empty() => s.name.clone(),
        (Some(j), _) => (j + 1).to_string(),
        _ => t("auto.target.missing"),
    };
    Some(match target {
        MappingTarget::SessionSlot {
            track: tr,
            scene: sc,
        } => tf(
            "session.learn.slot",
            &[("track", &track(*tr)), ("scene", &scene(*sc))],
        ),
        MappingTarget::SessionScene(sc) => tf("session.learn.scene", &[("scene", &scene(*sc))]),
        MappingTarget::SessionStop(tr) => tf("session.learn.stop", &[("track", &track(*tr))]),
        MappingTarget::SessionQuantize => t("session.learn.quantize"),
        _ => return None,
    })
}
