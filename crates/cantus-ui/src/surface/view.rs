//! The mixer as a control surface shows it.

use crate::app::CantusApp;
use cantus_core::external_surface::{StripView, SurfaceView, TransportView};
use cantus_core::{MixerStrip, Project};

/// The name a strip is shown with: its own, or the first track that feeds it, or its number.
pub fn strip_name(p: &Project, i: usize, s: &MixerStrip) -> String {
    if !s.name.trim().is_empty() {
        return s.name.clone();
    }
    p.tracks
        .iter()
        .find(|t| t.strip == Some(s.id))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| format!("{}", i + 1))
}

/// Whether a track feeding the strip is armed (a surface's Rec arms the tracks of a strip).
pub fn strip_armed(p: &Project, s: &MixerStrip) -> bool {
    p.tracks.iter().any(|t| t.strip == Some(s.id) && t.armed)
}

/// The whole mixer and transport as the surfaces show them.
pub fn surface_view(app: &CantusApp) -> SurfaceView {
    let p = app.project();
    let meters = app.engine.meters();
    let strips = p
        .strips
        .iter()
        .enumerate()
        .map(|(i, s)| StripView {
            name: strip_name(p, i, s),
            gain_db: s.gain_db,
            pan: s.pan,
            mute: s.muted,
            solo: s.soloed,
            arm: strip_armed(p, s),
            selected: app.selected_strip == Some(s.id),
            meter: meters.strips.get(i).copied().unwrap_or(0.0),
        })
        .collect();
    let master = StripView {
        name: "Master".into(),
        gain_db: p.master.gain_db,
        pan: p.master.pan,
        meter: meters.master[0].max(meters.master[1]),
        ..Default::default()
    };
    let (bar, beat, tick) = crate::views::bars::bbt(p, app.engine.position());
    SurfaceView {
        strips,
        master,
        transport: TransportView {
            playing: app.engine.is_playing(),
            recording: app.is_recording(),
            looping: p.loop_enabled,
            click: p.metronome,
            scrubbing: false,
            time: format!("{bar:03}.{beat:02}.{tick:03}"),
            beats: true,
        },
    }
}
