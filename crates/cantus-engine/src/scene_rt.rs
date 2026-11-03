//! The mix cues' switches on the audio thread (round 36, ADR 0093 §3): mute and solo (as
//! audibility), polarity, swap, the inserts' switch, each insert's bypass, the strip EQ and
//! the master's polarity, inserts' switch and bypasses, region by region of the snapshot's
//! `mix_scenes`.
//!
//! Read at the automation's control points (`processor_auto`), by **tick**: the region whose
//! start is the last at or before the song position. A change never cuts: the strips (and the
//! master) whose switches change are faded out over the control segment before the grid
//! point where the cue's tick is reached, switched there, and faded in over the 16 samples
//! after (a seek or a loop wrap switches at once and fades in). The grid is the engine's
//! 16-sample one counted from the transport's origin, so a block size, an offline render or
//! a parallel run hears the same. Nothing allocates: the timeline is the snapshot's, and the
//! switches are written into the strips' own fields.

use super::*;
use crate::automation::{At, Ramp, GRID};
use cantus_core::mix_scene::{region_at, MixScene};

impl Processor {
    /// Take back the fades of the last control point (they last one segment).
    fn scene_clear(&mut self, nodes: &exec::Nodes) {
        for (slot, on) in self.auto.scene_env.iter_mut().enumerate() {
            if std::mem::take(on) {
                if let Some(mut s) = nodes.strip(slot) {
                    s.scene_env = None;
                }
            }
        }
        self.auto.master_env = None;
    }

    /// Write region `to`'s switches; fade in what changed from region `from` when `fade`.
    fn scene_apply(
        &mut self,
        scenes: &[MixScene],
        from: usize,
        to: usize,
        len: u64,
        fade: bool,
        nodes: &exec::Nodes,
    ) {
        let (Some(a), Some(b)) = (scenes.get(from), scenes.get(to)) else {
            return;
        };
        let sr = self.sample_rate as f32;
        for (si, st) in b.strips.iter().enumerate().take(MAX_STRIPS) {
            if a.strips.get(si) == Some(st) && from != to {
                continue;
            }
            let Some(&slot) = self.strip_to_slot.get(si) else {
                continue;
            };
            let Some(mut s) = nodes.strip(slot) else {
                continue;
            };
            s.muted = st.silent;
            s.phase_invert = st.phase_invert;
            s.swap = st.swap;
            s.fx_enabled = st.fx_enabled;
            let n = s.effects.len().min(MAX_EFFECTS_PER_TRACK);
            for (k, by) in st.bypass.iter().enumerate().take(n) {
                s.bypass[k] = *by;
            }
            s.eq_flat = st.eq.is_flat();
            for (i, band) in st.eq.bands.iter().enumerate() {
                s.eq_l.set_band(i, sr, band.freq_hz, band.gain_db, band.q);
                s.eq_r.set_band(i, sr, band.freq_hz, band.gain_db, band.q);
            }
            if fade {
                s.scene_env = Ramp::of(0.0, 1.0, len);
                self.auto.scene_env[slot] = true;
            }
        }
        if a.master != b.master || from == to {
            let m = &b.master;
            self.scene_master(m.fx_enabled, &m.bypass);
            self.auto.master_sign = Some(if m.phase_invert { -1.0 } else { 1.0 });
            if fade {
                self.auto.master_env = Ramp::of(0.0, 1.0, len);
            }
        }
    }

    /// Fade out, over this control segment, what changes when region `to` begins.
    fn scene_fade_out(
        &mut self,
        scenes: &[MixScene],
        from: usize,
        to: usize,
        len: u64,
        nodes: &exec::Nodes,
    ) {
        let (Some(a), Some(b)) = (scenes.get(from), scenes.get(to)) else {
            return;
        };
        for (si, st) in b.strips.iter().enumerate().take(MAX_STRIPS) {
            if a.strips.get(si) == Some(st) {
                continue;
            }
            let Some(&slot) = self.strip_to_slot.get(si) else {
                continue;
            };
            if let Some(mut s) = nodes.strip(slot) {
                s.scene_env = Ramp::of(1.0, 0.0, len);
                self.auto.scene_env[slot] = true;
            }
        }
        if a.master != b.master {
            self.auto.master_env = Ramp::of(1.0, 0.0, len);
        }
    }

    /// A snapshot was installed: its strips hold the project's switches (region 0); put
    /// the region of the playhead on them at once (an install is itself a cut).
    pub(super) fn scene_install(&mut self, snapshot: &Snapshot) {
        let nodes = self.nodes.clone();
        self.scene_clear(&nodes);
        self.auto.scene = 0;
        self.auto.master_sign = None;
        let scenes = &snapshot.mix_scenes;
        if scenes.is_empty() {
            return;
        }
        let at = region_at(scenes, self.transport.position_at(self.transport.samples()));
        if at != 0 {
            self.scene_apply(scenes, 0, at, GRID, false, &nodes);
            self.auto.scene = at;
        }
    }

    /// A control point at transport sample `now`, `len` samples to the next: the region
    /// played now, the fades, and when the next control point must come (`None`: no say).
    pub(super) fn scene_point(
        &mut self,
        snap: &Snapshot,
        at: &At,
        now: u64,
        len: u64,
        playing: bool,
        nodes: &exec::Nodes,
    ) -> Option<u64> {
        self.scene_clear(nodes);
        let scenes = &snap.mix_scenes;
        if scenes.is_empty() {
            return None;
        }
        let grid = now + len;
        let cur = region_at(scenes, at.tick);
        if cur != self.auto.scene {
            let from = self.auto.scene;
            self.scene_apply(scenes, from, cur, len, true, nodes);
            self.auto.scene = cur;
            // Back here when the fade in ends.
            return Some(grid);
        }
        if !playing {
            return None;
        }
        let next = region_at(scenes, at.next_tick);
        if next != cur {
            self.scene_fade_out(scenes, cur, next, len, nodes);
            return Some(grid);
        }
        // The grid point just before the next region begins, to fade out there.
        let tb = scenes.get(cur + 1)?.at.0 as f64;
        let at_sample = now + self.transport.samples_until(tb);
        Some(grid.max((at_sample.saturating_sub(1) / GRID) * GRID))
    }
}
