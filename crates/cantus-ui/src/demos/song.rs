//! The builder every demo song is written with: tracks with their instruments and their
//! named, coloured mixer strips; inserts with musical settings (a factory preset and a few
//! numbers moved); sends to shared returns; buses; the master chain; and clips of notes
//! with a player's feel — swing, timing that is not on the grid, velocities that breathe.
//!
//! Everything goes into a plain [`Project`]: what a demo builds is exactly what a
//! musician could have built by hand, saved and opened again.

use super::theory::{Ev, Rng};
use crate::i18n::t;
use cantus_core::{
    Color, Dock, DrumPad, EffectId, EffectKind, EffectSlot, InstrumentKind, InstrumentSpec,
    LoopRegion, Project, PulsusParams, Send, StripId, Ticks, TrackIcon,
};

/// How a part is played: how far off the grid (in beats), how much the velocities move,
/// and the swing (a fraction of `swing_grid` that every second subdivision is late by).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Feel {
    pub jitter: f64,
    pub vel: f64,
    pub swing: f64,
    pub swing_grid: f64,
}

impl Feel {
    /// A machine: on the grid, every velocity as written.
    pub const TIGHT: Feel = Feel {
        jitter: 0.0,
        vel: 0.0,
        swing: 0.0,
        swing_grid: 0.25,
    };
    /// A player: a few milliseconds either side, velocities within a few steps.
    pub const HUMAN: Feel = Feel {
        jitter: 0.012,
        vel: 6.0,
        swing: 0.0,
        swing_grid: 0.25,
    };

    pub fn swung(self, swing: f64, grid: f64) -> Feel {
        Feel {
            swing,
            swing_grid: grid,
            ..self
        }
    }
}

/// A track in the song and the strip it plays through.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub track: usize,
    pub strip: StripId,
}

/// The palette the demos colour with: families of sound, so the arrangement reads at a
/// glance (drums warm, low end deep, keys gold, pads cool, leads bright).
pub mod colors {
    use cantus_core::Color;
    pub const KICK: Color = Color::rgb(0xE0, 0x6A, 0x3A);
    pub const SNARE: Color = Color::rgb(0xE8, 0x8A, 0x3C);
    pub const HATS: Color = Color::rgb(0xE6, 0xB0, 0x48);
    pub const PERC: Color = Color::rgb(0xD9, 0x5D, 0x5D);
    pub const BASS: Color = Color::rgb(0x8A, 0x5C, 0xD6);
    pub const KEYS: Color = Color::rgb(0xD8, 0xC0, 0x4A);
    pub const ORGAN: Color = Color::rgb(0xC9, 0x8A, 0x3E);
    pub const PAD: Color = Color::rgb(0x4F, 0xA3, 0xC7);
    pub const STRINGS: Color = Color::rgb(0x4A, 0x8F, 0xD8);
    pub const LEAD: Color = Color::rgb(0xC9, 0x6F, 0x9E);
    pub const PLUCK: Color = Color::rgb(0x5C, 0xB8, 0x6E);
    pub const BELLS: Color = Color::rgb(0x6C, 0xC8, 0xB4);
    pub const BRASS: Color = Color::rgb(0xE0, 0x8E, 0x3A);
    pub const RETURN: Color = Color::rgb(0x7A, 0x86, 0x99);
    pub const BUS: Color = Color::rgb(0xB0, 0x56, 0x48);
}

/// How long a phrase is: sections longer than this are laid out a phrase per clip.
const PHRASE_BARS: f64 = 8.0;

pub struct Song {
    pub p: Project,
    feel: Vec<Feel>,
    rng: Rng,
    /// Beats in a bar.
    pub bar: f64,
}

impl Song {
    pub fn new(name: &str, bpm: f64, seed: u64) -> Song {
        let mut p = Project::new(name);
        p.tempo_bpm = bpm;
        p.loop_enabled = false;
        Song {
            p,
            feel: Vec::new(),
            rng: Rng::new(seed),
            bar: 4.0,
        }
    }

