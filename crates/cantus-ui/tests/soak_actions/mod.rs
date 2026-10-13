//! The soak's edits (round 22): random, seeded, through the same commands and methods the
//! interface uses — notes, clips, automation, the mixer, effects added, removed and
//! swapped, device windows opened and closed, undo/redo storms, recording passes against
//! the fake input, saves and opens, the demo songs.
#![allow(dead_code)]

use cantus_core::audio::TrackInput;
use cantus_core::automation::AutoTarget;
use cantus_core::{Command, EffectKind, Ticks, TrackId};
use cantus_ui::CantusApp;
use std::path::Path;

/// xorshift64*: the soak replays from its seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn pick<T: Copy>(&mut self, v: &[T]) -> Option<T> {
        (!v.is_empty()).then(|| v[self.below(v.len())])
    }
}

/// What an action did, counted for the report.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Notes,
    Clips,
    Automation,
    Mixer,
    Effects,
    Windows,
    UndoStorm,
    Transport,
    Record,
    SaveOpen,
    Demo,
}

pub const EFFECTS: [EffectKind; 11] = [
    EffectKind::Space,
    EffectKind::Saturator,
    EffectKind::Stereo,
    EffectKind::Multiband,
    EffectKind::Maximizer,
    EffectKind::Compressor,
    EffectKind::Limiter,
    EffectKind::Eq,
    EffectKind::Delay,
    EffectKind::Reverb,
    EffectKind::Gate,
];

fn note_clips(app: &CantusApp) -> Vec<(TrackId, cantus_core::ClipId)> {
    app.project()
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.audio.is_none())
                .map(move |c| (t.id, c.id))
        })
        .collect()
}

fn strips(app: &CantusApp) -> Vec<cantus_core::StripId> {
    app.project().strips.iter().map(|s| s.id).collect()
}

