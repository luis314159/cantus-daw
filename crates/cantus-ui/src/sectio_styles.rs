//! Sectio's slicing styles and its editor's commands (round 37, ADR 0099): a style says how
//! a loop is cut and how its slices play, applied to the user's own loop (no audio ships
//! with one); the editor moves, adds and removes points and re-cuts the loop — each one
//! undo step, the zones rebuilt from the points.

use super::spec_of;
use crate::app::CantusApp;
#[allow(unused_imports)]
use crate::i18n::t;
use crate::i18n::tf;
use cantus_core::inst::sampler::Zone;
use cantus_core::sectio::{SliceMode, Slicing, FIRST_KEY, MAX_SLICES};
use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec, TrackId};
use std::path::Path;

/// A slicing style. Stored as `sl.style` (1-based); a style preset on an empty Exemplar
/// plays a few built-in sounds until a loop is dropped on it.
#[derive(Clone, Copy, Debug)]
pub struct SliceStyle {
    pub key: &'static str,
    pub mode: SliceMode,
    pub sens: f32,
    pub grid: u8,
    /// Every slice cuts the one before (one exclusive group).
    pub choke: bool,
    /// Each slice plays to its end whatever the key does — else only while held (a gate).
    pub one_shot: bool,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// One voice and a glide (ms): legato.
    pub legato: Option<f32>,
    pub tags: &'static [&'static str],
}

#[allow(clippy::too_many_arguments)] // a table row, read as one
const fn style(
    key: &'static str,
    mode: SliceMode,
    sens: f32,
    grid: u8,
    choke: bool,
    one_shot: bool,
    env: (f32, f32),
    legato: Option<f32>,
    tags: &'static [&'static str],
) -> SliceStyle {
    SliceStyle {
        key,
        mode,
        sens,
        grid,
        choke,
        one_shot,
        attack_ms: env.0,
        release_ms: env.1,
        legato,
        tags,
    }
}

pub const STYLES: &[SliceStyle] = &[
    style(
        "slicer.style.break",
        SliceMode::Transients,
        0.6,
        4,
        true,
        true,
        (0.1, 8.0),
        None,
        &[
            "preset.role.drums",
            "preset.style.lofi",
            "preset.style.trap",
            "slicer.tag.style",
        ],
    ),
    style(
        "slicer.style.oneshots",
        SliceMode::Transients,
        0.4,
        4,
        false,
        true,
        (0.1, 200.0),
        None,
        &[
            "preset.role.drums",
            "preset.style.house",
            "preset.style.pop",
            "slicer.tag.style",
        ],
    ),
    style(
        "slicer.style.stutter",
        SliceMode::Grid,
        0.5,
        8,
        true,
        true,
        (0.5, 3.0),
        None,
        &[
            "preset.role.drums",
            "preset.style.trap",
            "preset.style.house",
            "slicer.tag.style",
        ],
    ),
    style(
        "slicer.style.markers",
        SliceMode::Markers,
        0.5,
        4,
        true,
        true,
        (0.1, 10.0),
        None,
        &["preset.role.drums", "preset.style.rock", "slicer.tag.style"],
    ),
    style(
        "slicer.style.vocal",
        SliceMode::Grid,
        0.5,
        1,
        false,
        false,
        (3.0, 15.0),
        None,
        &[
            "preset.role.lead",
            "preset.style.pop",
            "preset.style.rnb",
            "slicer.tag.style",
        ],
    ),
    style(
        "slicer.style.melodic",
        SliceMode::Grid,
        0.5,
        2,
        false,
        false,
        (5.0, 40.0),
        Some(40.0),
        &[
            "preset.role.keys",
            "preset.style.ambient",
            "preset.style.film",
            "slicer.tag.style",
        ],
    ),
];

/// The style `spec` stores, if any.
pub fn style_of(spec: &NativeSpec) -> Option<&'static SliceStyle> {
    let i = spec.get("sl.style").round() as usize;
    i.checked_sub(1).and_then(|i| STYLES.get(i))
}

