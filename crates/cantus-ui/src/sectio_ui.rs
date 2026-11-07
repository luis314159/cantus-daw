//! Sectio in the app (round 37, ADR 0099): a loop sliced into an Exemplar, its slices
//! dumped to a clip, the clip's three gestures — each one undo step.
//!
//! A file is sliced where its markers say (WAV `cue `/`smpl`, its ACID chunk's beats give
//! the tempo), else at its transients, on a beat grid, or by hand. When the loop's tempo is
//! known every point is put on the sample the engine plays a tick at, at that tempo
//! (`cantus_engine::transport::sample_at`), so the dumped clip replays the file to the bit
//! at the file's own tempo.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::inst::sampler::Zone;
use cantus_core::sectio::{self, SliceMode, Slicing};
use cantus_core::sectio_ops;
use cantus_core::{ClipId, Command, InstrumentKind, InstrumentSpec, NativeSpec, Ticks, TrackId};
use std::path::Path;

#[path = "sectio_styles.rs"]
pub mod styles;

/// The file's audio, mono, with its rate and length.
pub struct Loop {
    pub mono: Vec<f32>,
    pub rate: u32,
    pub frames: u32,
}

/// Decode `path` (any format Exemplar plays).
pub fn read_loop(path: &Path) -> Result<Loop, String> {
    let d = cantus_engine::samples::assets::decode_any(path)?;
    let ch = d.channels.max(1) as usize;
    // A sample that is not a number is silence to the analysis (one would blind it).
    let mono: Vec<f32> = d
        .frames
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .map(|v| if v.is_finite() { v } else { 0.0 })
        .collect();
    Ok(Loop {
        frames: mono.len() as u32,
        mono,
        rate: d.rate,
    })
}

/// Where to cut `path`: by `mode` (markers fall back to transients when the file has
/// none), `sens` (transients), `grid` steps a beat (a grid); points on the engine's samples
/// at the loop's tempo when it is known.
pub fn analyse(path: &Path, mode: SliceMode, sens: f32, grid: u8) -> Result<Slicing, String> {
    analyse_counted(path, mode, sens, grid).map(|(s, _)| s)
}

/// [`analyse`], and how many points past the 64th were left out (review 2 #9).
pub fn analyse_counted(
    path: &Path,
    mode: SliceMode,
    sens: f32,
    grid: u8,
) -> Result<(Slicing, usize), String> {
    analyse_full(path, mode, sens, grid, 0.0).map(|a| (a.slicing, a.dropped))
}

/// The longest loop a slice point can address: points and lengths are stored as `f32`
/// parameters, exact to 2²⁴ frames (5 min 49 s at 48 kHz).
pub const LONGEST: u32 = 16_777_215;

/// What [`analyse_full`] found, and what it had to give up to fit.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub slicing: Slicing,
    /// Points past the 64th left out (markers only: transients keep their strongest 64,
    /// a grid coarsens).
    pub dropped: usize,
    /// The grid's steps a beat when the asked ones made more than 64 slices.
    pub coarsened: Option<u8>,
    /// The loop had no tempo of its own and the song's was used.
    pub song_tempo: bool,
}

impl Analysis {
    /// The status line's report.
    pub fn said(&self) -> String {
        let s = &self.slicing;
        let mut out = tf(
            "slicer.sliced",
            &[("n", &s.points.len()), ("mode", &t(s.mode.name_key()))],
        );
        if self.dropped > 0 {
            out = tf("slicer.dropped", &[("n", &self.dropped)]);
        }
        if let Some(g) = self.coarsened {
            out.push_str(" · ");
            out.push_str(&tf("slicer.coarsened", &[("n", &g)]));
        }
        if self.song_tempo {
            out.push_str(" · ");
            out.push_str(&t("slicer.song_tempo"));
        }
        out
    }
}

