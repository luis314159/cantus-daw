//! Arcus's notes and controllers (a child of `arcus`): building the instance, the notes on
//! and off (slur, re-bow, pluck), the bow's direction, the controllers, the tuning and the
//! notes' own articulation.

use super::voice::{NoteTech, Shared};
use super::*;
use crate::math;

impl Arcus {
    /// Build an instance (allocates: never on the audio thread).
    pub fn new(sample_rate: f32) -> Self {
        let hammer = HammerShape::new();
        let hammer_area = hammer.shape.iter().sum::<f32>() / hammer.shape.len() as f32;
        let s = ArcusSettings::default();
        let mut alloc = Allocator::new(s.poly);
        alloc.set_mode(s.mode, Default::default());
        let mut body = Body::new(sample_rate);
        body.set(&s.body());
        let mut out = Output::new(sample_rate);
        out.set(&s.output());
        Arcus {
            sr: sample_rate,
            s,
            voices: (0..MAX_VOICES + GHOSTS)
                .map(|_| Voice::new(sample_rate))
                .collect(),
            alloc,
            ctl: Controls::default(),
            sustain: false,
            clock: 0,
            lanes: Lanes::new(MAX_VOICES + GHOSTS, CHUNK),
            used: [false; MAX_VOICES + GHOSTS],
            buf_l: vec![0.0; CHUNK],
            buf_r: vec![0.0; CHUNK],
            body,
            upper_body: upper::body::UpperBody::new(sample_rate), // r42:arcus-upper
            rad_c: 0.0,
            rad: [(0.0, 0.0); 4],
            eq: eq::MatchEq::default(),
            out,
            hammer,
            hammer_area,
            tuning: [0.0; 128],
            pending: None,
            next_dir: BowDir::Down,
            table: Some(&pitch::TABLE),
            stolen: 0,
            newest: 0,
            peak: 0.0,
            cpu_ns: 0.0,
        }
    }