/// One small edit of `kind`.
pub fn edit(app: &mut CantusApp, rng: &mut Rng, kind: Kind) {
    match kind {
        Kind::Notes => {
            let Some((track, clip)) = rng.pick(&note_clips(app)) else {
                return;
            };
            if rng.below(3) == 0 {
                let ids: Vec<_> = app
                    .project()
                    .track(track)
                    .and_then(|t| t.clip(clip))
                    .map(|c| c.notes.iter().take(2).map(|n| n.id).collect())
                    .unwrap_or_default();
                if !ids.is_empty() {
                    app.exec(Command::RemoveNotes { track, clip, ids });
                }
            } else {
                let notes: Vec<_> = (0..1 + rng.below(6))
                    .map(|_| {
                        let at = Ticks(rng.below(16) as i64 * 240);
                        let pitch = 48 + rng.below(30) as u8;
                        app.bus.new_note(at, Ticks(240), pitch, 90)
                    })
                    .collect();
                app.exec(Command::InsertNotes { track, clip, notes });
            }
        }
        Kind::Clips => {
            let tracks: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
            match rng.below(3) {
                0 => {
                    if let Some(t) = rng.pick(&tracks) {
                        let at = Ticks::beats(rng.below(32) as i64 * 4);
                        app.create_clip(t, at, Ticks::beats(4));
                    }
                }
                1 => {
                    if let Some((t, c)) = rng.pick(&note_clips(app)) {
                        if note_clips(app).len() > 4 {
                            app.delete_clip(t, c);
                        }
                    }
                }
                _ => {
                    if let Some((t, c)) = rng.pick(&note_clips(app)) {
                        app.select_clip(t, c);
                        app.duplicate_clips();
                    }
                }
            }
        }
        Kind::Automation => {
            if let Some(s) = rng.pick(&strips(app)) {
                app.create_automation_clip(AutoTarget::StripGain(s));
            }
        }
        Kind::Mixer => {
            if let Some(s) = rng.pick(&strips(app)) {
                match rng.below(3) {
                    0 => app.exec(Command::SetStripGain {
                        id: s,
                        gain_db: -24.0 + rng.unit() * 30.0,
                    }),
                    1 => app.exec(Command::SetStripPan {
                        id: s,
                        pan: rng.unit() * 2.0 - 1.0,
                    }),
                    _ => app.exec(Command::SetStripMute {
                        id: s,
                        muted: rng.below(4) == 0,
                    }),
                };
                app.end_gesture();
            }
        }
        Kind::Effects => {
            let Some(s) = rng.pick(&strips(app)) else {
                return;
            };
            let chain: Vec<_> = app
                .project()
                .effects_of(s)
                .map(|e| e.iter().map(|e| e.id).collect())
                .unwrap_or_default();
            match rng.below(4) {
                0 | 1 if chain.len() < 6 => {
                    let k = EFFECTS[rng.below(EFFECTS.len())];
                    app.add_effect(s, k);
                }
                2 if !chain.is_empty() => {
                    let e = chain[rng.below(chain.len())];
                    let slot = app.bus.new_effect(EFFECTS[rng.below(EFFECTS.len())]);
                    app.exec(Command::ReplaceEffect {
                        strip: s,
                        effect: e,
                        slot: Box::new(slot),
                    });
                }
                _ if !chain.is_empty() => {
                    let e = chain[rng.below(chain.len())];
                    app.remove_effect(s, e);
                }
                _ => {}
            }
        }
        Kind::Windows => {
            // A device window opened, or the one open closed (the frame draws them).
            if app.fx.open.is_some() || app.inst.open.is_some() {
                app.close_fx_editor();
                app.close_inst_editor();
            } else if rng.below(2) == 0 {
                let open: Vec<_> = app
                    .project()
                    .strips
                    .iter()
                    .flat_map(|s| s.effects.iter().map(move |e| (s.id, e.id, e.spec.kind)))
                    .collect();
                if let Some(o) = rng.pick(&open) {
                    app.fx.open = Some(o);
                }
            } else {
                let tracks: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
                if let Some(t) = rng.pick(&tracks) {
                    app.open_inst_editor(t);
                }
            }
            let views = [
                cantus_ui::app::CentralView::Mixer,
                cantus_ui::app::CentralView::PianoRoll,
                cantus_ui::app::CentralView::ChannelRack,
            ];
            app.show_central(views[rng.below(3)]);
        }
        Kind::UndoStorm => {
            let n = 10 + rng.below(60);
            for _ in 0..n {
                app.undo();
            }
            for _ in 0..n {
                app.redo();
            }
        }
        Kind::Transport => match rng.below(3) {
            0 => app.toggle_play(),
            1 => app.seek(Ticks::beats(rng.below(64) as i64)),
            _ => {
                let on = !app.project().loop_enabled;
                app.exec(Command::SetLoopEnabled { enabled: on });
            }
        },
        Kind::Record | Kind::SaveOpen | Kind::Demo => {}
    }
}

/// Arm one audio track on input 1 (made if there is none), ready to record.
pub fn arm_audio_track(app: &mut CantusApp) -> Option<TrackId> {
    let has = app
        .project()
        .tracks
        .iter()
        .find(|t| t.input.is_some())
        .map(|t| t.id);
    let id = match has {
        Some(id) => id,
        None => {
            app.add_audio_track();
            app.project()
                .tracks
                .iter()
                .rev()
                .find(|t| t.input.is_some())?
                .id
        }
    };
    app.exec(Command::SetTrackInput {
        id,
        input: Some(TrackInput::mono(0)),
    });
    app.exec(Command::SetTrackArmed { id, armed: true });
    Some(id)
}

/// Save to `path` and open it again: the project must come back as it was saved.
pub fn save_and_open(app: &mut CantusApp, path: &Path) -> Result<(), String> {
    app.save_to_path(path);
    if app.status.error {
        return Err(app.status.text.clone());
    }
    let saved = cantus_project::load(path, true).map_err(|e| e.to_string())?;
    app.open(path);
    if app.status.error {
        return Err(app.status.text.clone());
    }
    if app.project() != &saved.project {
        return Err("the project opened is not the one saved".into());
    }
    Ok(())
}