/// [`analyse`] with all it reports: at most 64 slices — the strongest transients, spread
/// over the whole loop; a grid coarsened until it fits — and `song_bpm` lent to a loop with
/// no tempo of its own when a grid needs one. A loop past [`LONGEST`] frames is refused.
pub fn analyse_full(
    path: &Path,
    mode: SliceMode,
    sens: f32,
    grid: u8,
    song_bpm: f64,
) -> Result<Analysis, String> {
    use cantus_core::sectio::MAX_SLICES;
    let lp = read_loop(path)?;
    if lp.frames > LONGEST {
        return Err(t("slicer.too_long"));
    }
    let marks = std::fs::read(path)
        .ok()
        .and_then(|b| cantus_dsp::sectio::markers(&b));
    let own = marks
        .as_ref()
        .and_then(|m| m.bpm())
        .map(|b| b as f64)
        .or_else(|| cantus_dsp::sectio::tempo(&lp.mono, lp.rate as f32));
    let mut song_tempo = false;
    let marked = marks.as_ref().map(|m| m.points()).unwrap_or_default();
    let (mut dropped, mut coarsened) = (0usize, None);
    let mut bpm = own.unwrap_or(0.0);
    let (mode, mut points) = match mode {
        SliceMode::Markers if !marked.is_empty() => {
            let mut p = marked;
            p.retain(|x| *x < lp.frames);
            p.sort_unstable();
            p.dedup();
            if p.first() != Some(&0) {
                p.insert(0, 0);
            }
            dropped = p.len().saturating_sub(MAX_SLICES);
            (SliceMode::Markers, p)
        }
        SliceMode::Markers | SliceMode::Transients => {
            let ranked = cantus_dsp::sectio::transients_ranked(&lp.mono, lp.rate as f32, sens);
            (
                SliceMode::Transients,
                cantus_dsp::sectio::strongest(&ranked, MAX_SLICES),
            )
        }
        SliceMode::Grid => {
            if own.is_none() && song_bpm > 0.0 {
                bpm = song_bpm;
                song_tempo = true;
            }
            let mut steps = grid.max(1) as u32;
            let mut p = cantus_dsp::sectio::grid(lp.frames, lp.rate, bpm, steps, 0);
            while p.len() > MAX_SLICES && steps > 1 {
                steps /= 2;
                p = cantus_dsp::sectio::grid(lp.frames, lp.rate, bpm, steps, 0);
                coarsened = Some(steps as u8);
            }
            // Still too many (a long loop at one step a beat): every n-th beat.
            if p.len() > MAX_SLICES {
                let every = p.len().div_ceil(MAX_SLICES);
                p = p.into_iter().step_by(every).collect();
            }
            (SliceMode::Grid, p)
        }
        SliceMode::Manual => (SliceMode::Manual, vec![0]),
    };
    if points.first() != Some(&0) {
        points.insert(0, 0);
    }
    let grid = coarsened.unwrap_or(grid);
    let s = Slicing {
        mode,
        sens,
        bpm: bpm as f32,
        grid,
        len: lp.frames,
        rate: lp.rate,
        points,
    };
    Ok(Analysis {
        slicing: snap(s).tidy(),
        dropped,
        coarsened,
        song_tempo,
    })
}

/// Every point on the sample the engine starts a note at, for the tick nearest it, at the
/// loop's tempo: what makes the dump exact.
pub fn snap(mut s: Slicing) -> Slicing {
    if s.bpm <= 0.0 || s.rate == 0 {
        return s;
    }
    let (rate, bpm) = (s.rate, s.bpm as f64);
    for p in s.points.iter_mut() {
        let tick = sectio::tick_of_frame(*p, rate, bpm);
        *p = cantus_engine::transport::sample_at(0.0, tick as f64, bpm, rate as f64) as u32;
    }
    s
}