/// The style's instrument-wide numbers on `spec`.
fn style_values(st: &SliceStyle, index: usize, spec: &mut NativeSpec) {
    spec.set("sl.style", index as f32 + 1.0);
    spec.set("sl.mode", st.mode.index());
    spec.set("sl.sens", st.sens);
    spec.set("sl.grid", st.grid as f32);
    spec.set("amp.attack", st.attack_ms);
    spec.set("amp.sustain", 100.0);
    spec.set("amp.release", st.release_ms);
    if let Some(g) = st.legato {
        spec.set("voices", 1.0);
        spec.set("glide", g);
    }
}

/// Each zone present played the style's way: choked or not, one-shot or held.
pub fn finish_style(spec: &mut NativeSpec) {
    let Some(st) = style_of(spec) else {
        return;
    };
    let excl = if st.choke { 1.0 } else { 0.0 };
    let trig = if st.one_shot { 1.0 } else { 0.0 };
    for n in 0..MAX_SLICES {
        if Zone::of(spec, n).is_none() {
            break;
        }
        spec.set(&format!("z{n}.excl"), excl);
        spec.set(&format!("z{n}.offby"), excl);
        spec.set(&format!("z{n}.trig"), trig);
    }
}

/// A style preset's target made the user's: when `now` is a sliced Exemplar, its own loop
/// re-cut and played the style's way (one undo step, as any preset); otherwise the preset
/// as it is. A preset that is not a style passes through untouched.
pub fn through_style(now: &NativeSpec, target: &NativeSpec, song_bpm: f64) -> NativeSpec {
    // Only a style as the library ships it (no points of its own): A/B, a saved user preset
    // or an undo carry their points and every slice's settings, and are applied as they are.
    let Some(st) = style_of(target) else {
        return target.clone();
    };
    if now.kind != InstrumentKind::Sampler || target.get("sl.n") >= 1.0 {
        return target.clone();
    }
    let Some(sample) = Slicing::of(now)
        .and(Zone::of(now, 0))
        .and_then(|z| z.sample)
    else {
        return target.clone();
    };
    // The loop's own tempo, else the song's (a grid needs one).
    let tempo = Slicing::of(now)
        .map(|s| s.bpm as f64)
        .filter(|b| *b > 0.0)
        .unwrap_or(song_bpm);
    let Ok(a) = super::analyse_full(Path::new(&sample), st.mode, st.sens, st.grid, tempo) else {
        return target.clone();
    };
    let s = a.slicing;
    let mut base = NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in &target.params {
        if cantus_core::inst::sampler::zone_key(k).is_none() {
            base.params.insert(k.clone(), *v);
        }
    }
    // Its macros' names and routes too.
    for (k, v) in &target.text {
        if cantus_core::inst::sampler::zone_key(k).is_none() {
            base.text.insert(k.clone(), v.clone());
        }
    }
    let mut spec = spec_of(&sample, &s, Some(&base));
    spec.set("sl.style", target.get("sl.style"));
    finish_style(&mut spec);
    spec
}

/// Four macros on every style: the slices' release, attack, pitch and glide.
const STYLE_MACROS: &[(&str, &str)] = &[
    ("macro1.name", "Release"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "amp.release"),
    ("macro2.name", "Attack"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "amp.attack"),
    ("macro3.name", "Pitch"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "tune"),
    ("macro4.name", "Glide"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "glide"),
];

/// Exemplar's presets for the library: its own and Sectio's styles (each with built-in
/// sounds to audition until a loop is dropped on it).
pub fn sampler_presets() -> &'static [crate::views::inst::FactoryPreset] {
    use std::sync::OnceLock;
    static ALL: OnceLock<Vec<crate::views::inst::FactoryPreset>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = crate::views::inst::sampler::FACTORY_PRESETS.to_vec();
        for (i, st) in STYLES.iter().enumerate() {
            let mut spec = NativeSpec::new(InstrumentKind::Sampler);
            style_values(st, i, &mut spec);
            for (k, v) in [
                ("mod1.amt", 0.5),
                ("mod2.amt", 0.3),
                ("mod3.amt", 0.2),
                ("mod4.amt", 0.3),
            ] {
                spec.set(k, v);
            }
            audition_content(st, &mut spec);
            finish_style(&mut spec);
            let values: Vec<(&'static str, f32)> = spec
                .params
                .iter()
                .map(|(k, v)| (&*Box::leak(k.clone().into_boxed_str()), *v))
                .collect();
            all.push(crate::views::inst::FactoryPreset {
                label_key: st.key,
                values: Box::leak(values.into_boxed_slice()),
                text: STYLE_MACROS,
                tags: st.tags,
            });
        }
        all
    })
}