    pub fn ticks(&self, bars: f64) -> Ticks {
        Ticks::from_beats_f64(bars * self.bar)
    }

    /// A track with its instrument, colour and icon, on the next free strip, which takes
    /// the track's name and colour.
    pub fn part(
        &mut self,
        name: &str,
        instrument: InstrumentSpec,
        color: Color,
        icon: TrackIcon,
    ) -> Part {
        let mut track = self.p.make_track(name);
        track.instrument = instrument;
        track.color = color;
        track.icon = Some(icon);
        let strip = track.strip.expect("a demo has more strips than tracks");
        self.p.tracks.push(track);
        self.feel.push(Feel::HUMAN);
        let s = self.p.strip_mut(strip).expect("the strip exists");
        s.name = name.to_string();
        s.color = Some(color);
        Part {
            track: self.p.tracks.len() - 1,
            strip,
        }
    }

    pub fn feel(&mut self, part: Part, feel: Feel) {
        self.feel[part.track] = feel;
    }

    /// The channel's fader and pan.
    pub fn mix(&mut self, strip: StripId, gain_db: f32, pan: f32) {
        if let Some(s) = self.p.strip_mut(strip) {
            s.gain_db = gain_db;
            s.pan = pan;
        }
    }

    /// The channel's own volume before its strip (FL's channel volume): where each
    /// instrument is brought to a working level, so the faders sit where a mix sits.
    pub fn trim(&mut self, part: Part, gain_db: f32) {
        self.p.tracks[part.track].gain_db = gain_db;
    }

    /// A strip no track plays through, named: a bus in the middle of the desk, or a
    /// return docked on the right.
    fn free_strip(&mut self, name: &str, color: Color, dock: Dock) -> StripId {
        let used = |p: &Project, id: StripId| {
            p.tracks.iter().any(|t| t.strip == Some(id))
                || p.strip(id).is_some_and(|s| !s.name.is_empty())
        };
        let id = self
            .p
            .strips
            .iter()
            .filter(|s| s.dock == dock)
            .map(|s| s.id)
            .find(|id| !used(&self.p, *id))
            .expect("a free strip for a bus or a return");
        let s = self.p.strip_mut(id).expect("the strip exists");
        s.name = name.to_string();
        s.color = Some(color);
        id
    }

    /// A bus: the parts' strips leave into it instead of the master.
    pub fn bus(&mut self, name: &str, color: Color, parts: &[Part]) -> StripId {
        let id = self.free_strip(name, color, Dock::Middle);
        for part in parts {
            if let Some(s) = self.p.strip_mut(part.strip) {
                s.output = Some(id);
            }
        }
        id
    }

    /// A return on the right of the desk, fed by sends.
    pub fn ret(&mut self, name: &str) -> StripId {
        self.free_strip(name, colors::RETURN, Dock::Right)
    }

    /// A post-fader send at `level_db`.
    pub fn send(&mut self, from: StripId, to: StripId, level_db: f32) {
        if let Some(s) = self.p.strip_mut(from) {
            let mut send = Send::new(to);
            send.level = cantus_core::db_to_linear(level_db);
            s.sends.push(send);
        }
    }

    /// An insert: the kind's factory preset `preset` (if named), then `values`.
    pub fn insert(
        &mut self,
        strip: StripId,
        kind: EffectKind,
        preset: Option<&str>,
        values: &[(&str, f32)],
    ) -> EffectId {
        let slot = self.effect(kind, preset, values);
        let id = slot.id;
        if let Some(s) = self.p.strip_mut(strip) {
            s.effects.push(slot);
        }
        id
    }

    /// An insert on the master (ADR 0026: it runs on the summed mix before the master's
    /// fader).
    pub fn master(&mut self, kind: EffectKind, preset: Option<&str>, values: &[(&str, f32)]) {
        let slot = self.effect(kind, preset, values);
        self.p.master.effects.push(slot);
    }