/// An Exemplar playing `s` from `sample`: the instrument's own numbers from `base` (when
/// re-slicing an Exemplar), its zones and slice points new. Each slice starts at once and
/// holds (attack at its least, sustain full), so a slice plays its frames as they are.
pub fn spec_of(sample: &str, s: &Slicing, base: Option<&NativeSpec>) -> NativeSpec {
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    if let Some(b) = base {
        for (k, v) in &b.params {
            let zone = cantus_core::inst::sampler::zone_key(k).is_some();
            if !zone && (!k.starts_with("sl.") || k == "sl.style") {
                spec.params.insert(k.clone(), *v);
            }
        }
        for (k, v) in &b.text {
            if cantus_core::inst::sampler::zone_key(k).is_none() {
                spec.text.insert(k.clone(), v.clone());
            }
        }
    } else {
        spec.set("amp.attack", 0.1);
        spec.set("amp.sustain", 100.0);
        spec.set("amp.release", 5.0);
    }
    for (k, v) in s.values() {
        spec.set(&k, v);
    }
    let zones: Vec<Zone> = s.zones(sample);
    for (n, z) in zones.iter().enumerate() {
        for (k, v) in z.values_for(n) {
            spec.set(&k, v);
        }
        for (k, text) in z.texts_for(n) {
            let _ = spec.set_text(&k, Some(text));
        }
    }
    spec
}

/// The dumped clip's notes: each slice at the tick its point is on, until the next.
pub fn dump_notes(s: &Slicing) -> Vec<sectio::DumpNote> {
    dump_notes_at(s, 0.0)
}

/// [`dump_notes`], a loop with no tempo of its own (a one-shot, too few hits, sliced by
/// hand) read at the song's `song_bpm`; none when neither has one (review 2 #6).
pub fn dump_notes_at(s: &Slicing, song_bpm: f64) -> Vec<sectio::DumpNote> {
    let mut s = s.clone();
    if s.bpm <= 0.0 {
        if song_bpm <= 0.0 {
            return Vec::new();
        }
        s.bpm = song_bpm as f32;
    }
    let mut ticks: Vec<i64> = s.points.iter().map(|p| s.tick_of(*p)).collect();
    ticks.push(s.tick_of(s.len));
    // Two points on one tick (a tempo too slow for them) play once.
    let mut out = sectio::dump(&ticks);
    out.retain(|n| n.1 > 0);
    out
}

/// A gesture on a dumped clip.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Gesture {
    Random(u64),
    Stutter {
        amount: f64,
        repeats: u32,
        seed: u64,
    },
    Quantize {
        grid: i64,
        swing: f64,
    },
}

impl CantusApp {
    /// Slice `path` into `track`'s instrument (an Exemplar, made one if it is not), as one
    /// undo step. Whether it was.
    pub fn slice_into_track(&mut self, track: TrackId, path: &Path, mode: SliceMode) -> bool {
        let base = self
            .native_of(track)
            .filter(|n| n.kind == InstrumentKind::Sampler);
        let (sens, grid) = base
            .as_ref()
            .map(|b| (b.get("sl.sens"), b.get("sl.grid").round() as u8))
            .unwrap_or((0.5, 4));
        // A slicing style the Exemplar holds cuts the loop its own way.
        let style = base.as_ref().and_then(styles::style_of);
        let mode = style.map_or(mode, |st| st.mode);
        let tempo = self.project().tempo_bpm;
        let a = match analyse_full(path, mode, sens, grid.max(1), tempo) {
            Ok(a) => a,
            Err(e) => {
                self.set_error(tf("slicer.failed", &[("why", &e)]));
                return false;
            }
        };
        let s = a.slicing.clone();
        let mut spec = spec_of(&path.to_string_lossy(), &s, base.as_ref());
        // A new loop on a styled Exemplar is played the style's way (its first cut).
        styles::finish_style(&mut spec);
        let ok = self.exec(Command::Batch {
            label_key: "cmd.slice_sample".into(),
            commands: vec![Command::SetInstrument {
                track,
                spec: Box::new(InstrumentSpec::Native(spec)),
            }],
        });
        if ok {
            self.set_status(a.said());
        }
        ok
    }