/// A style preset's sounds before a loop: built-ins — a kit from C1 up for a drum style,
/// the choir for a vocal one, the plucked string for a melodic one.
fn audition_content(st: &SliceStyle, spec: &mut NativeSpec) {
    if st.tags.contains(&"preset.role.drums") {
        let gens = [
            12.0, 16.0, 13.0, 16.0, 14.0, 17.0, 13.0, 18.0, 15.0, 16.0, 13.0, 17.0,
        ];
        for (n, g) in gens.iter().enumerate() {
            let key = FIRST_KEY as f32 + n as f32;
            // The zones' own gain stages the audition at the library's level for its role
            // (measured by `preset_render.rs`); a user's loop replaces these zones.
            for (k, v) in [
                ("gen", *g),
                ("root", key),
                ("lo", key),
                ("hi", key),
                ("gain", 3.2),
            ] {
                spec.set(&format!("z{n}.{k}"), v);
            }
            spec.set(&format!("z{n}.group"), n as f32);
        }
    } else {
        let g = if st.key == "slicer.style.vocal" {
            21.0
        } else {
            7.0
        };
        // The choir's chords stay under full scale on the dense riff (−1.7 dB on its level).
        let gain = if g == 21.0 { 4.0 } else { 9.7 };
        for (k, v) in [
            ("gen", g),
            ("root", 60.0),
            ("lo", 24.0),
            ("hi", 96.0),
            ("gain", gain),
        ] {
            spec.set(&format!("z0.{k}"), v);
        }
    }
}

/// A preview of re-cutting the loop another way: the points only.
/// `song_bpm` lent to a loop with no tempo of its own, as *Apply* lends it.
pub fn preview(
    spec: &NativeSpec,
    mode: SliceMode,
    sens: f32,
    grid: u8,
    song_bpm: f64,
) -> Option<Slicing> {
    let z = Zone::of(spec, 0)?;
    super::analyse_full(Path::new(&z.sample?), mode, sens, grid, song_bpm)
        .ok()
        .map(|a| a.slicing)
}

/// The zero crossing of channel 0 of `data` nearest frame `at`, within `reach` frames.
pub fn zero_crossing(data: &[f32], channels: usize, at: u32, reach: u32) -> u32 {
    let ch = channels.max(1);
    let frames = (data.len() / ch) as u32;
    if frames < 2 {
        return 0;
    }
    let at = at.min(frames - 1);
    let v = |f: u32| data[f as usize * ch];
    for d in 0..=reach {
        for f in [at.saturating_sub(d), (at + d).min(frames - 1)] {
            if f > 0 && (v(f - 1) <= 0.0) != (v(f) <= 0.0) {
                return f;
            }
        }
    }
    at
}

/// The spec with its slices at `points` (zones rebuilt from them). Each slice's own settings
/// come with it: `moved` (a point moved between its neighbours: the slices keep their order)
/// by index, else from the old slice its start lies in.
fn with_points(spec: &NativeSpec, mut s: Slicing, moved: bool) -> Option<NativeSpec> {
    let sample = Zone::of(spec, 0)?.sample?;
    s.points.sort_unstable();
    s.points.dedup();
    if s.points.first() != Some(&0) {
        s.points.insert(0, 0);
    }
    // On the samples the engine starts a tick at (the dump replays the file to the bit).
    let s = super::snap(s).tidy();
    let next = spec_of(&sample, &s, Some(spec));
    Some(carry(spec, next, &s, moved))
}

/// `next` with each slice's own settings from the old slice whose audio it starts in (a
/// point added inside slice 2 leaves slice 5's reverse on slice 5's audio).
fn carry(old: &NativeSpec, mut next: NativeSpec, s: &Slicing, by_index: bool) -> NativeSpec {
    let Some(was) = Slicing::of(old) else {
        return next;
    };
    for (n, p) in s.points.iter().enumerate() {
        let j = if by_index {
            n
        } else {
            was.points.iter().rposition(|q| q <= p).unwrap_or(0)
        };
        if let Some(old) = Zone::of(old, j) {
            for f in [
                cantus_core::inst::sampler::zf::TUNE,
                cantus_core::inst::sampler::zf::GAIN,
                cantus_core::inst::sampler::zf::PAN,
                cantus_core::inst::sampler::zf::REV,
                cantus_core::inst::sampler::zf::EXCL,
                cantus_core::inst::sampler::zf::OFFBY,
                cantus_core::inst::sampler::zf::TRIG,
                cantus_core::inst::sampler::zf::LPF,
            ] {
                let key = format!("z{n}.{}", cantus_core::inst::sampler::zone_field_name(f));
                next.set(&key, old.values[f]);
            }
        }
    }
    if old.get("sl.style") >= 1.0 {
        next.set("sl.style", old.get("sl.style"));
    }
    next
}