    pub fn settings(&self) -> &ArcusSettings {
        &self.s
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    /// New settings (audio thread, no allocation): the voices take them at their next
    /// control step.
    pub fn set(&mut self, s: &ArcusSettings) {
        self.s = *s;
        self.s.poly = s.poly.clamp(1, MAX_VOICES);
        // Model 2's match: the hair's noise and the pluck's ring on the settings the voices
        // read, the output's correction designed.
        let m = self.s.match_now();
        if self.s.bow_noise_db > -60.0 {
            self.s.bow_noise_db += m.noise_db;
        }
        self.s.pizz_decay *= m.pizz_decay;
        self.s.vib_depth *= m.vib_depth;
        self.s.vib_rate *= m.vib_rate;
        self.eq.design(&m, self.sr);
        self.alloc.set_polyphony(self.s.poly);
        self.alloc.set_mode(self.s.mode, Default::default());
        self.body.set(&self.s.body());
        self.upper_body.set(upper::body::shape_of(&self.s)); // r42:arcus-upper
        self.out.set(&self.s.output());
        let fr = self.s.instrument.spec().radiation_hz;
        self.rad_c = if fr > 0.0 {
            math::exp(-std::f32::consts::TAU * fr / self.sr)
        } else {
            0.0
        };
    }

    /// The pitch table the bowed notes are corrected by (`None`: none — the tool that
    /// measures the table plays without one).
    pub fn set_pitch_table(&mut self, table: Option<&'static pitch::PitchTable>) {
        self.table = table;
    }

    /// The track's tuning (ADR 0155): semitones each key sits from 12-TET at A440, `NAN`
    /// for a key it leaves unmapped. All zeros changes nothing, to the bit.
    pub fn set_tuning(&mut self, offsets: &[f32; 128]) {
        self.tuning = *offsets;
    }

    /// The next note's own articulation and bow (ADR 0158).
    pub fn note_art(&mut self, id: u64, art: Option<Art>, bow: Option<BowDir>) {
        self.pending = Some((id, art, bow));
    }

    /// A controller, 0 … 1: `which` 0 CC1, 1 CC11, 2 breath (CC2), 3 aftertouch, 4 CC21,
    /// 5 CC65 (portamento on). Where each goes is `dyn.src`'s and `vib.src`'s choice.
    pub fn controller(&mut self, which: u8, value: f32) {
        let v = value.clamp(0.0, 1.0);
        let src = self.s.dyn_src;
        let vib = self.s.vib_src;
        match which {
            0 if matches!(src, DynSource::Auto | DynSource::ModWheel) => self.ctl.dynamic = Some(v),
            1 => match src {
                DynSource::Expression => self.ctl.dynamic = Some(v),
                DynSource::Auto => self.ctl.expression = v,
                _ => {}
            },
            2 if vib == VibSource::Cc2 => self.ctl.vibrato = Some(v),
            2 if matches!(src, DynSource::Auto | DynSource::Breath) => self.ctl.dynamic = Some(v),
            3 if vib == VibSource::Aftertouch => self.ctl.vibrato = Some(v),
            3 if matches!(src, DynSource::Auto | DynSource::Aftertouch) => {
                self.ctl.dynamic = Some(v)
            }
            4 if vib == VibSource::Cc21 => self.ctl.vibrato = Some(v),
            5 => self.ctl.porta = v >= 0.5,
            _ => {}
        }
    }

    pub fn controls(&self) -> &Controls {
        &self.ctl
    }

    /// The pitch bend in semitones as the processor sends it (±2 for plain MIDI).
    pub fn pitch_bend(&mut self, semitones: f32) {
        self.ctl.bend = (semitones / 2.0).clamp(-1.0, 1.0);
    }

    pub fn sustain(&mut self, down: bool) {
        if down == self.sustain {
            return;
        }
        self.sustain = down;
        let mut released = [0usize; crate::voices::MAX_VOICES];
        let n = self.alloc.set_sustain(down, &mut released);
        for v in released.iter().take(n) {
            if let Some(v) = self.voices.get_mut(*v) {
                v.release();
            }
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active()).count()
    }

    pub fn polyphony(&self) -> usize {
        self.alloc.polyphony()
    }

    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    pub fn newest(&self) -> &Voice {
        &self.voices[self.newest.min(MAX_VOICES - 1)]
    }

    /// The bow of a note that starts bowed: its own, or the alternation's next (a down-bow
    /// after silence).
    fn bow_for(&mut self, own: Option<BowDir>, silent: bool) -> BowDir {
        let d = match (own, self.s.dir) {
            (Some(d), _) => d,
            (None, DirMode::Down) => BowDir::Down,
            (None, DirMode::Up) => BowDir::Up,
            (None, DirMode::Alternate) if silent => BowDir::Down,
            (None, DirMode::Alternate) => self.next_dir,
        };
        self.next_dir = match d {
            BowDir::Down => BowDir::Up,
            BowDir::Up => BowDir::Down,
        };
        d
    }

    pub fn note_on(&mut self, key: u8, velocity: f32, id: u64) {
        let (art, own_bow) = match self.pending.take() {
            Some((pid, art, bow)) if pid == id => (art, bow),
            _ => (None, None),
        };
        if !self.tuning[(key as usize).min(127)].is_finite() {
            return; // a key the tuning's keyboard map leaves unmapped plays nothing
        }
        let silent = self.voices.iter().all(|v| !v.active());
        let mono = self.s.mode != PlayMode::Poly;
        match self.alloc.note_on(key, id, velocity) {
            Action::Start {
                voice,
                key,
                id,
                velocity,
                stolen,
            } => {
                // A slur its own art asks for, on a mono voice still bowed (a sustained or
                // tremolo note: a staccato's or a spiccato's stroke is not slurred out of).
                let slur = art == Some(Art::Legato)
                    && mono
                    && voice < MAX_VOICES
                    && self.voices[voice].active()
                    && matches!(
                        self.voices[voice].technique(),
                        NoteTech::Arco | NoteTech::Tremolo
                    );
                if slur {
                    self.glide(voice, key, id, velocity, Some(NoteTech::Arco));
                } else {
                    self.start(voice, key, id, velocity, stolen, art, own_bow, silent);
                }
            }
            Action::Glide { voice, key, id } => {
                if voice >= MAX_VOICES {
                    return;
                }
                let sounding = self.voices[voice].technique();
                // r42 fix2-arcus — an overlapped note's own articulation reads as Exemplar's
                // does (ADR 0158, the sample libraries' convention): a sustain, a legato or
                // a tremolo slurs out of a sustained (or tremolo) note; a staccato, a
                // spiccato or a pizzicato is its own stroke. A note without one keeps the
                // instrument's rule: an arco instrument slurs, a plucked or tremolo one
                // never does.
                let slur = match art {
                    Some(Art::Sustain | Art::Legato | Art::Tremolo) => {
                        matches!(sounding, NoteTech::Arco | NoteTech::Tremolo)
                    }
                    Some(Art::Staccato | Art::Spiccato | Art::Pizzicato) => false,
                    None => sounding != NoteTech::Pizzicato && self.s.technique == Technique::Arco,
                };
                if slur {
                    let tech = art.map(|a| NoteTech::of(self.s.technique, Some(a)));
                    self.glide(voice, key, id, velocity, tech);
                } else {
                    // The string rings on, the bow changes (or the finger plucks again).
                    self.start(voice, key, id, velocity, false, art, own_bow, false);
                }
            }
            Action::Release { voice } => {
                if let Some(v) = self.voices.get_mut(voice) {
                    v.release();
                }
            }
            Action::Nothing => {}
        }
    }

    fn glide(&mut self, voice: usize, key: u8, id: u64, velocity: f32, tech: Option<NoteTech>) {
        let sh = Shared {
            s: &self.s,
            sr: self.sr,
            ctl: &self.ctl,
            tuning: &self.tuning,
            hammer: &self.hammer,
            hammer_area: self.hammer_area,
            table: self.table,
            clock: self.clock,
        };
        self.voices[voice].glide_to(&sh, key, id, velocity, tech);
        self.newest = voice;
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        voice: usize,
        key: u8,
        id: u64,
        velocity: f32,
        stolen: bool,
        art: Option<Art>,
        own_bow: Option<BowDir>,
        silent: bool,
    ) {
        if voice >= MAX_VOICES {
            return;
        }
        let mono = self.s.mode != PlayMode::Poly;
        // One cello is one set of strings: a mono voice still ringing is re-bowed (or
        // plucked again) on its string; a poly voice taken by a new note fades out.
        let keep = mono && self.voices[voice].active();
        if stolen && !keep {
            let ghost = (MAX_VOICES..MAX_VOICES + GHOSTS)
                .find(|&g| !self.voices[g].active())
                .unwrap_or(MAX_VOICES);
            self.voices[ghost].clear();
            self.voices.swap(voice, ghost);
            self.voices[ghost].fade_out(self.sr);
            self.voices[voice].clear();
            self.stolen += 1;
        }
        let tech = NoteTech::of(self.s.technique, art);
        let dir = if tech.is_bowed() {
            self.bow_for(own_bow, silent)
        } else {
            own_bow.unwrap_or(BowDir::Down)
        };
        let sh = Shared {
            s: &self.s,
            sr: self.sr,
            ctl: &self.ctl,
            tuning: &self.tuning,
            hammer: &self.hammer,
            hammer_area: self.hammer_area,
            table: self.table,
            clock: self.clock,
        };
        self.voices[voice].start(&sh, key, id, velocity, tech, dir, keep);
        self.newest = voice;
    }

    pub fn note_off(&mut self, key: u8, id: u64) {
        if let Action::Release { voice } = self.alloc.note_off(key, id) {
            if let Some(v) = self.voices.get_mut(voice) {
                v.release();
            }
        }
    }

    /// Stop the note at once (a choke): it fades out.
    pub fn choke(&mut self, key: u8, id: u64) {
        let found = (0..MAX_VOICES).find(|&u| {
            self.voices[u].active() && self.voices[u].id() == id && self.voices[u].key() == key
        });
        if let Some(u) = found {
            self.voices[u].fade_out(self.sr);
            let _ = self.alloc.note_off(key, id);
        }
    }

    /// A per-note expression: `0` tuning (semitones), `1` pressure, `2` slide (0 … 1).
    pub fn expression(&mut self, id: u64, which: u8, value: f32) {
        for v in self.voices.iter_mut().take(MAX_VOICES) {
            if v.active() && v.id() == id {
                v.expression(which, value);
            }
        }
    }

    pub fn all_notes_off(&mut self) {
        self.sustain(false);
        for v in 0..MAX_VOICES {
            if self.voices[v].active() && self.alloc.is_held(v) {
                let (key, id) = (self.voices[v].key(), self.voices[v].id());
                let _ = self.alloc.note_off(key, id);
            }
        }
        for v in self.voices.iter_mut() {
            v.release();
        }
        if self.s.mode != PlayMode::Poly {
            self.alloc.clear();
        }
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.clear();
        }
        self.alloc.clear();
        self.body.reset();
        self.upper_body.reset(); // r42:arcus-upper
        self.rad = [(0.0, 0.0); 4];
        self.eq.reset();
        self.out.reset();
        self.clock = 0;
        self.sustain = false;
        self.ctl = Controls::default();
        self.pending = None;
        self.next_dir = BowDir::Down;
        self.stolen = 0;
        self.peak = 0.0;
    }

    /// Exactly at rest: no voice, the body and the output still.
    pub fn at_rest(&self) -> bool {
        self.voices.iter().all(|v| !v.active())
            && self.alloc.is_clear()
            && self.rad.iter().all(|r| *r == (0.0, 0.0))
            && self.eq.is_still()
            && self.body.is_still()
            && self.upper_body.is_still() // r42:arcus-upper
            && self.out.is_still()
    }

    /// The engine skipped `frames` samples at rest.
    pub fn skip(&mut self, frames: usize) {
        self.clock += frames as u64;
    }

    pub fn take_stolen(&mut self) -> u32 {
        std::mem::take(&mut self.stolen)
    }
}