    fn effect(
        &mut self,
        kind: EffectKind,
        preset: Option<&str>,
        values: &[(&str, f32)],
    ) -> EffectSlot {
        let mut slot = self.p.make_effect(kind);
        if let Some(name) = preset {
            let (_, preset_values) = crate::views::fx::factory_presets(kind)
                .iter()
                .find(|(k, _)| *k == name)
                .unwrap_or_else(|| panic!("no factory preset {name} for {kind:?}"));
            for (k, v) in preset_values.iter() {
                slot.spec.set(k, *v);
            }
        }
        for (k, v) in values {
            slot.spec
                .set(k, *v)
                .unwrap_or_else(|| panic!("{kind:?} has no parameter {k}"));
        }
        slot
    }

    /// Key the effect's detector from another strip (a sidechain).
    pub fn key(&mut self, strip: StripId, effect: EffectId, from: StripId) {
        if let Some(slot) = self.p.strip_mut(strip).and_then(|s| s.effect_mut(effect)) {
            slot.sidechain = Some(from);
        }
    }

    /// Let the effect hear a part's notes (a gate opened by a MIDI part, ADR 0022).
    pub fn midi(&mut self, strip: StripId, effect: EffectId, from: Part) {
        let track = self.p.tracks[from.track].id;
        if let Some(slot) = self.p.strip_mut(strip).and_then(|s| s.effect_mut(effect)) {
            slot.midi_from = Some(track);
        }
    }

    /// A clip of `bars` bars at bar `at` (from 0), holding `evs` (beats from the clip's
    /// start), played with the part's feel. Notes are kept inside the clip.
    pub fn clip(&mut self, part: Part, name: &str, at: f64, bars: f64, evs: &[Ev]) {
        let start = self.ticks(at);
        let length = self.ticks(bars);
        let mut clip = self.p.make_clip(name, start, length);
        let feel = self.feel[part.track];
        let end = length.as_beats_f64();
        let mut evs = evs.to_vec();
        evs.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.pitch.cmp(&b.pitch)));
        for e in evs {
            if e.at < 0.0 || e.at >= end {
                continue;
            }
            let mut at = e.at;
            if feel.swing > 0.0 {
                let step = at / feel.swing_grid;
                let nearest = step.round();
                if (step - nearest).abs() < 1e-6 && (nearest as i64) % 2 == 1 {
                    at += feel.swing * feel.swing_grid;
                }
            }
            if feel.jitter > 0.0 {
                at += self.rng.bi() * feel.jitter;
            }
            let at = at.clamp(0.0, end - 1.0 / 64.0);
            let len = e.len.min(end - at).max(1.0 / 64.0);
            let vel = (e.vel as f64 + self.rng.bi() * feel.vel)
                .round()
                .clamp(1.0, 127.0) as u8;
            let n = self.p.make_note(
                Ticks::from_beats_f64(at),
                Ticks::from_beats_f64(len),
                e.pitch,
                vel,
            );
            clip.notes.push(n);
        }
        clip.notes.sort_by_key(|n| (n.start, n.pitch));
        // A long section is laid out in eight-bar phrases, one clip each, as a producer
        // would: the arrangement reads in phrases, and each can be moved on its own.
        let phrase = self.ticks(PHRASE_BARS);
        if bars > PHRASE_BARS && (bars / PHRASE_BARS).fract() == 0.0 {
            for i in 0..(bars / PHRASE_BARS) as i64 {
                let from = phrase * i;
                let mut c = self.p.make_clip(name, start + from, phrase);
                c.notes = clip
                    .notes
                    .iter()
                    .filter(|n| n.start >= from && n.start < from + phrase)
                    .map(|n| {
                        let mut n = n.clone();
                        n.start -= from;
                        n.length = n.length.min(phrase - n.start);
                        n
                    })
                    .collect();
                if !c.notes.is_empty() {
                    self.p.tracks[part.track].clips.push(c);
                }
            }
        } else {
            self.p.tracks[part.track].clips.push(clip);
        }
    }

    /// Finish: the loop around the whole song (off), the clips in time order.
    pub fn finish(mut self) -> Project {
        let end = self.p.content_end();
        self.p.loop_region = Some(LoopRegion {
            start: Ticks::ZERO,
            end,
        });
        for t in &mut self.p.tracks {
            t.clips.sort_by_key(|c| c.start);
        }
        self.p
    }

    /// `bars` bars of the finished song from bar `from`, moved to the start and looping:
    /// what the program opens on. Clips are cut at the window's end; a clip that starts
    /// before the window is left out (the demos' clips begin on their sections).
    pub fn excerpt(self, from: f64, bars: f64) -> Project {
        let start = self.ticks(from);
        let end = self.ticks(from + bars);
        let mut p = self.finish();
        for t in &mut p.tracks {
            t.clips.retain(|c| c.start >= start && c.start < end);
            for c in &mut t.clips {
                if c.end() > end {
                    c.length = end - c.start;
                    let keep = c.length;
                    c.notes.retain(|n| n.start < keep);
                    for n in &mut c.notes {
                        n.length = n.length.min(keep - n.start);
                    }
                }
                c.start -= start;
            }
        }
        // A track with nothing in the window is left out with its strip; an empty strip
        // takes the strip's place at the end of the desk's middle, so the desk keeps its
        // size and the strips after it close up.
        let idle: Vec<StripId> = p
            .tracks
            .iter()
            .filter(|t| t.clips.is_empty())
            .filter_map(|t| t.strip)
            .collect();
        p.tracks.retain(|t| !t.clips.is_empty());
        for id in idle {
            p.strips.retain(|s| s.id != id);
            let fresh = p.make_strip("");
            let at = p
                .strips
                .iter()
                .position(|s| s.dock == Dock::Right)
                .unwrap_or(p.strips.len());
            p.strips.insert(at, fresh);
        }
        p.loop_region = Some(LoopRegion {
            start: Ticks::ZERO,
            end: end - start,
        });
        p.loop_enabled = true;
        p
    }
}