impl CantusApp {
    fn set_slices(&mut self, track: TrackId, next: NativeSpec, label: &str) -> bool {
        self.exec(Command::Batch {
            label_key: label.into(),
            commands: vec![Command::SetInstrument {
                track,
                spec: Box::new(InstrumentSpec::Native(next)),
            }],
        })
    }

    /// Move slice `i`'s start (never the first's) to `frame`, kept between its neighbours,
    /// one undo step.
    pub fn move_slice(&mut self, track: TrackId, i: usize, frame: u32) -> bool {
        let Some(spec) = self.native_of(track) else {
            return false;
        };
        let Some(mut s) = Slicing::of(&spec) else {
            return false;
        };
        if i == 0 || i >= s.points.len() {
            return false;
        }
        let gap = (s.rate / 200).max(1); // 5 ms
        let lo = s.points[i - 1] + gap;
        let hi = s
            .points
            .get(i + 1)
            .copied()
            .unwrap_or(s.len)
            .saturating_sub(gap);
        if lo >= hi {
            return false;
        }
        let f = frame.clamp(lo, hi);
        if f == s.points[i] {
            return false;
        }
        s.points[i] = f;
        match with_points(&spec, s, true) {
            Some(next) => self.set_slices(track, next, "cmd.slice_move"),
            None => false,
        }
    }

    /// A new slice point at `frame`, one undo step (none within 5 ms of another, at most 64).
    pub fn add_slice(&mut self, track: TrackId, frame: u32) -> bool {
        let Some(spec) = self.native_of(track) else {
            return false;
        };
        let Some(mut s) = Slicing::of(&spec) else {
            return false;
        };
        let gap = (s.rate / 200).max(1);
        if s.points.len() >= MAX_SLICES
            || frame >= s.len
            || s.points.iter().any(|p| p.abs_diff(frame) < gap)
        {
            return false;
        }
        s.points.push(frame);
        s.mode = SliceMode::Manual;
        match with_points(&spec, s, false) {
            Some(next) => self.set_slices(track, next, "cmd.slice_add"),
            None => false,
        }
    }

    /// Slice `i`'s point taken away (never the first), one undo step.
    pub fn remove_slice(&mut self, track: TrackId, i: usize) -> bool {
        let Some(spec) = self.native_of(track) else {
            return false;
        };
        let Some(mut s) = Slicing::of(&spec) else {
            return false;
        };
        if i == 0 || i >= s.points.len() {
            return false;
        }
        s.points.remove(i);
        s.mode = SliceMode::Manual;
        match with_points(&spec, s, false) {
            Some(next) => self.set_slices(track, next, "cmd.slice_remove"),
            None => false,
        }
    }

    /// Re-cut the loop as `mode`, `sens`, `grid` say, one undo step (the instrument's own
    /// numbers and its style kept).
    pub fn reslice(&mut self, track: TrackId, mode: SliceMode, sens: f32, grid: u8) -> bool {
        let Some(spec) = self.native_of(track) else {
            return false;
        };
        let Some(sample) = Zone::of(&spec, 0).and_then(|z| z.sample) else {
            return false;
        };
        let tempo = self.project().tempo_bpm;
        let a = match super::analyse_full(Path::new(&sample), mode, sens, grid.max(1), tempo) {
            Ok(a) => a,
            Err(e) => {
                self.set_error(tf("slicer.failed", &[("why", &e)]));
                return false;
            }
        };
        let next = carry(
            &spec,
            spec_of(&sample, &a.slicing, Some(&spec)),
            &a.slicing,
            false,
        );
        let ok = self.set_slices(track, next, "cmd.slice_again");
        if ok {
            self.set_status(a.said());
        }
        ok
    }
}