    /// A new track whose Exemplar plays `path` sliced: what dropping a loop on the empty
    /// part of the channel rack or the browser's *Slice* does (one undo step).
    pub fn add_sliced_track(&mut self, path: &Path) -> Option<TrackId> {
        let tempo = self.project().tempo_bpm;
        let s = match analyse_full(path, SliceMode::Markers, 0.5, 4, tempo) {
            Ok(a) => a.slicing,
            Err(e) => {
                self.set_error(tf("slicer.failed", &[("why", &e)]));
                return None;
            }
        };
        let name = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t("slicer.track"));
        let mut tr = self.bus.new_track(name);
        tr.instrument = InstrumentSpec::Native(spec_of(&path.to_string_lossy(), &s, None));
        let id = tr.id;
        let index = self.project().tracks.len();
        if !self.exec(Command::InsertTrack {
            index,
            track: Box::new(tr),
        }) {
            return None;
        }
        self.select_track(id);
        Some(id)
    }

    /// Dump `track`'s slices to a new clip at `at`, one undo step: the loop replayed slice by
    /// slice. The clip's id.
    pub fn dump_slices(&mut self, track: TrackId, at: Ticks) -> Option<ClipId> {
        let s = self.native_of(track).and_then(|n| Slicing::of(&n))?;
        let notes = dump_notes_at(&s, self.project().tempo_bpm);
        if notes.is_empty() {
            self.set_error(t("slicer.no_tempo"));
            return None;
        }
        let end = notes.iter().map(|n| n.0 + n.1).max().unwrap_or(0).max(1);
        let mut clip = self.bus.new_clip(t("slicer.dump_name"), at, Ticks(end));
        for (tick, len, key, vel) in notes {
            clip.notes
                .push(self.bus.new_note(Ticks(tick), Ticks(len), key, vel));
        }
        let id = clip.id;
        self.exec(Command::Batch {
            label_key: "cmd.slice_dump".into(),
            commands: vec![Command::InsertClip {
                track,
                clip: Box::new(clip),
            }],
        })
        .then_some(id)
    }

    /// Random, Stutter or Quantize on a clip's notes, one undo step.
    pub fn slice_gesture(&mut self, track: TrackId, clip: ClipId, g: Gesture) -> bool {
        let Some(c) = self
            .project()
            .track(track)
            .and_then(|t| t.clips.iter().find(|c| c.id == clip))
            .cloned()
        else {
            return false;
        };
        let (label, commands) = match g {
            Gesture::Random(seed) => (
                "cmd.slice_random",
                vec![Command::SetNotes {
                    track,
                    clip,
                    notes: sectio_ops::random(&c.notes, seed),
                }],
            ),
            Gesture::Quantize { grid, swing } => (
                "cmd.slice_quantize",
                vec![Command::SetNotes {
                    track,
                    clip,
                    notes: sectio_ops::quantize(&c.notes, grid, swing),
                }],
            ),
            Gesture::Stutter {
                amount,
                repeats,
                seed,
            } => {
                let bus = &mut self.bus;
                let mut id = || bus.new_note(Ticks::ZERO, Ticks(1), 0, 1).id;
                let (kept, made) = sectio_ops::stutter(&c.notes, amount, repeats, seed, &mut id);
                let gone: Vec<_> = c
                    .notes
                    .iter()
                    .filter(|n| !kept.iter().any(|k| k.id == n.id))
                    .map(|n| n.id)
                    .collect();
                let mut cmds = Vec::new();
                if !gone.is_empty() {
                    cmds.push(Command::RemoveNotes {
                        track,
                        clip,
                        ids: gone,
                    });
                }
                if !made.is_empty() {
                    cmds.push(Command::InsertNotes {
                        track,
                        clip,
                        notes: made,
                    });
                }
                ("cmd.slice_stutter", cmds)
            }
        };
        if commands.is_empty() {
            return false;
        }
        self.exec(Command::Batch {
            label_key: label.into(),
            commands,
        })
    }
}