/// A native instrument from its factory preset, with `values` moved.
pub fn native(kind: InstrumentKind, preset: &str, values: &[(&str, f32)]) -> InstrumentSpec {
    let found = crate::views::inst::factory_presets(kind)
        .iter()
        .find(|p| p.label_key == preset)
        .unwrap_or_else(|| panic!("no factory preset {preset} for {kind:?}"));
    let mut spec = crate::views::inst::preset_spec(kind, found);
    for (k, v) in values {
        spec.set(k, *v)
            .unwrap_or_else(|| panic!("{kind:?} has no parameter {k}"));
    }
    InstrumentSpec::Native(spec)
}

/// A factory kit by its label key (`pulsus.kit.808`, …).
pub fn kit(label: &str) -> PulsusParams {
    PulsusParams::factory()
        .into_iter()
        .find(|k| k.label_key == label)
        .unwrap_or_else(|| panic!("no factory kit {label}"))
        .kit
}

/// The pads of `kit` on these pitches, as a kit of their own (a drum track of its own:
/// its own strip, its own inserts), named in the interface language.
pub fn pads(kit: &PulsusParams, pitches: &[u8]) -> InstrumentSpec {
    pads_with(kit, pitches, |_| {})
}

/// [`pads`], with each pad adjusted by `tune`.
pub fn pads_with(
    kit: &PulsusParams,
    pitches: &[u8],
    tune: impl Fn(&mut DrumPad),
) -> InstrumentSpec {
    let chosen: Vec<DrumPad> = kit
        .pads
        .iter()
        .filter(|p| pitches.contains(&p.pitch))
        .cloned()
        .map(|mut pad| {
            pad.name = crate::app::localized_pad_name(&pad);
            tune(&mut pad);
            pad
        })
        .collect();
    assert_eq!(chosen.len(), pitches.len(), "a pad is missing from the kit");
    InstrumentSpec::Pulsus(PulsusParams::of(chosen))
}

/// A section's name in the interface language (`intro`, `verse`, `chorus`, …).
pub fn section(key: &str) -> String {
    t(&format!("demos.section.{key}"))
}
