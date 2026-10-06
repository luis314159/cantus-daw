//! Where a sample dragged out of the browser lands (ADR 0030).
//!
//! Views that can take a sample publish a target as they draw (`publish`): a Pulsus pad,
//! Exemplar's key map, the channel rack, the arrangement. A release is remembered and
//! resolved at the start of the next frame against everything the frame before drew —
//! so a floating window drawn after the dock still counts, and the target drawn last
//! (the one on top) wins.

use crate::app::CantusApp;
use cantus_core::{Command, DrumPad, InstrumentSpec, PadVoice, PulsusParams, TrackId};
use egui::{Context, Id, Pos2, Rect};
use std::path::Path;

/// Something a sample can be dropped on.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// A pad of a Pulsus kit, by index: the pad becomes a sample pad playing the file.
    Pad { track: TrackId, pad: usize },
    /// Exemplar's key map, spanning keys `k0..=k1` across its width: a zone on the key
    /// under the pointer.
    Exemplar { track: TrackId, k0: u8, k1: u8 },
    /// The channel rack: a new sample channel, or the sample of a one-pad channel.
    Rack,
    /// The arrangement: an audio clip at the pointer (round 19's import).
    Arrangement,
}

#[derive(Clone, Default)]
struct Targets {
    current: Vec<(Rect, Target)>,
    last: Vec<(Rect, Target)>,
}

fn id() -> Id {
    Id::new("cantus.samples.drop_targets")
}

/// Publish a drop target for this frame.
pub fn publish(ctx: &Context, rect: Rect, target: Target) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Targets>(id())
            .current
            .push((rect, target))
    });
}

/// Start a frame: what the last frame published becomes what drops are resolved against.
pub fn begin_frame(ctx: &Context) {
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Targets>(id());
        t.last = std::mem::take(&mut t.current);
    });
}

/// The targets the last complete frame drew, bottom first.
pub fn targets(ctx: &Context) -> Vec<(Rect, Target)> {
    ctx.data(|d| {
        d.get_temp::<Targets>(id())
            .map(|t| t.last)
            .unwrap_or_default()
    })
}

/// The target a release at `pos` would land on: the last drawn that contains it.
pub fn target_at(ctx: &Context, pos: Pos2) -> Option<(Rect, Target)> {
    targets(ctx)
        .into_iter()
        .rev()
        .find(|(r, _)| r.contains(pos))
}

/// Resolve a release remembered last frame. Called once a frame, first thing.
pub fn resolve(app: &mut CantusApp, ctx: &Context) {
    begin_frame(ctx);
    let Some((path, pos)) = app.samples.released.take() else {
        return;
    };
    let Some((rect, target)) = target_at(ctx, pos) else {
        return;
    };
    land(app, &path, target, rect, pos);
}

/// Apply a drop: one undo step each.
pub fn land(app: &mut CantusApp, path: &Path, target: Target, rect: Rect, pos: Pos2) {
    let text = path.to_string_lossy().into_owned();
    let name = file_stem(path);
    match target {
        Target::Pad { track, pad } => app.set_pad_sample(track, pad, Some(text)),
        Target::Exemplar { track, k0, k1 } => {
            let w = rect.width() / (k1 as f32 - k0 as f32 + 1.0);
            let key =
                (k0 as f32 + ((pos.x - rect.left()) / w.max(1e-3)).floor()).clamp(0.0, 127.0) as u8;
            crate::views::inst::sampler::drop_files(app, track, &[path.to_path_buf()], key);
        }
        Target::Rack => {
            let row = app.rack.geometry().and_then(|g| {
                g.rows
                    .iter()
                    .find(|r| pos.y >= r.top && pos.y < r.top + r.height)
                    .map(|r| r.track)
            });
            // On a one-pad channel: its sample. Anywhere else: a new channel below.
            let one_pad = row.and_then(|i| {
                let t = app.project().tracks.get(i)?;
                (t.instrument.pulsus()?.pads.len() == 1).then_some(t.id)
            });
            match one_pad {
                Some(track) => app.set_pad_sample(track, 0, Some(text)),
                None => {
                    let index = row.map(|r| r + 1).unwrap_or(app.project().tracks.len());
                    app.add_sample_channel(path, index);
                }
            }
        }
        Target::Arrangement => {
            crate::audio_files::drop_audio(app, &[path.to_path_buf()], Some(pos));
        }
    }
    app.samples.notice = Some(crate::i18n::tf(
        "samples.notice.dropped",
        &[("name", &name)],
    ));
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl CantusApp {
    /// A new channel playing `path` on one sample pad (FL's sampler channel), inserted at
    /// `index`, with a mixer strip of its own: one undo step.
    pub fn add_sample_channel(&mut self, path: &Path, index: usize) -> Option<TrackId> {
        let name = file_stem(path);
        let mut pad = DrumPad::new(60, &name, PadVoice::Sample);
        pad.decay = cantus_core::PadField::Decay.info(PadVoice::Sample).default;
        pad.choke = 0;
        pad.sample = Some(path.to_string_lossy().into_owned());
        let mut track = self.bus.new_track(name);
        let n = self.project().tracks.len();
        track.color = cantus_core::Color::PALETTE[n % cantus_core::Color::PALETTE.len()];
        track.strip = crate::audio_files::free_strips(self.project())
            .first()
            .copied();
        track.instrument = InstrumentSpec::Pulsus(PulsusParams::single(pad));
        let id = track.id;
        self.engine.samples.retry(path);
        let index = index.min(n);
        let ok = self.exec(Command::Batch {
            label_key: "cmd.samples_add_channel".into(),
            commands: vec![Command::InsertTrack {
                index,
                track: Box::new(track),
            }],
        });
        if ok {
            self.select_track(id);
            Some(id)
        } else {
            None
        }
    }
}

/// While a sample is dragged: what it would land on, for the view to light it.
pub fn hovered(app: &CantusApp, ctx: &Context) -> Option<(Rect, Target)> {
    app.samples.drag.as_ref()?;
    let pos = ctx.input(|i| i.pointer.latest_pos())?;
    target_at(ctx, pos)
}

/// End of a frame: a sample let go of becomes `released`, resolved next frame.
pub fn finish_drag(app: &mut CantusApp, ctx: &Context) {
    if app.samples.drag.is_none() {
        return;
    }
    let (down, pos) = ctx.input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
    if !down {
        let drag = app.samples.drag.take().expect("checked");
        if let Some(pos) = pos {
            app.samples.released = Some((drag.path, pos));
            ctx.request_repaint();
        }
    }
}
